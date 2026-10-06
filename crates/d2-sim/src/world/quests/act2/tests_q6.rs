// Spec: specs/world/quests-act2.md §8, §10 (Test vectors)
//! Tests for [`super::q6`]: A2Q6 The Seven Tombs callback by callback,
//! the true-tomb choice, the lair objects and the chain-13 hooks, on the
//! quests' fake world.

use super::q6::*;
use super::Timer;
use crate::rng::Seed;
use crate::units::{RoomId, UnitId};
use crate::world::quests::tests::*;
use crate::world::quests::*;

const P3: UnitId = UnitId(3);
const JERHYN_U: UnitId = UnitId(0x30);
const TYRAEL_U: UnitId = UnitId(0x31);
const MESHIF_U: UnitId = UnitId(0x32);
const DROGNAN_U: UnitId = UnitId(0x33);
const ATMA_U: UnitId = UnitId(0x34);
const DURIEL_U: UnitId = UnitId(0x35);
const ORIFICE_U: UnitId = UnitId(0x50);
const LAIR_U: UnitId = UnitId(0x51);
const DOOR_U: UnitId = UnitId(0x52);
const INIT37_U: UnitId = UnitId(0x53);
const R1: RoomId = RoomId(1);
const R2: RoomId = RoomId(2);

fn npc_kind(class: u16) -> UnitKind {
    UnitKind::Monster {
        class: u32::from(class),
        superunique: None,
        owner: None,
    }
}

fn act2_player(guid: u32, level: u32) -> Player {
    Player {
        guid,
        act: Some(1),
        level: Some(level),
        ..Player::default()
    }
}

/// P1 in Lut Gholein with the Act II NPCs.
fn fake() -> Fake {
    let mut f = Fake::new();
    f.p(P1).act = Some(1);
    f.p(P1).level = Some(40);
    for (u, class) in [
        (JERHYN_U, JERHYN),
        (TYRAEL_U, TYRAEL1),
        (MESHIF_U, npc::MESHIF1),
        (DROGNAN_U, DROGNAN),
        (ATMA_U, ATMA),
        (DURIEL_U, 211),
    ] {
        f.monsters.insert(u, (u.0, class, npc_kind(class)));
    }
    f
}

fn i13(ctl: &QuestControl) -> usize {
    ctl.find(13).unwrap()
}

fn ex(ctl: &QuestControl) -> &Extra {
    &ctl.record(13).unwrap().extra.a2.q6
}

fn exm(ctl: &mut QuestControl) -> &mut Extra {
    &mut ctl.record_mut(13).unwrap().extra.a2.q6
}

fn set(f: &mut Fake, p: UnitId, slot: u8, b: u8) {
    f.p(p).quests.flags[0].set(slot, b);
}

/// One event to chain 13's record only.
fn ev(ctl: &mut QuestControl, f: &mut Fake, args: EventArgs) {
    let i = i13(ctl);
    act1::callback(ctl, f, i, args, None, false);
}

/// Event 0 from P1 to the NPC unit `n`: the lines chain 13 adds.
fn text(ctl: &mut QuestControl, f: &mut Fake, n: UnitId) -> TextList {
    let mut list = TextList::new();
    let args = EventArgs {
        event: event::NPC_ACTIVATE,
        target: Some(n),
        player: Some(P1),
        ..EventArgs::default()
    };
    let i = i13(ctl);
    act1::callback(ctl, f, i, args, Some(&mut list), false);
    list
}

/// Event 11 from P1: NPC unit `n` of `class`, message `msg`.
fn say(ctl: &mut QuestControl, f: &mut Fake, n: UnitId, class: u16, msg: u32) {
    let args = EventArgs {
        event: event::SCROLL_MESSAGE,
        target: Some(n),
        player: Some(P1),
        a: u32::from(class),
        b: msg,
    };
    ev(ctl, f, args);
}

fn level(ctl: &mut QuestControl, f: &mut Fake, p: UnitId, old: u32, new: u32) {
    let args = EventArgs {
        event: event::CHANGED_LEVEL,
        target: Some(p),
        player: Some(p),
        a: old,
        b: new,
    };
    ev(ctl, f, args);
}

fn chat_end(ctl: &mut QuestControl, f: &mut Fake, n: UnitId) {
    let args = EventArgs {
        event: event::NPC_DEACTIVATE,
        target: Some(n),
        player: Some(P1),
        ..EventArgs::default()
    };
    ev(ctl, f, args);
}

fn status_of(ctl: &QuestControl, f: &mut Fake, p: UnitId) -> Option<u8> {
    let pf = f.flags(p);
    act1::status_fn(ctl, f, i13(ctl), p, &pf, 0x0059_CA50)
}

/// `5D 0D <flags> <status> 0000`.
fn s5d(status: u8) -> Vec<u8> {
    vec![0x5D, 0x0D, 0, status, 0, 0]
}

/// 0x28 for a player whose record is `f`'s current one.
fn s28(f: &Fake, p: UnitId) -> Vec<u8> {
    let mut m = hex("28 06 00000000 00");
    m.extend_from_slice(&f.flags(p).0);
    m
}

/// P1's 0x28 as Meshif's 450 sends it: before 14.4 is cleared.
fn s28_with_14_4(f: &Fake) -> Vec<u8> {
    let mut m = hex("28 06 00000000 00");
    let mut r = f.flags(P1);
    r.set(14, 4);
    m.extend_from_slice(&r.0);
    m
}

// ------------------------------------------------------------ §8.1

// Covers: specs/world/quests-act2.md §8.1
#[test]
fn true_tomb_drlg_seed_vector() {
    // Vector: DRLG seed {12345, 666}: lo' mod 7 = 3, 3 (retry), 3, 6 →
    // staff tomb 69, Duriel tomb 72.
    let mut s = Seed::new(12345, 666);
    assert_eq!(true_tombs(&mut s), (69, 72));
    let mut t = Seed::new(12345, 666);
    let draws: Vec<u32> = (0..4).map(|_| t.step() % 7).collect();
    assert_eq!(draws, [3, 3, 3, 6]);
    // Exactly four draws were taken.
    assert_eq!(s, t);
}

// ------------------------------------------------------------ §8.3

