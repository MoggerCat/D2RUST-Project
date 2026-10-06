// Spec: specs/world/quests.md §3, §5, §10.1, §10.4, §10.5
//! A1Q1–A1Q3 through every state on the wired host (`WiredWorld`: the
//! real `QuestControl` on `EconomyQuests` over the action sim's own
//! units and stat lists). Messages go through the real host frame
//! (C→S 0x31, 0x30); the events without a message path here (level
//! changes, kills, the updater, the Malus object) are raised on the
//! wired quest world directly, as their callers would. The seams no
//! written spec provides are staged in `trade_quests::Rest`.

use d2_sim::items::tables::ItemRec;
use d2_sim::items::{q, ItemRequest, ItemTables};
use d2_sim::stats::StatLists;
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::economy::{EconomyQuests, ItemSpawn};
use d2_sim::world::npc::class;
use d2_sim::world::quests::{act1, PlayerQuests, QuestChain};

use super::trade_quests::Fx;
use super::*;

/// Runs `$body` with `$ctl` (the quest control) and `$w` (the wired
/// quest world) bound, as the host's quest calls do.
macro_rules! quests {
    ($f:expr, |$ctl:ident, $w:ident| $body:expr) => {{
        let s = &mut $f.h.game;
        s.world.with_economy(&mut s.game, &mut s.events, |econ, p| {
            let mut $w = EconomyQuests::new(econ, &mut *p.rest);
            let $ctl = &mut *p.quests;
            $body
        })
    }};
}

/// A unit through the action sim's allocator.
fn alloc(f: &mut Fx, ty: UnitType, class: u32) -> UnitId {
    let req = AllocRequest {
        ty,
        class,
        room: None,
        add: true,
        fixed_guid: None,
        mode: 1,
        allied: false,
    };
    let s = &mut f.h.game;
    s.events
        .with(&mut s.game, |g, v| v.allocate(g, &req, 0, 0))
        .unwrap()
}

/// An NPC the 0x31 / 0x30 handlers and the text refresh can find.
fn npc(f: &mut Fx, class: u16) -> UnitId {
    let n = alloc(f, UnitType::Monster, u32::from(class));
    let g = f.guid(n);
    f.world().rest.guids.insert(n, g);
    f.world().state.add_npc(n);
    n
}

/// C→S 0x31 (NPC GUID, message, pad).
fn say(f: &mut Fx, n: UnitId, msg: u16) -> Vec<Vec<u8>> {
    let mut m = vec![0x31];
    m.extend_from_slice(&f.guid(n).to_le_bytes());
    m.extend_from_slice(&msg.to_le_bytes());
    m.extend_from_slice(&[0, 0]);
    let (code, got) = send(&mut f.h, &m);
    assert_eq!(code, ResultCode::Done);
    got
}

/// C→S 0x30 (chat end with the NPC).
fn chat_end(f: &mut Fx, n: UnitId) -> Vec<Vec<u8>> {
    let mut m = vec![0x30, 1, 0, 0, 0];
    m.extend_from_slice(&f.guid(n).to_le_bytes());
    let (code, got) = send(&mut f.h, &m);
    assert_eq!(code, ResultCode::Done);
    got
}

/// What the quest calls raised directly sent, in order.
fn taken(f: &mut Fx) -> Vec<Vec<u8>> {
    let sent = std::mem::take(&mut f.world().rest.sent);
    sent.into_iter().map(|m| m.1).collect()
}

fn word(f: &Fx, slot: usize) -> u16 {
    let r = f.record();
    u16::from_le_bytes([r[2 * slot], r[2 * slot + 1]])
}

fn state(f: &mut Fx, chain: u8) -> (u8, u8) {
    let r = f.world().quests.record(chain).unwrap();
    (r.state, r.status)
}

fn level_change(f: &mut Fx, old: u32, new: u32) -> Vec<Vec<u8>> {
    let p = f.player;
    quests!(f, |ctl, w| ctl.changed_level(&mut w, p, old, new));
    taken(f)
}

