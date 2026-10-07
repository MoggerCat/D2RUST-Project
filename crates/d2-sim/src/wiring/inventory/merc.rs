// Spec: specs/world/hirelings.md §5 r4, §11; specs/world/hirelings-2.md §17, §19; specs/items/inventory-moves.md §7.23; specs/world/vendors-2.md §7.3
//! The hireling seams of the 0x61 give (`inventory-moves.md` §7.23) on
//! the inventory model: the player's hireling (`0x00574EC0(7, 0)`) from
//! the hireling lists the host lends to [`super::InvState::hirelings`],
//! and the swap `0x0054CED0` ([`crate::world::hirelings::items::swap`])
//! with [`MercItems`] as its [`HirelingItems`] provider.
//!
//! A host without hireling lists (`InvState::hirelings` = `None`) keeps
//! the rest's answers ([`super::InvRest`]'s `MovePending` part).

use super::{InvDesk, InvRest};
use crate::items::moves::{Guid, InventoryOps, MovePending, MoveUnits, Owner};
use crate::units::lifecycle::LifecycleHooks;
use crate::units::{UnitId, UnitType};
use crate::world::hirelings::items::{swap, HirelingItems, FLAG_10, FLAG_20};

impl<H: LifecycleHooks, R: InvRest + ?Sized> InvDesk<'_, '_, H, R> {
    /// `0x00574EC0(game, player, 7, 0)` (`hirelings.md` §5 rule 4) on the
    /// lent lists: the first living node's monster. `None` (outer): no
    /// lists lent.
    pub(crate) fn lent_hireling(&self, player: Owner) -> Option<Option<Owner>> {
        let st = self.state.hirelings.as_ref()?;
        let p = self.unit_of(player);
        let found = p
            .and_then(|p| st.first_node(p, false))
            .and_then(|n| self.econ.game.lists.find_unit(UnitType::Monster, n.guid))
            .and_then(|u| self.owner_of(u));
        Some(found)
    }

    /// The player whose lent hireling list holds `merc`'s node.
    fn merc_owner(&self, merc: Owner) -> Option<Owner> {
        let st = self.state.hirelings.as_ref()?;
        let (&p, _) = st
            .lists
            .iter()
            .find(|(_, l)| l.nodes.iter().any(|n| n.guid == merc.guid))?;
        self.owner_of(p)
    }

    /// `0x0065A590` (`inventory-moves.md` §7.23 rule 2): the room test of
    /// `intents-events.md` §9 rule 10 (the player's room is in the
    /// hireling's room's room list: its adjacency array, the room itself
    /// included). `None`: no lists lent.
    ///
    /// PROVISIONAL (inventory-moves.md §7.23 r2; REC-none): §7.23 names
    /// the call "belongs to the player"; the same address is the 0x4B
    /// room test (`0x0065A590` → `0x00619790`), whose argument order is
    /// read here as (player, hireling) as there.
    pub(crate) fn lent_owns_hireling(&self, player: Owner, merc: Owner) -> Option<bool> {
        self.state.hirelings.as_ref()?;
        let l = &self.econ.game.lists;
        let room = |o: Owner| {
            self.unit_of(o)
                .and_then(|u| l.unit(u))
                .and_then(|e| e.room())
        };
        let (Some(pr), Some(mr)) = (room(player), room(merc)) else {
            return Some(false);
        };
        Some(l.room(mr).is_some_and(|r| r.adjacent.contains(&pr)))
    }

    /// `0x0054CED0(game, player, merc, item)` (`hirelings.md` §11) on the
    /// lent lists: the player is the owner of the list holding the merc
    /// (the 0x61 caller's player: its hireling is that list's first
    /// living node). Returns false when no lists are lent; with lists and
    /// no owner nothing happens. The swap's result is not read
    /// (`inventory-moves.md` §7.23: the give ignores it).
    pub(crate) fn lent_equip_on_merc(&mut self, merc: Owner, item: Guid) -> bool {
        if self.state.hirelings.is_none() {
            return false;
        }
        let Some(player) = self.merc_owner(merc) else {
            return true;
        };
        let expansion = MoveUnits::expansion(self);
        swap(&mut MercItems { d: self }, expansion, player, merc, item);
        true
    }
}