// Covers: specs/world/quests-act2.md §8.3
#[test]
fn chat_tables() {
    let (mut ctl, _) = control();
    let mut f = fake();
    let i = i13(&ctl);
    assert!(ctl.records[i].not_intro && ctl.records[i].state == 0);
    // State 0 → nothing.
    assert_eq!(text(&mut ctl, &mut f, JERHYN_U), []);
    // State 1 → table state 0 (Jerhyn 430, menu 1 sent as 0).
    ctl.records[i].state = 1;
    assert_eq!(text(&mut ctl, &mut f, JERHYN_U), [(430, 0)]);
    assert_eq!(text(&mut ctl, &mut f, ATMA_U), []);
    // State 2 → table state 1; Drognan only while game 12.13 is clear.
    ctl.records[i].state = 2;
    assert_eq!(text(&mut ctl, &mut f, ATMA_U), [(434, 2)]);
    assert_eq!(text(&mut ctl, &mut f, DROGNAN_U), [(439, 2)]);
    ctl.game.set(12, 13);
    assert_eq!(text(&mut ctl, &mut f, DROGNAN_U), []);
    assert_eq!(text(&mut ctl, &mut f, ATMA_U), [(434, 2)]);
    // Tyrael: table state 2 only with door mode 2, and nothing else.
    ctl.records[i].state = 3;
    assert_eq!(text(&mut ctl, &mut f, TYRAEL_U), []);
    exm(&mut ctl).door_mode = 2;
    assert_eq!(text(&mut ctl, &mut f, TYRAEL_U), [(302, 0)]);
    // State 3 → table state 2 (Jerhyn has no line there).
    assert_eq!(text(&mut ctl, &mut f, JERHYN_U), []);
    // State 4 without 14.13 → nothing; with 14.13 → table state 3.
    ctl.records[i].state = 4;
    assert_eq!(text(&mut ctl, &mut f, JERHYN_U), []);
    set(&mut f, P1, 14, 13);
    // Atma's 14.6 is clear: table state 6 first.
    assert_eq!(text(&mut ctl, &mut f, ATMA_U), [(445, 0)]);
    set(&mut f, P1, 14, 6);
    assert_eq!(text(&mut ctl, &mut f, ATMA_U), [(445, 2)]);
    assert_eq!(text(&mut ctl, &mut f, JERHYN_U), [(442, 0)]);
    // State 5 with 14.13 → table state 4.
    ctl.records[i].state = 5;
    assert_eq!(text(&mut ctl, &mut f, JERHYN_U), [(442, 2)]);
    // 14.0 with 14.13 falls through to the table; without it, nothing.
    set(&mut f, P1, 14, 0);
    assert_eq!(text(&mut ctl, &mut f, JERHYN_U), [(442, 2)]);
    f.p(P1).quests.flags[0].clear(14, 13);
    ctl.records[i].state = 2;
    assert_eq!(text(&mut ctl, &mut f, ATMA_U), []);
    // 14.3 → table state 3, before the 14.0 test.
    set(&mut f, P1, 14, 3);
    assert_eq!(text(&mut ctl, &mut f, JERHYN_U), [(442, 0)]);
    // 14.4 → 5 for Meshif, else 4.
    f.p(P1).quests.flags[0].clear(14, 3);
    set(&mut f, P1, 14, 4);
    assert_eq!(text(&mut ctl, &mut f, MESHIF_U), [(450, 0)]);
    assert_eq!(text(&mut ctl, &mut f, JERHYN_U), [(442, 2)]);
    // GUID listed → table state 4.
    let mut f = fake();
    ctl.records[i].state = 1;
    ctl.records[i].guids.add(1);
    assert_eq!(text(&mut ctl, &mut f, JERHYN_U), [(442, 2)]);
    assert!(f.log.is_empty());
}

// Covers: specs/world/quests-act2.md §8.3
#[test]
fn wants_to_talk() {
    let (mut ctl, _) = control();
    let mut f = fake();
    let i = i13(&ctl);
    let act = |ctl: &QuestControl, f: &mut Fake, n: u16| {
        act1::active_fn(ctl, f, i13(ctl), P1, n, 0x0059_D300)
    };
    // Jerhyn: state 1 with 14.3 and 14.4 clear, or 14.3; never with 14.0.
    assert!(!act(&ctl, &mut f, JERHYN));
    ctl.records[i].state = 1;
    assert!(act(&ctl, &mut f, JERHYN));
    set(&mut f, P1, 14, 4);
    assert!(!act(&ctl, &mut f, JERHYN));
    assert!(act(&ctl, &mut f, npc::MESHIF1));
    set(&mut f, P1, 14, 3);
    ctl.records[i].state = 4;
    assert!(act(&ctl, &mut f, JERHYN));
    set(&mut f, P1, 14, 0);
    assert!(!act(&ctl, &mut f, JERHYN));
    // Tyrael: not-intro, Duriel killed, portal not opened.
    assert!(!act(&ctl, &mut f, TYRAEL1));
    exm(&mut ctl).duriel_killed = true;
    assert!(act(&ctl, &mut f, TYRAEL1));
    exm(&mut ctl).portal_opened = true;
    assert!(!act(&ctl, &mut f, TYRAEL1));
    exm(&mut ctl).portal_opened = false;
    ctl.records[i].not_intro = false;
    assert!(!act(&ctl, &mut f, TYRAEL1));
    // The six townsfolk with 14.13 while their bit is clear.
    let mut f = fake();
    assert!(!act(&ctl, &mut f, ATMA));
    set(&mut f, P1, 14, 13);
    for (c, b) in TOWNSFOLK {
        assert!(act(&ctl, &mut f, c));
        set(&mut f, P1, 14, b);
        assert!(!act(&ctl, &mut f, c));
    }
    assert!(!act(&ctl, &mut f, 198));
}

// ------------------------------------------------------------ §8.4

