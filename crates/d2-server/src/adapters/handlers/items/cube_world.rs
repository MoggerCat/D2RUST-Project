// Spec: specs/world/cube.md §1, §2, §8; specs/sim/intents-events.md §2.4
//! [`CubeWorld`] for the server: the economy wiring's [`EconomyCube`]
//! for items, stats, unit records and creation; the staged state for
//! interaction, inventory lists, the date and sounds; the checks
//! `cube.md` §2 writes (`0x00549350`, `0x00549150`, `0x0055BF50`'s flag
//! part); and [`ItemPending`] for the calls no written spec owns.

use std::collections::BTreeMap;

use d2_sim::rng::Seed;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::economy::{CubeRest, EconomyCube};
use d2_sim::world::cube::{CraftProperty, CubeWorld, ItemRequest, StatRead};

use super::{Interaction, ItemError, ItemHooks, ItemPending, Staged};
use crate::adapters::UnitFacts;

/// Item flag the targeting reset clears (`cube.md` §2 step 3.1).
const FLAG_TARGETED: u32 = 0x4;
/// Range argument of the ground-item distance test (`cube.md` §2 step 1).
const PUT_RANGE: i32 = 10;
/// Interaction "stash" (`cube.md` §1: type 2, object class 0x10B).
const STASH_TYPE: u8 = 2;
const STASH_CLASS: u32 = 0x10B;
/// Item modes (`cube.md` §2 step 1).
const MODE_STORED: u8 = 0;
const MODE_GROUND: u8 = 3;
const MODE_CURSOR: u8 = 4;

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
    fn interaction(&self, _: UnitId) -> Option<(u8, u32)> {
        unreachable!("{OVERRIDDEN}")
    }
    fn set_interaction(&mut self, _: UnitId, _: u8, _: u32) {
        unreachable!("{OVERRIDDEN}")
    }
    fn reset_interaction(&mut self, _: UnitId) {
        unreachable!("{OVERRIDDEN}")
    }
    fn inventory_pass(&mut self, _: UnitId) {
        unreachable!("{OVERRIDDEN}")
    }
    fn interacting_with_stash(&self, _: UnitId) -> bool {
        unreachable!("{OVERRIDDEN}")
    }
    fn trading(&self, _: UnitId) -> bool {
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
pub(super) struct ServerCube<'e, 'a, 'r> {
    econ: EconomyCube<'e, 'a, ItemHooks, InfoRest<'r>>,
    staged: &'e mut Staged,
    pending: &'e mut dyn ItemPending,
    facts: &'e BTreeMap<UnitId, UnitFacts>,
    player: UnitId,
    sent: Vec<Vec<u8>>,
    errors: Vec<ItemError>,
}