/// A linked monster of `chain` dies, killed by the player.
fn kill(f: &mut Fx, chain: u8, victim: UnitId) -> Vec<Vec<u8>> {
    let p = f.player;
    f.world()
        .rest
        .chains
        .insert(victim, QuestChain(vec![chain]));
    quests!(f, |ctl, w| ctl.monster_killed(&mut w, victim, Some(p)));
    taken(f)
}

/// `n` updater ticks.
fn ticks(f: &mut Fx, n: u32) -> Vec<Vec<u8>> {
    for _ in 0..n {
        quests!(f, |ctl, w| ctl.update(&mut w));
    }
    taken(f)
}

/// The fixture's stat data has no stats: the player gets an extended
/// list on the items tests' synthetic itemstatcost (every stat plain).
fn real_stats(f: &mut Fx) {
    let p = f.player;
    let guid = f.guid(p);
    let sys = &mut f.h.game.events.sys;
    sys.stats = StatLists::new(crate::adapters::handlers::items::moves::tests::stat_data());
    sys.stats
        .alloc_extended(&mut sys.hooks, p, UnitType::Player, guid, 1, 0, None);
}

fn set_base_stat(f: &mut Fx, unit: UnitId, stat: u16, v: i32) {
    let sys = &mut f.h.game.events.sys;
    sys.stats.unit_set(&mut sys.hooks, unit, stat, v, 0);
}

/// The quest slots `slots` completed before (bit 0) in the entering
/// player's record: §3 switches those quests off.
fn done(slots: &[u8]) -> impl FnOnce(&mut PlayerQuests) + '_ {
    move |q| {
        for &s in slots {
            q.flags[0].set(s, 0);
        }
    }
}

// Covers: specs/world/quests.md §10.4 text, §10.4 l2 r1, §10.4 l2 r2, §10.4 l3 r5, §10.4 l3 r6, §10.4 l4 r1, §10.4 l4 r2, §10.1 r3
#[test]
fn den_of_evil_through_every_state() {
    let mut f = Fx::new(|_| {});
    real_stats(&mut f);
    let (akara, p) = (f.akara, f.player);
    // State 1 at creation; the sequence at game entry stops there.
    assert_eq!(state(&mut f, 1), (1, 0));
    assert_eq!(state(&mut f, 2), (0, 0));
    // 64 → state 2 (bit 2); the chat end → status 1 to the player.
    say(&mut f, akara, 64);
    assert_eq!(state(&mut f, 1), (2, 0));
    assert_eq!(chat_end(&mut f, akara), [hex("5d 01 00 01 0000")]);
    assert_eq!(word(&f, 1), 0x0004);
    // Leaving town: state 3, bit 3 (status already 1), nothing sent.
    assert!(level_change(&mut f, 1, 2).is_empty());
    assert_eq!(state(&mut f, 1), (3, 1));
    assert_eq!(word(&f, 1), 0x000C);
    // Entering the Den: status 2, bit 4.
    assert_eq!(level_change(&mut f, 2, 8), [hex("5d 01 00 02 0000")]);
    assert_eq!(word(&f, 1), 0x001C);
    // Three left in a fully visited Den: 0x20, status 4, left 3.
    let victim = alloc(&mut f, UnitType::Monster, 1);
    f.world().rest.den = (40, 37, 10, 10);
    assert_eq!(kill(&mut f, 1, victim), [hex("5d 01 20 04 0300")]);
    // Cleared: state 4, the killer 13 + 1, game 1.13, 0x28 + `89 00`,
    // sound 35, the timer.
    let victim = alloc(&mut f, UnitType::Monster, 1);
    f.world().rest.den = (40, 40, 10, 10);
    f.take_log();
    let got = kill(&mut f, 1, victim);
    assert_eq!(got.len(), 2);
    assert_eq!((got[0][0], &got[1][..]), (0x28, &[0x89u8, 0][..]));
    assert_eq!(state(&mut f, 1).0, 4);
    assert_eq!(word(&f, 1), 0x201E);
    assert!(f.game_record()[3] & 0x20 != 0);
    assert_eq!(f.take_log(), [format!("sound {} 35", p.0)]);
    // The timer: status 5 nine updater ticks later, once.
    assert!(ticks(&mut f, 8).is_empty());
    assert_eq!(ticks(&mut f, 1), [hex("5d 01 00 05 0000")]);
    assert!(ticks(&mut f, 30).is_empty());
    // 76: the reward on the real stat list, the sequence opens chain 2.
    let got = say(&mut f, akara, 76);
    assert_eq!(got.iter().map(|m| m[0]).collect::<Vec<_>>(), [0x27, 0x29]);
    assert_eq!(state(&mut f, 1), (5, 13));
    assert_eq!(state(&mut f, 2).0, 1);
    assert_eq!(word(&f, 1), 0x2001);
    assert_eq!(word(&f, 41), 0x2002);
    assert_eq!(f.h.game.events.sys.stats.unit_base(p, 5, 0), 1);
    assert!(f.world().quests.faults.is_empty());
    assert_eq!(f.errors(), Vec::<String>::new());
}

