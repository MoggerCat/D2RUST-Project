// Spec: specs/world/cube.md §1, §2, §8; specs/items/inventory.md §1.4, §2.4, §5.1, §5.3; specs/items/inventory-moves.md §6.4; specs/sim/intents-events.md §2.4
//! [`CubeWorld`] for the server: the economy wiring's [`EconomyCube`]
//! for items, stats, unit records and creation (the player's interact
//! info on the unit record too); the game's one inventory model
//! ([`InvParts`], `d2_sim::wiring::inventory`) for the item list, the
//! checks (`0x00549350`, `0x00549150`: `inventory.md` §5.1), the
//! targeting reset (`0x0055BF50`, §5.3), placement (`0x00560200`, §2.4)
//! and removal (§8 step 1: the direct 0x9D of §6.4, the §1.4 unlink, the
//! free); the staged date and sounds; and [`ItemPending`] for the calls
//! no written spec owns.

use std::cell::RefCell;
use std::collections::BTreeMap;

use d2_sim::items::moves::{iflag, page};
use d2_sim::rng::Seed;
use d2_sim::units::UnitId;
use d2_sim::wiring::economy::{CubeRest, EconomyCube};
use d2_sim::wiring::inventory::InvDesk;
use d2_sim::world::cube::{CraftProperty, CubeWorld, ItemRequest, StatRead};

use super::moves::{take_sent, InvParts, MoveRest};
use super::{CubeHooks, ItemError, ItemPending, Staged};

/// The player data item creation reads (`generation.md` §9 step 5): the
/// name and the client's hardcore flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CreationInfo {
    pub name: [u8; 16],
    pub hardcore: Option<bool>,
}

/// The rest [`EconomyCube`] calls itself: only `player_info` (from its
/// `create_item`). Every other [`CubeRest`] call is answered by
/// [`ServerCube`] before it reaches the economy.
pub(super) struct InfoRest<'r>(pub &'r BTreeMap<UnitId, CreationInfo>);

const OVERRIDDEN: &str = "ServerCube answers every CubeRest call but player_info";

impl CubeRest for InfoRest<'_> {
    fn player_info(&self, player: UnitId) -> Option<([u8; 16], Option<bool>)> {
        self.0.get(&player).map(|c| (c.name, c.hardcore))
    }
    fn local_date(&self) -> (u8, u8) {
        unreachable!("{OVERRIDDEN}")
    }
    fn attach_sound(&mut self, _: UnitId, _: u8) {
        unreachable!("{OVERRIDDEN}")
    }
    fn send(&mut self, _: UnitId, _: &[u8]) {
        unreachable!("{OVERRIDDEN}")
    }
    fn inventory_pass(&mut self, _: UnitId) {
        unreachable!("{OVERRIDDEN}")
    }
    fn inventory(&self, _: UnitId) -> Vec<UnitId> {
        unreachable!("{OVERRIDDEN}")
    }
    fn socketed(&self, _: UnitId) -> Vec<UnitId> {
        unreachable!("{OVERRIDDEN}")
    }
    fn place(&mut self, _: UnitId, _: UnitId) -> bool {
        unreachable!("{OVERRIDDEN}")
    }
    fn remove_cube_item(&mut self, _: UnitId, _: UnitId) {
        unreachable!("{OVERRIDDEN}")
    }
    fn targeting_reset(&mut self, _: UnitId) {
        unreachable!("{OVERRIDDEN}")
    }
    fn put_item_check(&self, _: UnitId, _: u32) -> u32 {
        unreachable!("{OVERRIDDEN}")
    }
    fn cube_check(&self, _: UnitId, _: u32) -> bool {
        unreachable!("{OVERRIDDEN}")
    }
    fn duplicate(&mut self, _: UnitId, _: bool) -> Option<UnitId> {
        unreachable!("{OVERRIDDEN}")
    }
    fn tempered_affix(&mut self, _: UnitId, _: bool) -> u16 {
        unreachable!("{OVERRIDDEN}")
    }
    fn drop_runeword_stats(&mut self, _: UnitId) {
        unreachable!("{OVERRIDDEN}")
    }
    fn repair(&mut self, _: UnitId) {
        unreachable!("{OVERRIDDEN}")
    }
    fn recharge(&mut self, _: UnitId) {
        unreachable!("{OVERRIDDEN}")
    }
    fn quest_item_hook(&mut self, _: UnitId, _: UnitId, _: [u8; 4]) {
        unreachable!("{OVERRIDDEN}")
    }
    fn cow_portal(&mut self, _: UnitId) -> bool {
        unreachable!("{OVERRIDDEN}")
    }
}

