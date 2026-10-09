// Spec: specs/world/vendors.md §3, §6, §7, §9.2; specs/world/quests.md §1.1, §6.7; specs/items/generation.md §2, §3; specs/sim/units.md §3
//! [`VendorWorld`] on the real providers: game fields
//! ([`crate::wiring::economy::GameFields`]), unit records (GUID, mode,
//! unit +0xC8), the unit lists (GUID lookups), the stat lists, the
//! player's quest flags, item creation and removal through
//! [`crate::wiring::economy::Economy`], the item data of its store and
//! the price inputs read from them. Inventories, player data, messages
//! and the item functions no items spec writes stay a seam
//! ([`VendorRest`]).

use super::npc_vendors::VendorDesk;
use super::NpcRest;
use super::{InteractionError, PlayerQuestsRef};
use crate::items::{create::CreateError, ItemRequest};
use crate::stats::{key_layer, key_stat};
use crate::units::lifecycle::LifecycleHooks;
use crate::units::{UnitId, UnitType};
use crate::wiring::economy::{EconomyError, ItemSpawn, QuestRest};
use crate::world::quests;
use crate::world::vendors::price::{Bonus, PriceItem};
use crate::world::vendors::{stat, store, Transaction, VendorWorld};

/// Item mode of a created store item (`vendors.md` §3.1 rule 2: "mode
/// 4").
const MODE_INVENTORY: u32 = 4;

/// The vendor calls no written spec provides yet, each with its expected
/// provider (`docs/handoff/impl-vendors.md` §4).
pub trait VendorRest {
    // ---- rooms (DRLG)
    fn players_in_level(&self, level: u16) -> i32;
    fn player_level_id(&self, player: UnitId) -> u16;
    // ---- player data (player spec)
    fn gold_cap(&self, player: UnitId) -> i32;
    fn stash_cap(&self, player: UnitId) -> i32;
    fn drop_gold(&mut self, player: UnitId, amount: i32);
    fn last_bought(&self, player: UnitId) -> u32;
    fn set_last_bought(&mut self, player: UnitId, guid: u32);
    fn has_cursor_item(&self, player: UnitId) -> bool;
    // ---- item functions not in the items specs
    fn copy_item(&mut self, item: UnitId) -> Option<UnitId>;
    fn has_filled_sockets(&self, item: UnitId) -> bool;
    /// The items in the item's inventory (sockets), in inventory order.
    fn socketed(&self, item: UnitId) -> Vec<UnitId>;
    /// The (B) bonus entries of `0x00625560` (`vendors.md` Open question
    /// 1).
    fn price_bonuses(&self, item: UnitId) -> Vec<Bonus>;
    fn recharge(&mut self, item: UnitId);
    fn repair_broken(&mut self, item: UnitId);
    // ---- messages (transport)
    fn send_transaction(&mut self, player: UnitId, t: Transaction);
    /// The buy price of a store item the player was just shown (S→C 0x9C
    /// action 11). The original client computes it from its own tables;
    /// d2rs's preview client has no price tables, so the host publishes it
    /// here (d2rs-own, unverified). Default: not published.
    fn store_price(&mut self, _player: UnitId, _item_guid: u32, _price: u32) {}
    // ---- NPC inventories (inventory spec)
    fn new_store_inventory(&mut self, npc_class: u16, npc: Option<UnitId>);
    fn place_in_store(&mut self, npc_class: u16, item: UnitId) -> bool;
    fn remove_store_item(&mut self, npc_class: u16, item: UnitId);
    fn take_from_store(&mut self, npc_class: u16, item: UnitId);
    fn place_in_gamble(&mut self, npc_class: u16, player: u32, item: UnitId) -> bool;
    fn remove_gamble_item(&mut self, npc_class: u16, player: u32, item: UnitId);
    fn refresh_npc_inventory(&mut self, npc: UnitId);
    fn add_trade_inventory(&mut self, npc_class: u16, item: UnitId);
    // ---- player inventories (inventory spec)
    fn owns_item(&self, player: UnitId, item: UnitId) -> bool;
    fn in_inventory(&self, player: UnitId, item: UnitId) -> bool;
    fn equipped_items(&self, player: UnitId) -> Vec<UnitId>;
    fn find_tome(&self, player: UnitId, scroll: UnitId) -> Option<(UnitId, i32)>;
    fn add_to_tome(&mut self, tome: UnitId, k: i32);
    fn find_partial_stack(&self, player: UnitId, item: UnitId) -> Option<(UnitId, i32)>;
    fn can_belt(&self, player: UnitId, item: UnitId) -> bool;
    fn put_in_belt(&mut self, player: UnitId, item: UnitId) -> bool;
    fn equip_ammo(&mut self, player: UnitId, item: UnitId) -> bool;
    fn place_in_backpack(&mut self, player: UnitId, item: UnitId) -> bool;
    fn take_from_cursor(&mut self, player: UnitId, item: UnitId) -> bool;
    fn lower_book_skill(&mut self, player: UnitId, item: UnitId, n: i32);
    fn remove_stored(&mut self, player: UnitId, item: UnitId);
    fn unequip(&mut self, player: UnitId, item: UnitId) -> bool;
}

