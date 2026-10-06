// Spec: specs/world/quests.md §10.1, §10.4, §10.5 (Test vectors)
//! A1Q1–A1Q3 callback by callback and the sequence walk, from the spec's
//! test vectors and rules, on the quests' fake world.

use super::tests::*;
use super::*;

const KASHYA_U: UnitId = UnitId(0x13);
const CHARSI_U: UnitId = UnitId(0x14);
const P3: UnitId = UnitId(3);

fn npc_kind(class: u16) -> UnitKind {
    UnitKind::Monster {
        class: u32::from(class),
        superunique: None,
        owner: None,
    }
}

/// The fake with Kashya and Charsi beside Akara, Flavie and Warriv.
fn fake() -> Fake {
    let mut f = Fake::new();
    for (u, class) in [(KASHYA_U, npc::KASHYA), (CHARSI_U, npc::CHARSI)] {
        f.monsters.insert(u, (u.0, class, npc_kind(class)));
    }
    f
}

fn player(guid: u32, level: Option<u32>) -> Player {
    Player {
        guid,
        act: Some(0),
        level,
        ..Player::default()
    }
}

/// Event 0 to one chain's record only: the lines it adds.
fn text(ctl: &mut QuestControl, f: &mut Fake, chain: u8, npc_u: UnitId) -> TextList {
    let mut list = TextList::new();
    let args = EventArgs {
        event: event::NPC_ACTIVATE,
        target: Some(npc_u),
        player: Some(P1),
        ..EventArgs::default()
    };
    let i = ctl.find(chain).unwrap();
    act1::callback(ctl, f, i, args, Some(&mut list), false);
    list
}

/// C→S 0x31 to the NPC unit `n` (GUID = unit id in the fake).
fn say(ctl: &mut QuestControl, f: &mut Fake, n: UnitId, msg: u16) {
    let mut m = vec![0x31];
    m.extend_from_slice(&n.0.to_le_bytes());
    m.extend_from_slice(&msg.to_le_bytes());
    m.extend_from_slice(&[0, 0]);
    assert_eq!(ctl.quest_message(f, P1, &m), 0);
}

fn sent(f: &Fake) -> Vec<(UnitId, Vec<u8>)> {
    f.sent.clone()
}

fn kill(ctl: &mut QuestControl, f: &mut Fake, chain: u8, victim: UnitId, killer: UnitId) {
    f.chains.insert(victim, QuestChain(vec![chain]));
    ctl.monster_killed(f, victim, Some(killer));
}

// ------------------------------------------------------------ §10.1

// Covers: specs/world/quests.md §10.1 r1, §10.1 r2, §10.1 r3
#[test]
fn sequence_walk() {
    // Vector: from chain 1 (state 5), chain 2 state 5, chain 4 state 0
    // → chain 4 state 1; the walk stops (returns 1).
    let (mut ctl, _) = control();
    let mut f = fake();
    ctl.record_mut(1).unwrap().state = 5;
    ctl.record_mut(2).unwrap().state = 5;
    assert!(act1::sequence(&mut ctl, &mut f, 1));
    assert_eq!(ctl.record(4).unwrap().state, 1);
    assert_eq!(ctl.record(3).unwrap().state, 0);
    // Chain 1 short of its pass state: returns 1, nothing moves.
    let (mut ctl, _) = control();
    assert!(act1::sequence(&mut ctl, &mut f, 1));
    assert_eq!(ctl.record(2).unwrap().state, 0);
    // A switched-off quest passes the call on whatever its state: 1 →
    // 2 (off) → 4 (state 6 = its pass state) → 3 opens.
    let (mut ctl, _) = control();
    ctl.record_mut(1).unwrap().state = 5;
    ctl.record_mut(2).unwrap().not_intro = false;
    ctl.record_mut(4).unwrap().state = 6;
    assert!(act1::sequence(&mut ctl, &mut f, 1));
    assert_eq!(ctl.record(2).unwrap().state, 0);
    assert_eq!(ctl.record(3).unwrap().state, 1);
    // Any other state stops the walk: chain 2 at 3.
    let (mut ctl, _) = control();
    ctl.record_mut(1).unwrap().state = 5;
    ctl.record_mut(2).unwrap().state = 3;
    assert!(act1::sequence(&mut ctl, &mut f, 1));
    assert_eq!(ctl.record(4).unwrap().state, 0);
    // Chain 5 below state 2: returns 1 without passing on; chain 6 never
    // passes on (its state-0 timer `0x00596580` is not specified).
    let (mut ctl, _) = control();
    ctl.record_mut(3).unwrap().state = 5;
    f.log.clear();
    assert!(act1::sequence(&mut ctl, &mut f, 3));
    assert_eq!(f.log, ["unhandled 6 0x596580"]);
    assert!(act1::sequence(&mut ctl, &mut f, 5));
    assert!(ctl.faults.is_empty());
}

// ------------------------------------------------------------ §10.4