// Covers: specs/world/quests-act2.md §8.4
#[test]
fn level_changes() {
    let (mut ctl, _) = control();
    let mut f = fake();
    let i = i13(&ctl);
    // Outside 40–74: nothing, not even the quick remove.
    ctl.records[i].guids.add(1);
    level(&mut ctl, &mut f, P1, 40, 39);
    assert_eq!(ctl.records[i].guids.0, [1]);
    assert_eq!(ctl.records[i].extra.tomb_level, 0);
    // Old level 40 → quick remove; level 41 changes nothing else, but
    // the staff tomb is read and stored (68 in the fake).
    level(&mut ctl, &mut f, P1, 40, 41);
    assert!(ctl.records[i].guids.0.is_empty());
    assert_eq!(ctl.records[i].extra.tomb_level, 68);
    assert_eq!(ctl.records[i].state, 0);
    // The staff tomb: state := 2, status 2 (silent), 14.2 for the mover.
    ctl.records[i].flags = 5;
    level(&mut ctl, &mut f, P1, 67, 68);
    let r = &ctl.records[i];
    assert_eq!((r.state, r.status, r.flags), (2, 2, 0));
    assert!(f.flags(P1).get(14, 2));
    assert!(f.sent.is_empty());
    // State 2 already: stop (status untouched).
    ctl.records[i].status = 0;
    level(&mut ctl, &mut f, P1, 67, 68);
    assert_eq!(ctl.records[i].status, 0);
    // Status > 1 is kept.
    ctl.records[i].state = 1;
    ctl.records[i].status = 3;
    level(&mut ctl, &mut f, P1, 67, 68);
    assert_eq!((ctl.records[i].state, ctl.records[i].status), (2, 3));

    // Canyon: state 0 → 2 and 14.2; status 0 → status 1 to all.
    let (mut ctl, _) = control();
    let mut f = fake();
    f.players.insert(P2, act2_player(2, 40));
    set(&mut f, P1, 7, 0);
    level(&mut ctl, &mut f, P1, 45, CANYON);
    let r = ctl.record(13).unwrap();
    assert_eq!((r.state, r.status), (2, 1));
    assert!(f.flags(P1).get(14, 2));
    assert!(!f.flags(P2).get(14, 2));
    // P1 (7.0 set, state 2 < 3) reads the status byte, P2 (7.0 clear) 0.
    assert_eq!(f.sent, [(P1, s5d(1)), (P2, s5d(0))]);
    // Canyon again: state 2, status 1: nothing.
    f.sent.clear();
    level(&mut ctl, &mut f, P1, 45, CANYON);
    assert!(f.sent.is_empty());

    // Duriel dead: the staff tomb and the Canyon fall through; level 73
    // with state ≤ 1 → state 2, status 2 (silent), 14.2.
    let (mut ctl, _) = control();
    let mut f = fake();
    let i = i13(&ctl);
    exm(&mut ctl).duriel_killed = true;
    level(&mut ctl, &mut f, P1, 45, CANYON);
    level(&mut ctl, &mut f, P1, 67, 68);
    assert_eq!(ctl.records[i].state, 0);
    assert_eq!(ctl.records[i].extra.tomb_level, 0);
    ctl.records[i].state = 1;
    level(&mut ctl, &mut f, P1, 72, DURIEL_LAIR);
    assert_eq!((ctl.records[i].state, ctl.records[i].status), (2, 2));
    assert!(f.flags(P1).get(14, 2));
    // Status > 1 → state 2 only.
    let mut f = fake();
    ctl.records[i].state = 0;
    ctl.records[i].status = 4;
    level(&mut ctl, &mut f, P1, 72, DURIEL_LAIR);
    assert_eq!((ctl.records[i].state, ctl.records[i].status), (2, 4));
    assert!(!f.flags(P1).get(14, 2));
    // Duriel alive, level 73 is neither tomb nor Canyon: falls through.
    exm(&mut ctl).duriel_killed = false;
    ctl.records[i].state = 1;
    ctl.records[i].status = 0;
    set(&mut f, P1, 14, 0);
    level(&mut ctl, &mut f, P1, 72, DURIEL_LAIR);
    assert_eq!((ctl.records[i].state, ctl.records[i].status), (2, 2));
    // 14.0: the flag iterate leaves 14.2 clear.
    assert!(!f.flags(P1).get(14, 2));

    // Intro: quick remove only.
    let (mut ctl, _) = control();
    let mut f = fake();
    let i = i13(&ctl);
    ctl.records[i].not_intro = false;
    ctl.records[i].guids.add(1);
    level(&mut ctl, &mut f, P1, 40, 68);
    assert!(ctl.records[i].guids.0.is_empty());
    assert_eq!(
        (ctl.records[i].state, ctl.records[i].extra.tomb_level),
        (0, 0)
    );
}

// ------------------------------------------------------------ §8.5

// Covers: specs/world/quests-act2.md §8.5 text, §8.5 r0
#[test]
fn status_function() {
    let (mut ctl, _) = control();
    let mut f = fake();
    let i = i13(&ctl);
    // Vector: 7.0 set, staff missing, game type 3 → 8 (+0x14 written by
    // chain 9's event 9, §4.10).
    f.game_type = 3;
    exm(&mut ctl).missing = true;
    exm(&mut ctl).missing_status = 8;
    // 7.0 clear → 0 first.
    assert_eq!(status_of(&ctl, &mut f, P1), Some(0));
    set(&mut f, P1, 7, 0);
    assert_eq!(status_of(&ctl, &mut f, P1), Some(8));
    exm(&mut ctl).missing = false;
    ctl.records[i].status = 7;
    // State 0 < 3 → the status byte.
    assert_eq!(status_of(&ctl, &mut f, P1), Some(7));
    ctl.records[i].state = 3;
    assert_eq!(status_of(&ctl, &mut f, P1), Some(12));
    set(&mut f, P1, 14, 5);
    assert_eq!(status_of(&ctl, &mut f, P1), Some(7));
    ctl.records[i].not_intro = false;
    assert_eq!(status_of(&ctl, &mut f, P1), Some(0));
    set(&mut f, P1, 14, 4);
    assert_eq!(status_of(&ctl, &mut f, P1), Some(6));
    set(&mut f, P1, 14, 3);
    assert_eq!(status_of(&ctl, &mut f, P1), Some(5));
    set(&mut f, P1, 14, 0);
    assert_eq!(status_of(&ctl, &mut f, P1), Some(0));
}

// ------------------------------------------------------------ §8.6

