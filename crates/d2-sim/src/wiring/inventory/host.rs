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
use crate::items::moves::{deferred, mode, MoveFatal, MovePending, MoveUnits};
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

    /// The NPC grid placement of a store item (`0x00560200`, `world/vendors.md`
    /// §3.1 rule 4): the NPC's inventory (made on first use, `0x0063ABD0`,
    /// the monster record by page, `inventory.md` §1.3) takes the item on
    /// its store page at the first free position; no send (the store
    /// stream is the trade open's 0x9C action 11, `vendors.md` §4 step 3).
    /// False when the page has no room.
    pub fn store_place(&mut self, npc: UnitId, item: UnitId) -> bool {
        if let (Some(kind), Some(r)) = (self.kind_of(npc), self.econ.units.get(npc)) {
            self.state.add_inventory(npc, kind, r.guid);
        }
        self.place(npc, item, (0, 0), true, false)
    }

    /// The placement of a gamble-list item (`world/vendors.md` §5.1
    /// step 7): the node's inventory for (`npc`, `player` GUID), owned by
    /// the NPC and made on first use (`0x0063ABD0`), takes the item at the
    /// first free position of its page (`0x00560200`, no send: the list
    /// reaches the client by the gamble open's 0x9C action 11). False when
    /// the page has no room. Recorded: `a2-npc-elzix-gamble` frame 18, the
    /// 14 items in mode 0 at grid positions (ring at (9, 0)).
    pub fn gamble_place(&mut self, npc: UnitId, player: u32, item: UnitId) -> bool {
        let (Some(kind), Some(guid)) =
            (self.kind_of(npc), self.econ.units.get(npc).map(|r| r.guid))
        else {
            return false;
        };
        let mut inv = self
            .state
            .gambles
            .remove(&(npc, player))
            .unwrap_or_else(|| Inventory::new(npc, kind, guid));
        let t = self.tables;
        let item = self.state.items.contains_key(&item).then_some(item);
        let placed = place_in_page(&mut inv, self, t, item, 0, 0, true, false);
        self.state.gambles.insert((npc, player), inv);
        self.sync_out();
        self.flush_equip();
        placed
    }

    /// Unlinks a gamble-list item from the gamble inventory that holds it
    /// (`0x0063AAF0`), as [`Self::store_unlink`] does for a store item:
    /// the unlinked item is in mode 4. False when no gamble inventory
    /// holds it.
    pub fn gamble_unlink(&mut self, item: UnitId) -> bool {
        let Some(key) = self
            .state
            .gambles
            .iter()
            .find(|(_, i)| i.items().contains(&item))
            .map(|(&k, _)| k)
        else {
            return false;
        };
        let Some(mut inv) = self.state.gambles.remove(&key) else {
            return false;
        };
        let removed = inv.unlink(self, item);
        self.state.gambles.insert(key, inv);
        self.sync_out();
        self.flush_equip();
        if removed {
            let g = self.guid_of(item);
            self.set_mode(g, mode::CURSOR);
        }
        removed
    }

    /// Unlinks a store item from the NPC inventory that holds it (a
    /// monster-owned inventory; `0x0063AAF0`, §1.4 rule 1), for the take
    /// of a purchase and the removal of a store clear (`vendors.md` §6
    /// rule 4, §7.1 rule 10). False when no NPC inventory holds it.
    pub fn store_unlink(&mut self, item: UnitId) -> bool {
        let holder = self
            .state
            .inventories
            .iter()
            .find(|(_, i)| {
                matches!(
                    i.owner_kind,
                    crate::items::inventory::UnitKind::Monster { .. }
                ) && i.items().contains(&item)
            })
            .map(|(&u, _)| u);
        let Some(npc) = holder else {
            return false;
        };
        let removed = self.remove(npc, item);
        if removed {
            // The taken store item reaches the client in mode 4 (1.14d,
            // `items-vendor-akara-buy` frame 24: the 0x9C action 12 record).
            let g = self.guid_of(item);
            self.set_mode(g, mode::CURSOR);
        }
        removed
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

    /// Takes `item` off the owner's cursor and consumes it (`0x0055EEA0`;
    /// `world/vendors.md` §7.2 rule 9, mode 4). False when it is not the
    /// owner's cursor item.
    ///
    /// PROVISIONAL (world/vendors.md §7.2 r9; REC-278): the routine has no
    /// written body; it is read as the stack merge's consume of a cursor
    /// item (`items/inventory-moves.md` §7.12: cursor := none with S→C
    /// 0x42, then the free `0x00557FD0`), which the recorded sale from the
    /// cursor (S→C 0x42 before the 0x2A) matches; 0x42 names the player
    /// (the client clears its own cursor item); settled by a capture of a
    /// cursor sale with the 0x42 bytes.
    pub fn take_cursor(&mut self, owner: UnitId, item: UnitId) -> bool {
        if self.state.cursor_of(owner) != Some(item) {
            return false;
        }
        let Some(o) = self.owner_of(owner) else {
            return false;
        };
        // S→C 0x42 names the player whose cursor clears
        // (`client/msg-stats-items.md` §3 rule 1).
        let pg = self.guid_of(owner);
        MovePending::send(self, o, crate::items::moves::layouts::clear_cursor(0, pg));
        self.free(item);
        true
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

    /// The shown store item (`vendors.md` §4 step 3) to the trading
    /// player: S→C 0x9C action 0x0B with the store stream
    /// (`deferred::store_item_message`), queued now.
    pub fn send_store_item(&mut self, player: UnitId, item: UnitId) -> Result<(), MoveFatal> {
        let (Some(o), g) = (self.owner_of(player), self.guid_of(item)) else {
            return Ok(());
        };
        for m in deferred::store_item_message(self, g)? {
            MovePending::send(self, o, m);
        }
        Ok(())
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