// Covers: specs/world/quests.md §10.4 r1, §10.4 r2, §10.4 r3, §10.4 r4
#[test]
fn den_npc_text() {
    let (mut ctl, _) = control();
    let mut f = fake();
    // Vector: Akara, state 1, not-intro 1, R slot 1 = 0 → message state 0.
    assert_eq!(text(&mut ctl, &mut f, 1, AKARA_U), [(64, 0)]);
    // Vector: R has 1.1 (any state) → message state 3: Akara 76.
    f.p(P1).quests.flags[0].set(1, bit::REWARD_PENDING);
    for s in [0, 4, 5] {
        ctl.record_mut(1).unwrap().state = s;
        assert_eq!(text(&mut ctl, &mut f, 1, AKARA_U), [(76, 0)]);
    }
    // Vector: state 4, R lacks 1.0, 1.1, 1.13, GUID not in the list.
    f.p(P1).quests.flags[0] = QuestFlags::default();
    ctl.record_mut(1).unwrap().state = 4;
    assert!(text(&mut ctl, &mut f, 1, AKARA_U).is_empty());
    // With 1.13, state 4 → message state 3.
    f.p(P1).quests.flags[0].set(1, bit::PRIMARY_GOAL_DONE);
    assert_eq!(text(&mut ctl, &mut f, 1, AKARA_U), [(76, 0)]);
    // GUID in the record's list → message state 4 (Warriv: 80).
    ctl.record_mut(1).unwrap().guids.add(1);
    assert_eq!(text(&mut ctl, &mut f, 1, WARRIV_U), [(80, 2)]);
    // 1.0, or not-intro 0: nothing.
    let (mut ctl, _) = control();
    f.p(P1).quests.flags[0] = QuestFlags::default();
    f.p(P1).quests.flags[0].set(1, bit::REWARD_GRANTED);
    assert!(text(&mut ctl, &mut f, 1, AKARA_U).is_empty());
    f.p(P1).quests.flags[0] = QuestFlags::default();
    ctl.record_mut(1).unwrap().not_intro = false;
    assert!(text(&mut ctl, &mut f, 1, AKARA_U).is_empty());
}

// Covers: specs/world/quests.md §10.4 l2 r1, §10.4 l2 r2, §10.4 l2 r3
#[test]
fn den_level_changes() {
    // Vector: a = 1, b = 2, state 2, status 0, one player with slot 1 =
    // 0x0004 → state 3, slot 1 = 0x0014 (bit 4, bug kept), status 1 and
    // `5d 01 00 01 0000`.
    let (mut ctl, _) = control();
    let mut f = fake();
    f.p(P1).quests.flags[0].set(1, bit::STARTED);
    ctl.record_mut(1).unwrap().state = 2;
    ctl.record_mut(1).unwrap().guids.add(1);
    ctl.changed_level(&mut f, P1, 1, 2);
    let r = ctl.record(1).unwrap();
    assert_eq!((r.state, r.status), (3, 1));
    assert!(!r.has_callback(event::NPC_DEACTIVATE));
    assert!(!r.guids.contains(1));
    assert_eq!(f.flags(P1).word(1), 0x0014);
    assert_eq!(sent(&f), [(P1, hex("5d 01 00 01 0000"))]);
    // Status already 1: bit 3, nothing sent.
    let (mut ctl, _) = control();
    let mut f = fake();
    ctl.record_mut(1).unwrap().state = 2;
    ctl.record_mut(1).unwrap().status = 1;
    ctl.changed_level(&mut f, P1, 1, 2);
    assert_eq!(f.flags(P1).word(1), 0x0008);
    assert!(f.sent.is_empty());
    // Entering level 8 from state 1, status 0: state 3, status 2 sent,
    // then I2 sets bit 4; a second entry changes nothing.
    let (mut ctl, _) = control();
    let mut f = fake();
    ctl.changed_level(&mut f, P1, 2, 8);
    let r = ctl.record(1).unwrap();
    assert_eq!((r.state, r.status), (3, 2));
    assert!(r.extra.entered && !r.has_callback(event::NPC_DEACTIVATE));
    assert_eq!(f.flags(P1).word(1), 0x0010);
    assert_eq!(sent(&f), [(P1, hex("5d 01 00 02 0000"))]);
    f.sent.clear();
    ctl.changed_level(&mut f, P1, 2, 8);
    assert!(f.sent.is_empty());
    // not-intro 0: entering level 8 does nothing.
    let (mut ctl, _) = control();
    ctl.record_mut(1).unwrap().not_intro = false;
    ctl.changed_level(&mut f, P1, 2, 8);
    assert_eq!(ctl.record(1).unwrap().state, 1);
}

// Covers: specs/world/quests.md §10.4 l3 r1, §10.4 l3 r2, §10.4 l3 r3, §10.4 l3 r4, §10.4 l3 r6, §10.4 l3 r7
#[test]
fn den_kills_before_the_clear() {
    // Vector: P = 10, V = 10, spawned 40, killed 37, state 3 → left 3;
    // `5d 01 20 04 0300`; callback 2 null.
    let (mut ctl, _) = control();
    let mut f = fake();
    ctl.record_mut(1).unwrap().state = 3;
    f.den = (40, 37, 10, 10);
    kill(&mut ctl, &mut f, 1, UnitId(0x40), P1);
    let r = ctl.record(1).unwrap();
    assert_eq!((r.extra.monsters_left, r.flags, r.status), (3, 0x20, 4));
    assert!(!r.has_callback(event::NPC_DEACTIVATE));
    assert!(r.extra.guids.contains(1));
    assert_eq!(sent(&f), [(P1, hex("5d 01 20 04 0300"))]);
    // Step 7: P > V, status 4 and left > 5: again, one 0x5D per kill.
    f.sent.clear();
    f.den = (40, 30, 5, 10);
    kill(&mut ctl, &mut f, 1, UnitId(0x41), P1);
    assert_eq!(sent(&f), [(P1, hex("5d 01 20 04 0a00"))]);
    // P > V with status 0: nothing sent.
    let (mut ctl, _) = control();
    let mut f = fake();
    f.den = (40, 39, 5, 10);
    kill(&mut ctl, &mut f, 1, UnitId(0x40), P1);
    assert!(f.sent.is_empty());
    assert_eq!(ctl.record(1).unwrap().extra.monsters_left, 1);
    // not-intro 0: nothing at all.
    let (mut ctl, _) = control();
    ctl.record_mut(1).unwrap().not_intro = false;
    kill(&mut ctl, &mut f, 1, UnitId(0x40), P1);
    assert!(ctl.record(1).unwrap().extra.guids.0.is_empty());
}