// Covers: specs/world/quests-act2.md §8.6
#[test]
fn orifice_operate_and_insert() {
    let mut f = fake();
    f.objects.insert(ORIFICE_U, (0x50, ORIFICE, 0));
    // Mode 0 without `hst `: sound 19, 1.
    assert_eq!(orifice_operate(&mut f, ORIFICE_U, P1), 1);
    assert_eq!(f.log, ["sound 1 19"]);
    // With it: interact unit, mode 1, the insert dialog, 0.
    f.log.clear();
    f.p(P1).items.push(*b"hst ");
    assert_eq!(orifice_operate(&mut f, ORIFICE_U, P1), 0);
    assert_eq!(
        f.log,
        [
            "interact 1 Some((2, 80))",
            "mode 80 1",
            "insert dialog 1 80"
        ]
    );
    // Mode 1, the orifice is the interact unit: reset, mode 2, 0.
    f.log.clear();
    assert_eq!(orifice_operate(&mut f, ORIFICE_U, P1), 0);
    assert_eq!(f.log, ["interact 1 None", "mode 80 2"]);
    // Mode 1, another interact unit: nothing.
    f.log.clear();
    f.objects.get_mut(&ORIFICE_U).unwrap().2 = 1;
    f.interact.insert(P1, (2, 0x99));
    assert_eq!(orifice_operate(&mut f, ORIFICE_U, P1), 0);
    assert!(f.log.is_empty());
    // A busy player: not specified, reported.
    f.objects.get_mut(&ORIFICE_U).unwrap().2 = 0;
    f.busy.push(P1);
    assert_eq!(orifice_operate(&mut f, ORIFICE_U, P1), 1);
    assert_eq!(f.log, ["unhandled 13 0x59dc70"]);
    // C→S 0x44: an orifice refuses any other cursor item (result 4).
    assert_eq!(insert_result(ORIFICE, *b"vip "), 4);
    assert_eq!(insert_result(ORIFICE, *b"hst "), 5);
    assert_eq!(insert_result(153, *b"vip "), 5);
}

// ------------------------------------------------------------ §8.7

/// The orifice at (100, 200) in R1; R2 holds y ≥ 202.
fn orifice_world(f: &mut Fake) {
    f.objects.insert(ORIFICE_U, (0x50, ORIFICE, 0));
    f.pos.insert(ORIFICE_U, (100, 200, R1));
    f.rooms.insert(R1, (0, 0, 150, 202));
    f.rooms.insert(R2, (0, 202, 150, 300));
}

// Covers: specs/world/quests-act2.md §8.6, §8.7
#[test]
fn staff_hand_in() {
    let (mut ctl, _) = control();
    let mut f = fake();
    orifice_world(&mut f);
    // Vector: live Range 440 → period (440 − 75) / 20 = 18.
    f.missile_ranges.insert(338, 440);
    f.players.insert(P2, act2_player(2, 40));
    f.players.insert(P3, act2_player(3, 40));
    f.p(P3).act = Some(0);
    for p in [P1, P2, P3] {
        f.p(p).items = vec![*b"hst ", *b"vip ", *b"msf "];
    }
    f.party.insert(P1, vec![P1, P2, P3]);
    ctl.tick = 7;
    staff_inserted(&mut ctl, &mut f, P1, ORIFICE_U);
    let del = ["delete hst ", "delete vip ", "delete msf "];
    let mut want = vec!["mode 80 1", "mode 80 2"];
    want.extend(del);
    want.extend(del);
    want.extend([
        "mode 80 1",
        "room portal RoomId(1) false",
        "room portal RoomId(2) false",
    ]);
    assert_eq!(f.log, want);
    for p in [P1, P2] {
        assert!(f.flags(p).get(10, 0) && f.flags(p).get(10, 13));
        assert!(f.p(p).items.is_empty());
    }
    // P3 is not in Act II.
    assert!(!f.flags(P3).get(10, 0));
    assert_eq!(f.p(P3).items.len(), 3);
    // FX 3: 0x28 then `89 03` to every player.
    assert_eq!(ctl.fx, 3);
    assert_eq!(
        f.sent,
        [
            (P1, s28(&f, P1)),
            (P1, hex("89 03")),
            (P2, s28(&f, P2)),
            (P2, hex("89 03")),
            (P3, s28(&f, P3)),
            (P3, hex("89 03")),
        ]
    );
    let e = ex(&ctl);
    assert!(e.objects_update && e.staff_removed && e.timer_active);
    assert_eq!(
        ctl.timers,
        [QuestTimer {
            func: TimerFn::Act2(Timer::LairObjects),
            chain: 13,
            due: 25,
            period: 18,
        }]
    );
    // A second hand-in (a trading party member) adds no timer; the
    // trading member keeps his items.
    let mut f2 = fake();
    orifice_world(&mut f2);
    f2.players.insert(P2, act2_player(2, 40));
    f2.p(P2).items = vec![*b"hst "];
    f2.party.insert(P1, vec![P2]);
    f2.trading.push(P2);
    hand_in(&mut ctl, &mut f2, P1, ORIFICE_U);
    assert_eq!(ctl.timers.len(), 1);
    assert!(f2.flags(P2).get(10, 0) && f2.flags(P2).get(10, 13));
    assert_eq!(f2.p(P2).items, [*b"hst "]);
}

// Covers: specs/world/quests-act2.md §8.7, §edge-cases-original-bugs r10
#[test]
fn hand_in_without_missile_row() {
    // A table without row 338: the original reads a null row (crash).
    let (mut ctl, _) = control();
    let mut f = fake();
    orifice_world(&mut f);
    hand_in(&mut ctl, &mut f, P1, ORIFICE_U);
    assert_eq!(ctl.faults, [QuestError::Fatal(0x0059_DD80)]);
    assert!(ctl.timers.is_empty());
    assert!(!ex(&ctl).timer_active);
    // A short Range gives a negative signed quotient, passed as is.
    let (mut ctl, _) = control();
    f.missile_ranges.insert(338, 15);
    hand_in(&mut ctl, &mut f, P1, ORIFICE_U);
    assert_eq!(ctl.timers[0].period, (-3i32) as u32);
}

// ------------------------------------------------------------ §8.8