// Covers: specs/world/quests.md §3 r2, §10.1 r2, §10.5 r2, §10.5 r3, §10.5 r4, §10.5 r5, §10.5 r7
#[test]
fn burial_grounds_through_every_state() {
    // Den of Evil done by the first player: switched off, so its
    // sequence passes the call on and opens chain 2 (§3, §10.1).
    let mut f = Fx::new(done(&[1]));
    let (kashya, p) = (f.kashya, f.player);
    assert_eq!(state(&mut f, 2), (1, 0));
    say(&mut f, kashya, 81);
    assert_eq!(state(&mut f, 2).0, 2);
    assert_eq!(word(&f, 2), 0x0004);
    assert_eq!(chat_end(&mut f, kashya), [hex("5d 02 00 01 0000")]);
    assert!(level_change(&mut f, 1, 2).is_empty());
    assert_eq!((state(&mut f, 2), word(&f, 2)), ((3, 1), 0x000C));
    assert_eq!(level_change(&mut f, 2, 17), [hex("5d 02 00 02 0000")]);
    assert_eq!(word(&f, 2), 0x001C);
    // Blood Raven dies with the player near her: 13 + 1, sound 34, the
    // game's 2.13, timer 15.
    let raven = alloc(&mut f, UnitType::Monster, 267);
    f.world().rest.near = vec![p];
    f.take_log();
    assert!(kill(&mut f, 2, raven).is_empty());
    assert_eq!(state(&mut f, 2).0, 4);
    assert_eq!(word(&f, 2), 0x201E);
    assert_eq!(f.take_log(), [format!("sound {} 34", p.0)]);
    assert!(f.game_record()[5] & 0x20 != 0);
    assert!(ticks(&mut f, 15).is_empty());
    assert_eq!(ticks(&mut f, 1), [hex("5d 02 00 03 0000")]);
    // 92: state 5, status 13, chain 4 opens; 2.0 with bits 2–4 kept;
    // 0x28, the text refresh, then the mercenary's 0x50 (NPC control).
    let got = say(&mut f, kashya, 92);
    assert_eq!(
        got.iter().map(|m| m[0]).collect::<Vec<_>>(),
        [0x28, 0x27, 0x29, 0x50]
    );
    assert_eq!(state(&mut f, 2), (5, 13));
    assert_eq!(state(&mut f, 4).0, 1);
    assert_eq!(word(&f, 2), 0x201D);
    let hire = f.world().npc.record(class::KASHYA).unwrap().hire.as_ref();
    assert!(hire.unwrap().slots.iter().any(|s| s.hired));
    assert!(f.world().quests.faults.is_empty());
    assert_eq!(f.errors(), Vec::<String>::new());
}

