// Spec: specs/tools/scenario.md §4 (run model: the d2rs scenario host)
//! The scenario host's rest: the `WiredWorld` seams the play app's
//! `AppRest` fills, as no-ops, so a scenario runs on the wired host
//! (inventory model, item loads from a save, quest records) without the
//! client crate. Every answer is a stand-in (`// d2rs-own, unverified`):
//! a scenario that reaches one of these seams compares nothing it could
//! settle; the trace's gaps name the host (`scenario.md` §4 rule 10).

use std::collections::BTreeMap;

use d2_server::adapters::handlers::world::Outbox;
use d2_sim::units::{RoomId, UnitId};
use d2_sim::wiring::economy::QuestRest;
use d2_sim::wiring::interaction::{HirelingRest, NpcRest, PlayerQuestsRef, VendorRest};
use d2_sim::world::npc::{ImbueMods, InvEntry, ItemFacts};
use d2_sim::world::quests::{PlayerQuests, QuestChain, TextList, UnitKind};
use d2_sim::world::vendors::price::Bonus;
use d2_sim::world::vendors::Transaction;

/// The no-op rest of the scenario host (see the module docs).
#[derive(Default)]
pub struct ScenarioRest {
    pub expansion: bool,
    pub quests: BTreeMap<UnitId, PlayerQuests>,
    pub chains: BTreeMap<UnitId, QuestChain>,
    pub save_flags: BTreeMap<UnitId, u16>,
    pub owners: BTreeMap<UnitId, (u32, u8)>,
    pub names: BTreeMap<UnitId, Vec<u8>>,
    pub last_bought: BTreeMap<UnitId, u32>,
    pub sent: Vec<(UnitId, Vec<u8>)>,
}

impl Outbox for ScenarioRest {
    fn take_sent(&mut self) -> Vec<(UnitId, Vec<u8>)> {
        std::mem::take(&mut self.sent)
    }
}

impl PlayerQuestsRef for ScenarioRest {
    fn quests_ref(&self, player: UnitId) -> Option<&PlayerQuests> {
        self.quests.get(&player)
    }
}