// Covers: specs/world/quests-act2.md §8.8
#[test]
fn lair_object_timer() {
    let run =
        |ctl: &mut QuestControl, f: &mut Fake| super::run_timer(ctl, f, Timer::LairObjects, 13);
    // +0x0D clear: +0x03 := 0, removed.
    let (mut ctl, _) = control();
    let mut f = fake();
    exm(&mut ctl).timer_active = true;
    assert!(run(&mut ctl, &mut f));
    assert!(!ex(&ctl).timer_active);
    // +0x0D set, no object known: retry (+0x03 stays 1).
    exm(&mut ctl).timer_active = true;
    exm(&mut ctl).objects_update = true;
    assert!(!run(&mut ctl, &mut f));
    assert!(ex(&ctl).timer_active && !ex(&ctl).lair_open);
    // The orifice known: the lair entrance at (x − 13, y + 3) in R2,
    // mode 1, has-portal on both rooms (staff handed in this game).
    orifice_world(&mut f);
    exm(&mut ctl).orifice_seen = true;
    exm(&mut ctl).orifice_guid = 0x50;
    f.object_spawns.push(Some(LAIR_U));
    f.objects.insert(LAIR_U, (0x51, 100, 0));
    assert!(run(&mut ctl, &mut f));
    assert_eq!(
        f.log,
        [
            "spawn object 100 87 203",
            "mode 81 1",
            "room portal RoomId(1) true",
            "room portal RoomId(2) true",
        ]
    );
    let e = ex(&ctl);
    assert!(e.lair_open && !e.objects_update && !e.timer_active);
    // Staff handed in before this game (+0x0C): mode 2, no has-portal.
    f.log.clear();
    exm(&mut ctl).objects_update = true;
    exm(&mut ctl).staff_in = true;
    f.object_spawns.push(Some(LAIR_U));
    assert!(run(&mut ctl, &mut f));
    assert_eq!(f.log, ["spawn object 100 87 203", "mode 81 2"]);
    // The creation fails and no init-37 object: retry.
    f.log.clear();
    exm(&mut ctl).objects_update = true;
    exm(&mut ctl).lair_open = false;
    assert!(!run(&mut ctl, &mut f));
    assert!(!ex(&ctl).lair_open);
    // The init-37 object in mode 0: mode 1 plus the end-animation event
    // (+0x0C clear), or mode 2 (+0x0C set).
    let (mut ctl, _) = control();
    let mut f = fake();
    f.frame = 100;
    f.objects.insert(INIT37_U, (0x53, 0, 0));
    exm(&mut ctl).objects_update = true;
    exm(&mut ctl).init37_seen = true;
    exm(&mut ctl).init37_guid = 0x53;
    assert!(run(&mut ctl, &mut f));
    assert_eq!(f.log, ["mode 83 1", "event1 83 116"]);
    assert!(ex(&ctl).lair_open);
    // Not in mode 0: (a) does nothing → retry.
    f.log.clear();
    exm(&mut ctl).objects_update = true;
    assert!(!run(&mut ctl, &mut f));
    f.objects.get_mut(&INIT37_U).unwrap().2 = 0;
    exm(&mut ctl).staff_in = true;
    assert!(run(&mut ctl, &mut f));
    assert_eq!(f.log, ["mode 83 2"]);
}

// Covers: specs/world/quests-act2.md §8.8
#[test]
fn lair_object_inits_and_warp() {
    let (mut ctl, _) = control();
    let mut f = fake();
    f.objects.insert(ORIFICE_U, (0x50, ORIFICE, 0));
    f.objects.insert(DOOR_U, (0x52, 153, 0));
    f.objects.insert(INIT37_U, (0x53, 0, 0));
    // Orifice init: nothing handed in → only the GUID.
    orifice_init(&mut ctl, &mut f, ORIFICE_U);
    assert!(ex(&ctl).orifice_seen && ex(&ctl).orifice_guid == 0x50);
    assert!(f.log.is_empty() && ctl.timers.is_empty());
    // +0x0C, no timer, lair closed: timer period 1, mode 2.
    exm(&mut ctl).staff_in = true;
    orifice_init(&mut ctl, &mut f, ORIFICE_U);
    assert_eq!(f.log, ["mode 80 2"]);
    assert_eq!(
        (ctl.timers[0].func, ctl.timers[0].period),
        (TimerFn::Act2(Timer::LairObjects), 1)
    );
    assert!(ex(&ctl).timer_active);
    // Lair open: mode 2, no timer.
    f.log.clear();
    exm(&mut ctl).timer_active = false;
    exm(&mut ctl).lair_open = true;
    orifice_init(&mut ctl, &mut f, ORIFICE_U);
    assert_eq!(f.log, ["mode 80 2"]);
    assert_eq!(ctl.timers.len(), 1);
    // Init 37: 2 when intro, lair open or objects pending, else 0.
    f.log.clear();
    init37(&mut ctl, &mut f, INIT37_U);
    exm(&mut ctl).lair_open = false;
    init37(&mut ctl, &mut f, INIT37_U);
    exm(&mut ctl).objects_update = true;
    init37(&mut ctl, &mut f, INIT37_U);
    assert_eq!(f.log, ["mode 83 2", "mode 83 0", "mode 83 2"]);
    assert!(ex(&ctl).init37_seen && ex(&ctl).init37_guid == 0x53);
    // Door init 38: +0x18, or 2 when intro.
    f.log.clear();
    door_init(&mut ctl, &mut f, DOOR_U);
    exm(&mut ctl).door_mode = 2;
    door_init(&mut ctl, &mut f, DOOR_U);
    assert_eq!(f.log, ["mode 82 0", "mode 82 2"]);
    assert!(ex(&ctl).door_seen && ex(&ctl).door_guid == 0x52);
    // Warp check for level 73: closed while not-intro and closed lair.
    assert_eq!(warp_check(72, 73), WarpCheck::Delegate(0x0059_DB20));
    assert_eq!(lair_warp_open(&ctl), Some(false));
    exm(&mut ctl).lair_open = true;
    assert_eq!(lair_warp_open(&ctl), Some(true));
    exm(&mut ctl).lair_open = false;
    let i = i13(&ctl);
    ctl.records[i].not_intro = false;
    assert_eq!(lair_warp_open(&ctl), Some(true));
    f.log.clear();
    door_init(&mut ctl, &mut f, DOOR_U);
    init37(&mut ctl, &mut f, INIT37_U);
    assert_eq!(f.log, ["mode 82 2", "mode 83 2"]);
    ctl.records.retain(|r| r.chain != 13);
    assert_eq!(lair_warp_open(&ctl), None);
}

// ------------------------------------------------------------ §8.9

// Covers: specs/world/quests-act2.md §8.9 text, §8.9 r1
#[test]
fn arcane_dummy_objects() {
    // Vector: staff tomb 69 → index 3 (310) skipped.
    assert_eq!(arcane_list(69), [313, 312, 308, 311, 309, 307]);
    // The fake's staff tomb is 68: index 2 (308) skipped; the 7th call
    // wraps to the first entry.
    let (mut ctl, _) = control();
    let mut f = fake();
    let got: Vec<u16> = (0..7).map(|_| arcane_object(&mut ctl, &mut f)).collect();
    assert_eq!(got, [313, 312, 310, 311, 309, 307, 313]);
    assert!(ex(&ctl).arcane_made);
    assert_eq!(ex(&ctl).arcane_next, 1);
    // No chain 13: 307.
    ctl.records.retain(|r| r.chain != 13);
    assert_eq!(arcane_object(&mut ctl, &mut f), 307);
}