/// The cube's world on the server, for one handler call by `player`.
///
/// The economy and the inventory parts sit in cells: the §5.1 checks the
/// cube asks through `&self` run on an inventory desk, which borrows the
/// economy mutably (it fills its item data copies).
pub(super) struct ServerCube<'e, 'a, 'r, H> {
    econ: RefCell<EconomyCube<'e, 'a, H, InfoRest<'r>>>,
    inv: RefCell<Option<&'e mut InvParts>>,
    staged: &'e mut Staged,
    pending: &'e mut dyn ItemPending,
    player: UnitId,
    sent: Vec<Vec<u8>>,
    errors: Vec<ItemError>,
}

/// The inventory desk of one cube call.
type CubeDesk<'d, 'a, H> = InvDesk<'d, 'a, H, dyn MoveRest + Send + Sync>;

impl<'e, 'a, 'r, H: CubeHooks> ServerCube<'e, 'a, 'r, H> {
    pub(super) fn new(
        econ: EconomyCube<'e, 'a, H, InfoRest<'r>>,
        staged: &'e mut Staged,
        pending: &'e mut dyn ItemPending,
        inv: Option<&'e mut InvParts>,
        player: UnitId,
    ) -> Self {
        Self {
            econ: RefCell::new(econ),
            inv: RefCell::new(inv),
            staged,
            pending,
            player,
            sent: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// The messages for the acting client, in order, and the errors.
    pub(super) fn finish(self) -> (Vec<Vec<u8>>, Vec<ItemError>) {
        let mut errors: Vec<_> = self
            .econ
            .into_inner()
            .errors
            .into_iter()
            .map(ItemError::Economy)
            .collect();
        errors.extend(self.errors);
        (self.sent, errors)
    }

    /// The economy cube (reads).
    fn ec(&self) -> std::cell::Ref<'_, EconomyCube<'e, 'a, H, InfoRest<'r>>> {
        self.econ.borrow()
    }

    /// The economy cube (writes).
    fn ec_mut(&mut self) -> &mut EconomyCube<'e, 'a, H, InfoRest<'r>> {
        self.econ.get_mut()
    }

    /// Runs `f` on the inventory desk over this call's economy (`None`:
    /// a host without inventory parts).
    fn read_desk<T>(&self, f: impl FnOnce(&CubeDesk<'_, 'a, H>) -> T) -> Option<T> {
        let mut inv = self.inv.borrow_mut();
        let parts = inv.as_deref_mut()?;
        let mut ec = self.econ.borrow_mut();
        let d = parts.desk(&mut *ec.econ);
        Some(f(&d))
    }

    /// Runs `f` on the inventory desk; the messages it queued go to the
    /// acting client in order (another receiver is an error, as for
    /// [`CubeWorld::send`]).
    fn with_desk<T>(&mut self, f: impl FnOnce(&mut CubeDesk<'_, 'a, H>) -> T) -> Option<T> {
        let parts = self.inv.get_mut().as_deref_mut()?;
        let mut d = parts.desk(&mut *self.econ.get_mut().econ);
        let out = f(&mut d);
        let sent = take_sent(&mut d);
        for (unit, bytes) in sent {
            match unit {
                Some(u) if u == self.player => self.sent.push(bytes),
                Some(u) => self.errors.push(ItemError::OtherPlayer(u)),
                None => {}
            }
        }
        Some(out)
    }
}

impl<H: CubeHooks> CubeWorld for ServerCube<'_, '_, '_, H> {
    fn expansion(&self) -> bool {
        self.ec().expansion()
    }
    fn game_type(&self) -> u8 {
        self.ec().game_type()
    }
    fn ladder(&self) -> bool {
        self.ec().ladder()
    }
    fn difficulty(&self) -> u8 {
        self.ec().difficulty()
    }
    fn item_format(&self) -> u16 {
        self.ec().item_format()
    }
    fn local_date(&self) -> (u8, u8) {
        self.staged.local_date
    }
    fn game_seed(&mut self) -> &mut Seed {
        self.ec_mut().game_seed()
    }