// Covers: specs/world/quests.md §10.4 l3 r5, §5 text
#[test]
fn den_clear_iterates_and_timer() {
    // Vector: P = V = 10, spawned = killed = 40 → state 4; game 1.13;
    // killers get 13, 1; others 14 + `5d 01 00 0c 0000` + 0x28; `89 00`;
    // timer 8. P2 is the killer's party member in Act I (I3), P3 has no
    // room level (0: no 0x5D).
    let (mut ctl, _) = control();
    let mut f = fake();
    f.players.insert(P2, player(2, Some(8)));
    f.players.insert(P3, player(3, Some(0)));
    f.party.insert(P1, vec![P1, P2]);
    f.den = (40, 40, 10, 10);
    ctl.tick = 100;
    kill(&mut ctl, &mut f, 1, UnitId(0x40), P1);
    let r = ctl.record(1).unwrap();
    assert_eq!(r.state, 4);
    assert!(r.extra.done && r.extra.timer);
    assert!(!r.has_callback(event::MONSTER_KILLED));
    assert!(ctl.game.get(1, bit::PRIMARY_GOAL_DONE));
    assert_eq!(f.flags(P1).word(1), 0x2002);
    assert_eq!(f.flags(P2).word(1), 0x2002);
    assert_eq!(f.flags(P3).word(1), 0x4000);
    assert_eq!(f.log, ["sound 1 35", "sound 2 35"]);
    let ids: Vec<(UnitId, u8)> = f.sent.iter().map(|m| (m.0, m.1[0])).collect();
    assert_eq!(
        ids,
        [
            (P3, 0x28),
            (P1, 0x28),
            (P1, 0x89),
            (P2, 0x28),
            (P2, 0x89),
            (P3, 0x28),
            (P3, 0x89),
        ]
    );
    assert_eq!(ctl.timers.len(), 1);
    // Vector: timer made at updater tick 100, state still 4 → tick 109:
    // broadcast(5, 0), timer removed.
    f.sent.clear();
    for _ in 0..8 {
        ctl.update(&mut f);
    }
    assert!(f.sent.is_empty());
    ctl.update(&mut f);
    assert_eq!(ctl.tick, 109);
    assert_eq!(
        sent(&f),
        [
            (P1, hex("5d 01 00 05 0000")),
            (P2, hex("5d 01 00 05 0000")),
            (P3, hex("5d 01 00 0c 0000")),
        ]
    );
    assert!(ctl.timers.is_empty() && !ctl.record(1).unwrap().extra.timer);
}

// Covers: specs/world/quests.md §10.4 l4 r1, §10.4 l4 r2, §10.4 l4 r3, §10.4 text
#[test]
fn den_akara_messages() {
    // Vector: msg 76 with R slot 1 = 0x2002, state 4, chain 2 state 0
    // not-intro 1 → chain 1 state 5, status 13; chain 2 state 1; R slot 1
    // = 0x2001; slot 41 = 0x2002; stat 5 + 1.
    let (mut ctl, _) = control();
    let mut f = fake();
    f.p(P1).quests.flags[0].set(1, bit::PRIMARY_GOAL_DONE);
    f.p(P1).quests.flags[0].set(1, bit::REWARD_PENDING);
    f.p(P1).quests.flags[0].set(1, bit::ENTER_AREA);
    ctl.record_mut(1).unwrap().state = 4;
    ctl.record_mut(1).unwrap().flags = 0x20;
    say(&mut ctl, &mut f, AKARA_U, 76);
    let r = ctl.record(1).unwrap();
    assert_eq!((r.state, r.status, r.flags), (5, 13, 0));
    assert!(!r.has_callback(event::NPC_DEACTIVATE) && r.guids.contains(1));
    assert_eq!(ctl.record(2).unwrap().state, 1);
    assert_eq!(f.flags(P1).word(1), 0x2001);
    assert_eq!(f.flags(P1).word(41), 0x2002);
    assert_eq!(f.players[&P1].stats[&5], 1);
    // The refresh: 0x27 then 0x29.
    assert_eq!(f.sent_ids(), [0x27, 0x29]);
    // 76 from another NPC, or without 1.1: nothing.
    let (mut ctl, _) = control();
    let mut f = fake();
    f.p(P1).quests.flags[0].set(1, bit::REWARD_PENDING);
    say(&mut ctl, &mut f, KASHYA_U, 76);
    assert_eq!(f.flags(P1).word(1), 0x0002);
    f.p(P1).quests.flags[0] = QuestFlags::default();
    say(&mut ctl, &mut f, AKARA_U, 76);
    assert_eq!(f.flags(P1).word(1), 0);
    assert!(f.sent.is_empty());
    // 1.1 without 1.13: bits and the skill point, state unchanged.
    f.p(P1).quests.flags[0].set(1, bit::REWARD_PENDING);
    say(&mut ctl, &mut f, AKARA_U, 76);
    assert_eq!(ctl.record(1).unwrap().state, 1);
    assert_eq!(f.flags(P1).word(1), 0x0001);
    assert_eq!(f.players[&P1].stats[&5], 1);
    // Message 64 from any state (no guard): state 2, I2 bit 2.
    let (mut ctl, _) = control();
    let mut f = fake();
    ctl.record_mut(1).unwrap().state = 4;
    say(&mut ctl, &mut f, AKARA_U, 64);
    let r = ctl.record(1).unwrap();
    assert!(r.state == 2 && r.extra.talked);
    assert_eq!(f.flags(P1).word(1), 0x0004);
    // Chat end with another NPC: nothing; with Akara: status 1 and
    // callback 2 cleared.
    f.sent.clear();
    ctl.npc_deactivate(&mut f, P1, KASHYA_U);
    assert!(f.sent.is_empty());
    ctl.npc_deactivate(&mut f, P1, AKARA_U);
    assert_eq!(sent(&f), [(P1, hex("5d 01 00 01 0000"))]);
    assert!(!ctl.record(1).unwrap().has_callback(event::NPC_DEACTIVATE));
    // Event 10: the player's GUID leaves both lists.
    ctl.record_mut(1).unwrap().guids.add(1);
    ctl.record_mut(1).unwrap().extra.guids.add(1);
    ctl.player_leaves(&mut f, P1);
    assert!(!ctl.record(1).unwrap().guids.contains(1));
    assert!(!ctl.record(1).unwrap().extra.guids.contains(1));
}

