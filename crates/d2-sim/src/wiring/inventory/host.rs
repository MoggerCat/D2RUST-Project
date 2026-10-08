// Spec: specs/items/inventory.md §1.4, §2.4, §5.1, §5.3; specs/items/inventory-moves.md §6.4; specs/world/cube.md §2, §8; specs/world/vendors.md §7
//! The inventory model for the other item systems (vendors, the cube):
//! one inventory per owner unit, read and written by every system that
//! touches a player's items. The item-move code (`items::moves`) runs on
//! [`InvDesk`] through its seams; the vendor and cube adapters of a host
//! call the entry points here, keyed by unit, instead of keeping lists of
//! their own.
//!
//! Reads that need no unit record ([`InvState::holds`],
//! [`InvState::items_of`], [`InvState::cursor_of`],
//! [`InvState::body_items`], [`InvState::fillers`]) are on the state;
//! the rules (§2.4 placement, §1.4 unlink, §5.1 checks, §5.3 targeting
//! reset, the §6.4 direct 0x9D) run on the desk, each the
//! `items::inventory` / `items::moves` function of its rule.

use super::{InvDesk, InvError, InvRest, InvState};
use crate::items::inventory::{
    grid_id, ground_or_owned_check, place_in_page, stored_item_check, targeting_reset, Inventory,
    BODY_GRID,
};
use crate::items::moves::{deferred, MoveFatal, MovePending, MoveUnits};
use crate::units::lifecycle::LifecycleHooks;
use crate::units::UnitId;

impl InvState {
    /// The owner's inventory.
    pub fn of(&self, owner: UnitId) -> Option<&Inventory> {
        self.inventories.get(&owner)
    }

    /// The owner's item list in link order (§1.4 rule 1); empty without an
    /// inventory.
    pub fn items_of(&self, owner: UnitId) -> Vec<UnitId> {
        self.of(owner).map_or_else(Vec::new, |i| i.items().to_vec())
    }

    /// The owner's cursor item (§1.4 rule 3).
    pub fn cursor_of(&self, owner: UnitId) -> Option<UnitId> {
        self.of(owner)?.cursor()
    }

    /// The item is the owner's: in its item list or its cursor item (the
    /// owned-item test of §5.1 `0x00549220` without the item lookup).
    pub fn holds(&self, owner: UnitId, item: UnitId) -> bool {
        self.of(owner)
            .is_some_and(|i| i.contains(item) || i.cursor() == Some(item))
    }

    /// The items at the body locations (grid 0, §1.2), in location order.
    pub fn body_items(&self, owner: UnitId) -> Vec<UnitId> {
        let Some(inv) = self.of(owner) else {
            return Vec::new();
        };
        (0..BODY_GRID.0)
            .filter_map(|loc| inv.item_at(grid_id::BODY, i32::from(loc), 0))
            .collect()
    }

    /// The items socketed into `item`: its own inventory's item list, in
    /// link order (the inventory an item with fillers owns, §1).
    pub fn fillers(&self, item: UnitId) -> Vec<UnitId> {
        self.items_of(item)
    }
}

impl<H: LifecycleHooks, R: InvRest + ?Sized> InvDesk<'_, '_, H, R> {
    /// Item placement into a page (`0x00560200`, §2.4 steps 1–9: the
    /// targeting reset, then the cursor item `item` into the page its item
    /// data names, at (x, y) or a free position). False when the owner has
    /// no inventory or the placement fails. A cursor-mode item still in a
    /// room is a caller error (§2.4 rule 2: the mode test does not check
    /// the room): recorded, not placed.
    pub fn place(
        &mut self,
        owner: UnitId,
        item: UnitId,
        (x, y): (i32, i32),
        find_free: bool,
        send: bool,
    ) -> bool {
        let in_room = self
            .econ
            .game
            .lists
            .unit(item)
            .and_then(|e| e.room())
            .is_some();
        if in_room && self.econ.units.get(item).is_some_and(|r| r.mode == 4) {
            self.state.errors.push(InvError::PlacedWithRoom(item));
            return false;
        }
        let Some(o) = self.owner_of(owner) else {
            return false;
        };
        let t = self.tables;
        let item = self.state.items.contains_key(&item).then_some(item);
        self.with_inv(o, |inv, d| {
            place_in_page(inv, d, t, item, x, y, find_free, send)
        })
        .unwrap_or(false)
    }

    /// Unlinks `item` from the owner's inventory (`0x0063AAF0`, §1.4 rule
    /// 1). False when it is not there.
    pub fn remove(&mut self, owner: UnitId, item: UnitId) -> bool {
        let Some(o) = self.owner_of(owner) else {
            return false;
        };
        self.with_inv(o, |inv, d| inv.unlink(d, item))
            .unwrap_or(false)
    }

    /// Frees an item unit (`0x00557FD0`, the inventory wiring's reading:
    /// the unit removal `0x00555600`, item data dropped).
    pub fn free(&mut self, item: UnitId) {
        let g = self.guid_of(item);
        MovePending::free_item(self, g);
    }

    /// The targeting reset `0x0055BF50` (§5.3) of the owner's item list.
    pub fn reset_targeting(&mut self, owner: UnitId) {
        let Some(o) = self.owner_of(owner) else {
            return;
        };
        self.with_inv(o, |inv, d| targeting_reset(inv, d));
    }

    /// Stored item check `0x00549150` (§5.1) of the GUID for `player` (a
    /// player without an inventory is checked against an empty one).
    pub fn check_stored(&self, player: UnitId, guid: u32) -> u8 {
        self.unit_check(player, guid, stored_item_check)
    }

    /// Ground or owned check `0x00549350` (§5.1).
    pub fn check_ground_or_owned(&self, player: UnitId, guid: u32) -> u8 {
        self.unit_check(player, guid, ground_or_owned_check)
    }

    fn unit_check(&self, player: UnitId, guid: u32, f: fn(&Inventory, &Self, u32) -> u8) -> u8 {
        match self.owner_of(player) {
            Some(o) => f(&self.inv_or_empty(o), self, guid),
            None => 1,
        }
    }

    /// Direct S→C 0x9C with `action` for `item`, queued to the owner now
    /// (`deferred::send_item_world`; the store item's action 11,
    /// `vendors.md` §4 step 3).
    pub fn send_item_world(
        &mut self,
        owner: UnitId,
        item: UnitId,
        action: u8,
        flags: u32,
    ) -> Result<(), MoveFatal> {
        let (Some(o), g) = (self.owner_of(owner), self.guid_of(item)) else {
            return Ok(());
        };
        deferred::send_item_world(self, o, g, action, flags)
    }

    /// Direct S→C 0x9D action 5 (`0x0053D010`, §6.4) for an item of the
    /// owner, queued now through `MovePending::send`: the stored page set
    /// to `shown` first (`0x00628320`), item flags OR-ed with `flags` in
    /// the message.
    pub fn send_item_page(
        &mut self,
        owner: UnitId,
        item: UnitId,
        flags: u32,
        shown: u8,
    ) -> Result<(), MoveFatal> {
        let (Some(o), g) = (self.owner_of(owner), self.guid_of(item)) else {
            return Ok(());
        };
        self.set_stored_page(g, shown);
        deferred::send_item_page(self, o, g, flags, shown)
    }
}