    fn player_class(&self, player: UnitId) -> u8 {
        self.ec().player_class(player)
    }
    fn stat(&self, unit: UnitId, read: StatRead, stat: u16) -> i32 {
        self.ec().stat(unit, read, stat)
    }
    fn set_stat(&mut self, unit: UnitId, stat: u16, value: i32) {
        self.ec_mut().set_stat(unit, stat, value)
    }
    fn attach_sound(&mut self, player: UnitId, event: u8) {
        self.staged.sounds.push((player, event));
    }
    /// Only the acting player's client receives (every cube path sends
    /// to the player it acts for).
    fn send(&mut self, player: UnitId, msg: &[u8]) {
        if player == self.player {
            self.sent.push(msg.to_vec());
        } else {
            self.errors.push(ItemError::OtherPlayer(player));
        }
    }

    /// The interact info on the player's unit record (the economy
    /// cube's, `0x00554100` / `0x00554120` / `0x00554190`).
    fn interaction(&self, player: UnitId) -> Option<(u8, u32)> {
        self.ec().interaction(player)
    }
    fn set_interaction(&mut self, player: UnitId, unit_type: u8, guid: u32) {
        self.ec_mut().set_interaction(player, unit_type, guid)
    }
    fn reset_interaction(&mut self, player: UnitId) {
        self.ec_mut().reset_interaction(player)
    }
    fn inventory_pass(&mut self, player: UnitId) {
        self.pending.inventory_pass(player, &mut self.sent);
    }
    fn interacting_with_stash(&self, player: UnitId) -> bool {
        self.ec().interacting_with_stash(player)
    }
    fn trading(&self, player: UnitId) -> bool {
        self.ec().trading(player)
    }