// ------------------------------------------------------------ §8.10

// Covers: specs/world/quests-act2.md §8.10, §edge-cases-original-bugs r11
#[test]
fn true_tomb_clue_item() {
    // Reachable only through `read_clue` with `trs ` (no such item in the
    // 1.14d tables, edge case 11): 0x50 with u16 13 and tomb − 66.
    let (mut ctl, _) = control();
    let mut f = fake();
    read_clue(&mut ctl, &mut f, P1, *b"trs ");
    assert_eq!(f.sent, [(P1, hex("50 0D00 0200 00000000000000000000"))]);
    assert_eq!(ctl.record(13).unwrap().extra.tomb_level, 68);
}

// ------------------------------------------------------------ §8.11

// Covers: specs/world/quests-act2.md §8.11
#[test]
fn tyrael_portal() {
    let (mut ctl, _) = control();
    let mut f = fake();
    let i = i13(&ctl);
    // P1 in Duriel's Lair, P2 there with 14.3, P3 in town.
    f.p(P1).level = Some(73);
    f.pos.insert(P1, (10, 20, R1));
    f.players.insert(P2, act2_player(2, 73));
    set(&mut f, P2, 14, 3);
    f.players.insert(P3, act2_player(3, 40));
    f.party.insert(P1, vec![P1, P3]);
    ctl.records[i].callbacks &= !(1 << 2);
    say(&mut ctl, &mut f, TYRAEL_U, TYRAEL1, 302);
    assert_eq!(ctl.records[i].state, 4);
    assert!(f.flags(P1).get(14, 13) && f.flags(P1).get(14, 3));
    assert!(!f.flags(P3).get(14, 13));
    // Completion flag: only P3 lacks 14.0, 14.3, 14.4.
    assert!(f.flags(P3).get(14, 14));
    assert!(!f.flags(P1).get(14, 14) && !f.flags(P2).get(14, 14));
    assert_eq!(f.sent, [(P3, hex("5D 0D 00 0C 0000"))]);
    assert_eq!(
        f.log,
        [
            "portal 10 20 59 40",
            // `0x00538680` (open question 7) for P1.
            "unhandled 13 0x538680",
            // P1's party (`0x0059C9A0`, not fully specified).
            "unhandled 13 0x59c9a0",
        ]
    );
    let e = ex(&ctl);
    assert!(e.portal_opened && e.chat_tyrael && !e.portal_opening);
    assert!(ctl.records[i].has_callback(2));
    // Once opened: nothing more.
    f.log.clear();
    f.sent.clear();
    say(&mut ctl, &mut f, TYRAEL_U, TYRAEL1, 302);
    assert!(f.log.is_empty() && f.sent.is_empty());
    // Intro game: nothing.
    let (mut ctl, _) = control();
    let mut f = fake();
    f.pos.insert(P1, (10, 20, R1));
    ctl.records[i].not_intro = false;
    say(&mut ctl, &mut f, TYRAEL_U, TYRAEL1, 302);
    assert!(f.log.is_empty() && !ex(&ctl).portal_opened);
}

// Covers: specs/world/quests-act2.md §8.11
#[test]
fn jerhyn_and_meshif_messages() {
    let (mut ctl, _) = control();
    let mut f = fake();
    let i = i13(&ctl);
    ctl.records[i].state = 1;
    ctl.records[i].callbacks &= !(1 << 2);
    // 430: refresh (still state 1: Jerhyn's 430), state 2, +0x08,
    // callback 2, chain 10's sequence.
    say(&mut ctl, &mut f, JERHYN_U, JERHYN, 430);
    assert_eq!(f.sent_ids(), [0x27, 0x29]);
    assert!(f.log.contains(&"0x27 48 [(430, 0)]".to_string()));
    assert_eq!(ctl.records[i].state, 2);
    assert!(ex(&ctl).chat_start && ctl.records[i].has_callback(2));
    // 442 without 14.3: nothing.
    f.sent.clear();
    say(&mut ctl, &mut f, JERHYN_U, JERHYN, 442);
    assert!(f.sent.is_empty() && !ex(&ctl).chat_end);
    // 442 with 14.3, without 14.13: state stays.
    set(&mut f, P1, 14, 3);
    say(&mut ctl, &mut f, JERHYN_U, JERHYN, 442);
    assert_eq!(ctl.records[i].state, 2);
    assert!(ex(&ctl).chat_end);
    assert!(f.flags(P1).get(14, 4) && !f.flags(P1).get(14, 3));
    assert_eq!(f.sent_ids(), [0x27, 0x29]);
    // With 14.13: state 5.
    f.p(P1).quests.flags[0].clear(14, 4);
    set(&mut f, P1, 14, 3);
    set(&mut f, P1, 14, 13);
    say(&mut ctl, &mut f, JERHYN_U, JERHYN, 442);
    assert_eq!(ctl.records[i].state, 5);

    // Meshif 450 with 14.4, 10.0 and 14.13.
    let (mut ctl, _) = control();
    let mut f = fake();
    f.players.insert(P2, act2_player(2, 40));
    set(&mut f, P1, 7, 0);
    set(&mut f, P1, 10, 0);
    set(&mut f, P1, 14, 4);
    set(&mut f, P1, 14, 13);
    f.p(P1).items = vec![*b"hst ", *b"vip ", *b"msf ", *b"box "];
    // Without 14.4 nothing happens.
    say(&mut ctl, &mut f, MESHIF_U, npc::MESHIF1, 449);
    f.p(P1).quests.flags[0].clear(14, 8);
    say(&mut ctl, &mut f, MESHIF_U, npc::MESHIF1, 450);
    assert_eq!(f.p(P1).items, [*b"box "]);
    assert!(ctl.game.get(14, 13));
    let r = ctl.record(13).unwrap();
    assert_eq!((r.state, r.status), (5, 13));
    let fl = f.flags(P1);
    assert!(fl.get(14, 0) && !fl.get(14, 4));
    assert_eq!(r.guids.0, [1]);
    // Status 13 to all (P1 still at 14.4 → 6; P2 without 7.0 → 0), then
    // P1's 0x28 with 14.0 set and 14.4 not yet cleared.
    assert_eq!(
        f.sent,
        [(P1, s5d(6)), (P2, s5d(0)), (P1, s28_with_14_4(&f))]
    );
    // Meshif 450 without 14.13: no game bit, no status.
    let (mut ctl, _) = control();
    let mut f = fake();
    set(&mut f, P1, 14, 4);
    say(&mut ctl, &mut f, MESHIF_U, npc::MESHIF1, 450);
    assert!(!ctl.game.get(14, 13));
    assert_eq!(ctl.record(13).unwrap().state, 0);
    assert_eq!(f.sent, [(P1, s28_with_14_4(&f))]);
    assert!(f.flags(P1).get(14, 0) && !f.flags(P1).get(14, 4));
}

