// Spec: specs/world/npc.md §1.1, §2; specs/world/vendors.md §1; specs/world/quests.md §1.7; specs/world/hirelings.md Inputs; specs/items/generation.md §1.2 (the item format)
//! The seams of the app's wired host (`d2_server::adapters::handlers::
//! world::WiredWorld`) that no written spec provides: the rest `R` of the
//! NPC, vendor, quest and hireling wiring (`docs/handoff/
//! wire-interaction.md` §6 names each call's owner). The players'
//! interaction is the unit record's (`UnitRecord::interact`), not the
//! rest's.
//!
//! Every answer is the narrowest one, as `Pending`'s defaults are: a
//! query answers "none" (no unit, no item, no spot, not allowed, out of
//! range), an action that would change state outside `d2-sim` does
//! nothing and is logged in [`AppRest::log`]. What the rest itself must
//! hold (the players' quest records, names, last-bought GUIDs, the
//! messages the rests send) is kept as set. Nothing here decides an
//! outcome; a call that would need a decision is refused.

use std::collections::BTreeMap;

use d2_server::adapters::handlers::world::Outbox;
use d2_sim::units::UnitType;

use super::npc_seams::{encode_text_list, SnapRef};
use d2_sim::units::{RoomId, UnitId};
use d2_sim::wiring::economy::QuestRest;
use d2_sim::wiring::interaction::{HirelingRest, NpcRest, PlayerQuestsRef, VendorRest};
use d2_sim::world::npc::{self, ImbueMods, InvEntry, ItemFacts};
use d2_sim::world::quests::{PlayerQuests, QuestChain, TextList, UnitKind};
use d2_sim::world::vendors::price::Bonus;
use d2_sim::world::vendors::Transaction;

/// The rest of the app's wired host (module docs).
#[derive(Debug, Default)]
pub struct AppRest {
    /// Expansion game (the item format, `generation.md` §1.2).
    pub expansion: bool,
    /// The players' quest records (player data, `quests.md` §1.7), set at
    /// the join.
    pub quests: BTreeMap<UnitId, PlayerQuests>,
    /// The players' names (client record +0x0D), set at the join.
    pub names: BTreeMap<UnitId, Vec<u8>>,
    pub last_bought: BTreeMap<UnitId, u32>,
    /// Messages the rests send, in send order ([`Outbox`]).
    pub sent: Vec<(UnitId, Vec<u8>)>,
    /// Calls that did nothing (no provider), in call order.
    pub log: Vec<String>,
    /// The players and monsters at the last sync (`npc_seams`).
    pub snap: SnapRef,
}

impl AppRest {
    fn note(&mut self, s: String) {
        self.log.push(s);
    }
}

impl Outbox for AppRest {
    fn take_sent(&mut self) -> Vec<(UnitId, Vec<u8>)> {
        std::mem::take(&mut self.sent)
    }
}

impl PlayerQuestsRef for AppRest {
    fn quests_ref(&self, player: UnitId) -> Option<&PlayerQuests> {
        self.quests.get(&player)
    }
}