impl NpcRest for ScenarioRest {
    fn item_format(&self) -> u16 {
        if self.expansion {
            101
        } else {
            2
        }
    }
    fn distance(&self, _: UnitId, _: UnitId) -> i32 {
        0
    }
    fn axis_check(&self, _: UnitId, _: UnitId) -> u32 {
        0
    }
    fn unit_check(&self, _: UnitId, _: u32) -> u32 {
        0
    }
    fn clear_path(&mut self, _: UnitId) {}
    fn approach(&mut self, _: UnitId, _: UnitId) {}
    fn player_busy(&self, _: UnitId) -> u32 {
        0
    }
    fn start_allowed(&self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn tristram_cain_busy(&self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn pet(&self, _: UnitId, _: u8, _: u8) -> Option<UnitId> {
        None
    }
    fn pets(&self, _: UnitId) -> Vec<UnitId> {
        Vec::new()
    }
    fn player_name(&self, player: UnitId) -> Vec<u8> {
        self.names.get(&player).cloned().unwrap_or_default()
    }
    fn reset_stats(&mut self, _: UnitId) {}
    fn reset_skills(&mut self, _: UnitId) {}
    fn act_change(&mut self, _: UnitId, _: u32, _: u32) {}
    fn activate_waypoint(&mut self, _: UnitId, _: u32) {}
    fn npc_ai_param(&mut self, _: UnitId, _: u32) {}
    fn stat_sent(&mut self, _: UnitId, _: u16, _: u32) {}
    fn respec_sound(&mut self, _: UnitId) {}
    fn encode_text_list(&self, _: &TextList) -> [u8; 34] {
        [0; 34]
    }
    fn socket_granted(&mut self, _: UnitId) {}
    fn personalize_granted(&mut self, _: UnitId) {}
    fn inventory_entries(&self, _: UnitId) -> Vec<InvEntry> {
        Vec::new()
    }
    fn stage_inventory(&mut self, _: UnitId, _: Vec<InvEntry>) {}
    fn take_identified(&mut self) -> Vec<UnitId> {
        Vec::new()
    }
    fn identify(&mut self, _: UnitId) {}
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
    fn spawn_mercenary(&mut self, _: UnitId, _: u32, _: u8) -> Option<UnitId> {
        None
    }
}

impl HirelingRest for ScenarioRest {
    fn set_mode(&mut self, _: UnitId, _: u8) {}
    fn set_state_stat(&mut self, _: UnitId, _: u16, _: u16, _: i32) {}
    fn skill_count(&self) -> u32 {
        0
    }
    fn skill_reqlevel(&self, _: u32) -> Option<i16> {
        None
    }
    fn set_skill_level(&mut self, _: UnitId, _: u32, _: i32) {}
    fn set_owner(&mut self, u: UnitId, guid: u32, t: u8) {
        self.owners.insert(u, (guid, t));
    }
    fn owner(&self, u: UnitId) -> Option<(u32, u8)> {
        self.owners.get(&u).copied()
    }
    fn join_team(&mut self, _: UnitId, _: UnitId) {}
    fn hireling_ai(&mut self, _: UnitId) {}
    fn free_unit(&mut self, _: UnitId) {}
    fn queue_room_removal(&mut self, _: UnitId) {}
    fn death_event(&mut self, _: UnitId) {}
    fn dismiss(&mut self, _: UnitId) {}
    fn warp_to(&mut self, _: UnitId, _: UnitId) {}
    fn level_events(&mut self, _: UnitId, _: UnitId) {}
    fn reapply_item_stats(&mut self, _: UnitId) {}
}

impl VendorRest for ScenarioRest {
    fn players_in_level(&self, _: u16) -> i32 {
        0
    }
    fn player_level_id(&self, _: UnitId) -> u16 {
        0
    }
    fn gold_cap(&self, _: UnitId) -> i32 {
        0
    }
    fn stash_cap(&self, _: UnitId) -> i32 {
        0
    }
    fn drop_gold(&mut self, _: UnitId, _: i32) {}
    fn last_bought(&self, p: UnitId) -> u32 {
        self.last_bought.get(&p).copied().unwrap_or(0)
    }
    fn set_last_bought(&mut self, p: UnitId, guid: u32) {
        self.last_bought.insert(p, guid);
    }
    fn store_price(&mut self, _: UnitId, _: u32, _: u32) {}
    fn has_cursor_item(&self, _: UnitId) -> bool {
        false
    }
    fn copy_item(&mut self, _: UnitId) -> Option<UnitId> {
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
    fn send_transaction(&mut self, _: UnitId, _: Transaction) {}
    fn new_store_inventory(&mut self, _: u16, _: Option<UnitId>) {}
    fn place_in_store(&mut self, _: u16, _: UnitId) -> bool {
        false
    }
    fn remove_store_item(&mut self, _: u16, _: UnitId) {}
    fn take_from_store(&mut self, _: u16, _: UnitId) {}
    fn place_in_gamble(&mut self, _: u16, _: u32, _: UnitId) -> bool {
        false
    }
    fn remove_gamble_item(&mut self, _: u16, _: u32, _: UnitId) {}
    fn refresh_npc_inventory(&mut self, _: UnitId) {}
    fn add_trade_inventory(&mut self, _: u16, _: UnitId) {}
    fn owns_item(&self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn in_inventory(&self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn equipped_items(&self, _: UnitId) -> Vec<UnitId> {
        Vec::new()
    }
    fn find_tome(&mut self, _: UnitId, _: UnitId) -> Option<(UnitId, i32)> {
        None
    }
    fn add_to_tome(&mut self, _: UnitId, _: i32) {}
    fn find_partial_stack(&mut self, _: UnitId, _: UnitId) -> Option<(UnitId, i32)> {
        None
    }
    fn can_belt(&mut self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn put_in_belt(&mut self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn equip_ammo(&mut self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn place_in_backpack(&mut self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn take_from_cursor(&mut self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn lower_book_skill(&mut self, _: UnitId, _: UnitId, _: i32) {}
    fn remove_stored(&mut self, _: UnitId, _: UnitId) {}
    fn unequip(&mut self, _: UnitId, _: UnitId) -> bool {
        false
    }
}

impl QuestRest for ScenarioRest {
    fn client_save_flags(&self, p: UnitId) -> Option<u16> {
        self.save_flags.get(&p).copied()
    }
    fn set_client_save_flags(&mut self, p: UnitId, flags: u16) {
        self.save_flags.insert(p, flags);
    }
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
    fn quest_chain(&mut self, u: UnitId) -> Option<&mut QuestChain> {
        self.chains.get_mut(&u)
    }
    fn unit_act(&self, _: UnitId) -> Option<u8> {
        None
    }
    fn unit_level(&self, _: UnitId) -> Option<u32> {
        None
    }
    fn unit_kind(&self, u: UnitId) -> UnitKind {
        if self.quests.contains_key(&u) {
            UnitKind::Player
        } else {
            UnitKind::Other
        }
    }
    fn players_near(&self, _: UnitId) -> Vec<UnitId> {
        self.quests.keys().copied().collect()
    }
    fn party_members(&self, _: UnitId) -> Option<Vec<UnitId>> {
        None
    }
    fn attach_sound(&mut self, _: UnitId, _: u16) {}
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
        Default::default()
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
    fn object_mode(&self, _: UnitId) -> i32 {
        0
    }
    fn set_object_mode(&mut self, _: UnitId, _: i32) {}
    fn mercenary_reward(&mut self, _: UnitId, _: u16) {}
    fn unit_position(&self, _: UnitId) -> Option<(i32, i32, RoomId)> {
        None
    }
    fn room_contains(&self, _: RoomId, _: i32, _: i32) -> bool {
        false
    }
    fn room_at(&self, _: RoomId, _: i32, _: i32) -> Option<RoomId> {
        None
    }
    fn free_spot_at(
        &mut self,
        _: RoomId,
        _: i32,
        _: i32,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
    ) -> Option<(i32, i32, RoomId)> {
        None
    }
    fn spawn_monster(
        &mut self,
        _: RoomId,
        _: i32,
        _: i32,
        _: u16,
        _: u8,
        _: u32,
    ) -> Option<UnitId> {
        None
    }
    fn or_unit_flags(&mut self, _: UnitId, _: u32) {}
    fn monsters(&self) -> Vec<UnitId> {
        Vec::new()
    }
    fn npc_chat_clients(&self, _: UnitId) -> Option<Vec<UnitId>> {
        None
    }
    fn remove_monster(&mut self, _: UnitId) {}
    fn drop_preset_monster(&mut self, _: u8, _: u16) {}
    fn find_object_near(&self, _: UnitId, _: u16) -> Option<UnitId> {
        None
    }
    fn create_object(&mut self, _: RoomId, _: i32, _: i32, _: u16) -> Option<UnitId> {
        None
    }
    fn object_anim_length(&self, _: UnitId) -> i32 {
        0
    }
    fn schedule_object_event(&mut self, _: UnitId, _: u8, _: i32) {}
    fn open_quest_message(&mut self, _: UnitId, _: UnitId, _: u16) {}
    fn unhandled(&mut self, _: u8, _: u32) {}
}