// Covers: specs/world/quests-act2.md §8.11
#[test]
fn townsfolk_messages() {
    let (mut ctl, _) = control();
    let mut f = fake();
    // Any NPC (here Atma for all).
    for (m, b) in [(444, 9), (445, 6), (446, 7), (447, 11), (449, 8), (452, 10)] {
        assert!(!f.flags(P1).get(14, b));
        say(&mut ctl, &mut f, ATMA_U, ATMA, m);
        assert!(f.flags(P1).get(14, b));
    }
    assert!(f.sent.is_empty() && f.log.is_empty());
}

// Covers: specs/world/quests-act2.md §8.11, §edge-cases-original-bugs r12, §edge-cases-original-bugs r13
#[test]
fn duriel_kill() {
    let (mut ctl, _) = control();
    let mut f = fake();
    let i = i13(&ctl);
    f.frame = 40;
    f.pos.insert(DURIEL_U, (5, 5, R2));
    f.objects.insert(DOOR_U, (0x52, 153, 0));
    exm(&mut ctl).door_seen = true;
    exm(&mut ctl).door_guid = 0x52;
    // P1 the killer with P2 in his party; P3 in the lair with 14.0.
    f.players.insert(P2, act2_player(2, 40));
    f.players.insert(P3, act2_player(3, 73));
    set(&mut f, P3, 14, 0);
    f.party.insert(P1, vec![P1, P2]);
    ctl.records[i].callbacks |= 1 << 2;
    ctl.tick = 3;
    let args = EventArgs {
        event: event::MONSTER_KILLED,
        target: Some(DURIEL_U),
        player: Some(P1),
        ..EventArgs::default()
    };
    ev(&mut ctl, &mut f, args);
    let r = &ctl.records[i];
    assert_eq!(r.state, 3);
    assert!(!r.has_callback(2) && !r.has_callback(8));
    assert_eq!(
        ctl.timers,
        [QuestTimer {
            func: TimerFn::Act2(Timer::DurielStatus),
            chain: 13,
            due: 11,
            period: 8,
        }]
    );
    assert!(f.flags(P1).get(14, 5) && f.flags(P2).get(14, 5));
    assert!(!f.flags(P3).get(14, 5));
    let e = ex(&ctl);
    assert!(e.duriel_killed);
    assert_eq!((e.duriel_room, e.door_mode), (Some(R2), 2));
    assert_eq!(ctl.fx, 8);
    // FX 8, then the door (mode 1, end animation at frame + 16); the
    // `0x00545990` stub adds nothing.
    assert_eq!(f.log, ["mode 82 1", "event1 82 56"]);
    assert_eq!(
        f.sent,
        [
            (P1, s28(&f, P1)),
            (P1, hex("89 08")),
            (P2, s28(&f, P2)),
            (P2, hex("89 08")),
            (P3, s28(&f, P3)),
            (P3, hex("89 08")),
        ]
    );
    // The status timer: status ∉ {3, 4, 5} → status 3 to all; removed.
    f.sent.clear();
    set(&mut f, P1, 7, 0);
    exm(&mut ctl).status_timer = true;
    assert!(super::run_timer(&mut ctl, &mut f, Timer::DurielStatus, 13));
    assert_eq!(ctl.records[i].status, 3);
    assert!(!ex(&ctl).status_timer);
    // P1: 14.5 → status byte; P2: no 7.0 → 0; P3: 14.0 and no 14.13/14 → no 5D.
    assert_eq!(f.sent, [(P1, s5d(3)), (P2, s5d(0))]);
    f.sent.clear();
    ctl.records[i].status = 4;
    assert!(super::run_timer(&mut ctl, &mut f, Timer::DurielStatus, 13));
    assert_eq!(ctl.records[i].status, 4);
    assert!(f.sent.is_empty());

    // Edge case 12: in an intro game the callbacks are cleared anyway;
    // no state, no timer, no killer credit; lair players still get 14.5.
    let (mut ctl, _) = control();
    let mut f = fake();
    f.players.insert(P3, act2_player(3, 73));
    ctl.records[i].not_intro = false;
    ctl.records[i].callbacks |= 1 << 2;
    ev(&mut ctl, &mut f, args);
    let r = &ctl.records[i];
    assert_eq!(r.state, 0);
    assert!(!r.has_callback(2) && !r.has_callback(8));
    assert!(ctl.timers.is_empty());
    assert!(!f.flags(P1).get(14, 5) && f.flags(P3).get(14, 5));
    assert_eq!(ex(&ctl).door_mode, 2);
    assert_eq!(ex(&ctl).duriel_room, None);
}

// Covers: specs/world/quests-act2.md §8.11
#[test]
fn chat_end_callback() {
    let (mut ctl, _) = control();
    let mut f = fake();
    let i = i13(&ctl);
    set(&mut f, P1, 7, 0);
    ctl.records[i].state = 2;
    ctl.records[i].callbacks |= 1 << 2;
    // Tyrael without +0x09, other NPCs, Jerhyn without a pending talk:
    // nothing.
    chat_end(&mut ctl, &mut f, TYRAEL_U);
    chat_end(&mut ctl, &mut f, ATMA_U);
    chat_end(&mut ctl, &mut f, JERHYN_U);
    assert!(f.sent.is_empty() && ctl.records[i].has_callback(2));
    // Jerhyn with +0x08: status 1 to all, callback 2 cleared, 14.2.
    exm(&mut ctl).chat_start = true;
    chat_end(&mut ctl, &mut f, JERHYN_U);
    assert_eq!(f.sent, [(P1, s5d(1))]);
    assert!(!ex(&ctl).chat_start && !ctl.records[i].has_callback(2));
    assert!(f.flags(P1).get(14, 2));
    // +0x0A: status 6 to all.
    f.sent.clear();
    exm(&mut ctl).chat_end = true;
    chat_end(&mut ctl, &mut f, JERHYN_U);
    assert_eq!(f.sent, [(P1, s5d(6))]);
    assert!(!ex(&ctl).chat_end);
    // Tyrael with +0x09: status 4 to all.
    f.sent.clear();
    ctl.records[i].callbacks |= 1 << 2;
    exm(&mut ctl).chat_tyrael = true;
    chat_end(&mut ctl, &mut f, TYRAEL_U);
    assert_eq!(f.sent, [(P1, s5d(4))]);
    assert!(!ex(&ctl).chat_tyrael && !ctl.records[i].has_callback(2));
}