/// Item tables with the Horadric Malus only.
fn malus_tables() -> ItemTables {
    let malus = ItemRec {
        code: *b"hdm ",
        level: 1,
        quest: 4,
        ..ItemRec::default()
    };
    ItemTables {
        items: vec![malus],
        ..ItemTables::default()
    }
}

// Covers: specs/world/quests.md §10.1 r1, §10.5 l2 r1, §10.5 l2 r2, §10.5 l2 r4, §10.5 l2 r5, §10.5 l2 r10, §10.5 l2 r13, §10.5 l2 r16
#[test]
fn tools_of_the_trade_through_every_state() {
    // Quests 1, 2 and 4 done: the walk 1 → 2 → 4 → 3 opens chain 3.
    let mut f = Fx::new(done(&[1, 2, 4]));
    real_stats(&mut f);
    let p = f.player;
    assert_eq!(state(&mut f, 3), (1, 0));
    let charsi = npc(&mut f, class::CHARSI);
    say(&mut f, charsi, 146);
    assert_eq!(state(&mut f, 3).0, 2);
    assert_eq!(word(&f, 3), 0);
    assert_eq!(chat_end(&mut f, charsi), [hex("5d 03 00 01 0000")]);
    assert_eq!(word(&f, 3), 0x0004);
    assert!(level_change(&mut f, 1, 2).is_empty());
    assert_eq!((state(&mut f, 3), word(&f, 3)), ((3, 1), 0x000C));
    // The Malus object: known at init; operated at level 8 it drops `hdm `
    // and is shown taken; state 4.
    let malus = alloc(&mut f, UnitType::Monster, 2);
    quests!(f, |ctl, w| act1::malus_init(ctl, &mut w, malus));
    set_base_stat(&mut f, p, 12, 8);
    f.world().rest.drop_ok = true;
    f.take_log();
    quests!(f, |ctl, w| act1::malus_operate(ctl, &mut w, malus, p));
    assert_eq!(f.take_log(), [format!("drop {} hdm  2", malus.0)]);
    assert_eq!(f.world().rest.object_modes[&malus], 2);
    assert_eq!(state(&mut f, 3), (4, 1));
    // The player carries a real `hdm ` item: Charsi's 163 finishes it.
    f.world().tables = malus_tables();
    let mut rq = ItemRequest {
        item: 0,
        format: 101,
        quality: q::NORMAL,
        ..ItemRequest::default()
    };
    let spawn = ItemSpawn {
        room: None,
        mode: 4,
        init_flags: 1,
    };
    let s = &mut f.h.game;
    let item = s
        .world
        .with_economy(&mut s.game, &mut s.events, |econ, _| {
            econ.create_item(&mut rq, false, spawn)
        })
        .unwrap();
    f.world().rest.inventory.insert(p, vec![item]);
    let got = say(&mut f, charsi, 163);
    assert_eq!(
        got.iter().map(|m| m[0]).collect::<Vec<_>>(),
        [0x28, 0x27, 0x29]
    );
    let r = f.world().quests.record(3).unwrap();
    assert!(r.state == 5 && r.extra.rewarded && r.extra.malus_items == 0);
    assert_eq!(word(&f, 3), 0x200E);
    assert!(f.game_record()[7] & 0x20 != 0);
    let log = f.take_log();
    assert!(log.contains(&format!("delete {} hdm ", p.0)));
    // The sequence passed to chain 6, whose state-0 timer is open.
    assert!(log.contains(&"unhandled 6 0x596580".to_string()));
    // The chat end: status 13 set; the status function reports 10 (3.1).
    assert_eq!(chat_end(&mut f, charsi), [hex("5d 03 00 0a 0000")]);
    assert_eq!(state(&mut f, 3), (5, 13));
    // The imbue: 3.0, the record inactive.
    quests!(f, |ctl, w| act1::imbue_granted(ctl, &mut w, p));
    assert_eq!(word(&f, 3), 0x200D);
    assert!(!f.world().quests.record(3).unwrap().active);
    assert!(f.world().quests.faults.is_empty());
    assert_eq!(f.errors(), Vec::<String>::new());
}