impl<'e, 'a, 'r> ServerCube<'e, 'a, 'r> {
    pub(super) fn new(
        econ: EconomyCube<'e, 'a, ItemHooks, InfoRest<'r>>,
        staged: &'e mut Staged,
        pending: &'e mut dyn ItemPending,
        facts: &'e BTreeMap<UnitId, UnitFacts>,
        player: UnitId,
    ) -> Self {
        Self {
            econ,
            staged,
            pending,
            facts,
            player,
            sent: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// The messages for the acting client, in order, and the errors.
    pub(super) fn finish(self) -> (Vec<Vec<u8>>, Vec<ItemError>) {
        let mut errors: Vec<_> = self
            .econ
            .errors
            .into_iter()
            .map(ItemError::Economy)
            .collect();
        errors.extend(self.errors);
        (self.sent, errors)
    }

    fn active(&self, player: UnitId) -> Option<Interaction> {
        self.staged
            .interactions
            .get(&player)
            .copied()
            .filter(|i| i.active)
    }

    fn inventory_of(&self, player: UnitId) -> &[UnitId] {
        self.staged
            .inventories
            .get(&player)
            .map_or(&[][..], |i| &i.items[..])
    }

    fn is_cursor(&self, player: UnitId, item: UnitId) -> bool {
        self.staged
            .inventories
            .get(&player)
            .is_some_and(|i| i.cursor == Some(item))
    }

    fn unit_class(&self, ty: UnitType, guid: u32) -> Option<u32> {
        let u = self.econ.econ.game.lists.find_unit(ty, guid)?;
        self.econ.econ.units.get(u).map(|r| r.class)
    }
}

impl CubeWorld for ServerCube<'_, '_, '_> {
    fn expansion(&self) -> bool {
        self.econ.expansion()
    }
    fn game_type(&self) -> u8 {
        self.econ.game_type()
    }
    fn ladder(&self) -> bool {
        self.econ.ladder()
    }
    fn difficulty(&self) -> u8 {
        self.econ.difficulty()
    }
    fn item_format(&self) -> u16 {
        self.econ.item_format()
    }
    fn local_date(&self) -> (u8, u8) {
        self.staged.local_date
    }
    fn game_seed(&mut self) -> &mut Seed {
        self.econ.game_seed()
    }

    fn player_class(&self, player: UnitId) -> u8 {
        self.econ.player_class(player)
    }
    fn stat(&self, unit: UnitId, read: StatRead, stat: u16) -> i32 {
        self.econ.stat(unit, read, stat)
    }
    fn set_stat(&mut self, unit: UnitId, stat: u16, value: i32) {
        self.econ.set_stat(unit, stat, value)
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

    fn interaction(&self, player: UnitId) -> Option<(u8, u32)> {
        self.active(player).map(|i| (i.unit_type, i.guid))
    }
    /// `0x00554120`: only when no interaction is active.
    fn set_interaction(&mut self, player: UnitId, unit_type: u8, guid: u32) {
        if self.active(player).is_none() {
            self.staged.interactions.insert(
                player,
                Interaction {
                    guid,
                    unit_type,
                    active: true,
                },
            );
        }
    }
    fn reset_interaction(&mut self, player: UnitId) {
        self.staged.interactions.insert(player, Interaction::RESET);
    }
    fn inventory_pass(&mut self, player: UnitId) {
        self.pending.inventory_pass(player, &mut self.sent);
    }
    fn interacting_with_stash(&self, player: UnitId) -> bool {
        self.active(player).is_some_and(|i| {
            i.unit_type == STASH_TYPE
                && self.unit_class(UnitType::Object, i.guid) == Some(STASH_CLASS)
        })
    }
    /// `0x005678A0`: interaction type 0 with a live (player) unit.
    fn trading(&self, player: UnitId) -> bool {
        self.active(player).is_some_and(|i| {
            i.unit_type == 0
                && self
                    .econ
                    .econ
                    .game
                    .lists
                    .find_unit(UnitType::Player, i.guid)
                    .is_some()
        })
    }

    fn inventory(&self, player: UnitId) -> Vec<UnitId> {
        self.inventory_of(player).to_vec()
    }
    fn item_by_guid(&self, guid: u32) -> Option<UnitId> {
        self.econ.item_by_guid(guid)
    }
    fn item_guid(&self, item: UnitId) -> u32 {
        self.econ.item_guid(item)
    }
    fn item_page(&self, item: UnitId) -> u8 {
        self.econ.item_page(item)
    }
    fn set_item_page(&mut self, item: UnitId, page: u8) {
        self.econ.set_item_page(item, page)
    }
    fn item_mode(&self, item: UnitId) -> u8 {
        self.econ.item_mode(item)
    }
    fn set_item_mode(&mut self, item: UnitId, mode: u8) {
        self.econ.set_item_mode(item, mode)
    }
    fn item_class(&self, item: UnitId) -> Option<u32> {
        self.econ.item_class(item)
    }
    fn set_item_class(&mut self, item: UnitId, class: u32) {
        self.econ.set_item_class(item, class)
    }
    fn class_is_type(&self, class: u32, ty: u16) -> bool {
        self.econ.class_is_type(class, ty)
    }
    fn item_quality(&self, item: UnitId) -> u8 {
        self.econ.item_quality(item)
    }
    fn item_file_index(&self, item: UnitId) -> u32 {
        self.econ.item_file_index(item)
    }
    fn item_level(&self, item: UnitId) -> i32 {
        self.econ.item_level(item)
    }
    fn set_item_level(&mut self, item: UnitId, level: i32) {
        self.econ.set_item_level(item, level)
    }
    fn item_flags(&self, item: UnitId) -> u32 {
        self.econ.item_flags(item)
    }
    fn set_item_flag(&mut self, item: UnitId, flag: u32) {
        self.econ.set_item_flag(item, flag)
    }
    fn item_sockets(&self, item: UnitId) -> i32 {
        self.econ.item_sockets(item)
    }
    fn max_sockets(&self, item: UnitId) -> i32 {
        self.econ.max_sockets(item)
    }
    fn add_sockets(&mut self, item: UnitId, n: i32) {
        self.econ.add_sockets(item, n)
    }
    fn item_seed(&mut self, item: UnitId) -> &mut Seed {
        self.econ.item_seed(item)
    }
    fn socketed(&self, item: UnitId) -> Vec<UnitId> {
        self.pending.socketed(item)
    }
    fn duplicate(&mut self, item: UnitId, fillers: bool) -> Option<UnitId> {
        self.pending.duplicate(item, fillers)
    }
    fn item_init(&mut self, item: UnitId) -> Option<UnitId> {
        self.econ.item_init(item)
    }
    fn create_item(&mut self, request: &ItemRequest) -> Option<UnitId> {
        self.econ.create_item(request)
    }
    fn tempered_affix(&mut self, item: UnitId, prefix: bool) -> u16 {
        self.pending.tempered_affix(item, prefix)
    }
    fn set_tempered(&mut self, item: UnitId, prefix: u16, suffix: u16) {
        self.econ.set_tempered(item, prefix, suffix)
    }
    fn unique_found(&self, index: u32) -> bool {
        self.econ.unique_found(index)
    }
    fn set_unique_found(&mut self, index: u32, found: bool) {
        self.econ.set_unique_found(index, found)
    }
    fn drop_runeword_stats(&mut self, item: UnitId) {
        self.pending.drop_runeword_stats(item)
    }
    fn add_craft_property(&mut self, item: UnitId, prop: &CraftProperty) {
        self.econ.add_craft_property(item, prop)
    }
    fn repair(&mut self, item: UnitId) {
        self.pending.repair(item)
    }
    fn recharge(&mut self, item: UnitId) {
        self.pending.recharge(item)
    }
    fn place(&mut self, player: UnitId, item: UnitId) -> bool {
        let inv = self.staged.inventories.entry(player).or_default();
        self.pending.place(inv, player, item, &mut self.sent)
    }
    fn free_item(&mut self, item: UnitId) {
        self.econ.free_item(item)
    }
    /// `cube.md` §8 step 1 for one item: the pending part (0x9D, the
    /// inventory removal), then the free.
    ///
    /// TODO(cube.md §8 step 1): the spec frees through `0x0055DF10` →
    /// `0x00557FD0`; read here as the unit free `0x00555600`
    /// (`units.md` §3.2) the economy wiring runs. Confirm the two agree.
    fn remove_cube_item(&mut self, player: UnitId, item: UnitId) {
        let inv = self.staged.inventories.entry(player).or_default();
        self.pending
            .remove_cube_item(inv, player, item, &mut self.sent);
        self.econ.free_item(item);
    }
    /// `0x0055BF50`: every inventory item with item flag 0x4 gets it
    /// cleared. The 0x3F it may queue is recorded, not sent
    /// ([`Staged::targeting_resets`]).
    fn targeting_reset(&mut self, player: UnitId) {
        for item in self.inventory_of(player).to_vec() {
            if let Some(i) = self.econ.econ.items.get_mut(item) {
                i.flags &= !FLAG_TARGETED;
            }
        }
        self.staged.targeting_resets.push(player);
    }
    /// `0x00549350` (`cube.md` §2 step 1). Act and positions are the
    /// staged [`UnitFacts`]; a ground item or player without them counts
    /// as missing (as in the unit-target lookup).
    fn put_item_check(&self, player: UnitId, item: u32) -> u32 {
        let Some(it) = self.item_by_guid(item) else {
            return 1;
        };
        match self.item_mode(it) {
            m if m > MODE_CURSOR => 1,
            MODE_GROUND => {
                let (Some(t), Some(p)) = (self.facts.get(&it), self.facts.get(&player)) else {
                    return 1;
                };
                if t.act != p.act {
                    return 2;
                }
                // TODO(cube.md §2 step 1): the failed distance test's
                // value is written as "non-zero"; 1 is the "out of
                // range" code of `intents-events.md` §2.3.
                let near = (t.pos.x - p.pos.x).abs() <= PUT_RANGE
                    && (t.pos.y - p.pos.y).abs() <= PUT_RANGE;
                u32::from(!near)
            }
            _ if self.inventory_of(player).contains(&it) || self.is_cursor(player, it) => 0,
            _ => 1,
        }
    }
    /// `0x00549150` (`cube.md` §2 step 2).
    fn cube_check(&self, player: UnitId, cube: u32) -> bool {
        self.item_by_guid(cube).is_some_and(|c| {
            self.item_mode(c) == MODE_STORED && self.inventory_of(player).contains(&c)
        })
    }
    fn quest_item_hook(&mut self, player: UnitId, item: UnitId, code: [u8; 4]) {
        self.pending.quest_item_hook(player, item, code)
    }
    fn cow_portal(&mut self, player: UnitId) -> bool {
        self.pending.cow_portal(player)
    }
}