impl NpcRest for AppRest {
    /// Game +0x78: 101 expansion, 2 classic (`generation.md` §1.2).
    fn item_format(&self) -> u16 {
        if self.expansion {
            101
        } else {
            2
        }
    }
    /// d2rs-own, unverified (`npc_seams`): the snapshot's distance.
    fn distance(&self, a: UnitId, b: UnitId) -> i32 {
        self.snap.lock().map_or(i32::MAX, |s| s.distance(a, b))
    }
    /// `npc.md` §3: both axes within 50 sub-tiles (the snapshot).
    fn axis_check(&self, p: UnitId, n: UnitId) -> u32 {
        self.snap.lock().map_or(1, |s| s.axis_check(p, n))
    }
    /// d2rs-own, unverified (REC-52): `0x00548F80` accepts a known unit.
    fn unit_check(&self, _: UnitId, guid: u32) -> u32 {
        let known = self
            .snap
            .lock()
            .is_ok_and(|s| s.units.values().any(|u| u.guid == guid));
        u32::from(!known)
    }
    fn clear_path(&mut self, u: UnitId) {
        self.note(format!("clear path {}", u.0));
    }
    fn approach(&mut self, p: UnitId, n: UnitId) {
        self.note(format!("approach {} {}", p.0, n.0));
    }
    /// d2rs-own, unverified (REC-52, `0x00535060`): free; the interact
    /// unit is checked by the module and the cursor item is not read.
    fn player_busy(&self, _: UnitId) -> u32 {
        0
    }
    /// d2rs-own, unverified (REC-52, `0x00457490` unspecified).
    fn start_allowed(&self, _: UnitId, _: UnitId) -> bool {
        true
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
    fn reset_stats(&mut self, p: UnitId) {
        self.note(format!("reset stats {}", p.0));
    }
    fn reset_skills(&mut self, p: UnitId) {
        self.note(format!("reset skills {}", p.0));
    }
    fn act_change(&mut self, p: UnitId, level: u32, arg: u32) {
        self.note(format!("act change {} {level} {arg}", p.0));
    }
    fn activate_waypoint(&mut self, p: UnitId, level: u32) {
        self.note(format!("activate waypoint {} {level}", p.0));
    }
    fn npc_ai_param(&mut self, n: UnitId, param: u32) {
        self.note(format!("ai param {} {param:#x}", n.0));
    }
    fn stat_sent(&mut self, p: UnitId, stat: u16, value: u32) {
        self.note(format!("stat sent {} {stat} {value}", p.0));
    }
    fn respec_sound(&mut self, p: UnitId) {
        self.note(format!("respec sound {}", p.0));
    }
    /// `0x00661480` is not specified: the inverse of the client's read
    /// ([`encode_text_list`], d2rs-own, unverified).
    fn encode_text_list(&self, list: &TextList) -> [u8; 34] {
        encode_text_list(list)
    }
    fn socket_granted(&mut self, p: UnitId) {
        self.note(format!("socket granted {}", p.0));
    }
    fn personalize_granted(&mut self, p: UnitId) {
        self.note(format!("personalize granted {}", p.0));
    }
    fn inventory_entries(&self, _: UnitId) -> Vec<InvEntry> {
        Vec::new()
    }
    fn identify(&mut self, item: UnitId) {
        self.note(format!("identify {}", item.0));
    }
    fn cursor_item(&self, _: UnitId) -> Option<UnitId> {
        None
    }
    fn item_facts(&self, _: UnitId) -> ItemFacts {
        ItemFacts::default()
    }
    fn put_back(&mut self, p: UnitId, item: UnitId) {
        self.note(format!("put back {} {}", p.0, item.0));
    }
    fn remove_cursor_item(&mut self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn duplicate(&mut self, _: UnitId, _: UnitId) -> Option<UnitId> {
        None
    }
    fn create_imbued(&mut self, _: UnitId, _: UnitId, _: &ImbueMods) -> Option<UnitId> {
        None
    }
    fn item_refresh(&mut self, item: UnitId) {
        self.note(format!("item refresh {}", item.0));
    }
    fn personal_name(&self, _: UnitId) -> Vec<u8> {
        Vec::new()
    }
    fn set_personal_name(&mut self, item: UnitId, _: &[u8]) {
        self.note(format!("personal name {}", item.0));
    }
    fn place_or_drop(&mut self, p: UnitId, item: UnitId) {
        self.note(format!("place or drop {} {}", p.0, item.0));
    }
    fn spawn_mercenary(&mut self, _: UnitId, class: u32, _: u8) -> Option<UnitId> {
        self.note(format!("spawn mercenary {class}"));
        None
    }
}

impl HirelingRest for AppRest {
    fn set_mode(&mut self, u: UnitId, mode: u8) {
        self.note(format!("mode {} {mode}", u.0));
    }
    fn set_state_stat(&mut self, u: UnitId, state: u16, stat: u16, value: i32) {
        self.note(format!("state stat {} {state} {stat} {value}", u.0));
    }
    fn skill_count(&self) -> u32 {
        0
    }
    fn skill_reqlevel(&self, _: u32) -> Option<i16> {
        None
    }
    fn set_skill_level(&mut self, u: UnitId, skill: u32, level: i32) {
        self.note(format!("skill level {} {skill} {level}", u.0));
    }
    fn set_owner(&mut self, u: UnitId, guid: u32, t: u8) {
        self.note(format!("owner {} {guid} {t}", u.0));
    }
    fn owner(&self, _: UnitId) -> Option<(u32, u8)> {
        None
    }
    fn join_team(&mut self, m: UnitId, p: UnitId) {
        self.note(format!("join team {} {}", m.0, p.0));
    }
    fn hireling_ai(&mut self, m: UnitId) {
        self.note(format!("hireling ai {}", m.0));
    }
    fn free_unit(&mut self, u: UnitId) {
        self.note(format!("free unit {}", u.0));
    }
    fn queue_room_removal(&mut self, u: UnitId) {
        self.note(format!("room removal {}", u.0));
    }
    fn death_event(&mut self, u: UnitId) {
        self.note(format!("death event {}", u.0));
    }
    fn dismiss(&mut self, u: UnitId) {
        self.note(format!("dismiss {}", u.0));
    }
    fn warp_to(&mut self, pet: UnitId, p: UnitId) {
        self.note(format!("warp to {} {}", pet.0, p.0));
    }
    fn level_events(&mut self, p: UnitId, m: UnitId) {
        self.note(format!("level events {} {}", p.0, m.0));
    }
    fn reapply_item_stats(&mut self, m: UnitId) {
        self.note(format!("reapply item stats {}", m.0));
    }
}

impl VendorRest for AppRest {
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
    fn drop_gold(&mut self, p: UnitId, amount: i32) {
        self.note(format!("drop gold {} {amount}", p.0));
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
    /// `0x0055A2A0`: no items spec writes the copy.
    fn copy_item(&mut self, item: UnitId) -> Option<UnitId> {
        self.note(format!("copy {}", item.0));
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
    fn recharge(&mut self, item: UnitId) {
        self.note(format!("recharge {}", item.0));
    }
    fn repair_broken(&mut self, item: UnitId) {
        self.note(format!("repair {}", item.0));
    }
    fn send_item_stat(&mut self, p: UnitId, item: UnitId, stat: u16) {
        self.note(format!("item stat {} {} {stat}", p.0, item.0));
    }
    /// S→C 0x2A through its builder `0x0053D740` (`npc.md` §9).
    fn send_transaction(&mut self, p: UnitId, t: Transaction) {
        let m = npc::transaction(t.kind, t.code, t.guid, t.gold as u32);
        self.sent.push((p, m.to_vec()));
    }
    fn new_store_inventory(&mut self, class: u16, _: Option<UnitId>) {
        self.note(format!("new store {class}"));
    }
    /// The NPC grid (`0x00560200`, inventory spec): no room.
    fn place_in_store(&mut self, _: u16, _: UnitId) -> bool {
        false
    }
    fn remove_store_item(&mut self, class: u16, item: UnitId) {
        self.note(format!("unstore {class} {}", item.0));
    }
    fn take_from_store(&mut self, class: u16, item: UnitId) {
        self.note(format!("take {class} {}", item.0));
    }
    fn place_in_gamble(&mut self, _: u16, _: u32, _: UnitId) -> bool {
        false
    }
    fn remove_gamble_item(&mut self, class: u16, _: u32, item: UnitId) {
        self.note(format!("ungamble {class} {}", item.0));
    }
    fn refresh_npc_inventory(&mut self, n: UnitId) {
        self.note(format!("refresh {}", n.0));
    }
    fn add_trade_inventory(&mut self, class: u16, item: UnitId) {
        self.note(format!("trade inv {class} {}", item.0));
    }
    fn owns_item(&self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn in_inventory(&self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn equipped_items(&self, _: UnitId) -> Vec<UnitId> {
        Vec::new()
    }
    fn find_tome(&self, _: UnitId, _: UnitId) -> Option<(UnitId, i32)> {
        None
    }
    fn add_to_tome(&mut self, tome: UnitId, k: i32) {
        self.note(format!("add to tome {} {k}", tome.0));
    }
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
    fn place_in_backpack(&mut self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn take_from_cursor(&mut self, _: UnitId, _: UnitId) -> bool {
        false
    }
    fn lower_book_skill(&mut self, p: UnitId, item: UnitId, n: i32) {
        self.note(format!("lower book {} {} {n}", p.0, item.0));
    }
    fn remove_stored(&mut self, p: UnitId, item: UnitId) {
        self.note(format!("remove stored {} {}", p.0, item.0));
    }
    fn unequip(&mut self, _: UnitId, _: UnitId) -> bool {
        false
    }
}

impl QuestRest for AppRest {
    /// No act change provider: the game holds no act beyond the ones
    /// created, and no spec says when Act II counts as present.
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
    fn set_player_byte_4c(&mut self, p: UnitId, v: u8) {
        self.note(format!("byte 4c {} {v}", p.0));
    }
    fn quest_chain(&mut self, _: UnitId) -> Option<&mut QuestChain> {
        None
    }
    fn unit_act(&self, u: UnitId) -> Option<u8> {
        self.snap.lock().ok()?.units.get(&u).map(|u| u.act)
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
    /// d2rs-own, unverified (REC-52): every player (single player).
    fn players_near(&self, _: UnitId) -> Vec<UnitId> {
        self.quests.keys().copied().collect()
    }
    fn party_members(&self, _: UnitId) -> Option<Vec<UnitId>> {
        None
    }
    fn attach_sound(&mut self, u: UnitId, sound: u16) {
        self.note(format!("sound {} {sound}", u.0));
    }
    fn send(&mut self, player: UnitId, msg: &[u8]) {
        self.sent.push((player, msg.to_vec()));
    }
    /// S→C 0x27 (`npc.md` §2 step 5): type 1, the NPC's GUID, the
    /// encoded list.
    fn send_text_list(&mut self, p: UnitId, n: UnitId, list: &[(u16, u32)]) {
        let guid = self
            .snap
            .lock()
            .ok()
            .and_then(|s| s.units.get(&n).map(|u| u.guid));
        let Some(guid) = guid else {
            self.note(format!("text list {} {} {}: no guid", p.0, n.0, list.len()));
            return;
        };
        let mut m = vec![0x27, 1];
        m.extend_from_slice(&guid.to_le_bytes());
        m.extend_from_slice(&encode_text_list(list));
        self.sent.push((p, m));
    }
    fn inventory(&self, _: UnitId) -> Vec<UnitId> {
        Vec::new()
    }
    fn delete_item(&mut self, p: UnitId, code: [u8; 4]) {
        self.note(format!("delete item {} {code:?}", p.0));
    }
    fn reward_item(&mut self, p: UnitId, code: [u8; 4], _: i32, _: u8, _: bool) -> Option<UnitId> {
        self.note(format!("reward item {} {code:?}", p.0));
        None
    }
    fn drop_item_at(&mut self, u: UnitId, code: [u8; 4], _: u8) -> bool {
        self.note(format!("drop item {} {code:?}", u.0));
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
    fn create_portal(&mut self, p: UnitId, x: i32, y: i32, class: u16, level: u32) -> bool {
        self.note(format!("portal {} {x} {y} {class} {level}", p.0));
        false
    }
    fn schedule_quest_event(&mut self, o: UnitId, frame: i32) {
        self.note(format!("quest event {} {frame}", o.0));
    }
    fn object_mode(&self, _: UnitId) -> i32 {
        0
    }
    fn set_object_mode(&mut self, o: UnitId, mode: i32) {
        self.note(format!("object mode {} {mode}", o.0));
    }
    fn mercenary_reward(&mut self, p: UnitId, n: u16) {
        self.note(format!("mercenary reward {} {n}", p.0));
    }
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
        x: i32,
        y: i32,
        class: u16,
        _: u8,
        _: u32,
    ) -> Option<UnitId> {
        self.note(format!("spawn monster {class} {x} {y}"));
        None
    }
    fn or_unit_flags(&mut self, u: UnitId, flags: u32) {
        self.note(format!("unit flags {} {flags:#x}", u.0));
    }
    fn monsters(&self) -> Vec<UnitId> {
        self.snap
            .lock()
            .map(|s| s.of_type(UnitType::Monster))
            .unwrap_or_default()
    }
    fn npc_chat_clients(&self, _: UnitId) -> Option<Vec<UnitId>> {
        None
    }
    fn remove_monster(&mut self, m: UnitId) {
        self.note(format!("remove monster {}", m.0));
    }
    fn drop_preset_monster(&mut self, act: u8, class: u16) {
        self.note(format!("drop preset monster {act} {class}"));
    }
    fn find_object_near(&self, _: UnitId, _: u16) -> Option<UnitId> {
        None
    }
    fn create_object(&mut self, _: RoomId, x: i32, y: i32, class: u16) -> Option<UnitId> {
        self.note(format!("create object {class} {x} {y}"));
        None
    }
    fn object_anim_length(&self, _: UnitId) -> i32 {
        0
    }
    fn schedule_object_event(&mut self, o: UnitId, ev: u8, frame: i32) {
        self.note(format!("object event {} {ev} {frame}", o.0));
    }
    fn open_quest_message(&mut self, p: UnitId, o: UnitId, msg: u16) {
        self.note(format!("quest message {} {} {msg}", p.0, o.0));
    }
    fn unhandled(&mut self, chain: u8, function: u32) {
        self.note(format!("unhandled {chain} {function:#x}"));
    }
}