// Covers: specs/world/quests-act2.md §8.2, §8.11
#[test]
fn start_join_and_leave() {
    let start = |ctl: &mut QuestControl, f: &mut Fake, ev_id: u8| {
        let args = EventArgs {
            event: ev_id,
            target: Some(P1),
            player: Some(P1),
            ..EventArgs::default()
        };
        ev(ctl, f, args);
    };
    let (mut ctl, _) = control();
    let mut f = fake();
    let i = i13(&ctl);
    // Event 14: clear 10.9 only.
    set(&mut f, P1, 10, 9);
    set(&mut f, P1, 14, 2);
    start(&mut ctl, &mut f, event::PLAYER_JOINED_GAME);
    assert!(!f.flags(P1).get(10, 9));
    assert_eq!((ctl.records[i].state, ctl.records[i].status), (0, 0));
    // Event 13: 10.0 → +0x0C, +0x0D; 14.2 → status 1, state 2.
    set(&mut f, P1, 10, 9);
    set(&mut f, P1, 10, 0);
    start(&mut ctl, &mut f, event::PLAYER_STARTED_GAME);
    assert!(!f.flags(P1).get(10, 9));
    assert!(ex(&ctl).staff_in && ex(&ctl).objects_update);
    assert_eq!((ctl.records[i].state, ctl.records[i].status), (2, 1));
    // 14.3 → 5, 5; 14.4 → status 6, state 5.
    set(&mut f, P1, 14, 3);
    start(&mut ctl, &mut f, event::PLAYER_STARTED_GAME);
    assert_eq!((ctl.records[i].state, ctl.records[i].status), (5, 5));
    set(&mut f, P1, 14, 4);
    start(&mut ctl, &mut f, event::PLAYER_STARTED_GAME);
    assert_eq!((ctl.records[i].state, ctl.records[i].status), (5, 6));
    // 14.0 or 14.15 → +0x3D only.
    let (mut ctl, _) = control();
    let mut f = fake();
    set(&mut f, P1, 14, 15);
    set(&mut f, P1, 14, 4);
    start(&mut ctl, &mut f, event::PLAYER_STARTED_GAME);
    assert!(ex(&ctl).completed_before && !ex(&ctl).staff_in);
    assert_eq!((ctl.records[i].state, ctl.records[i].status), (0, 0));
    assert!(f.sent.is_empty());
    // Event 10: the player leaves the record list.
    ctl.records[i].guids.add(1);
    start(&mut ctl, &mut f, event::PLAYER_LEAVES_GAME);
    assert!(ctl.records[i].guids.0.is_empty());
}

// Covers: specs/world/quests-act2.md §8.11
#[test]
fn portal_check_destination() {
    let (mut ctl, _) = control();
    let mut f = fake();
    assert_eq!(portal_check(73), WarpCheck::Delegate(0x0059_DFD0));
    // +0x3C clear: the portal's own destination.
    assert_eq!(portal_destination(&ctl, &mut f), PortalDest::Default);
    assert!(f.log.is_empty());
    exm(&mut ctl).portal_opening = true;
    assert_eq!(portal_destination(&ctl, &mut f), PortalDest::NoSpot);
    f.spawn_loc = Some((5, 6, RoomId(3)));
    f.spot = Some((1, 1));
    assert_eq!(
        portal_destination(&ctl, &mut f),
        PortalDest::Spot(6, 7, RoomId(3))
    );
    assert_eq!(
        f.log,
        [
            "spawn location 1 40 12",
            "spawn location 1 40 12",
            "spot near 5 6 3 0xbe11 7",
        ]
    );
}

// ------------------------------------------------------------ §10

// Covers: specs/world/quests-act2.md §10
#[test]
fn chain13_hooks() {
    let (mut ctl, _) = control();
    let mut f = fake();
    let i = i13(&ctl);
    // `0x0059D7C0` / `0x0059D7E0` / `0x0059DFB0` by state.
    let hooks = |ctl: &QuestControl| {
        (
            palace_closed_hook(ctl),
            jerhyn_waiting_hook(ctl),
            tyrael_portal_hook(ctl),
        )
    };
    assert_eq!(hooks(&ctl), (false, false, false));
    ctl.records[i].state = 1;
    assert_eq!(hooks(&ctl), (false, true, false));
    ctl.records[i].state = 2;
    assert_eq!(hooks(&ctl), (true, false, false));
    ctl.records[i].state = 4;
    assert_eq!(hooks(&ctl), (true, false, true));
    ctl.records[i].state = 1;
    ctl.records[i].not_intro = false;
    assert_eq!(hooks(&ctl), (true, false, true));
    ctl.records[i].not_intro = true;
    // `0x0059DF50`: +0x3D → true; +0x0F → no living player within 12.
    assert!(!tyrael_leave_hook(&ctl, &mut f, TYRAEL_U));
    exm(&mut ctl).portal_opened = true;
    f.living_near = true;
    assert!(!tyrael_leave_hook(&ctl, &mut f, TYRAEL_U));
    f.living_near = false;
    assert!(tyrael_leave_hook(&ctl, &mut f, TYRAEL_U));
    assert_eq!(f.log, ["living within 12", "living within 12"]);
    exm(&mut ctl).completed_before = true;
    f.living_near = true;
    assert!(tyrael_leave_hook(&ctl, &mut f, TYRAEL_U));
    // `0x0059C750`: the flag iterate for all (state > 1 → 14.2 unless
    // 14.0).
    f.players.insert(P2, act2_player(2, 40));
    set(&mut f, P2, 14, 0);
    tyrael_iterate_hook(&ctl, &mut f);
    assert!(!f.flags(P1).get(14, 2));
    ctl.records[i].state = 2;
    tyrael_iterate_hook(&ctl, &mut f);
    assert!(f.flags(P1).get(14, 2) && !f.flags(P2).get(14, 2));
    // Without chain 13.
    ctl.records.retain(|r| r.chain != 13);
    assert_eq!(hooks(&ctl), (true, false, true));
    assert!(!tyrael_leave_hook(&ctl, &mut f, TYRAEL_U));
}