/// [`HirelingItems`] on the inventory desk: each call is the inventory
/// model's operation of the same rule (`items::moves` seams on
/// [`InvDesk`]), the item copy `0x0055A2A0` ([`InvDesk::copy_of`]) and
/// the game's timer queue.
pub struct MercItems<'x, 'd, 'a, H, R: ?Sized> {
    pub d: &'x mut InvDesk<'d, 'a, H, R>,
}

impl<H: LifecycleHooks, R: InvRest + ?Sized> MercItems<'_, '_, '_, H, R> {
    fn unit(&self, o: Owner) -> Option<UnitId> {
        self.d.unit_of(o)
    }
}

impl<H: LifecycleHooks, R: InvRest + ?Sized> HirelingItems for MercItems<'_, '_, '_, H, R> {
    fn has_inventory(&self, unit: Owner) -> bool {
        InventoryOps::has_inventory(self.d, unit)
    }
    /// `0x0063ABD0` (`items/inventory.md` §1.3: the owner kind decides
    /// the grids).
    fn create_inventory(&mut self, unit: Owner) {
        let Some(u) = self.unit(unit) else {
            return;
        };
        if let Some(kind) = self.d.kind_of(u) {
            self.d.state.add_inventory(u, kind, unit.guid);
        }
    }
    /// `0x0062EA80`: itemtypes `BodyLoc1` / `BodyLoc2` of the item's
    /// type; no record → (0, 0).
    fn body_locs(&self, item: Guid) -> (u8, u8) {
        self.d
            .item_unit(item)
            .and_then(|u| self.d.state.items.get(&u))
            .and_then(|it| self.d.tables.itype_of(it.record))
            .map_or((0, 0), |t| (t.bodyloc1, t.bodyloc2))
    }
    fn class(&self, unit: Owner) -> u32 {
        MoveUnits::unit_class(self.d, unit)
    }
    fn is_type(&self, item: Guid, ty: u32) -> bool {
        u16::try_from(ty).is_ok_and(|t| MoveUnits::is_type(self.d, item, t))
    }
    fn body_item(&self, unit: Owner, loc: u8) -> Option<Guid> {
        InventoryOps::body_item(self.d, unit, loc)
    }
    /// `0x0055A2A0(game, item, owner, 1)`: [`InvDesk::copy_of`] with
    /// fillers (`vendors-2.md` §7.3; the owner argument is not read).
    fn duplicate(&mut self, _owner: Owner, item: Guid) -> Option<Guid> {
        let src = self.d.item_unit(item)?;
        let copy = self.d.copy_of(src, true)?;
        Some(self.d.guid_of(copy))
    }
    fn set_mode(&mut self, item: Guid, mode: u8) {
        MoveUnits::set_mode(self.d, item, mode);
    }
    /// `0x00540E60` on the game's timer queue (`tick.md` §5); a = 0: any
    /// argument.
    fn cancel_timers(&mut self, unit: Owner, ty: u8, a: Guid) {
        let Some(u) = self.unit(unit) else {
            return;
        };
        let arg = (a != 0).then_some(a);
        self.d.econ.game.timers.cancel_unit_events(u, ty, arg);
    }
    fn equip_from_cursor(&mut self, unit: Owner, item: Guid, loc: u8, skip: bool) {
        InventoryOps::equip_from_cursor(self.d, unit, item, loc, skip);
    }
    /// `0x0055EEA0` (the 0x61 give's potion consume, the same call).
    fn consume(&mut self, item: Guid) {
        MovePending::consume_one(self.d, item);
    }
    fn clear_cursor(&mut self, player: Owner) {
        InventoryOps::set_cursor(self.d, player, None);
    }
    /// TODO(spec: inventory-moves.md §7.23): `0x0055DF00` (the hireling
    /// inventory pass of the take) and `0x0055DBC0` (§5.7, the seam's
    /// call) are not told apart in a written body; the seam's inventory
    /// pass is called.
    fn inventory_pass(&mut self, unit: Owner) {
        MovePending::inventory_pass(self.d, unit);
    }
    /// TODO(spec: inventory-moves.md §7.23): `0x0055F4F0` has no written
    /// body or provider; nothing runs.
    fn refresh_0055f4f0(&mut self, _unit: Owner) {}
    /// `0x005417D0(game, unit, id, frame + 1, 0, 0)` on the game's timer
    /// queue.
    fn event_next_frame(&mut self, unit: Owner, id: u32) {
        let Some(u) = self.unit(unit) else {
            return;
        };
        let at = self.d.econ.game.frame.wrapping_add(1);
        if let Err(e) = self.d.econ.game.schedule_event(u, id, at, None, 0, 0) {
            self.d.state.errors.push(super::InvError::Economy(e.into()));
        }
    }
    fn unlink(&mut self, unit: Owner, item: Guid) {
        InventoryOps::unlink(self.d, unit, item);
    }
    fn clear_slot(&mut self, unit: Owner, loc: u8) {
        InventoryOps::clear_body_slot(self.d, unit, loc);
    }
    /// `0x0055C730(game, old, merc, 0)` (§11 rule 4 "Order and units").
    fn stat_refresh_unlink(&mut self, unit: Owner) {
        MovePending::stat_refresh_unlink(self.d, unit, 0);
    }
    fn requirements(&self, item: Guid, unit: Owner, equipping: bool) -> bool {
        InventoryOps::requirements(self.d, item, unit, equipping)
    }
    /// 0x10 through `0x00628170`: the command-flag setter (the take's
    /// "command flag 0x10", `inventory-moves.md` §7.23; `vendors-2.md`
    /// §7.3 step 5); 0x20 through `0x006280D0`: the item flags.
    fn item_flag_on(&mut self, item: Guid, flag: u32) {
        if flag == FLAG_10 {
            let f = MoveUnits::cmd_flags(self.d, item);
            MoveUnits::set_cmd_flags(self.d, item, f | flag);
        } else {
            debug_assert_eq!(flag, FLAG_20);
            let f = MoveUnits::item_flags(self.d, item);
            MoveUnits::set_item_flags(self.d, item, f | flag);
        }
    }
    /// PROVISIONAL (hirelings.md §11 r4; REC-none): rule 4's unlink at
    /// its start already took old out of the merc's inventory; "leaves
    /// the merc's inventory" is read as that unlink, no second call.
    fn leave_inventory(&mut self, _unit: Owner, _item: Guid) {}
    /// TODO(spec: hirelings.md §11 r4): `0x00621000(unit, 1)` has no
    /// written body; nothing runs.
    fn call_00621000(&mut self, _unit: Owner, _arg: u32) {}
    /// `0x0063C180` then `0x0055FB10` (`inventory-moves.md` §7.23 take);
    /// `0x0055FB10` with none is not called (no item for the seam).
    fn become_cursor(&mut self, player: Owner, item: Option<Guid>) {
        InventoryOps::set_cursor(self.d, player, item);
        if let Some(i) = item {
            MovePending::give_cursor_item(self.d, player, i);
        }
    }
    /// `0x0063BDB0` (§2.2 on grid 0) at `loc`, then the page.
    fn put_back(&mut self, unit: Owner, item: Guid, page: u8, loc: u8) {
        InventoryOps::place_body(self.d, unit, item, loc);
        MoveUnits::set_page(self.d, item, page);
    }
    /// TODO(spec: hirelings.md §11 r4): `0x00628280(item, 0xFF)` has no
    /// written body; nothing runs.
    fn call_00628280(&mut self, _item: Guid, _arg: u8) {}
    /// TODO(spec: hirelings.md §11 r4): `0x0055C460` has no written body
    /// or provider; nothing runs.
    fn refresh_0055c460(&mut self, _merc: Owner) {}
}
