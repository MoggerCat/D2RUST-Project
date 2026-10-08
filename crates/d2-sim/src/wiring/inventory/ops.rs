// Spec: specs/items/inventory.md §1–§5
// Spec: specs/items/inventory-moves.md (§6–§11, split out of `inventory.md`)
//! [`InventoryOps`] on [`InvDesk`]: each operation is the
//! `items::inventory` function of the same rule, on the owner's
//! [`crate::items::inventory::Inventory`] (lent out of the state for a
//! mutating call) with the desk as its world.

use super::{InvDesk, InvError, InvRest};
use crate::items::inventory::{
    auto_belt_gate, auto_equip_location, belt, belt_item_check, busy, compact_belt,
    cursor_item_check, equip, equip_check, equip_from_cursor, find_free_position, free_belt_slot,
    grid_id, ground_or_owned_check, item_move_gate, owned_item_check, place_at_body, place_at_page,
    place_in_belt_slot, place_in_page_from_cursor, requirements_met, stack_test, stored_item_check,
    stored_or_equipped_check, targeting_reset, trading, InvWorld, UnitKind, NO_GUID,
};
use crate::items::moves::{Guid, InventoryOps, Owner};
use crate::units::lifecycle::LifecycleHooks;

impl<H: LifecycleHooks, R: InvRest + ?Sized> InvDesk<'_, '_, H, R> {
    /// A guarded item lookup: the unit of `item`, or none.
    fn iu(&self, item: Guid) -> Option<crate::units::UnitId> {
        self.item_unit(item)
    }

    /// A §5 check on the owner's inventory.
    fn check(
        &self,
        player: Owner,
        item: Guid,
        f: fn(&crate::items::inventory::Inventory, &Self, u32) -> u8,
    ) -> u32 {
        u32::from(f(&self.inv_or_empty(player), self, item))
    }
}

impl<H: LifecycleHooks, R: InvRest + ?Sized> InvDesk<'_, '_, H, R> {
    /// The grid item list of grid `g` (grid list order).
    fn grid_items(&self, owner: Owner, g: usize) -> Vec<Guid> {
        self.inventory(owner)
            .and_then(|i| i.grid(g))
            .map(|gr| gr.items.iter().map(|&u| self.guid_of(u)).collect())
            .unwrap_or_default()
    }
}

impl<H: LifecycleHooks, R: InvRest + ?Sized> InventoryOps for InvDesk<'_, '_, H, R> {
    // ---- §1.4 ----------------------------------------------------------

    fn has_inventory(&self, owner: Owner) -> bool {
        self.inventory(owner).is_some()
    }
    fn cursor(&self, owner: Owner) -> Option<Guid> {
        let c = self.inventory(owner)?.cursor()?;
        Some(self.guid_of(c))
    }
    fn set_cursor(&mut self, owner: Owner, item: Option<Guid>) {
        let unit = item.and_then(|g| self.iu(g));
        self.with_inv(owner, |inv, d| inv.put_cursor(d, unit));
    }
    fn items(&self, owner: Owner) -> Vec<Guid> {
        self.inventory(owner)
            .map(|i| i.items().iter().map(|&u| self.guid_of(u)).collect())
            .unwrap_or_default()
    }
    fn unlink(&mut self, owner: Owner, item: Guid) -> bool {
        let Some(u) = self.iu(item) else {
            return false;
        };
        self.with_inv(owner, |inv, d| inv.unlink(d, u))
            .unwrap_or(false)
    }
    fn update_list(&self, owner: Owner) -> Vec<Guid> {
        self.inventory(owner)
            .map(|i| i.update_list().to_vec())
            .unwrap_or_default()
    }
    fn update_list_add(&mut self, owner: Owner, item: Guid) {
        if let Some(u) = self.unit_of(owner) {
            if let Some(inv) = self.state.inventories.get_mut(&u) {
                inv.push_update(item);
            }
        }
    }
    fn update_list_free(&mut self, owner: Owner) {
        if let Some(u) = self.unit_of(owner) {
            if let Some(inv) = self.state.inventories.get_mut(&u) {
                inv.take_updates();
            }
        }
    }
    fn weapon_in_use(&self, owner: Owner) -> Option<Guid> {
        let w = self.inventory(owner)?.weapon_guid;
        (w != NO_GUID).then_some(w)
    }

