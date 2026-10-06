// Spec: specs/world/npc.md §2–§4, §9; specs/world/vendors.md §3, §4, §7, §9 (end-to-end fixtures)
//! Fixtures shared by the end-to-end tests that run the server's
//! `TradeWorld` (`e2e_vendor.rs`, `e2e_single_player.rs`): the rest of
//! the NPC / vendor / quest wiring no written spec provides (staged
//! answers and a call log, never behaviour), and the synthetic item,
//! vendor and NPC tables. Each test crate uses a part of it.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use d2_data::fixup::maps::EquivMatrix;
use d2_data::tables::{Itemratio, Itemtypes, Monstats, Record};
use d2_server::adapters::handlers::world::Outbox;
use d2_sim::items::tables::ItemRec;
use d2_sim::items::{ty, ItemTables};
use d2_sim::units::UnitId;
use d2_sim::wiring::economy::QuestRest;
use d2_sim::wiring::interaction::{NpcRest, PlayerQuestsRef, VendorRest};
use d2_sim::world::npc::{self, class, ImbueMods, InvEntry, ItemFacts, MercInit};
use d2_sim::world::quests::{PlayerQuests, QuestChain, TextList, UnitKind};
use d2_sim::world::vendors::price::Bonus;
use d2_sim::world::vendors::{
    NpcPrices, Transaction, TypeRec, VendorItem, VendorTables, NO_CODE, XXX,
};

/// Item records (combined index; the vendor tables use the same order).
pub const CAP: usize = 0;
pub const BUC: usize = 1;
/// Rows of the synthetic tables.
pub const N_STATS: usize = 359;
pub const N_TYPES: usize = 80;
pub const N_MONSTATS: usize = 400;

pub fn blank<T: Record>() -> T {
    T::decode(&vec![0u8; T::SIZE])
}

/// The interaction seams no written spec provides
/// (`wire-interaction.md` §6): staged answers (positions as a fixed
/// distance, the player's interact unit, the player's quest records and
/// inventory, the NPC grid always having room, the carried-gold caps)
/// and a log of every call that would change state outside `d2-sim`.
/// The item copy `0x0055A2A0` answers null: no spec writes it.
#[derive(Default)]
pub struct Rest {
    pub interact: BTreeMap<UnitId, (u8, u32)>,
    pub quests: BTreeMap<UnitId, PlayerQuests>,
    /// The player's items (inventory spec): what `owns_item` answers.
    pub inventory: BTreeSet<UnitId>,
    pub last_bought: BTreeMap<UnitId, u32>,
    pub sent: Vec<(UnitId, Vec<u8>)>,
    pub log: Vec<String>,
}

impl Outbox for Rest {
    fn take_sent(&mut self) -> Vec<(UnitId, Vec<u8>)> {
        std::mem::take(&mut self.sent)
    }
}

impl PlayerQuestsRef for Rest {
    fn quests_ref(&self, player: UnitId) -> Option<&PlayerQuests> {
        self.quests.get(&player)
    }
}

