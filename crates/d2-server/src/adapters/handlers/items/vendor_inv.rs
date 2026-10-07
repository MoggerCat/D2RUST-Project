// Spec: specs/world/vendors.md §7, §8; specs/items/inventory.md §1.4, §1.2, §2.4
// Spec: specs/items/inventory-moves.md (§6–§11, split out of `inventory.md`)
//! The vendor world of the wired host on the game's one inventory model:
//! [`InvVendors`] wraps the interaction wiring's vendor world
//! ([`VendorDesk`] over the economy) and answers the player-inventory calls of
//! `vendors.md` §7–§8 from the inventory parts the item-move handlers use
//! ([`InvParts`], `d2_sim::wiring::inventory`): an item placed by C→S 0x18
//! is the player's at a vendor, and an item sold leaves that inventory.
//! Every other call goes to the wrapped world unchanged.
//!
//! Wired to the model: `has_cursor_item` (§1.4 rule 3), `owns_item`,
//! `in_inventory`, `equipped_items` (grid 0, §1.2), `place_in_backpack`
//! (§2.4), `remove_stored` (§1.4 unlink, then the free), `copy_item`
//! (§7.3, `InvDesk::copy_of`). Still the rest's
//! (no written rule for the routine): `take_from_cursor` (`0x0055EEA0`),
//! `unequip` (`0x00560CD0` by item), `can_belt` / `put_in_belt`
//! (`0x0055E9B0`'s arguments here), `equip_ammo` (Open question 3),
//! `find_tome`, `add_to_tome`, `find_partial_stack`, `lower_book_skill`.
//!
//! A host without inventory parts sees an empty inventory: nothing is
//! owned, placement fails (the inventory wiring's reading for a unit
//! without an inventory, `inventory.md` §5).

use d2_sim::items::moves::{deferred, MoveUnits};
use d2_sim::rng::Seed;
use d2_sim::units::lifecycle::LifecycleHooks;
use d2_sim::units::UnitId;
use d2_sim::wiring::economy::QuestRest;
use d2_sim::wiring::interaction::{NpcRest, PlayerQuestsRef, VendorDesk, VendorRest};
use d2_sim::wiring::inventory::{InvDesk, InvError, InvState};
use d2_sim::world::vendors::price::PriceItem;
use d2_sim::world::vendors::{NpcLink, Transaction, VendorWorld};

use super::moves::{take_sent, InvParts, MoveRest};

/// The interaction wiring's vendor world with the player inventories of
/// `inv`. Messages the inventory rules queue (`MovePending::send`) are
/// collected in `sent` (receiving unit, bytes), in call order.
pub struct InvVendors<'i, 'v, 'd, 'a, H, R> {
    pub inner: VendorDesk<'v, 'd, 'a, H, R>,
    pub inv: Option<&'i mut InvParts>,
    pub sent: Vec<(UnitId, Vec<u8>)>,
}

impl<'i, 'v, 'd, 'a, H, R> InvVendors<'i, 'v, 'd, 'a, H, R>
where
    H: LifecycleHooks,
    R: NpcRest + VendorRest + QuestRest + PlayerQuestsRef,
{
    pub fn new(inner: VendorDesk<'v, 'd, 'a, H, R>, inv: Option<&'i mut InvParts>) -> Self {
        Self {
            inner,
            inv,
            sent: Vec::new(),
        }
    }

    fn state(&self) -> Option<&InvState> {
        self.inv.as_deref().map(|p| &p.state)
    }

    /// Runs `f` on the inventory desk over the wrapped world's economy;
    /// what it sent is collected. `None` without inventory parts.
    fn with_desk<T>(
        &mut self,
        f: impl FnOnce(&mut InvDesk<'_, 'a, H, dyn MoveRest + Send + Sync>) -> T,
    ) -> Option<T> {
        let parts = self.inv.as_deref_mut()?;
        let mut d = parts.desk(&mut *self.inner.desk.econ);
        let out = f(&mut d);
        let sent = take_sent(&mut d);
        self.sent
            .extend(sent.into_iter().filter_map(|(u, b)| Some((u?, b))));
        Some(out)
    }
}

impl<H, R> NpcLink for InvVendors<'_, '_, '_, '_, H, R>
where
    H: LifecycleHooks,
    R: NpcRest + VendorRest + QuestRest + PlayerQuestsRef,
{
    fn npc_by_guid(&self, guid: u32) -> Option<UnitId> {
        self.inner.npc_by_guid(guid)
    }
    fn npc_class(&self, npc: UnitId) -> u16 {
        self.inner.npc_class(npc)
    }
    fn is_interact_unit(&self, player: UnitId, npc: UnitId) -> bool {
        self.inner.is_interact_unit(player, npc)
    }
    fn interaction_empty(&self, npc: UnitId) -> bool {
        self.inner.interaction_empty(npc)
    }
    fn hire_list_made(&self, class: u16) -> bool {
        self.inner.hire_list_made(class)
    }
    fn set_hire_list_made(&mut self, class: u16) {
        self.inner.set_hire_list_made(class)
    }
    fn make_hire_list(&mut self, class: u16, seed: &mut Seed) {
        self.inner.make_hire_list(class, seed)
    }
}