// Covers: specs/world/quests.md §10.4 text, §6.4
#[test]
fn den_active_fn() {
    let (ctl, _) = control();
    let mut f = fake();
    let i = ctl.find(1).unwrap();
    let f0 = ctl.records[i].active_fn.unwrap();
    assert!(act1::active_fn(&ctl, &mut f, i, P1, npc::AKARA, f0));
    assert!(!act1::active_fn(&ctl, &mut f, i, P1, npc::KASHYA, f0));
    let (mut ctl, _) = control();
    ctl.record_mut(1).unwrap().state = 2;
    assert!(!act1::active_fn(&ctl, &mut f, i, P1, npc::AKARA, f0));
    f.p(P1).quests.flags[0].set(1, bit::REWARD_PENDING);
    assert!(act1::active_fn(&ctl, &mut f, i, P1, npc::AKARA, f0));
    f.p(P1).quests.flags[0].set(1, bit::REWARD_GRANTED);
    assert!(!act1::active_fn(&ctl, &mut f, i, P1, npc::AKARA, f0));
}

// ------------------------------------------------------------ §10.5 A1Q2

// Covers: specs/world/quests.md §10.5 r1, §10.5 r9
#[test]
fn burial_npc_text_and_active() {
    let (mut ctl, _) = control();
    let mut f = fake();
    assert_eq!(ctl.record(2).unwrap().state, 0);
    assert!(text(&mut ctl, &mut f, 2, KASHYA_U).is_empty());
    for (state, want) in [(1, vec![(81, 0)]), (2, vec![(82, 2)]), (3, vec![(87, 2)])] {
        ctl.record_mut(2).unwrap().state = state;
        assert_eq!(text(&mut ctl, &mut f, 2, KASHYA_U), want);
    }
    ctl.record_mut(2).unwrap().state = 4;
    assert!(text(&mut ctl, &mut f, 2, KASHYA_U).is_empty());
    // No not-intro test.
    ctl.record_mut(2).unwrap().state = 1;
    ctl.record_mut(2).unwrap().not_intro = false;
    assert_eq!(text(&mut ctl, &mut f, 2, KASHYA_U), [(81, 0)]);
    ctl.record_mut(2).unwrap().guids.add(1);
    assert_eq!(text(&mut ctl, &mut f, 2, KASHYA_U), [(92, 2)]);
    f.p(P1).quests.flags[0].set(2, bit::REWARD_PENDING);
    assert_eq!(text(&mut ctl, &mut f, 2, KASHYA_U), [(92, 0)]);
    // Active: Kashya, R lacks 2.0, and 2.1 or (not-intro, state 1).
    let (mut ctl, _) = control();
    let mut f = fake();
    let i = ctl.find(2).unwrap();
    let f0 = ctl.records[i].active_fn.unwrap();
    assert!(!act1::active_fn(&ctl, &mut f, i, P1, npc::KASHYA, f0));
    ctl.records[i].state = 1;
    assert!(act1::active_fn(&ctl, &mut f, i, P1, npc::KASHYA, f0));
    assert!(!act1::active_fn(&ctl, &mut f, i, P1, npc::AKARA, f0));
    f.p(P1).quests.flags[0].set(2, bit::REWARD_GRANTED);
    assert!(!act1::active_fn(&ctl, &mut f, i, P1, npc::KASHYA, f0));
}

// Covers: specs/world/quests.md §10.5 r2, §10.5 r3, §10.5 r6, §10.5 r8
#[test]
fn burial_start_and_area() {
    let (mut ctl, _) = control();
    let mut f = fake();
    ctl.record_mut(2).unwrap().state = 1;
    say(&mut ctl, &mut f, KASHYA_U, 81);
    assert!(ctl.record(2).unwrap().extra.talked);
    assert_eq!(f.flags(P1).word(2), 0x0004);
    f.sent.clear();
    ctl.npc_deactivate(&mut f, P1, KASHYA_U);
    assert_eq!(sent(&f), [(P1, hex("5d 02 00 01 0000"))]);
    let r = ctl.record(2).unwrap();
    assert!(!r.extra.talked && !r.has_callback(event::NPC_DEACTIVATE));
    // Leaving town in state 2: state 3, J2 (status 1 → bit 3), nothing
    // sent.
    f.sent.clear();
    ctl.changed_level(&mut f, P1, 1, 2);
    assert_eq!(ctl.record(2).unwrap().state, 3);
    assert_eq!(f.flags(P1).word(2), 0x000C);
    assert!(f.sent.is_empty());
    // Entering the Burial Grounds (17) with status 1: status 2 through
    // J1, flags as they are, then J2 (bit 4).
    ctl.record_mut(2).unwrap().flags = 0x20;
    ctl.changed_level(&mut f, P1, 2, 17);
    let r = ctl.record(2).unwrap();
    assert_eq!((r.state, r.status, r.flags), (3, 2, 0x20));
    assert_eq!(sent(&f), [(P1, hex("5d 02 20 02 0000"))]);
    assert_eq!(f.flags(P1).word(2), 0x001C);
    // From state 0 (not opened yet): state 3, flags 0.
    let (mut ctl, _) = control();
    let mut f = fake();
    ctl.record_mut(2).unwrap().flags = 0x20;
    ctl.changed_level(&mut f, P1, 2, 17);
    let r = ctl.record(2).unwrap();
    assert_eq!((r.state, r.status, r.flags), (3, 2, 0));
    // Event 13 restores from bit 4.
    let (mut ctl, _) = control();
    let mut f = fake();
    f.p(P1).quests.flags[0].set(2, bit::ENTER_AREA);
    ctl.player_enters(&mut f, P1, 0).unwrap();
    let r = ctl.record(2).unwrap();
    assert_eq!((r.state, r.status), (3, 2));
    // Event 10 removes the player's GUID.
    ctl.record_mut(2).unwrap().guids.add(1);
    ctl.player_leaves(&mut f, P1);
    assert!(!ctl.record(2).unwrap().guids.contains(1));
}