impl NpcRest for Rest {
    /// Game +0x78; ≥ 1 keeps the request's quality (`quality.md` §3).
    fn item_format(&self) -> u16 {
        1
    }
    /// Unit distance (`0x00641530`, unit spec): within talk range (≤ 6).
    fn distance(&self, _: UnitId, _: UnitId) -> i32 {
        3
    }
    fn axis_check(&self, _: UnitId, _: UnitId) -> u32 {
        0
    }
    fn unit_check(&self, _: UnitId, _: u32) -> u32 {
        0
    }
    fn clear_path(&mut self, u: UnitId) {
        self.log.push(format!("clear path {}", u.0));
    }
    fn approach(&mut self, _: UnitId, _: UnitId) {
        self.log.push("approach".into());
    }
    fn player_busy(&self, _: UnitId) -> u32 {
        0
    }
    fn start_allowed(&self, _: UnitId, _: UnitId) -> bool {
        true
    }
    fn tristram_cain_busy(&self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn interact_unit(&self, player: UnitId) -> Option<(u8, u32)> {
        self.interact.get(&player).copied()
    }
    fn set_interact(&mut self, player: UnitId, t: u8, guid: u32) {
        self.interact.insert(player, (t, guid));
    }
    fn reset_interact(&mut self, player: UnitId) {
        self.interact.remove(&player);
    }
    fn pet(&self, _: UnitId, _: u8, _: u8) -> Option<UnitId> {
        None
    }
    fn pets(&self, _: UnitId) -> Vec<UnitId> {
        Vec::new()
    }
    fn player_name(&self, _: UnitId) -> Vec<u8> {
        b"tester".to_vec()
    }
    fn reset_stats(&mut self, _: UnitId) {}
    fn reset_skills(&mut self, _: UnitId) {}
    fn act_change(&mut self, _: UnitId, _: u32, _: u32) {}
    fn activate_waypoint(&mut self, _: UnitId, _: u32) {}
    fn npc_ai_param(&mut self, npc: UnitId, p: u32) {
        self.log.push(format!("ai param {} {p:#x}", npc.0));
    }
    fn stat_sent(&mut self, _: UnitId, stat: u16, value: u32) {
        self.log.push(format!("setstat {stat} {value}"));
    }
    fn respec_sound(&mut self, _: UnitId) {}
    /// `0x00661480` (`server-messages.tsv` 0x27 `partial`): 34 zero
    /// bytes.
    fn encode_text_list(&self, _: &TextList) -> [u8; 34] {
        [0; 34]
    }
    fn socket_granted(&mut self, _: UnitId) {}
    fn personalize_granted(&mut self, _: UnitId) {}
    fn inventory_entries(&self, _: UnitId) -> Vec<InvEntry> {
        Vec::new()
    }
    fn identify(&mut self, item: UnitId) {
        self.log.push(format!("identify {}", item.0));
    }
    fn cursor_item(&self, _: UnitId) -> Option<UnitId> {
        None
    }
    fn item_facts(&self, _: UnitId) -> ItemFacts {
        ItemFacts::default()
    }
    fn put_back(&mut self, _: UnitId, _: UnitId) {}
    fn remove_cursor_item(&mut self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn duplicate(&mut self, _: UnitId, _: UnitId) -> Option<UnitId> {
        None
    }
    fn create_imbued(&mut self, _: UnitId, _: UnitId, _: &ImbueMods) -> Option<UnitId> {
        None
    }
    fn item_refresh(&mut self, _: UnitId) {}
    fn personal_name(&self, _: UnitId) -> Vec<u8> {
        Vec::new()
    }
    fn set_personal_name(&mut self, _: UnitId, _: &[u8]) {}
    fn place_or_drop(&mut self, _: UnitId, _: UnitId) {}
    fn set_mode(&mut self, u: UnitId, mode: u8) {
        self.log.push(format!("mode {} {mode}", u.0));
    }
    fn spawn_mercenary(&mut self, _: UnitId, _: u32, _: u8) -> Option<UnitId> {
        None
    }
    fn init_mercenary(&mut self, _: UnitId, _: UnitId, _: &MercInit) {}
    fn revive_mercenary(&mut self, _: UnitId, _: UnitId) {}
}

impl VendorRest for Rest {
    fn players_in_level(&self, _: u16) -> i32 {
        1
    }
    fn player_level_id(&self, _: UnitId) -> u16 {
        1
    }
    fn gold_cap(&self, _: UnitId) -> i32 {
        100_000
    }
    fn stash_cap(&self, _: UnitId) -> i32 {
        100_000
    }
    fn drop_gold(&mut self, _: UnitId, amount: i32) {
        self.log.push(format!("drop gold {amount}"));
    }
    fn last_bought(&self, p: UnitId) -> u32 {
        self.last_bought.get(&p).copied().unwrap_or(u32::MAX)
    }
    fn set_last_bought(&mut self, p: UnitId, guid: u32) {
        self.last_bought.insert(p, guid);
    }
    fn has_cursor_item(&self, _: UnitId) -> bool {
        false
    }
    /// `0x0055A2A0`: no items spec writes the copy. Null: the vendor
    /// code's own refusal runs (`vendors.md` §7.1 rule 9.2, §7.2 rule 8).
    fn copy_item(&mut self, item: UnitId) -> Option<UnitId> {
        self.log.push(format!("copy {}", item.0));
        None
    }
    fn has_filled_sockets(&self, _: UnitId) -> bool {
        false
    }
    fn socketed(&self, _: UnitId) -> Vec<UnitId> {
        Vec::new()
    }
    fn price_bonuses(&self, _: UnitId) -> Vec<Bonus> {
        Vec::new()
    }
    fn recharge(&mut self, _: UnitId) {}
    fn repair_broken(&mut self, _: UnitId) {}
    fn send_item_stat(&mut self, _: UnitId, item: UnitId, stat: u16) {
        self.log.push(format!("item stat {} {stat}", item.0));
    }
    /// S→C 0x2A through its builder `0x0053D740` (`npc.md` §9).
    fn send_transaction(&mut self, p: UnitId, t: Transaction) {
        let m = npc::transaction(t.kind, t.code, t.guid, t.gold as u32);
        self.sent.push((p, m.to_vec()));
    }
    fn new_store_inventory(&mut self, class: u16, _: Option<UnitId>) {
        self.log.push(format!("new store {class}"));
    }
    /// The NPC grid (`0x00560200`, inventory spec): always room.
    fn place_in_store(&mut self, class: u16, item: UnitId) -> bool {
        self.log.push(format!("store {class} {}", item.0));
        true
    }
    fn remove_store_item(&mut self, _: u16, item: UnitId) {
        self.log.push(format!("unstore {}", item.0));
    }
    fn take_from_store(&mut self, _: u16, item: UnitId) {
        self.log.push(format!("take {}", item.0));
    }
    fn place_in_gamble(&mut self, _: u16, _: u32, _: UnitId) -> bool {
        true
    }
    fn remove_gamble_item(&mut self, _: u16, _: u32, _: UnitId) {}
    fn refresh_npc_inventory(&mut self, npc: UnitId) {
        self.log.push(format!("refresh {}", npc.0));
    }
    fn add_trade_inventory(&mut self, class: u16, item: UnitId) {
        self.log.push(format!("trade inv {class} {}", item.0));
    }
    fn owns_item(&self, _: UnitId, item: UnitId) -> bool {
        self.inventory.contains(&item)
    }
    fn in_inventory(&self, _: UnitId, item: UnitId) -> bool {
        self.inventory.contains(&item)
    }
    fn equipped_items(&self, _: UnitId) -> Vec<UnitId> {
        Vec::new()
    }
    fn find_tome(&self, _: UnitId, _: UnitId) -> Option<(UnitId, i32)> {
        None
    }
    fn add_to_tome(&mut self, _: UnitId, _: i32) {}
    fn find_partial_stack(&self, _: UnitId, _: UnitId) -> Option<(UnitId, i32)> {
        None
    }
    fn can_belt(&self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn put_in_belt(&mut self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn equip_ammo(&mut self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn place_in_backpack(&mut self, _: UnitId, item: UnitId) -> bool {
        self.log.push(format!("backpack {}", item.0));
        false
    }
    fn take_from_cursor(&mut self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn lower_book_skill(&mut self, _: UnitId, _: UnitId, _: i32) {}
    /// `0x0055DF10` (inventory spec): the staged inventory forgets it.
    fn remove_stored(&mut self, _: UnitId, item: UnitId) {
        self.log.push(format!("remove stored {}", item.0));
        self.inventory.remove(&item);
    }
    fn unequip(&mut self, _: UnitId, _: UnitId) -> bool {
        false
    }
}

impl QuestRest for Rest {
    fn has_act2(&self) -> bool {
        false
    }
    fn players(&self) -> Vec<UnitId> {
        self.quests.keys().copied().collect()
    }
    fn first_client_player(&self) -> Option<UnitId> {
        self.quests.keys().next().copied()
    }
    fn quests(&mut self, player: UnitId) -> Option<&mut PlayerQuests> {
        self.quests.get_mut(&player)
    }
    fn player_byte_4c(&self, _: UnitId) -> u8 {
        0
    }
    fn set_player_byte_4c(&mut self, _: UnitId, _: u8) {}
    fn quest_chain(&mut self, _: UnitId) -> Option<&mut QuestChain> {
        None
    }
    fn unit_act(&self, _: UnitId) -> Option<u8> {
        Some(0)
    }
    fn unit_level(&self, _: UnitId) -> Option<u32> {
        Some(1)
    }
    fn unit_kind(&self, _: UnitId) -> UnitKind {
        UnitKind::Other
    }
    fn players_near(&self, _: UnitId) -> Vec<UnitId> {
        Vec::new()
    }
    fn attach_sound(&mut self, u: UnitId, sound: u16) {
        self.log.push(format!("sound {} {sound}", u.0));
    }
    fn send(&mut self, player: UnitId, msg: &[u8]) {
        self.sent.push((player, msg.to_vec()));
    }
    fn send_text_list(&mut self, _: UnitId, _: UnitId, _: &[(u16, u32)]) {}
    fn inventory(&self, _: UnitId) -> Vec<UnitId> {
        Vec::new()
    }
    fn delete_item(&mut self, _: UnitId, _: [u8; 4]) {}
    fn reward_item(&mut self, _: UnitId, _: [u8; 4], _: i32, _: u8, _: bool) -> Option<UnitId> {
        None
    }
    fn drop_item_at(&mut self, _: UnitId, _: [u8; 4], _: u8) -> bool {
        false
    }
    fn den_region(&self) -> (u32, u32, u32, u32) {
        (0, 0, 0, 0)
    }
    fn true_tomb_level(&self) -> u32 {
        0
    }
    fn free_spot(&mut self, _: UnitId, _: u32, _: u32, _: u32, _: u32) -> Option<(i32, i32)> {
        None
    }
    fn create_portal(&mut self, _: UnitId, _: i32, _: i32, _: u16, _: u32) -> bool {
        false
    }
    fn schedule_quest_event(&mut self, _: UnitId, _: i32) {}
    fn set_object_opened(&mut self, _: UnitId) {}
    fn mercenary_reward(&mut self, _: UnitId, _: u16) {}
    fn unhandled(&mut self, chain: u8, function: u32) {
        self.log.push(format!("unhandled {chain} {function:#x}"));
    }
}

/// Every type is its own and type 0's; helm and shield are armor.
pub fn equiv() -> EquivMatrix {
    let n = N_TYPES;
    let words = n.div_ceil(32);
    let mut m = EquivMatrix {
        n,
        words,
        bits: vec![0; n * words],
    };
    let mut set = |i: usize, j: usize| m.bits[i * words + j / 32] |= 1 << (j % 32);
    for i in 1..n {
        set(i, 0);
        set(i, i);
    }
    set(usize::from(ty::HELM), usize::from(ty::ARMO));
    set(usize::from(ty::SHIE), usize::from(ty::ARMO));
    m
}

/// Items: a cap (helm, durability 12, defense 3–5) and a buckler
/// (shield, durability 12, defense 4–6).
pub fn item_tables() -> ItemTables {
    let mut ratio: Itemratio = blank();
    ratio.version = 0;
    let rec = |t: u16, code: &[u8; 4], (minac, maxac): (u32, u32)| ItemRec {
        code: *code,
        type_: t as i16,
        level: 1,
        durability: 12,
        minac,
        maxac,
        ..ItemRec::default()
    };
    ItemTables {
        items: vec![
            rec(ty::HELM, b"cap ", (3, 5)),
            rec(ty::SHIE, b"buc ", (4, 6)),
        ],
        itemtypes: (0..N_TYPES)
            .map(|_| {
                let mut t: Itemtypes = blank();
                t.class = 0xFF;
                t.staffmods = 0xFF;
                t.rare = 1;
                t
            })
            .collect(),
        equiv: equiv(),
        itemratio: vec![ratio],
        valshift: vec![0; N_STATS],
        stat_shift: 6,
        stat_mask: 0x3F,
        ..ItemTables::default()
    }
}

/// The vendor tables over the same items: Akara's column (0) holds the
/// cap as a permanent item and the buckler as a list entry (Min 1, Max
/// 3: one range draw on the NPC-control seed); one `npc.txt` row (sell
/// 1024, buy 512, repair 128, max buy 5000: `vendors.md` §9.3 Akara).
pub fn vendor_tables() -> VendorTables {
    let item = |code: &[u8; 4], t: u16, cost: u32, (minac, maxac): (u32, u32)| VendorItem {
        code: *code,
        normcode: *code,
        ubercode: NO_CODE,
        ultracode: NO_CODE,
        cost,
        type_: t as i16,
        type2: -1,
        level: 1,
        spawnable: 1,
        durability: 12,
        minac,
        maxac,
        nightmare_upgrade: XXX,
        hell_upgrade: XXX,
        ..VendorItem::default()
    };
    let mut cap = item(b"cap ", ty::HELM, 100, (3, 5));
    cap.perm_store = 1;
    cap.columns[0] = [1, 1, 0, 0, 0];
    let mut buc = item(b"buc ", ty::SHIE, 80, (4, 6));
    buc.columns[0] = [1, 3, 0, 0, 0];
    let mut itemtypes = vec![
        TypeRec {
            repair: 1,
            class: 7,
            storepage: 3,
            staffmods: 0xFF,
            ..TypeRec::default()
        };
        N_TYPES
    ];
    itemtypes[usize::from(ty::HELM)].storepage = 0;
    itemtypes[usize::from(ty::SHIE)].storepage = 0;
    VendorTables {
        items: vec![cap, buc],
        itemtypes,
        equiv: equiv(),
        stat_shift: 6,
        stat_mask: 0x3F,
        monster_levels: vec![[1, 1, 1]; N_MONSTATS],
        interact: vec![class::AKARA],
        npc: vec![NpcPrices {
            class: u32::from(class::AKARA),
            sell: 1024,
            buy: 512,
            rep: 128,
            quests: [(0, 0, 0, 0); 3],
            max_buy: [5000; 3],
        }],
        difficulty: vec![Default::default(); 3],
        ..VendorTables::default()
    }
}

/// Monstats: Akara is `npc` and `interact`.
pub fn monstats() -> Vec<Monstats> {
    let mut v: Vec<Monstats> = (0..N_MONSTATS).map(|_| blank()).collect();
    v[usize::from(class::AKARA)].npc = true;
    v[usize::from(class::AKARA)].interact = true;
    v
}

/// S→C 0x2A per `npc.md` §9, bytes 3–6 zero.
pub fn tx(kind: u8, code: u8, guid: u32, gold: i32) -> Vec<u8> {
    let mut m = vec![0x2A, kind, code, 0, 0, 0, 0];
    m.extend_from_slice(&guid.to_le_bytes());
    m.extend_from_slice(&gold.to_le_bytes());
    m
}
