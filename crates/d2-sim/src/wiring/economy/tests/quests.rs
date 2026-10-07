//! Quests → items and unit fields: Act I callbacks reading real item
//! data and stats and writing real stats (`quests-act1.md` §10.1–§10.3).

use super::*;
use crate::items::{q, ItemRequest};
use crate::wiring::economy::{EconomyQuests, ItemSpawn, QuestRest};
use crate::world::quests::{
    act1, bit, PlayerQuests, QuestChain, QuestControl, QuestTables, QuestWorld, UnitKind,
};

/// The rest of the quests' world: one player's quest records, an
/// inventory and a log.
struct Rest {
    player: UnitId,
    quests: PlayerQuests,
    inventory: Vec<UnitId>,
    cursor: Option<UnitId>,
    log: Vec<String>,
}

impl Rest {
    fn new(player: UnitId) -> Self {
        Self {
            player,
            quests: PlayerQuests::default(),
            inventory: Vec::new(),
            cursor: None,
            log: Vec::new(),
        }
    }
}

impl QuestRest for Rest {
    fn has_act2(&self) -> bool {
        true
    }
    fn players(&self) -> Vec<UnitId> {
        vec![self.player]
    }
    fn first_client_player(&self) -> Option<UnitId> {
        Some(self.player)
    }
    fn quests(&mut self, player: UnitId) -> Option<&mut PlayerQuests> {
        (player == self.player).then_some(&mut self.quests)
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
        UnitKind::Player
    }
    fn players_near(&self, _: UnitId) -> Vec<UnitId> {
        Vec::new()
    }
    fn party_members(&self, _: UnitId) -> Option<Vec<UnitId>> {
        None
    }
    fn attach_sound(&mut self, _: UnitId, sound: u16) {
        self.log.push(format!("sound {sound}"));
    }
    fn send(&mut self, _: UnitId, msg: &[u8]) {
        self.log.push(format!("send {:02x}", msg[0]));
    }
    fn send_text_list(&mut self, _: UnitId, _: UnitId, _: &[(u16, u32)]) {}
    fn inventory(&self, _: UnitId) -> Vec<UnitId> {
        self.inventory.clone()
    }
    fn quest_cursor_item(&self, _: UnitId) -> Option<UnitId> {
        self.cursor
    }
    fn delete_item(&mut self, _: UnitId, code: [u8; 4]) {
        self.log
            .push(format!("delete {}", String::from_utf8_lossy(&code)));
    }
    fn reward_item(&mut self, _: UnitId, _: [u8; 4], _: i32, _: u8, _: bool) -> Option<UnitId> {
        None
    }
    fn drop_item_at(&mut self, _: UnitId, code: [u8; 4], quality: u8) -> bool {
        self.log
            .push(format!("drop {} {quality}", String::from_utf8_lossy(&code)));
        true
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
    fn object_mode(&self, _: UnitId) -> i32 {
        0
    }
    fn set_object_mode(&mut self, object: UnitId, mode: i32) {
        self.log.push(format!("mode {} {mode}", object.0));
    }
    fn mercenary_reward(&mut self, _: UnitId, _: u16) {}
    fn unit_position(&self, _: UnitId) -> Option<(i32, i32, crate::units::RoomId)> {
        None
    }
    fn room_contains(&self, _: crate::units::RoomId, _: i32, _: i32) -> bool {
        false
    }
    fn room_at(&self, _: crate::units::RoomId, _: i32, _: i32) -> Option<crate::units::RoomId> {
        None
    }
    fn free_spot_at(
        &mut self,
        _: crate::units::RoomId,
        _: i32,
        _: i32,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
    ) -> Option<(i32, i32, crate::units::RoomId)> {
        None
    }
    fn spawn_monster(
        &mut self,
        _: crate::units::RoomId,
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
    fn create_object(&mut self, _: crate::units::RoomId, _: i32, _: i32, _: u16) -> Option<UnitId> {
        None
    }
    fn object_anim_length(&self, _: UnitId) -> i32 {
        0
    }
    fn schedule_object_event(&mut self, _: UnitId, _: u8, _: i32) {}
    fn open_quest_message(&mut self, _: UnitId, _: UnitId, _: u16) {}
    fn unhandled(&mut self, chain: u8, function: u32) {
        self.log.push(format!("unhandled {chain} {function:#x}"));
    }
}

fn control(w: &mut World) -> QuestControl {
    QuestControl::new(&QuestTables::load().unwrap(), &mut w.fields.seed).unwrap()
}

/// C→S 0x31 to the NPC with `guid` and message `index`.
fn message(guid: u32, index: u16) -> Vec<u8> {
    let mut m = vec![0x31];
    m.extend_from_slice(&guid.to_le_bytes());
    m.extend_from_slice(&index.to_le_bytes());
    m.extend_from_slice(&[0, 0]);
    m
}

/// §10.5 Tools of the Trade, Charsi's message 163 at state 4 (the Malus
/// taken): the Horadric Malus is found in the player's inventory through
/// the real item data (`has_item`).
#[test]
fn tools_of_the_trade_reads_real_items() {
    let mut w = World::new();
    w.data.monsters[usize::from(crate::world::quests::npc::CHARSI)].enabled = true;
    let p = w.spawn(UnitType::Player, 0);
    let charsi = w.spawn(
        UnitType::Monster,
        u32::from(crate::world::quests::npc::CHARSI),
    );
    let g = w.units.get(charsi).unwrap().guid;
    let mut ctl = control(&mut w);
    ctl.record_mut(3).unwrap().state = 4;
    let mut rest = Rest::new(p);
    // Without the malus: no state change.
    {
        let mut e = w.econ();
        let mut qw = EconomyQuests::new(&mut e, &mut rest);
        assert!(!qw.has_item(p, *b"hdm "));
        ctl.quest_message(&mut qw, p, &message(g, 163));
    }
    assert_ne!(ctl.record(3).unwrap().state, 5);
    let mut rq = ItemRequest {
        item: HORADRIC_MALUS as i32,
        format: 101,
        quality: q::NORMAL,
        ..ItemRequest::default()
    };
    let spawn = ItemSpawn {
        room: None,
        mode: 0,
        init_flags: 1,
    };
    let malus = w.econ().create_item(&mut rq, false, spawn).unwrap();
    rest.inventory.push(malus);
    rest.log.clear();
    {
        let mut e = w.econ();
        let mut qw = EconomyQuests::new(&mut e, &mut rest);
        assert!(qw.has_item(p, *b"hdm "));
        assert_eq!(qw.quest_items(p), [(malus, 3)]);
        ctl.quest_message(&mut qw, p, &message(g, 163));
    }
    assert_eq!(ctl.record(3).unwrap().state, 5);
    assert!(rest.log.contains(&"delete hdm ".to_string()));
    assert!(rest.quests.flags[0].get(3, bit::PRIMARY_GOAL_DONE));
    assert!(rest.quests.flags[0].get(3, bit::REWARD_PENDING));
}

/// §10.4 Den of Evil reward (Akara's message 76 with the reward
/// pending): the skill point is added to the player's real stat 5 and
/// the player's real GUID enters the record's list.
#[test]
fn den_reward_writes_real_stats() {
    let mut w = World::new();
    let p = w.spawn(UnitType::Player, 0);
    let akara = w.spawn(
        UnitType::Monster,
        u32::from(crate::world::quests::npc::AKARA),
    );
    let g = w.units.get(akara).unwrap().guid;
    w.set_stat(p, 5, 2);
    let mut ctl = control(&mut w);
    let mut rest = Rest::new(p);
    rest.quests.flags[0].set(1, bit::REWARD_PENDING);
    {
        let mut e = w.econ();
        let mut qw = EconomyQuests::new(&mut e, &mut rest);
        ctl.quest_message(&mut qw, p, &message(g, 76));
    }
    assert_eq!(w.stats.unit_base(p, 5, 0), 3);
    assert!(rest.quests.flags[0].get(1, bit::REWARD_GRANTED));
    assert!(!rest.quests.flags[0].get(1, bit::REWARD_PENDING));
    let guid = w.units.get(p).unwrap().guid;
    assert!(ctl.record(1).unwrap().guids.contains(guid));
}

/// §10.5 Malus: the level gate reads the real base stat 12.
#[test]
fn malus_level_gate_reads_real_stats() {
    let mut w = World::new();
    let p = w.spawn(UnitType::Player, 0);
    let malus = w.spawn(UnitType::Monster, MONSTER_CLASS);
    let mut ctl = control(&mut w);
    let mut rest = Rest::new(p);
    w.set_stat(p, 12, 7);
    {
        let mut e = w.econ();
        let mut qw = EconomyQuests::new(&mut e, &mut rest);
        act1::malus_operate(&mut ctl, &mut qw, malus, p);
    }
    // Level 7: sound event 19, nothing drops.
    assert_eq!(rest.log, ["sound 19"]);
    rest.log.clear();
    w.set_stat(p, 12, 8);
    {
        let mut e = w.econ();
        let mut qw = EconomyQuests::new(&mut e, &mut rest);
        act1::malus_operate(&mut ctl, &mut qw, malus, p);
    }
    assert_eq!(
        rest.log,
        ["drop hdm  2".to_string(), format!("mode {} 2", malus.0)]
    );
    assert_eq!(ctl.record(3).unwrap().state, 4);
}

/// Unit fields: GUID lookups, classes and the unit seed are the unit
/// records'.
#[test]
fn quest_unit_fields_on_real_records() {
    let mut w = World::new();
    let p = w.spawn(UnitType::Player, 4);
    let akara = w.spawn(
        UnitType::Monster,
        u32::from(crate::world::quests::npc::AKARA),
    );
    let (pg, ag) = (
        w.units.get(p).unwrap().guid,
        w.units.get(akara).unwrap().guid,
    );
    let seed = w.units.get(akara).unwrap().seed;
    let mut rest = Rest::new(p);
    let mut e = w.econ();
    let mut qw = EconomyQuests::new(&mut e, &mut rest);
    assert_eq!(qw.player_by_guid(pg), Some(p));
    assert_eq!(qw.guid(p), pg);
    assert_eq!(qw.player_class(p), 4);
    assert_eq!(
        qw.monster_by_guid(ag),
        Some((akara, crate::world::quests::npc::AKARA))
    );
    assert_eq!(qw.monster_class(p), None);
    let mut s = seed;
    assert_eq!(qw.unit_seed(akara).step(), s.step());
    assert_eq!(*qw.unit_seed(akara), s);
    qw.add_stat(p, 0, 5);
    assert_eq!(qw.stat(p, 0), 5);
}

/// A unit system as the wrapped dispatcher of [`QuestTick`]: every other
/// tick hook keeps its default.
struct Inner(crate::units::dispatch::UnitSystem<Hooks>);

impl crate::tick::EventDispatch for Inner {
    fn run_event(&mut self, game: &mut Game, run: &crate::tick::timer::TimerRun) {
        self.0.run_event(game, run);
    }
}

impl crate::tick::TickHooks for Inner {}

impl crate::wiring::economy::UnitSide for Inner {
    type Hooks = Hooks;
    fn unit_side(
        &mut self,
    ) -> (
        &mut crate::units::record::Units,
        &mut crate::stats::StatLists,
        &crate::units::hooks::UnitData,
        &mut Hooks,
    ) {
        self.0.unit_side()
    }
}

/// Tick step 8 (`tick.md` §3: frame % 20 = 0) runs the quest updater
/// (`quests.md` §5) through [`QuestTick`]: the Den of Evil status timer
/// (period 1, due at updater tick 1) runs at the second update (frame
/// 40), sets status 5 and sends 0x5D through the real quest world, and is
/// removed.
// Covers: specs/world/quests.md §5; specs/sim/tick.md §3
#[test]
fn quest_updater_runs_at_tick_step_8() {
    use crate::wiring::economy::QuestTick;
    use crate::world::quests::TimerFn;

    let mut w = World::new();
    let p = w.spawn(UnitType::Player, 0);
    let mut ctl = control(&mut w);
    let mut rest = Rest::new(p);
    ctl.record_mut(1).unwrap().state = 4;
    ctl.add_timer(1, TimerFn::DenOfEvilStatus, 1).unwrap();
    let World {
        mut game,
        units,
        stats,
        data,
        hooks,
        mut fields,
        tables,
        mut items,
    } = w;
    let mut inner = Inner(crate::units::dispatch::UnitSystem {
        units,
        stats,
        data,
        hooks,
        errors: Vec::new(),
    });
    let mut run = |game: &mut Game, ctl: &mut QuestControl, rest: &mut Rest, to: i32| {
        let mut q = QuestTick {
            sim: &mut inner,
            fields: &mut fields,
            tables: &tables,
            items: &mut items,
            quests: ctl,
            rest,
        };
        while game.frame < to {
            crate::tick::tick(game, &mut q);
        }
    };
    run(&mut game, &mut ctl, &mut rest, 39);
    assert_eq!(ctl.tick, 1);
    assert_ne!(ctl.record(1).unwrap().status, 5);
    assert!(rest.log.is_empty());
    run(&mut game, &mut ctl, &mut rest, 40);
    assert_eq!(ctl.tick, 2);
    assert_eq!(ctl.record(1).unwrap().status, 5);
    assert_eq!(rest.log, ["send 5d"]);
    // Removed: the next updates run nothing.
    run(&mut game, &mut ctl, &mut rest, 100);
    assert_eq!(ctl.tick, 5);
    assert_eq!(rest.log, ["send 5d"]);
}

/// `quests-helpers.md` §8 (`0x00558110`): the cursor item first (its
/// record's `quest` and stat 356 against the difficulty,
/// `questdiffcheck` not tested), then the inventory list in order with
/// page 1 skipped (`quest`, `questdiffcheck` and stat 356 tested); none
/// otherwise.
// Covers: specs/world/quests-helpers.md §8 text, §8 r2, §8 r3, §8 r4
#[test]
fn find_item_by_code_cursor_then_list() {
    let mut w = World::new();
    let p = w.spawn(UnitType::Player, 0);
    w.fields.difficulty = 1;
    let make = |w: &mut World| {
        let mut rq = ItemRequest {
            item: HORADRIC_MALUS as i32,
            format: 101,
            quality: q::NORMAL,
            ..ItemRequest::default()
        };
        let spawn = ItemSpawn {
            room: None,
            mode: 0,
            init_flags: 1,
        };
        w.econ().create_item(&mut rq, false, spawn).unwrap()
    };
    let a = make(&mut w);
    let b = make(&mut w);
    let mut rest = Rest::new(p);
    let find = |w: &mut World, rest: &mut Rest| {
        let mut e = w.econ();
        EconomyQuests::new(&mut e, rest).find_item(p, *b"hdm ")
    };
    // Rule 4: nothing anywhere.
    assert_eq!(find(&mut w, &mut rest), None);
    // The malus record is a quest item; with questdiffcheck set, stat 356
    // (0) < difficulty (1) skips the list entry.
    assert_ne!(w.tables.items[HORADRIC_MALUS].quest, 0);
    w.tables.items[HORADRIC_MALUS].questdiffcheck = 1;
    rest.inventory = vec![a, b];
    assert_eq!(find(&mut w, &mut rest), None);
    // Stat 356 ≥ difficulty on the second item: it is the one found.
    w.set_stat(b, 356, 1);
    assert_eq!(find(&mut w, &mut rest), Some(b));
    // Items on page 1 (trade) are skipped.
    w.items.get_mut(b).unwrap().inv_page = 1;
    assert_eq!(find(&mut w, &mut rest), None);
    w.items.get_mut(b).unwrap().inv_page = 0;
    // Rule 2: the cursor item is tried first (stat test applies since
    // `quest` ≠ 0).
    rest.cursor = Some(a);
    assert_eq!(find(&mut w, &mut rest), Some(b), "a: stat 0 < 1, skipped");
    w.set_stat(a, 356, 1);
    assert_eq!(find(&mut w, &mut rest), Some(a), "cursor before the list");
    // Without questdiffcheck the list does not test the stat.
    w.tables.items[HORADRIC_MALUS].questdiffcheck = 0;
    w.set_stat(a, 356, 0);
    rest.cursor = None;
    assert_eq!(find(&mut w, &mut rest), Some(a), "first in list order");
}