// Covers: specs/world/quests.md §10.5 r4, §10.5 r5
#[test]
fn burial_kill_and_timer() {
    let (mut ctl, _) = control();
    let mut f = fake();
    let raven = UnitId(0x40);
    f.monsters.insert(raven, (0x40, 267, npc_kind(267)));
    f.players.insert(P2, player(2, Some(17)));
    f.players.insert(P3, player(3, Some(1)));
    // P1 near Blood Raven; P2 not near but P3's party member... P3 is in
    // P1's party: J7 gives it 13, 1 (room level ≠ 0, Act I).
    f.near = vec![P1];
    f.party.insert(P1, vec![P1, P3]);
    ctl.record_mut(2).unwrap().state = 3;
    ctl.tick = 10;
    kill(&mut ctl, &mut f, 2, raven, P2);
    let r = ctl.record(2).unwrap();
    assert_eq!(r.state, 4);
    assert!(r.extra.killed && r.extra.kill_b1 && r.extra.kill_b2);
    assert_eq!((r.extra.kill_d4, r.extra.victim), (1, 0x40));
    assert!(!r.has_callback(event::NPC_DEACTIVATE) && r.has_callback(event::MONSTER_KILLED));
    assert!(ctl.game.get(2, bit::PRIMARY_GOAL_DONE));
    assert_eq!(f.flags(P1).word(2), 0x2002);
    assert_eq!(f.flags(P3).word(2), 0x2002);
    assert_eq!(f.flags(P2).word(2), 0x4000);
    // J5: 0x5D to P2 only, no 0x28; J6: sound 34 for 13-holders.
    assert_eq!(sent(&f), [(P2, hex("5d 02 00 0c 0000"))]);
    assert_eq!(f.log, ["sound 1 34", "sound 3 34"]);
    assert_eq!(ctl.timers.len(), 1);
    // Timer 15: broadcast(3, 0) at tick 10 + 16, once.
    f.sent.clear();
    for _ in 0..15 {
        ctl.update(&mut f);
    }
    assert!(f.sent.is_empty());
    ctl.update(&mut f);
    assert_eq!(
        sent(&f),
        [
            (P1, hex("5d 02 00 03 0000")),
            (P2, hex("5d 02 00 0c 0000")),
            (P3, hex("5d 02 00 03 0000")),
        ]
    );
    assert!(ctl.timers.is_empty());
    // Callback 8 stays: a second linked death repeats it (a new timer).
    kill(&mut ctl, &mut f, 2, raven, P2);
    assert_eq!(ctl.timers.len(), 1);
    assert!(ctl.faults.is_empty());
    // The victim's GUID must resolve (fatal otherwise).
    let (mut ctl, _) = control();
    let mut f = fake();
    kill(&mut ctl, &mut f, 2, UnitId(0x99), P1);
    assert_eq!(ctl.faults, [QuestError::Fatal(0x0059_0C40)]);
    // not-intro 0: nothing.
    let (mut ctl, _) = control();
    ctl.record_mut(2).unwrap().not_intro = false;
    kill(&mut ctl, &mut f, 2, raven, P1);
    assert_eq!(ctl.record(2).unwrap().state, 0);
}

// Covers: specs/world/quests.md §10.5 r7, §10.2
#[test]
fn burial_reward() {
    let (mut ctl, _) = control();
    let mut f = fake();
    let fl = &mut f.p(P1).quests.flags[0];
    fl.set(2, bit::PRIMARY_GOAL_DONE);
    fl.set(2, bit::REWARD_PENDING);
    fl.set(2, bit::STARTED);
    ctl.record_mut(2).unwrap().state = 4;
    ctl.record_mut(2).unwrap().flags = 0x20;
    say(&mut ctl, &mut f, KASHYA_U, 92);
    let r = ctl.record(2).unwrap();
    assert_eq!((r.state, r.status, r.flags), (5, 13, 0));
    assert!(r.guids.contains(1) && r.has_callback(event::NPC_DEACTIVATE));
    // Sequence: chain 2 at 5 passes to chain 4 (state 0 → 1).
    assert_eq!(ctl.record(4).unwrap().state, 1);
    // Bits 2–11 stay.
    assert_eq!(f.flags(P1).word(2), 0x2005);
    assert_eq!(f.sent_ids(), [0x28, 0x27, 0x29]);
    assert_eq!(f.log.iter().filter(|l| *l == "merc 150").count(), 1);
    // Without 2.13: bits and the mercenary, state unchanged.
    let (mut ctl, _) = control();
    let mut f = fake();
    f.p(P1).quests.flags[0].set(2, bit::REWARD_PENDING);
    say(&mut ctl, &mut f, KASHYA_U, 92);
    assert_eq!(ctl.record(2).unwrap().state, 0);
    assert_eq!(f.flags(P1).word(2), 0x0001);
    assert!(f.log.contains(&"merc 150".to_string()));
}