    /// The player's item list in link order (`inventory.md` §1.4 rule 1,
    /// `cube.md` OQ 5).
    fn inventory(&self, player: UnitId) -> Vec<UnitId> {
        self.inv
            .borrow()
            .as_deref()
            .map_or_else(Vec::new, |p| p.state.items_of(player))
    }
    fn item_by_guid(&self, guid: u32) -> Option<UnitId> {
        self.ec().item_by_guid(guid)
    }
    fn item_guid(&self, item: UnitId) -> u32 {
        self.ec().item_guid(item)
    }
    fn item_page(&self, item: UnitId) -> u8 {
        self.ec().item_page(item)
    }
    fn set_item_page(&mut self, item: UnitId, page: u8) {
        self.ec_mut().set_item_page(item, page)
    }
    fn item_mode(&self, item: UnitId) -> u8 {
        self.ec().item_mode(item)
    }
    fn set_item_mode(&mut self, item: UnitId, mode: u8) {
        self.ec_mut().set_item_mode(item, mode)
    }
    fn item_class(&self, item: UnitId) -> Option<u32> {
        self.ec().item_class(item)
    }
    fn set_item_class(&mut self, item: UnitId, class: u32) {
        self.ec_mut().set_item_class(item, class)
    }
    fn class_is_type(&self, class: u32, ty: u16) -> bool {
        self.ec().class_is_type(class, ty)
    }
    fn item_quality(&self, item: UnitId) -> u8 {
        self.ec().item_quality(item)
    }
    fn item_file_index(&self, item: UnitId) -> u32 {
        self.ec().item_file_index(item)
    }
    fn item_level(&self, item: UnitId) -> i32 {
        self.ec().item_level(item)
    }
    fn set_item_level(&mut self, item: UnitId, level: i32) {
        self.ec_mut().set_item_level(item, level)
    }
    fn item_flags(&self, item: UnitId) -> u32 {
        self.ec().item_flags(item)
    }
    fn set_item_flag(&mut self, item: UnitId, flag: u32) {
        self.ec_mut().set_item_flag(item, flag)
    }
    fn item_sockets(&self, item: UnitId) -> i32 {
        self.ec().item_sockets(item)
    }
    fn max_sockets(&self, item: UnitId) -> i32 {
        self.ec().max_sockets(item)
    }
    fn add_sockets(&mut self, item: UnitId, n: i32) {
        self.ec_mut().add_sockets(item, n)
    }
    fn item_seed(&mut self, item: UnitId) -> &mut Seed {
        self.ec_mut().item_seed(item)
    }
    /// The item's own inventory, in link order (its socket fillers).
    fn socketed(&self, item: UnitId) -> Vec<UnitId> {
        self.inv
            .borrow()
            .as_deref()
            .map_or_else(Vec::new, |p| p.state.fillers(item))
    }
    fn duplicate(&mut self, item: UnitId, fillers: bool) -> Option<UnitId> {
        self.pending.duplicate(item, fillers)
    }
    fn item_init(&mut self, item: UnitId) -> Option<UnitId> {
        self.ec_mut().item_init(item)
    }
    fn create_item(&mut self, request: &ItemRequest) -> Option<UnitId> {
        self.ec_mut().create_item(request)
    }
    fn tempered_affix(&mut self, item: UnitId, prefix: bool) -> u16 {
        self.pending.tempered_affix(item, prefix)
    }
    fn set_tempered(&mut self, item: UnitId, prefix: u16, suffix: u16) {
        self.ec_mut().set_tempered(item, prefix, suffix)
    }
    fn unique_found(&self, index: u32) -> bool {
        self.ec().unique_found(index)
    }
    fn set_unique_found(&mut self, index: u32, found: bool) {
        self.ec_mut().set_unique_found(index, found)
    }
    fn drop_runeword_stats(&mut self, item: UnitId) {
        self.pending.drop_runeword_stats(item)
    }
    fn add_craft_property(&mut self, item: UnitId, prop: &CraftProperty) {
        self.ec_mut().add_craft_property(item, prop)
    }
    fn repair(&mut self, item: UnitId) {
        self.pending.repair(item)
    }
    fn recharge(&mut self, item: UnitId) {
        self.pending.recharge(item)
    }
    /// `0x00560200(game, player, id, 0, 0, 1, 1, 0)` (`cube.md` §2 step
    /// 3.5, §8 steps 3–4): `inventory.md` §2.4 with a free position and
    /// "send", on the player's inventory.
    fn place(&mut self, player: UnitId, item: UnitId) -> bool {
        self.with_desk(|d| d.place(player, item, (0, 0), true, true))
            .unwrap_or(false)
    }
    fn free_item(&mut self, item: UnitId) {
        self.ec_mut().free_item(item)
    }
    /// `cube.md` §8 step 1 for one item: S→C 0x9D action 5 now
    /// (`0x0053D010`, `inventory-moves.md` §6.4: item flags | 0x20, the stored
    /// page set to 3 and shown), then removed from the inventory and freed
    /// (`0x0055DF10` → `0x00557FD0`: the §1.4 unlink, then the inventory
    /// wiring's free).
    ///
    /// TODO(cube.md §8 step 1): an unlink failure is not written (the
    /// item is in the list: the caller walks it); ignored, the free runs.
    fn remove_cube_item(&mut self, player: UnitId, item: UnitId) {
        let fatal = self.with_desk(|d| {
            let r = d.send_item_page(player, item, iflag::COPIED, page::CUBE);
            d.remove(player, item);
            d.free(item);
            r
        });
        if let Some(Err(e)) = fatal {
            self.errors.push(ItemError::Move(e));
        }
    }
    /// `0x0055BF50` (`inventory.md` §5.3) on the player's item list: flag
    /// 0x4 cleared, S→C 0x3F queued when `0x0044BE50` returns 0.
    fn targeting_reset(&mut self, player: UnitId) {
        self.with_desk(|d| d.reset_targeting(player));
    }
    /// `0x00549350` (`cube.md` §2 step 1 = `inventory.md` §5.1 ground or
    /// owned) on the inventory model. Without inventory parts: 1.
    fn put_item_check(&self, player: UnitId, item: u32) -> u32 {
        self.read_desk(|d| u32::from(d.check_ground_or_owned(player, item)))
            .unwrap_or(1)
    }
    /// `0x00549150` (`cube.md` §2 step 2 = `inventory.md` §5.1 stored
    /// item).
    fn cube_check(&self, player: UnitId, cube: u32) -> bool {
        self.read_desk(|d| d.check_stored(player, cube) == 0)
            .unwrap_or(false)
    }
    fn quest_item_hook(&mut self, player: UnitId, item: UnitId, code: [u8; 4]) {
        self.pending.quest_item_hook(player, item, code)
    }
    fn cow_portal(&mut self, player: UnitId) -> bool {
        self.pending.cow_portal(player)
    }
}