    // ---- §2 ------------------------------------------------------------

    fn place_at(&mut self, owner: Owner, item: Guid, page: u8, x: i32, y: i32) -> bool {
        let Some(u) = self.iu(item) else {
            return false;
        };
        let t = self.tables;
        self.with_inv(owner, |inv, d| place_at_page(inv, d, t, u, page, x, y))
            .unwrap_or(false)
    }
    /// §2.3 on a copy of the inventory (the search may create the grid).
    fn find_free(&self, owner: Owner, item: Guid, page: u8) -> Option<(i32, i32)> {
        let u = self.iu(item)?;
        let mut inv = self.inventory(owner)?.clone();
        find_free_position(&mut inv, self, self.tables, u, page)
    }
    fn place_in_page(
        &mut self,
        owner: Owner,
        item: Guid,
        x: i32,
        y: i32,
        find: bool,
        send: bool,
    ) -> bool {
        let u = self.iu(item);
        let t = self.tables;
        self.with_inv(owner, |inv, d| {
            place_in_page_from_cursor(inv, d, t, u, x, y, find, send)
        })
        .unwrap_or(false)
    }
    fn link_check(&mut self, owner: Owner, item: Guid, kind: u8) -> bool {
        match (self.unit_of(owner), self.iu(item)) {
            (Some(o), Some(i)) => InvWorld::link_check(self, o, i, kind),
            _ => false,
        }
    }
    /// §7.19 step 3: the target's inventory is created if needed
    /// (`0x0063ABD0`), the filler is linked into it (no grid).
    fn link_into_item(&mut self, target: Guid, filler: Guid) -> bool {
        let (Some(t), Some(f)) = (self.iu(target), self.iu(filler)) else {
            return self.rest.link_into_item(target, filler);
        };
        self.state.add_inventory(t, UnitKind::Item, target);
        self.with_inv(Owner::item(target), |inv, d| inv.link(d, f, None))
            .is_some()
    }

    // ---- §3 ------------------------------------------------------------

    fn beltable(&self, item: Guid) -> bool {
        self.iu(item)
            .and_then(|u| self.state.items.get(&u))
            .is_some_and(|d| belt::beltable(self.tables, d.record))
    }
    fn auto_belt_gate(&self, owner: Owner, item: Guid) -> bool {
        match (self.inventory(owner), self.iu(item)) {
            (Some(inv), Some(u)) => auto_belt_gate(inv, self, self.tables, u),
            _ => false,
        }
    }
    fn belt_free_slot(&self, owner: Owner, item: Guid) -> Option<u8> {
        free_belt_slot(self.inventory(owner)?, self, self.tables, self.iu(item)?)
    }
    fn belt_place(&mut self, owner: Owner, item: Guid, slot: u32) -> bool {
        let (Some(u), Ok(slot)) = (self.iu(item), u8::try_from(slot)) else {
            return false;
        };
        let t = self.tables;
        self.with_inv(owner, |inv, d| place_in_belt_slot(inv, d, t, u, slot))
            .unwrap_or(false)
    }
    fn belt_compact(&mut self, owner: Owner, slot: u8) {
        let t = self.tables;
        self.with_inv(owner, |inv, d| {
            compact_belt(inv, d, t, slot);
        });
    }

    fn belt_item(&self, owner: Owner, slot: u8) -> Option<Guid> {
        let u = self.inventory(owner)?.belt_item(slot)?;
        Some(self.guid_of(u))
    }
    fn belt_boxes(&self, belt: Option<Guid>) -> u8 {
        belt::belt_boxes_of(self, self.tables, belt.and_then(|g| self.iu(g)))
    }
    fn page_items(&self, owner: Owner, page: u8) -> Vec<Guid> {
        self.grid_items(owner, grid_id::PAGE + usize::from(page))
    }
    fn body_items(&self, owner: Owner) -> Vec<Guid> {
        self.grid_items(owner, grid_id::BODY)
    }

    // ---- §4 ------------------------------------------------------------