// ------------------------------------------------------------ §10.5 A1Q3

fn malus_fake() -> (QuestControl, Fake, UnitId) {
    let (ctl, _) = control();
    let mut f = fake();
    let malus = UnitId(0x71);
    f.objects.insert(malus, (0x71, q3::MALUS_OBJECT, 0));
    (ctl, f, malus)
}

use act1::q3;

// Covers: specs/world/quests.md §10.5 l2 r1, §10.5 l2 r2
#[test]
fn malus_object_init_and_operate() {
    let (mut ctl, mut f, malus) = malus_fake();
    act1::malus_init(&mut ctl, &mut f, malus);
    let r = ctl.record(3).unwrap();
    assert!(r.extra.malus_known && r.extra.malus_guid == 0x71);
    assert_eq!(f.log, ["mode 113 0"]);
    // Switched off: the object is shown taken; operating it only sounds.
    let (mut ctl, mut f, malus) = malus_fake();
    ctl.record_mut(3).unwrap().not_intro = false;
    act1::malus_init(&mut ctl, &mut f, malus);
    assert_eq!(ctl.record(3).unwrap().extra.malus_mode, 2);
    assert_eq!(f.objects[&malus].2, 2);
    f.log.clear();
    act1::malus_operate(&mut ctl, &mut f, malus, P1);
    assert_eq!(f.log, ["mode 113 2", "sound 1 19"]);
    // Mode ≠ 0: nothing.
    let (mut ctl, mut f, malus) = malus_fake();
    f.p(P1).stats.insert(12, 8);
    f.objects.get_mut(&malus).unwrap().2 = 2;
    act1::malus_operate(&mut ctl, &mut f, malus, P1);
    assert!(f.log.is_empty());
    // Status already 1: flags kept.
    let (mut ctl, mut f, malus) = malus_fake();
    f.p(P1).stats.insert(12, 8);
    ctl.record_mut(3).unwrap().status = 1;
    ctl.record_mut(3).unwrap().flags = 0x20;
    ctl.record_mut(3).unwrap().state = 4;
    act1::malus_operate(&mut ctl, &mut f, malus, P1);
    assert_eq!(ctl.record(3).unwrap().flags, 0x20);
    // State already 4: no K2.
    assert_eq!(f.flags(P1).word(3), 0);
}

// Covers: specs/world/quests.md §10.5 l2 r3, §10.5 l2 r14
#[test]
fn malus_npc_text_and_active() {
    let (mut ctl, mut f, _) = malus_fake();
    let i = ctl.find(3).unwrap();
    let f0 = ctl.records[i].active_fn.unwrap();
    // State 0: nothing; state 1 → message state 0 (146, menu 1 → 0).
    assert!(text(&mut ctl, &mut f, 3, CHARSI_U).is_empty());
    ctl.record_mut(3).unwrap().state = 1;
    assert_eq!(text(&mut ctl, &mut f, 3, CHARSI_U), [(146, 0)]);
    assert!(act1::active_fn(&ctl, &mut f, i, P1, npc::CHARSI, f0));
    ctl.record_mut(3).unwrap().state = 4;
    assert!(text(&mut ctl, &mut f, 3, CHARSI_U).is_empty());
    assert!(!act1::active_fn(&ctl, &mut f, i, P1, npc::CHARSI, f0));
    // Carrying the Malus at level 8 → message state 3, active.
    f.p(P1).items.push(*b"hdm ");
    f.p(P1).stats.insert(12, 8);
    let want = {
        let mut l = TextList::new();
        let t = ctl.records[i].msgs.unwrap();
        ctl.add_messages(t, &mut l, npc::CHARSI, 3);
        l
    };
    assert!(!want.is_empty());
    assert_eq!(text(&mut ctl, &mut f, 3, CHARSI_U), want);
    assert!(act1::active_fn(&ctl, &mut f, i, P1, npc::CHARSI, f0));
    // Level 7 with the Malus: nothing, not active.
    f.p(P1).stats.insert(12, 7);
    assert!(text(&mut ctl, &mut f, 3, CHARSI_U).is_empty());
    assert!(!act1::active_fn(&ctl, &mut f, i, P1, npc::CHARSI, f0));
    // R has 3.0 but not 3.13: nothing.
    f.p(P1).stats.insert(12, 8);
    f.p(P1).quests.flags[0].set(3, bit::REWARD_GRANTED);
    assert!(text(&mut ctl, &mut f, 3, CHARSI_U).is_empty());
    assert!(!act1::active_fn(&ctl, &mut f, i, P1, npc::CHARSI, f0));
}