impl<H, R> VendorWorld for InvVendors<'_, '_, '_, '_, H, R>
where
    H: LifecycleHooks,
    R: NpcRest + VendorRest + QuestRest + PlayerQuestsRef,
{
    fn difficulty(&self) -> u8 {
        self.inner.difficulty()
    }
    fn expansion(&self) -> bool {
        self.inner.expansion()
    }
    fn item_format(&self) -> u16 {
        self.inner.item_format()
    }
    fn game_type(&self) -> u8 {
        self.inner.game_type()
    }
    fn guid(&self, unit: UnitId) -> u32 {
        self.inner.guid(unit)
    }
    fn player_by_guid(&self, guid: u32) -> Option<UnitId> {
        self.inner.player_by_guid(guid)
    }
    fn item_by_guid(&self, guid: u32) -> Option<UnitId> {
        self.inner.item_by_guid(guid)
    }
    fn stat(&self, unit: UnitId, id: u16, layer: u16) -> i32 {
        self.inner.stat(unit, id, layer)
    }
    fn base_stat(&self, unit: UnitId, id: u16, layer: u16) -> i32 {
        self.inner.base_stat(unit, id, layer)
    }
    fn set_stat(&mut self, unit: UnitId, id: u16, layer: u16, value: i32) {
        self.inner.set_stat(unit, id, layer, value)
    }
    fn quest_slot(&self, player: UnitId, difficulty: u8, slot: u32) -> u16 {
        self.inner.quest_slot(player, difficulty, slot)
    }
    fn players_in_level(&self, level: u16) -> i32 {
        self.inner.players_in_level(level)
    }
    fn player_level_id(&self, player: UnitId) -> u16 {
        self.inner.player_level_id(player)
    }
    fn town_entered(&mut self, player: UnitId, level: u16) {
        self.inner.town_entered(player, level)
    }
    fn gold_cap(&self, player: UnitId) -> i32 {
        self.inner.gold_cap(player)
    }
    fn stash_cap(&self, player: UnitId) -> i32 {
        self.inner.stash_cap(player)
    }
    fn drop_gold(&mut self, player: UnitId, amount: i32) {
        self.inner.drop_gold(player, amount)
    }
    fn last_bought(&self, player: UnitId) -> u32 {
        self.inner.last_bought(player)
    }
    fn set_last_bought(&mut self, player: UnitId, guid: u32) {
        self.inner.set_last_bought(player, guid)
    }
    /// The inventory's cursor item (+0x20, §1.4 rule 3).
    fn has_cursor_item(&self, player: UnitId) -> bool {
        self.state().is_some_and(|s| s.cursor_of(player).is_some())
    }
    fn create_item(
        &mut self,
        npc_class: u16,
        record: usize,
        quality: u8,
        ilvl: i32,
    ) -> Option<UnitId> {
        self.inner.create_item(npc_class, record, quality, ilvl)
    }
    /// `0x0055A2A0` (§7.3) on the inventory model: buy and sell pass
    /// fillers 1 (§7.1 rule 9.2, §7.2). Without inventory parts: the
    /// wrapped world's.
    fn copy_item(&mut self, item: UnitId) -> Option<UnitId> {
        if self.inv.is_none() {
            return self.inner.copy_item(item);
        }
        self.with_desk(|d| d.copy_of(item, true)).flatten()
    }
    fn destroy_item(&mut self, item: UnitId) {
        self.inner.destroy_item(item)
    }
    fn item_record(&self, item: UnitId) -> usize {
        self.inner.item_record(item)
    }
    fn item_quality(&self, item: UnitId) -> u8 {
        self.inner.item_quality(item)
    }
    fn item_file_index(&self, item: UnitId) -> i32 {
        self.inner.item_file_index(item)
    }
    fn item_flags(&self, item: UnitId) -> u32 {
        self.inner.item_flags(item)
    }
    fn set_item_flags(&mut self, item: UnitId, flags: u32) {
        self.inner.set_item_flags(item, flags)
    }
    fn or_unit_flags(&mut self, item: UnitId, bits: u32) {
        self.inner.or_unit_flags(item, bits)
    }
    fn item_mode(&self, item: UnitId) -> u32 {
        self.inner.item_mode(item)
    }
    fn set_item_mode(&mut self, item: UnitId, mode: u32) {
        self.inner.set_item_mode(item, mode)
    }
    fn set_item_page(&mut self, item: UnitId, page: u8) {
        self.inner.set_item_page(item, page)
    }
    fn has_filled_sockets(&self, item: UnitId) -> bool {
        self.inner.has_filled_sockets(item)
    }
    fn price_item(&self, item: UnitId) -> Option<PriceItem> {
        self.inner.price_item(item)
    }
    fn recharge(&mut self, item: UnitId) {
        self.inner.recharge(item)
    }
    fn repair_broken(&mut self, item: UnitId) {
        self.inner.repair_broken(item)
    }
    fn identify(&mut self, item: UnitId) {
        self.inner.identify(item)
    }
    fn send_item_stat(&mut self, player: UnitId, item: UnitId, stat: u16) {
        self.inner.send_item_stat(player, item, stat)
    }
    fn send_transaction(&mut self, player: UnitId, t: Transaction) {
        self.inner.send_transaction(player, t)
    }
    fn new_store_inventory(&mut self, npc_class: u16, npc: Option<UnitId>) {
        self.inner.new_store_inventory(npc_class, npc)
    }
    fn place_in_store(&mut self, npc_class: u16, item: UnitId) -> bool {
        self.inner.place_in_store(npc_class, item)
    }
    fn remove_store_item(&mut self, npc_class: u16, item: UnitId) {
        self.inner.remove_store_item(npc_class, item)
    }
    fn take_from_store(&mut self, npc_class: u16, item: UnitId) {
        self.inner.take_from_store(npc_class, item)
    }
    fn place_in_gamble(&mut self, npc_class: u16, player: u32, item: UnitId) -> bool {
        self.inner.place_in_gamble(npc_class, player, item)
    }
    fn remove_gamble_item(&mut self, npc_class: u16, player: u32, item: UnitId) {
        self.inner.remove_gamble_item(npc_class, player, item)
    }
    fn refresh_npc_inventory(&mut self, npc: UnitId) {
        self.inner.refresh_npc_inventory(npc)
    }
    fn add_trade_inventory(&mut self, npc_class: u16, item: UnitId) {
        self.inner.add_trade_inventory(npc_class, item)
    }

    // ---- player inventories (§7, §8) on the model ------------------------

    /// `0x00557FF0` (§7.2 rule 3): the player's cursor item or an item
    /// of its item list (any page, body location or belt). A ground item
    /// is in neither, so its sale is refused here (code 11, result 3)
    /// before any copy.
    fn owns_item(&self, player: UnitId, item: UnitId) -> bool {
        self.state().is_some_and(|s| s.holds(player, item))
    }
    /// §8.1 rule 4 "in the player's inventory": the same reading as
    /// [`Self::owns_item`].
    fn in_inventory(&self, player: UnitId, item: UnitId) -> bool {
        self.state().is_some_and(|s| s.holds(player, item))
    }
    /// `0x0062FE60` over the 13 body locations: the items of grid 0
    /// (`inventory.md` §1.2), in location order.
    fn equipped_items(&self, player: UnitId) -> Vec<UnitId> {
        self.state().map_or_else(Vec::new, |s| s.body_items(player))
    }
    fn find_tome(&self, player: UnitId, scroll: UnitId) -> Option<(UnitId, i32)> {
        self.inner.find_tome(player, scroll)
    }
    fn add_to_tome(&mut self, tome: UnitId, k: i32) {
        self.inner.add_to_tome(tome, k)
    }
    fn find_partial_stack(&self, player: UnitId, item: UnitId) -> Option<(UnitId, i32)> {
        self.inner.find_partial_stack(player, item)
    }
    fn can_belt(&self, player: UnitId, item: UnitId) -> bool {
        self.inner.can_belt(player, item)
    }
    fn put_in_belt(&mut self, player: UnitId, item: UnitId) -> bool {
        self.inner.put_in_belt(player, item)
    }
    fn equip_ammo(&mut self, player: UnitId, item: UnitId) -> bool {
        self.inner.equip_ammo(player, item)
    }
    /// §7.1 rule 9.7 "auto-place (`0x00560200`)": `inventory.md` §2.4
    /// with a free position and "send" (the recorded purchase's copy
    /// appears as 0x9C action 4 the next frame, rule 10).
    fn place_in_backpack(&mut self, player: UnitId, item: UnitId) -> bool {
        self.with_desk(|d| d.place(player, item, (0, 0), true, true))
            .unwrap_or(false)
    }
    fn take_from_cursor(&mut self, player: UnitId, item: UnitId) -> bool {
        self.inner.take_from_cursor(player, item)
    }
    fn lower_book_skill(&mut self, player: UnitId, item: UnitId, n: i32) {
        self.inner.lower_book_skill(player, item, n)
    }
    /// §7.2 rule 9, mode 0 (`0x00579963`–`0x0057998F`): the item's
    /// stored page (+0x47) := its page (+0x45), S→C 0x9D action 5 with
    /// command flags 0x20 (`0x0053D010`, `inventory-moves.md` §6.4), then
    /// removed (`0x0055DF10`): unlinked from the player's inventory (§1.4)
    /// and freed (`0x0055DF10` → `0x00557FD0`, `cube.md` §8 step 1).
    fn remove_stored(&mut self, player: UnitId, item: UnitId) {
        self.with_desk(|d| {
            let g = d.guid_of(item);
            if let Some(o) = d.owner_of(player) {
                let page = MoveUnits::page(d, g);
                MoveUnits::set_stored_page(d, g, page);
                if let Err(e) = deferred::send_item_page(d, o, g, 0x20, page) {
                    d.state.errors.push(InvError::Move(e));
                }
            }
            d.remove(player, item);
            d.free(item);
        });
    }
    fn unequip(&mut self, player: UnitId, item: UnitId) -> bool {
        self.inner.unequip(player, item)
    }
}