    fn body_item(&self, owner: Owner, loc: u8) -> Option<Guid> {
        let u = self.inventory(owner)?.body_item(loc)?;
        Some(self.guid_of(u))
    }
    fn place_body(&mut self, owner: Owner, item: Guid, loc: u8) -> bool {
        let Some(u) = self.iu(item) else {
            return false;
        };
        self.with_inv(owner, |inv, d| place_at_body(inv, d, u, loc))
            .unwrap_or(false)
    }
    /// `0x0063BE30`: the §1.4 unlink before it already cleared the item's
    /// cell; a slot still held is logged ([`InvError::BodySlotHeld`]).
    fn clear_body_slot(&mut self, owner: Owner, loc: u8) {
        if let Some(u) = self.inventory(owner).and_then(|i| i.body_item(loc)) {
            self.state.errors.push(InvError::BodySlotHeld(u, loc));
        }
    }
    /// `0x0063E490` (§7.7): the item at the location, else the two-handed
    /// item in the other hand (equip check result 4).
    fn item_to_remove(&self, owner: Owner, loc: u8) -> Option<Guid> {
        let inv = self.inventory(owner)?;
        let u = inv.body_item(loc).or_else(|| {
            equip::other_hand(loc)
                .and_then(|o| inv.body_item(o))
                .filter(|&x| InvWorld::two_handed(self, x))
        })?;
        Some(self.guid_of(u))
    }
    /// `0x006289C0` ([`InvDesk::is_two_handed`]).
    fn two_handed(&self, item: Guid) -> bool {
        self.item_unit(item).is_some_and(|u| self.is_two_handed(u))
    }
    fn requirements(&self, item: Guid, unit: Owner, equipping: bool) -> bool {
        match self.unit_of(unit) {
            Some(u) => requirements_met(self, self.tables, self.iu(item), u, equipping),
            None => false,
        }
    }
    fn equip_check(&self, unit: Owner, loc: u8, item: Option<Guid>, skip: bool) -> u8 {
        let (Some(u), Some(inv)) = (self.unit_of(unit), self.inventory(unit)) else {
            return equip::res::NO;
        };
        let n = match item {
            Some(g) => match self.iu(g) {
                Some(n) => Some(n),
                None => return equip::res::NO,
            },
            None => None,
        };
        equip_check(inv, self, self.tables, u, loc, n, skip)
    }
    fn stack_test(&self, a: Guid, b: Guid) -> bool {
        match (self.iu(a), self.iu(b)) {
            (Some(a), Some(b)) => stack_test(self, self.tables, a, b),
            _ => false,
        }
    }
    fn equip_from_cursor(
        &mut self,
        player: Owner,
        item: Guid,
        loc: u8,
        skip: bool,
    ) -> (bool, bool) {
        let u = self.iu(item);
        let t = self.tables;
        self.with_inv(player, |inv, d| equip_from_cursor(inv, d, t, u, loc, skip))
            .map_or((false, false), |o| (o.ok, o.out))
    }
    fn auto_equip(&self, unit: Owner, item: Guid, skip: bool) -> Option<u8> {
        auto_equip_location(
            self.inventory(unit)?,
            self,
            self.tables,
            self.iu(item)?,
            skip,
        )
    }

    // ---- §5 ------------------------------------------------------------

    fn check_cursor_item(&self, player: Owner, item: Guid) -> u32 {
        self.check(player, item, cursor_item_check)
    }
    fn check_stored(&self, player: Owner, item: Guid) -> u32 {
        self.check(player, item, stored_item_check)
    }
    fn check_stored_or_equipped(&self, player: Owner, item: Guid) -> u32 {
        self.check(player, item, stored_or_equipped_check)
    }
    fn check_owned(&self, player: Owner, item: Guid) -> u32 {
        self.check(player, item, owned_item_check)
    }
    fn check_belt(&self, player: Owner, item: Guid) -> u32 {
        self.check(player, item, belt_item_check)
    }
    fn check_ground_or_owned(&self, player: Owner, item: Guid) -> u32 {
        self.check(player, item, ground_or_owned_check)
    }
    fn busy(&self, player: Owner) -> bool {
        busy(&self.inv_or_empty(player), self)
    }
    fn trading(&self, player: Owner) -> bool {
        trading(&self.inv_or_empty(player), self)
    }
    fn targeting_reset(&mut self, player: Owner) {
        self.with_inv(player, |inv, d| targeting_reset(inv, d));
    }
    /// §5.4 takes the player only; the item argument is not read.
    fn item_move_gate(&mut self, player: Owner, _item: Option<Guid>) -> bool {
        self.with_inv(player, |inv, d| item_move_gate(inv, d))
            .unwrap_or(false)
    }
}
