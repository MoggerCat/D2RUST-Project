// Spec: specs/world/hirelings.md §11; specs/world/hirelings-2.md §17
//! The hireling item swap `0x0054CED0(game, player, merc, item C)`
//! (§11), reached from the 0x61 give `0x0054D230`
//! (`items/inventory-moves.md` §7.23, [`crate::items::moves::handlers`]) when
//! C is allowed. Item placement, requirements (§4.2), the cursor equip
//! (§4.6), duplication and messages are other specs' and are reached
//! through [`HirelingItems`]. Units and items are the 0x61 path's
//! [`Owner`] / [`Guid`] so that path's `equip_on_merc` seam can be served
//! by [`swap`].
//!
//! TODO(hirelings-2.md §19, C→S 0x61): no provider of [`HirelingItems`]
//! exists yet, so the 0x61 give's `equip_on_merc` seam
//! (`wiring::inventory`) is not routed to [`swap`]. The duplicate
//! `0x0055A2A0` exists (`wiring::inventory::copy`, `InvDesk::copy_of`,
//! `vendors-2.md` §7.3); the provider belongs to the inventory wiring,
//! owned by the item interaction session (impl-items-wiring).

use crate::items::moves::{Guid, Owner};

use super::class;

/// Results of [`swap`].
pub mod res {
    /// Rule 1: classic game.
    pub const CLASSIC: u32 = 3;
    /// Rule 1 (no player inventory) and rule 4 fail.
    pub const NONE: u32 = 0;
    /// Rules 3 and 4 pass.
    pub const SWAPPED: u32 = 1;
}

/// Item type `shie` (2): the Act 3 hireling's left-hand rule (§11 rule 2).
pub const TYPE_SHIELD: u32 = 2;
/// Mode of the duplicate before the cursor equip (§11 rule 3).
pub const MODE_CURSOR: u8 = 4;
/// Mode of an item put back at its body location (§11 rule 4 fail).
pub const MODE_EQUIPPED: u8 = 1;
/// Page of an item put back at its body location (§11 rule 4 fail).
pub const PAGE_BODY: u8 = 3;
/// Item flag set on the replaced item through `0x00628170` (§11 rule 4).
pub const FLAG_10: u32 = 0x10;
/// Item flag set on the replaced item through `0x006280D0` (§11 rule 4).
pub const FLAG_20: u32 = 0x20;
/// Timer type 9 (periodic stats) cancelled with C's GUID on the merc and
/// on the player (`0x00540E60`, `hirelings-2.md` §17 rule 1).
pub const TIMER_STATS: u8 = 9;
/// Timer type 3 (regeneration) cancelled on the merc, any argument, then
/// scheduled again at frame + 1 (`0x00540E60`, `0x005417D0`, §17 rule 1).
pub const TIMER_REGEN: u8 = 3;
/// Event id queued at frame + 1 (`0x005417D0`, §11 rule 3).
pub const EVENT_3: u32 = 3;

/// Everything the swap reaches outside §11. `player` and `merc` are the
/// swap's units; each call takes the unit 1.14d passes it (rule 4 "Order
/// and units").
pub trait HirelingItems {
    /// The unit has an inventory.
    fn has_inventory(&self, unit: Owner) -> bool;
    /// Give the unit an inventory (§11 rule 1).
    fn create_inventory(&mut self, unit: Owner);
    /// C's two body locations: itemtypes `BodyLoc1` / `BodyLoc2`
    /// (`0x0062EA80`).
    fn body_locs(&self, item: Guid) -> (u8, u8);
    /// Monstats class of the unit.
    fn class(&self, unit: Owner) -> u32;
    /// The item is of item type `ty` (the 0x61 give's type test).
    fn is_type(&self, item: Guid, ty: u32) -> bool;
    /// The unit's item at a body location.
    fn body_item(&self, unit: Owner, loc: u8) -> Option<Guid>;
    /// `0x0055A2A0(game, item, owner, 1)` (`vendors-2.md` §7.3): a
    /// duplicate of `item` owned by `owner` (new GUID; socketed children
    /// recreated in mode 4 and inserted; flags 0x80000 set, 0x2000
    /// cleared; the source gets 0x8000000; replenish timers,
    /// `hirelings-2.md` §17 rule 3). `None`: the creation failed (also
    /// a socketed child's, §17 rule 2).
    fn duplicate(&mut self, owner: Owner, item: Guid) -> Option<Guid>;
    /// The item's mode.
    fn set_mode(&mut self, item: Guid, mode: u8);
    /// `0x00540E60(game, unit, type, a)` (`hirelings-2.md` §17 rule 1):
    /// cancel the unit's pending timer events of `ty` whose first
    /// argument equals `a` (`a` = 0: any). Rule 3: `(9, C's GUID)` on the
    /// merc then on the player, `(3, 0)` on the merc. Nothing is sent.
    fn cancel_timers(&mut self, unit: Owner, ty: u8, a: Guid);
    /// `inventory.md` §4.6 `0x005606B0(game, unit, item, loc, skip)`;
    /// §11 does not read its result.
    fn equip_from_cursor(&mut self, unit: Owner, item: Guid, loc: u8, skip: bool);
    /// `0x0055EEA0`: consume the item.
    fn consume(&mut self, item: Guid);
    /// The player's cursor item := none.
    fn clear_cursor(&mut self, player: Owner);
    /// `0x0055DF00(game, unit, 0, 0)` (the inventory pass,
    /// `inventory-moves.md` §7.23); §11 calls it on the merc.
    fn inventory_pass(&mut self, unit: Owner);
    /// `0x0055F4F0(game, unit, 0)`; §11 calls it on the merc.
    fn refresh_0055f4f0(&mut self, unit: Owner);
    /// `0x005417D0(game, unit, id, frame + 1, 0, 0)`: event `id` at the
    /// current frame + 1; §11 queues it on the merc.
    fn event_next_frame(&mut self, unit: Owner, id: u32);
    /// Unlink `item` from the unit's inventory; §11: it must be the item at
    /// the slot.
    fn unlink(&mut self, unit: Owner, item: Guid);
    /// Clear the unit's body slot `loc`.
    fn clear_slot(&mut self, unit: Owner, loc: u8);
    /// `0x0055C730`: the unit's stat refresh after an unlink.
    fn stat_refresh_unlink(&mut self, unit: Owner);
    /// `inventory.md` §4.2 `0x0062EAF0(item, unit, equipping)`.
    fn requirements(&self, item: Guid, unit: Owner, equipping: bool) -> bool;
    /// Set item flag `flag` (rule 4: 0x10 through `0x00628170`, 0x20
    /// through `0x006280D0`).
    fn item_flag_on(&mut self, item: Guid, flag: u32);
    /// The item leaves the unit's inventory (rule 4 pass).
    fn leave_inventory(&mut self, unit: Owner, item: Guid);
    /// `0x00621000(unit, arg)`.
    fn call_00621000(&mut self, unit: Owner, arg: u32);
    /// The player's cursor := `item` (`0x0063C180`), then `0x0055FB10`
    /// with it; `None` (a failed duplicate of old, `hirelings-2.md` §17
    /// rule 2): cursor := none, `0x0055FB10` with none.
    fn become_cursor(&mut self, player: Owner, item: Option<Guid>);
    /// Link `item` back into the unit's inventory at `page` and body
    /// location `loc` (rule 4 fail).
    fn put_back(&mut self, unit: Owner, item: Guid, page: u8, loc: u8);
    /// `0x00628280(item, arg)`.
    fn call_00628280(&mut self, item: Guid, arg: u8);
    /// `0x0055C460`: the merc refresh.
    fn refresh_0055c460(&mut self, merc: Owner);
}