// Covers: specs/world/quests.md §10.5 l2 r4, §10.5 l2 r5, §10.5 l2 r6, §10.5 l2 r9
#[test]
fn malus_start_chat_end_and_pick_up() {
    let (mut ctl, mut f, _) = malus_fake();
    say(&mut ctl, &mut f, CHARSI_U, 146);
    assert!(ctl.record(3).unwrap().state == 2 && ctl.record(3).unwrap().extra.talked);
    // No K2 at the message: bit 2 comes with the chat end.
    assert_eq!(f.flags(P1).word(3), 0);
    f.sent.clear();
    ctl.npc_deactivate(&mut f, P1, CHARSI_U);
    assert_eq!(f.flags(P1).word(3), 0x0004);
    assert_eq!(sent(&f), [(P1, hex("5d 03 00 01 0000"))]);
    assert!(!ctl.record(3).unwrap().extra.talked);
    // Leaving town in state 2: state 3, K2 bit 3, callback 3 cleared;
    // status already 1, nothing sent.
    f.sent.clear();
    ctl.changed_level(&mut f, P1, 1, 2);
    let r = ctl.record(3).unwrap();
    assert!(r.state == 3 && !r.has_callback(event::CHANGED_LEVEL));
    assert_eq!(f.flags(P1).word(3), 0x000C);
    assert!(f.sent.is_empty());
    // Picking up the Malus (event 4, active record): 3.6 and sound 36
    // once, status 2 each time.
    let item = UnitId(0x80);
    f.chains.insert(item, QuestChain(vec![3]));
    ctl.item_event(&mut f, event::ITEM_PICKED_UP, P1, item);
    ctl.item_event(&mut f, event::ITEM_PICKED_UP, P1, item);
    assert!(f.flags(P1).get(3, 6));
    assert_eq!(f.log.iter().filter(|l| *l == "sound 1 36").count(), 1);
    assert_eq!(
        sent(&f),
        [(P1, hex("5d 03 00 02 0000")), (P1, hex("5d 03 00 02 0000"))]
    );
    // Event 10: the +0x14 list loses the player.
    ctl.record_mut(3).unwrap().extra.guids.add(1);
    ctl.player_leaves(&mut f, P1);
    assert!(!ctl.record(3).unwrap().extra.guids.contains(1));
    // not-intro 0: event 3 clears callback 3.
    let (mut ctl, mut f, _) = malus_fake();
    ctl.record_mut(3).unwrap().not_intro = false;
    ctl.changed_level(&mut f, P1, 2, 3);
    assert!(!ctl.record(3).unwrap().has_callback(event::CHANGED_LEVEL));
    // The reward's chat end: status 13.
    let (mut ctl, mut f, _) = malus_fake();
    ctl.record_mut(3).unwrap().extra.rewarded = true;
    f.p(P1).quests.flags[0].set(3, bit::PRIMARY_GOAL_DONE);
    ctl.npc_deactivate(&mut f, P1, CHARSI_U);
    assert_eq!(sent(&f), [(P1, hex("5d 03 00 0d 0000"))]);
    assert!(!ctl.record(3).unwrap().extra.rewarded);
}

// Covers: specs/world/quests.md §10.5 l2 r10, §10.5 l2 r17
#[test]
fn malus_brought_to_charsi() {
    let (mut ctl, mut f, malus) = malus_fake();
    f.p(P1).stats.insert(12, 8);
    f.players.insert(P2, player(2, Some(1)));
    f.p(P2).stats.insert(12, 8);
    f.players.insert(P3, player(3, Some(1)));
    f.p(P3).stats.insert(12, 7);
    f.party.insert(P1, vec![P1, P2, P3]);
    act1::malus_operate(&mut ctl, &mut f, malus, P1);
    f.p(P1).items.push(*b"hdm ");
    f.sent.clear();
    f.log.clear();
    say(&mut ctl, &mut f, CHARSI_U, 163);
    let r = ctl.record(3).unwrap();
    assert!(r.state == 5 && r.extra.rewarded && r.extra.guids.contains(1));
    assert_eq!(r.extra.malus_items, 0);
    assert!(ctl.game.get(3, bit::PRIMARY_GOAL_DONE));
    assert!(f.flags(P1).get(3, 13) && f.flags(P1).get(3, 1));
    // K3: the level-8 member gets 13, 1; the level-7 one nothing.
    assert_eq!(f.flags(P2).word(3) & 0x2002, 0x2002);
    assert_eq!(f.flags(P3).word(3) & 0x2002, 0);
    assert!(!f.players[&P1].items.contains(b"hdm "));
    assert_eq!(f.sent_ids(), [0x28, 0x27, 0x29]);
    // Sequence: chain 3 at 5 passes to chain 6 (its timer unspecified).
    assert!(f.log.contains(&"unhandled 6 0x596580".to_string()));
    // Without the Malus: listed, text refreshed, nothing else.
    let (mut ctl, mut f, _) = malus_fake();
    say(&mut ctl, &mut f, CHARSI_U, 163);
    assert!(ctl.record(3).unwrap().extra.guids.contains(1));
    assert_eq!(f.flags(P1).word(3), 0);
    assert_eq!(f.sent_ids(), [0x27, 0x29]);
    // With 3.0: nothing, no refresh.
    let (mut ctl, mut f, _) = malus_fake();
    f.p(P1).quests.flags[0].set(3, bit::REWARD_GRANTED);
    say(&mut ctl, &mut f, CHARSI_U, 163);
    assert!(f.sent.is_empty() && ctl.record(3).unwrap().extra.guids.0.is_empty());
    // Not at state 4 but started with the Malus: chain 6's sequence.
    let (mut ctl, mut f, _) = malus_fake();
    f.p(P1).items.push(*b"hdm ");
    ctl.record_mut(3).unwrap().state = 2;
    ctl.record_mut(3).unwrap().extra.started_with_malus = true;
    say(&mut ctl, &mut f, CHARSI_U, 163);
    assert_eq!(ctl.record(3).unwrap().state, 2);
    assert!(f.log.contains(&"unhandled 6 0x596580".to_string()));
}