impl<H, R> VendorDesk<'_, '_, '_, H, R>
where
    H: LifecycleHooks,
    R: NpcRest + VendorRest + QuestRest + PlayerQuestsRef,
{
    /// The entries of `stat` on the unit's list as (layer, value), in key
    /// order (the full array of an extended list, else the base array).
    fn entries(&self, unit: UnitId, s: u16) -> Vec<(u16, i32)> {
        let stats = &self.desk.econ.stats;
        let Some(l) = stats.unit_list(unit) else {
            return Vec::new();
        };
        let all = if stats.is_extended(l) {
            stats.full_entries(l)
        } else {
            stats.base_entries(l)
        };
        all.into_iter()
            .filter(|&(k, _)| key_stat(k) == s)
            .map(|(k, v)| (key_layer(k), v))
            .collect()
    }

    /// The NPC unit of the call (the trade open's NPC, `vendors.md` §3.1
    /// rule 2 "owner NPC"), when it belongs to `npc_class`.
    fn record_npc(&self, npc_class: u16) -> Option<UnitId> {
        self.npc.filter(|&n| {
            self.desk
                .econ
                .units
                .get(n)
                .is_some_and(|r| r.class == u32::from(npc_class))
        })
    }
}

impl<H, R> VendorWorld for VendorDesk<'_, '_, '_, H, R>
where
    H: LifecycleHooks,
    R: NpcRest + VendorRest + QuestRest + PlayerQuestsRef,
{
    // ---- game

    fn difficulty(&self) -> u8 {
        self.desk.econ.fields.difficulty
    }
    fn expansion(&self) -> bool {
        self.desk.econ.fields.expansion
    }
    fn item_format(&self) -> u16 {
        NpcRest::item_format(&*self.desk.rest)
    }
    fn game_type(&self) -> u8 {
        self.desk.econ.fields.game_type
    }

    // ---- units and stats

    fn guid(&self, unit: UnitId) -> u32 {
        self.desk.econ.units.get(unit).map_or(u32::MAX, |r| r.guid)
    }
    fn player_by_guid(&self, guid: u32) -> Option<UnitId> {
        self.desk.econ.game.lists.find_unit(UnitType::Player, guid)
    }
    fn item_by_guid(&self, guid: u32) -> Option<UnitId> {
        self.desk.econ.game.lists.find_unit(UnitType::Item, guid)
    }
    fn stat(&self, unit: UnitId, id: u16, layer: u16) -> i32 {
        self.desk.econ.stats.unit_total(unit, id, layer)
    }
    fn base_stat(&self, unit: UnitId, id: u16, layer: u16) -> i32 {
        self.desk.econ.stats.unit_base(unit, id, layer)
    }
    fn set_stat(&mut self, unit: UnitId, id: u16, layer: u16, value: i32) {
        let e = &mut *self.desk.econ;
        e.stats.unit_set(&mut *e.hooks, unit, id, value, layer);
    }
    /// The 16-bit slot word of the player's flag record of difficulty `d`
    /// (`quests.md` §1.1: 42 slots × 16 bits, LSB first).
    fn quest_slot(&self, player: UnitId, difficulty: u8, slot: u32) -> u16 {
        let Some(q) = self.desk.rest.quests_ref(player) else {
            return 0;
        };
        let (Some(f), Ok(s)) = (q.flags.get(usize::from(difficulty)), usize::try_from(slot)) else {
            return 0;
        };
        match f.0.get(2 * s..2 * s + 2) {
            Some(b) => u16::from_le_bytes([b[0], b[1]]),
            None => 0,
        }
    }
    fn players_in_level(&self, level: u16) -> i32 {
        self.desk.rest.players_in_level(level)
    }
    fn player_level_id(&self, player: UnitId) -> u16 {
        self.desk.rest.player_level_id(player)
    }
    /// `0x00545100(game, player, act)` (`quests.md` §6.7, called from
    /// `0x00537340`) for the town's act.
    fn town_entered(&mut self, player: UnitId, level: u16) {
        let Some(act) = store::town_act(level) else {
            return;
        };
        let (_, mut w) = self.desk.quest_world();
        quests::npc_gossip(&mut w, player, act);
    }
    /// `0x00622E70`: level (stat 12) × 10000 (`world/stash.rs`).
    fn gold_cap(&self, player: UnitId) -> i32 {
        VendorWorld::stat(self, player, crate::world::stash::stat::LEVEL, 0)
            .wrapping_mul(crate::world::stash::GOLD_PER_LEVEL)
    }
    /// `0x00623460`: the 1.14d constant (`world/stash.rs`).
    fn stash_cap(&self, _player: UnitId) -> i32 {
        crate::world::stash::STASH_CAP
    }
    fn drop_gold(&mut self, player: UnitId, amount: i32) {
        self.desk.rest.drop_gold(player, amount);
    }
    fn last_bought(&self, player: UnitId) -> u32 {
        self.desk.rest.last_bought(player)
    }
    fn set_last_bought(&mut self, player: UnitId, guid: u32) {
        self.desk.rest.set_last_bought(player, guid);
    }
    fn has_cursor_item(&self, player: UnitId) -> bool {
        self.desk.rest.has_cursor_item(player)
    }

    // ---- items

    /// `0x00559CE0` through [`crate::wiring::economy::Economy::create_item`]:
    /// the request unit is the record's NPC (class and level,
    /// `generation.md` Inputs), the game's item format, mode 4.
    ///
    /// TODO(vendors.md §3.1 rule 2): the allocation flags of `0x00559CE0`
    /// are not written; 1 as drops and the cube, so the store item is in
    /// the unit lists the 0x32 GUID lookup searches. The forced flag and
    /// the other request fields stay 0.
    ///
    /// §3.1 rule 2 / `generation.md` §10.2: never-ethereal 1 (request
    /// flags2 0x02, no-sockets 0), and on success item flag 0x10
    /// (identified), which the §3.1 rule 4 / §5.1 step 7 repair needs
    /// (§9.2 rule 0).
    fn create_item(
        &mut self,
        npc_class: u16,
        record: usize,
        quality: u8,
        ilvl: i32,
    ) -> Option<UnitId> {
        let unit = self
            .record_npc(npc_class)
            .and_then(|n| self.desk.econ.request_unit(n, None));
        let mut rq = ItemRequest {
            unit,
            ilvl,
            item: record as i32,
            format: VendorWorld::item_format(self),
            quality,
            flags2: crate::items::req::NEVER_ETHEREAL,
            ..ItemRequest::default()
        };
        let spawn = ItemSpawn {
            room: None,
            mode: MODE_INVENTORY,
            init_flags: 1,
        };
        match self.desk.econ.create_item(&mut rq, false, spawn) {
            Ok(u) => {
                if let Some(i) = self.desk.econ.items.get_mut(u) {
                    i.flags |= crate::items::flag::IDENTIFIED;
                }
                Some(u)
            }
            Err(EconomyError::Create(CreateError::Fatal(f))) => {
                self.desk
                    .state
                    .errors
                    .push(InteractionError::Economy(f.into()));
                None
            }
            Err(EconomyError::Create(_)) => None,
            Err(e) => {
                self.desk.state.errors.push(InteractionError::Economy(e));
                None
            }
        }
    }
    fn copy_item(&mut self, item: UnitId) -> Option<UnitId> {
        self.desk.rest.copy_item(item)
    }
    /// `0x00555600` (`units.md` §3.2) and the item data.
    fn destroy_item(&mut self, item: UnitId) {
        if let Err(e) = self.desk.econ.free_item(item) {
            self.desk.state.errors.push(InteractionError::Economy(e));
        }
    }
    fn item_record(&self, item: UnitId) -> usize {
        self.desk
            .econ
            .items
            .get(item)
            .map_or(usize::MAX, |i| i.record)
    }
    fn item_quality(&self, item: UnitId) -> u8 {
        self.desk.econ.items.get(item).map_or(0, |i| i.quality)
    }
    fn item_file_index(&self, item: UnitId) -> i32 {
        self.desk.econ.items.get(item).map_or(-1, |i| i.file_index)
    }
    fn item_flags(&self, item: UnitId) -> u32 {
        self.desk.econ.items.get(item).map_or(0, |i| i.flags)
    }
    fn set_item_flags(&mut self, item: UnitId, flags: u32) {
        if let Some(i) = self.desk.econ.items.get_mut(item) {
            i.flags = flags;
        }
    }
    /// Unit +0xC8.
    fn or_unit_flags(&mut self, item: UnitId, bits: u32) {
        if let Some(r) = self.desk.econ.units.get_mut(item) {
            r.flags2 |= bits;
        }
    }
    fn item_mode(&self, item: UnitId) -> u32 {
        self.desk.econ.units.get(item).map_or(0, |r| r.mode)
    }
    /// TODO(vendors.md §7): the item mode set the vendor code calls is not
    /// named; unit +0x10 is written.
    fn set_item_mode(&mut self, item: UnitId, mode: u32) {
        if let Some(r) = self.desk.econ.units.get_mut(item) {
            r.mode = mode;
        }
    }
    fn set_item_page(&mut self, item: UnitId, page: u8) {
        if let Some(i) = self.desk.econ.items.get_mut(item) {
            i.inv_page = page;
        }
    }
    fn has_filled_sockets(&self, item: UnitId) -> bool {
        self.desk.rest.has_filled_sockets(item)
    }
    /// §9.2 Inputs from the item data and the item's stats: stats 70,
    /// 152, 72, 73, 252, 253, 254 as unit totals, 31 as the base, the
    /// entries of 107 and 204; the (B) bonuses and the socketed items
    /// from the rest.
    ///
    /// TODO(vendors.md §9.2 Inputs): the readers are not named; totals
    /// except the base of 31, and the entries of the full array of an
    /// extended list (else the base array).
    fn price_item(&self, item: UnitId) -> Option<PriceItem> {
        let i = self.desk.econ.items.get(item)?;
        let total = |s| self.desk.econ.stats.unit_total(item, s, 0);
        Some(PriceItem {
            record: i.record,
            quality: i.quality,
            flags: i.flags,
            file_index: i.file_index,
            prefix: i.prefix,
            suffix: i.suffix,
            auto_affix: i.auto_affix,
            ear_level: i.ear_level,
            format: i.format,
            quantity: total(stat::QUANTITY),
            armor_base: self.desk.econ.stats.unit_base(item, stat::ARMORCLASS, 0),
            indestructible: total(stat::INDESTRUCTIBLE),
            durability: total(stat::DURABILITY),
            max_durability: total(stat::MAXDURABILITY),
            replenish_durability: total(stat::REPLENISH_DURABILITY),
            replenish_quantity: total(stat::REPLENISH_QUANTITY),
            extra_stack: total(stat::EXTRA_STACK),
            item_skills: self.entries(item, stat::ITEM_SINGLESKILL),
            charges: self.entries(item, stat::CHARGED_SKILL),
            bonuses: self.desk.rest.price_bonuses(item),
            sockets: self
                .desk
                .rest
                .socketed(item)
                .into_iter()
                .filter_map(|s| self.desk.econ.items.get(s).map(|x| x.record))
                .collect(),
        })
    }
    fn recharge(&mut self, item: UnitId) {
        self.desk.rest.recharge(item);
    }
    fn repair_broken(&mut self, item: UnitId) {
        self.desk.rest.repair_broken(item);
    }
    /// `0x00562590` (the NPC side's identify, `npc.md` §6).
    fn identify(&mut self, item: UnitId) {
        NpcRest::identify(&mut *self.desk.rest, item);
    }
    /// S→C 0x3E (`0x0053D130(client, item, 1, stat, value, 0)`) with the
    /// item's base stat value (layer 0, `client/msg-stats-items.md` §5
    /// r1.2), through the rest's transport. An item without a record
    /// sends nothing. PROVISIONAL (`client/msg-stats-items.md` §5 r1.3;
    /// REC-400): field widths, see `units::messages::update_item_stat`.
    fn send_item_stat(&mut self, player: UnitId, item: UnitId, stat: u16) {
        let Some(guid) = self.desk.econ.units.get(item).map(|r| r.guid) else {
            return;
        };
        let value = self.desk.econ.stats.unit_base(item, stat, 0);
        let msg = crate::units::messages::update_item_stat(guid, stat, value, 0);
        QuestRest::send(&mut *self.desk.rest, player, &msg);
    }
    fn send_transaction(&mut self, player: UnitId, t: Transaction) {
        self.desk.rest.send_transaction(player, t);
    }

    // ---- NPC inventories

    fn new_store_inventory(&mut self, npc_class: u16, npc: Option<UnitId>) {
        self.desk.rest.new_store_inventory(npc_class, npc);
    }
    fn place_in_store(&mut self, npc_class: u16, item: UnitId) -> bool {
        self.desk.rest.place_in_store(npc_class, item)
    }
    fn remove_store_item(&mut self, npc_class: u16, item: UnitId) {
        self.desk.rest.remove_store_item(npc_class, item);
    }
    fn take_from_store(&mut self, npc_class: u16, item: UnitId) {
        self.desk.state.taken.push(item);
        self.desk.rest.take_from_store(npc_class, item);
    }
    fn place_in_gamble(&mut self, npc_class: u16, player: u32, item: UnitId) -> bool {
        self.desk.rest.place_in_gamble(npc_class, player, item)
    }
    fn remove_gamble_item(&mut self, npc_class: u16, player: u32, item: UnitId) {
        self.desk.rest.remove_gamble_item(npc_class, player, item);
    }
    fn refresh_npc_inventory(&mut self, npc: UnitId) {
        self.desk.rest.refresh_npc_inventory(npc);
    }
    fn add_trade_inventory(&mut self, npc_class: u16, item: UnitId) {
        if !self.desk.state.shown.contains(&item) {
            self.desk.state.shown.push(item);
        }
        self.desk.rest.add_trade_inventory(npc_class, item);
    }

    // ---- player inventories

    fn owns_item(&self, player: UnitId, item: UnitId) -> bool {
        self.desk.rest.owns_item(player, item)
    }
    fn in_inventory(&self, player: UnitId, item: UnitId) -> bool {
        self.desk.rest.in_inventory(player, item)
    }
    fn equipped_items(&self, player: UnitId) -> Vec<UnitId> {
        self.desk.rest.equipped_items(player)
    }
    fn find_tome(&self, player: UnitId, scroll: UnitId) -> Option<(UnitId, i32)> {
        self.desk.rest.find_tome(player, scroll)
    }
    fn add_to_tome(&mut self, tome: UnitId, k: i32) {
        self.desk.rest.add_to_tome(tome, k);
    }
    fn find_partial_stack(&self, player: UnitId, item: UnitId) -> Option<(UnitId, i32)> {
        self.desk.rest.find_partial_stack(player, item)
    }
    fn can_belt(&self, player: UnitId, item: UnitId) -> bool {
        self.desk.rest.can_belt(player, item)
    }
    fn put_in_belt(&mut self, player: UnitId, item: UnitId) -> bool {
        self.desk.rest.put_in_belt(player, item)
    }
    fn equip_ammo(&mut self, player: UnitId, item: UnitId) -> bool {
        self.desk.rest.equip_ammo(player, item)
    }
    fn place_in_backpack(&mut self, player: UnitId, item: UnitId) -> bool {
        self.desk.rest.place_in_backpack(player, item)
    }
    fn take_from_cursor(&mut self, player: UnitId, item: UnitId) -> bool {
        self.desk.rest.take_from_cursor(player, item)
    }
    fn lower_book_skill(&mut self, player: UnitId, item: UnitId, n: i32) {
        self.desk.rest.lower_book_skill(player, item, n);
    }
    fn remove_stored(&mut self, player: UnitId, item: UnitId) {
        self.desk.rest.remove_stored(player, item);
    }
    fn unequip(&mut self, player: UnitId, item: UnitId) -> bool {
        self.desk.rest.unequip(player, item)
    }
}