/// `0x0054CED0(game, player, merc, item C)` (§11 rules 1–4): 3 classic, 0
/// no player inventory or C fails the requirements, 1 swapped.
pub fn swap<W: HirelingItems>(
    w: &mut W,
    expansion: bool,
    player: Owner,
    merc: Owner,
    item: Guid,
) -> u32 {
    // Rule 1.
    if !expansion {
        return res::CLASSIC;
    }
    if !w.has_inventory(player) {
        return res::NONE;
    }
    if !w.has_inventory(merc) {
        w.create_inventory(merc);
    }
    // Rule 2.
    let (loc1, loc2) = w.body_locs(item);
    let target = if w.class(merc) == class::ACT3 && w.is_type(item, TYPE_SHIELD) {
        loc2
    } else {
        loc1
    };
    let Some(old) = w.body_item(merc, target) else {
        // Rule 3.
        equip_copy(w, player, merc, item, target, None);
        return res::SWAPPED;
    };
    // Rule 4: `0x0055C730(old, merc)`, `0x0055DF00(merc)`, then the
    // requirement check.
    w.unlink(merc, old);
    w.clear_slot(merc, target);
    w.stat_refresh_unlink(merc);
    w.inventory_pass(merc);
    if w.requirements(item, merc, false) {
        w.item_flag_on(old, FLAG_10);
        w.item_flag_on(old, FLAG_20);
        w.leave_inventory(merc, old);
        w.call_00621000(merc, 1);
        equip_copy(w, player, merc, item, target, Some(old));
        res::SWAPPED
    } else {
        w.put_back(merc, old, PAGE_BODY, target);
        w.set_mode(old, MODE_EQUIPPED);
        w.call_00628280(old, 0xFF);
        w.refresh_0055c460(merc);
        w.refresh_0055f4f0(merc);
        res::NONE
    }
}

/// Rule 3, with rule 4's "duplicate of old to the player" when `old` is
/// given (`hirelings-2.md` §17 rules 1–2).
///
/// A failed duplicate of C (§17 rule 2): the mode set is skipped
/// (`0x00624690` ignores a null unit), the two type-9 cancels run, the
/// equip runs with GUID −1 and equips nothing (`0x005606B0` finds no
/// unit: the seam is not called), C is still consumed and the cursor
/// cleared: the item is lost and the merc's slot stays empty. The tail
/// and result 1 are unchanged.
fn equip_copy<W: HirelingItems>(
    w: &mut W,
    player: Owner,
    merc: Owner,
    item: Guid,
    target: u8,
    old: Option<Guid>,
) {
    let copy = w.duplicate(merc, item);
    if let Some(copy) = copy {
        w.set_mode(copy, MODE_CURSOR);
    }
    w.cancel_timers(merc, TIMER_STATS, item);
    w.cancel_timers(player, TIMER_STATS, item);
    if let Some(copy) = copy {
        w.equip_from_cursor(merc, copy, target, true);
    }
    w.consume(item);
    w.clear_cursor(player);
    // Rule 4: the old item's duplicate after "cursor := none", before
    // the refresh calls (all four on the merc).
    if let Some(old) = old {
        let back = w.duplicate(player, old);
        w.become_cursor(player, back);
    }
    w.inventory_pass(merc);
    w.refresh_0055f4f0(merc);
    w.cancel_timers(merc, TIMER_REGEN, 0);
    w.event_next_frame(merc, EVENT_3);
}