// Covers: specs/world/quests.md §10.5 l2 r7, §10.5 l2 r8, §10.5 l2 r11, §10.5 l2 r12, §10.5 l2 r15
#[test]
fn malus_counts_and_reset() {
    // Event 13 with the Malus: counted, +0xA1; bit 2 → state 2 first.
    let (mut ctl, mut f, malus) = malus_fake();
    f.p(P1).items.push(*b"hdm ");
    f.p(P1).quests.flags[0].set(3, bit::STARTED);
    f.p(P1).quests.flags[0].set(3, bit::LEAVE_TOWN);
    ctl.player_enters(&mut f, P1, 0).unwrap();
    let r = ctl.record(3).unwrap();
    assert_eq!((r.state, r.status), (2, 1));
    assert!(r.extra.started_with_malus && r.extra.malus_items == 1);
    // Event 14: another carrier joins; the count reaching 1 with the
    // Malus taken sets status 2 (nothing sent).
    let (mut ctl, mut f, _) = malus_fake();
    ctl.player_enters(&mut f, P1, 0).unwrap();
    {
        let x = &mut ctl.record_mut(3).unwrap().extra;
        x.malus_known = true;
        x.malus_mode = 2;
    }
    f.p(P1).items.push(*b"hdm ");
    f.sent.clear();
    ctl.player_enters(&mut f, P1, 0).unwrap();
    let r = ctl.record(3).unwrap();
    assert_eq!((r.extra.malus_items, r.status), (1, 2));
    // Event 9: the carrier leaves; the count reaching 0 → status 3.
    f.p(P1).act = Some(0);
    f.sent.clear();
    let args = EventArgs {
        event: event::PLAYER_DROPPED_WITH_QUEST_ITEM,
        target: Some(UnitId(0x80)),
        player: Some(P1),
        ..EventArgs::default()
    };
    let i = ctl.find(3).unwrap();
    act1::callback(&mut ctl, &mut f, i, args, None, false);
    // Status 3 is set; the 0x5D reports the status function's value: the
    // leaving player still carries the Malus (K4) → 2.
    let r = ctl.record(3).unwrap();
    assert_eq!((r.extra.malus_items, r.status), (0, 3));
    assert_eq!(sent(&f), [(P1, hex("5d 03 00 02 0000"))]);
    // Event 6 → reset: state 3, flags 1, status 1 through K1, the object
    // back to mode 0.
    let (mut ctl, mut f, _) = malus_fake();
    f.p(P1).stats.insert(12, 8);
    act1::malus_operate(&mut ctl, &mut f, malus, P1);
    f.sent.clear();
    let args = EventArgs {
        event: event::EVENT6,
        player: Some(P1),
        ..EventArgs::default()
    };
    let i = ctl.find(3).unwrap();
    act1::callback(&mut ctl, &mut f, i, args, None, false);
    let r = ctl.record(3).unwrap();
    assert_eq!((r.state, r.flags, r.status), (3, 1, 1));
    assert_eq!((r.extra.malus_items, r.extra.malus_mode), (0, 0));
    assert!(r.extra.malus_known);
    assert_eq!(f.objects[&malus].2, 0);
    assert_eq!(sent(&f), [(P1, hex("5d 03 01 01 0000"))]);
    // Reset with the object gone: Malus no longer known.
    let (mut ctl, mut f, _) = malus_fake();
    f.p(P1).stats.insert(12, 8);
    act1::malus_operate(&mut ctl, &mut f, malus, P1);
    f.objects.clear();
    act1::callback(&mut ctl, &mut f, i, args, None, false);
    assert!(!ctl.record(3).unwrap().extra.malus_known);
}

// Covers: specs/world/quests.md §10.5 l2 r13
#[test]
fn malus_status_fn() {
    let (mut ctl, mut f, _) = malus_fake();
    let i = ctl.find(3).unwrap();
    let st = |ctl: &QuestControl, f: &mut Fake| {
        let pf = f.flags(P1);
        act1::status_fn(ctl, f, i, P1, &pf, 0x0059_1D30)
    };
    ctl.records[i].status = 7;
    assert_eq!(st(&ctl, &mut f), Some(7));
    ctl.records[i].state = 5;
    assert_eq!(st(&ctl, &mut f), Some(0));
    ctl.game.set(3, bit::PRIMARY_GOAL_DONE);
    assert_eq!(st(&ctl, &mut f), Some(4));
    f.p(P1).stats.insert(12, 8);
    assert_eq!(st(&ctl, &mut f), Some(12));
    f.p(P1).quests.flags[0].set(3, bit::COMPLETED_NOW);
    assert_eq!(st(&ctl, &mut f), Some(12));
    f.p(P1).quests.flags[0].set(3, bit::PRIMARY_GOAL_DONE);
    assert_eq!(st(&ctl, &mut f), Some(13));
    ctl.records[i].not_intro = false;
    assert_eq!(st(&ctl, &mut f), Some(0));
    // A party member with the Malus: 2, or 0 with 3.0.
    f.players.insert(P2, player(2, Some(1)));
    f.p(P2).items.push(*b"hdm ");
    f.party.insert(P1, vec![P1, P2]);
    assert_eq!(st(&ctl, &mut f), Some(2));
    f.p(P1).quests.flags[0].set(3, bit::REWARD_GRANTED);
    assert_eq!(st(&ctl, &mut f), Some(0));
    f.p(P1).quests.flags[0].set(3, bit::REWARD_PENDING);
    assert_eq!(st(&ctl, &mut f), Some(10));
    // In 0x52 (§6.2): written to slot 3.
    let (mut ctl, mut f, _) = malus_fake();
    ctl.record_mut(3).unwrap().status = 1;
    ctl.request_quest_data(&mut f, P1).unwrap();
    assert_eq!(f.sent.last().unwrap().1[1 + 3], 1);
}

// Covers: specs/world/quests.md §10.5 l2 r16
#[test]
fn malus_imbue_granted() {
    let (mut ctl, mut f, _) = malus_fake();
    f.p(P1).quests.flags[0].set(3, bit::REWARD_PENDING);
    act1::imbue_granted(&mut ctl, &mut f, P1);
    assert_eq!(f.flags(P1).word(3), 0x0001);
    assert!(!ctl.record(3).unwrap().active);
    // With 3.15 the record stays active.
    let (mut ctl, mut f, _) = malus_fake();
    f.p(P1).quests.flags[0].set(3, bit::COMPLETED_BEFORE);
    act1::imbue_granted(&mut ctl, &mut f, P1);
    assert!(ctl.record(3).unwrap().active);
}
