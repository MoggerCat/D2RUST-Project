// Spec: specs/world/quests-act2.md §6, §10, §4.9 (Test vectors)
//! Tests for [`super::q4`] (A2Q4 Arcane Sanctuary, chain 11, slot 12).

use super::q4;
use crate::rng::Seed;
use crate::units::{RoomId, UnitId};
use crate::world::quests::tests::*;
use crate::world::quests::*;

const DROGNAN_U: UnitId = UnitId(0x20);
const JERHYN_U: UnitId = UnitId(0x21);
const KAELAN_U: UnitId = UnitId(0x22);
const TOME: UnitId = UnitId(0x50);
const BLOCKER: UnitId = UnitId(0x51);
const PORTAL: UnitId = UnitId(0x52);
const P3: UnitId = UnitId(3);

fn npc(f: &mut Fake, u: UnitId, class: u16) {
    f.monsters.insert(
        u,
        (
            u.0,
            class,
            UnitKind::Monster {
                class: u32::from(class),
                superunique: None,
                owner: None,
            },
        ),
    );
}

fn setup() -> (QuestControl, Fake, usize) {
    let (ctl, _) = control();
    let mut f = Fake::new();
    f.p(P1).act = Some(1);
    f.p(P1).level = Some(40);
    npc(&mut f, DROGNAN_U, 177);
    npc(&mut f, JERHYN_U, 201);
    npc(&mut f, KAELAN_U, 331);
    let i = ctl.find(11).unwrap();
    (ctl, f, i)
}

fn add_player(f: &mut Fake, u: UnitId, level: u32) {
    f.players.insert(
        u,
        Player {
            guid: u.0,
            act: Some(1),
            level: Some(level),
            ..Player::default()
        },
    );
}

fn args(ev: u8, target: Option<UnitId>, a: u32, b: u32) -> EventArgs {
    EventArgs {
        event: ev,
        player: Some(P1),
        target,
        a,
        b,
    }
}

fn ev(ctl: &mut QuestControl, f: &mut Fake, i: usize, a: EventArgs) -> bool {
    q4::callback(ctl, f, i, a, None)
}

fn chat(ctl: &mut QuestControl, f: &mut Fake, i: usize, n: UnitId) -> TextList {
    let mut list = TextList::new();
    let a = args(event::NPC_ACTIVATE, Some(n), 0, 0);
    assert!(q4::callback(ctl, f, i, a, Some(&mut list)));
    list
}

fn say(ctl: &mut QuestControl, f: &mut Fake, i: usize, n: UnitId, msg: u32) {
    let class = f.monster_class(n).unwrap();
    let a = args(event::SCROLL_MESSAGE, Some(n), u32::from(class), msg);
    assert!(ev(ctl, f, i, a));
}

fn level(ctl: &mut QuestControl, f: &mut Fake, i: usize, old: u32, new: u32) {
    assert!(ev(
        ctl,
        f,
        i,
        args(event::CHANGED_LEVEL, Some(P1), old, new)
    ));
}

fn x4(ctl: &QuestControl, i: usize) -> &q4::Extra {
    &ctl.records[i].extra.a2.q4
}

// Covers: specs/world/quests-act2.md §6.4
#[test]
fn kaelan_seed_vector() {
    let (mut ctl, mut f, i) = setup();
    ctl.seed = Seed::new(12345, 666);
    // Palace closed: table state 7 (msg 186), no draw.
    assert_eq!(chat(&mut ctl, &mut f, i, KAELAN_U), [(186, 0)]);
    assert_eq!(ctl.seed, Seed::new(12345, 666));
    // Open: lo' 22752887 mod 3 = 2 → table state 10 (msg 189).
    ctl.records[i].extra.a2.q4.palace_open = true;
    assert_eq!(chat(&mut ctl, &mut f, i, KAELAN_U), [(189, 0)]);
    assert_eq!(ctl.seed.lo, 22_752_887);
    // Every chat open draws, even without a list.
    let a = args(event::NPC_ACTIVATE, Some(KAELAN_U), 0, 0);
    q4::callback(&mut ctl, &mut f, i, a, None);
    assert_ne!(ctl.seed.lo, 22_752_887);
}

// Covers: specs/world/quests-act2.md §6.4, §edge-cases-original-bugs r5
#[test]
fn chat_and_wants_to_talk() {
    let (mut ctl, mut f, i) = setup();
    assert!(chat(&mut ctl, &mut f, i, DROGNAN_U).is_empty());
    // index[state] (`0x00738D44`: −1, 0, 1, 2, 3, 4, 0).
    for (state, want) in [
        (1, (373, 0)),
        (2, (373, 2)),
        (3, (383, 2)),
        (4, (388, 2)),
        (6, (373, 0)),
    ] {
        ctl.records[i].state = state;
        let got = chat(&mut ctl, &mut f, i, DROGNAN_U);
        if state == 6 {
            // State 6 needs 12.13 (> 4).
            assert!(got.is_empty());
        } else {
            assert_eq!(got, [want], "state {state}");
        }
    }
    ctl.records[i].state = 5;
    assert!(chat(&mut ctl, &mut f, i, DROGNAN_U).is_empty());
    f.p(P1).quests.flags[0].set(12, 13);
    assert_eq!(chat(&mut ctl, &mut f, i, DROGNAN_U), [(399, 0)]);
    // GUID listed → 5; 12.1 → 4.
    ctl.records[i].guids.add(1);
    assert_eq!(chat(&mut ctl, &mut f, i, DROGNAN_U), [(399, 2)]);
    f.p(P1).quests.flags[0].set(12, 1);
    assert_eq!(chat(&mut ctl, &mut f, i, DROGNAN_U), [(399, 0)]);
    // Kaelan's table state 6 (msg 185) is never chosen (edge case 5).
    for open in [false, true] {
        ctl.records[i].extra.a2.q4.palace_open = open;
        for _ in 0..8 {
            assert_ne!(chat(&mut ctl, &mut f, i, KAELAN_U), [(185, 0)]);
        }
    }
    // Wants to talk.
    let (mut ctl, mut f, i) = setup();
    assert!(q4::active(&ctl, &mut f, i, P1, 331)); // blocker 0, 12.7 clear
    f.p(P1).quests.flags[0].set(12, 7);
    assert!(!q4::active(&ctl, &mut f, i, P1, 331));
    ctl.records[i].extra.a2.q4.blocker_mode = 2;
    assert!(!q4::active(&ctl, &mut f, i, P1, 331));
    ctl.records[i].extra.a2.q4.blocker_was_neutral = true;
    assert!(q4::active(&ctl, &mut f, i, P1, 331));
    f.p(P1).quests.flags[0].set(12, 8);
    assert!(!q4::active(&ctl, &mut f, i, P1, 331));
    assert!(!q4::active(&ctl, &mut f, i, P1, 177));
    ctl.records[i].state = 1;
    assert!(q4::active(&ctl, &mut f, i, P1, 177));
    assert!(!q4::active(&ctl, &mut f, i, P1, 201));
    ctl.records[i].state = 2;
    assert!(q4::active(&ctl, &mut f, i, P1, 201));
    f.p(P1).quests.flags[0].set(12, 1);
    assert!(!q4::active(&ctl, &mut f, i, P1, 201));
    f.p(P1).quests.flags[0].clear(12, 1);
    f.p(P1).quests.flags[0].set(12, 0);
    assert!(!q4::active(&ctl, &mut f, i, P1, 201));
}

// Covers: specs/world/quests-act2.md §6.5, §6.2, §6.3
#[test]
fn messages_and_chat_end() {
    let (mut ctl, mut f, i) = setup();
    say(&mut ctl, &mut f, i, KAELAN_U, 186);
    assert_eq!(f.flags(P1).word(12), 1 << 7);
    say(&mut ctl, &mut f, i, KAELAN_U, 188);
    assert_eq!(f.flags(P1).word(12), 1 << 7 | 1 << 8);
    // Drognan 373 with the blocker made: state 2, palace open, blocker
    // mode 2 and its collision freed; flag iterate (12.2); refresh.
    let (mut ctl, mut f, i) = setup();
    f.objects.insert(BLOCKER, (0x51, 318, 0));
    ctl.records[i].extra.a2.q4.blocker_made = true;
    ctl.records[i].extra.a2.q4.blocker_guid = 0x51;
    say(&mut ctl, &mut f, i, DROGNAN_U, 373);
    let x = x4(&ctl, i);
    assert!(x.palace_open && x.drognan_started && x.blocker_was_neutral);
    assert_eq!((x.blocker_mode, ctl.records[i].state), (2, 2));
    assert_eq!(f.log[..2], ["mode 81 2", "free collision 81"]);
    assert_eq!(f.flags(P1).word(12), 1 << 2);
    assert_eq!(f.sent_ids(), [0x27, 0x29]);
    // Chat end with Drognan: status 2 to all, callback 2 cleared, iterate.
    f.sent.clear();
    let end = args(event::NPC_DEACTIVATE, Some(DROGNAN_U), 0, 0);
    assert!(ev(&mut ctl, &mut f, i, end));
    assert_eq!(ctl.records[i].status, 2);
    assert!(!x4(&ctl, i).drognan_started);
    assert!(!ctl.records[i].has_callback(event::NPC_DEACTIVATE));
    assert_eq!(f.sent, [(P1, hex("5d 0b 00 02 0000"))]);
    // Jerhyn 377: state 3, refresh, status 3 to all, flag iterate.
    f.sent.clear();
    say(&mut ctl, &mut f, i, JERHYN_U, 377);
    assert_eq!((ctl.records[i].state, ctl.records[i].status), (3, 3));
    assert_eq!(f.sent_ids(), [0x27, 0x29, 0x5D]);
    assert_eq!(f.flags(P1).word(12), 1 << 2 | 1 << 3);
    // 373 from someone else, 377 from Drognan: nothing.
    let (mut ctl, mut f, i) = setup();
    say(&mut ctl, &mut f, i, JERHYN_U, 373);
    say(&mut ctl, &mut f, i, DROGNAN_U, 377);
    assert!(f.sent.is_empty() && ctl.records[i].state == 0);
    // 397–407: refresh; 12.1 → cleared and GUID added.
    f.p(P1).quests.flags[0].set(12, 1);
    say(&mut ctl, &mut f, i, DROGNAN_U, 402);
    assert!(!f.flags(P1).get(12, 1) && ctl.records[i].guids.contains(1));
    assert_eq!(f.sent_ids(), [0x27, 0x29]);
}

// Covers: specs/world/quests-act2.md §6.5
#[test]
fn tome_message_opens_the_canyon_portal() {
    let (mut ctl, mut f, i) = setup();
    f.p(P1).level = Some(74);
    f.pos.insert(P1, (100, 200, RoomId(9)));
    f.spot = Some((101, 199));
    ctl.records[i].status = 4;
    // Not in the tome room: status only.
    let a = args(event::SCROLL_MESSAGE, None, 0, 396);
    ev(&mut ctl, &mut f, i, a);
    assert_eq!(ctl.records[i].status, 5);
    assert!(f.log.is_empty() && f.sent.is_empty());
    ctl.records[i].extra.a2.q4.tome_room = Some(RoomId(9));
    ev(&mut ctl, &mut f, i, a);
    assert_eq!(f.log, ["spot 2 0xbe11 8 100", "portal 101 199 60 46"]);
    assert!(x4(&ctl, i).canyon_portal && ctl.game.get(12, 13));
    // Once only.
    f.log.clear();
    ev(&mut ctl, &mut f, i, a);
    assert!(f.log.is_empty());
}

// Covers: specs/world/quests-act2.md §6.6, §6.10
#[test]
fn level_changes() {
    // Entering the Sanctuary: state 4, status 4 to all, iterate (12.5).
    let (mut ctl, mut f, i) = setup();
    ctl.records[i].state = 3;
    ctl.records[i].status = 3;
    level(&mut ctl, &mut f, i, 54, 74);
    assert_eq!((ctl.records[i].state, ctl.records[i].status), (4, 4));
    assert_eq!(f.sent, [(P1, hex("5d 0b 00 04 0000"))]);
    assert_eq!(f.flags(P1).word(12), 1 << 5);
    // Status ≥ 4 with state already ≥ 4: nothing.
    f.sent.clear();
    f.p(P1).quests.flags[0] = QuestFlags::default();
    level(&mut ctl, &mut f, i, 54, 74);
    assert!(f.sent.is_empty() && f.flags(P1).word(12) == 0);
    // Status ≥ 4, state < 4: state 4 and the iterate only.
    let (mut ctl, mut f, i) = setup();
    ctl.records[i].state = 2;
    ctl.records[i].status = 5;
    level(&mut ctl, &mut f, i, 54, 74);
    assert_eq!(ctl.records[i].state, 4);
    assert!(f.sent.is_empty());
    assert_eq!(f.flags(P1).word(12), 0); // state 4 with status 5: no bit
                                         // Harem 1: 12.8, 12.7.
    level(&mut ctl, &mut f, i, 40, 50);
    assert_eq!(f.flags(P1).word(12), 1 << 7 | 1 << 8);
    // Leaving town at state 3 without 12.0/12.1: state 4, iterate; the
    // Jerhyn handling runs first (palace spawn not specified).
    let (mut ctl, mut f, i) = setup();
    ctl.records[i].state = 3;
    ctl.records[i].status = 2;
    level(&mut ctl, &mut f, i, 40, 41);
    assert_eq!(ctl.records[i].state, 4);
    assert_eq!(f.flags(P1).word(12), 1 << 4);
    assert_eq!(f.log, ["unhandled 11 0x59ef70"]);
    // With 12.1: state stays 3.
    let (mut ctl, mut f, i) = setup();
    ctl.records[i].state = 3;
    ctl.records[i].extra.a2.q4.jerhyn_palace = true;
    f.p(P1).quests.flags[0].set(12, 1);
    level(&mut ctl, &mut f, i, 40, 41);
    assert_eq!(ctl.records[i].state, 3);
    assert!(f.log.is_empty());
}

// Covers: specs/world/quests-act2.md §6.10
#[test]
fn jerhyn_start_side() {
    // Started, unit present, chat held: stop.
    let (mut ctl, mut f, i) = setup();
    ctl.records[i].extra.a2.q4.jerhyn_start = true;
    ctl.records[i].extra.a2.q4.jerhyn_guid = JERHYN_U.0;
    f.npc_held = true;
    q4::jerhyn(&mut ctl, &mut f);
    assert_eq!(f.log, ["hold chat 33"]);
    assert!(x4(&ctl, i).jerhyn_start);
    // No chat: removed, +0x0C cleared, then the palace spawn.
    f.npc_held = false;
    f.log.clear();
    q4::jerhyn(&mut ctl, &mut f);
    assert_eq!(
        f.log,
        ["hold chat 33", "remove unit 33", "unhandled 11 0x59ef70"]
    );
    assert!(!x4(&ctl, i).jerhyn_start);
    // Already at the palace: nothing.
    ctl.records[i].extra.a2.q4.jerhyn_palace = true;
    f.log.clear();
    q4::jerhyn(&mut ctl, &mut f);
    assert!(f.log.is_empty());
}

// Covers: specs/world/quests-act2.md §6.7
#[test]
fn horazon_journal() {
    let (mut ctl, mut f, i) = setup();
    f.objects.insert(TOME, (0x50, 357, 0));
    f.pos.insert(TOME, (5, 5, RoomId(9)));
    f.frame = 100;
    f.p(P1).level = Some(74);
    add_player(&mut f, P2, 40); // P1's party, in Act II
    add_player(&mut f, P3, 40); // elsewhere
    f.party.insert(P1, vec![P1, P2]);
    f.p(P1).quests.flags[0].set(12, 3);
    f.p(P1).quests.flags[0].set(12, 11);
    ctl.records[i].state = 4;
    q4::tome_operate(&mut ctl, &mut f, TOME, P1);
    assert_eq!(f.log, ["mode 80 1", "event1 80 116", "message 1 80 396"]);
    assert_eq!(ctl.records[i].state, 5);
    assert_eq!(x4(&ctl, i).tome_room, Some(RoomId(9)));
    // 12.13, 12.1, 12.0, bits 2–11 cleared, then 12.8, 12.7.
    let granted = 1 << 13 | 1 << 1 | 1 | 1 << 8 | 1 << 7;
    assert_eq!(f.flags(P1).word(12), granted);
    assert_eq!(f.flags(P2).word(12), granted);
    // Completion flag: P3 only.
    assert_eq!(f.flags(P3).word(12), 1 << 14);
    assert_eq!(f.sent, [(P3, hex("5d 0b 00 0c 0000"))]);
    // Read again (mode 1): no animation, the message again.
    f.log.clear();
    f.sent.clear();
    q4::tome_operate(&mut ctl, &mut f, TOME, P1);
    assert_eq!(f.log, ["message 1 80 396"]);
    assert_eq!(f.sent, [(P3, hex("5d 0b 00 0c 0000"))]);
}

// Covers: specs/world/quests-act2.md §6.8, §10
#[test]
fn harem_blocker() {
    let (mut ctl, mut f, i) = setup();
    let dispatcher = UnitId(0x70);
    f.pos.insert(dispatcher, (30, 40, RoomId(3)));
    ctl.records[i].extra.a2.q4.blocker_mode = 1;
    // First position fails, the second works.
    f.object_spawns = vec![None, Some(BLOCKER)];
    f.objects.insert(BLOCKER, (0x51, 318, 0));
    object_event(&mut ctl, &mut f, dispatcher, 0x7A);
    assert_eq!(
        f.log,
        [
            "spawn object 318 28 39",
            "spawn object 318 28 40",
            "flags 81 0x3000000"
        ]
    );
    let x = x4(&ctl, i);
    assert!(x.blocker_made && x.blocker_guid == 0x51 && x.blocker_mode == 0);
    // Made already, or the palace open: nothing.
    f.log.clear();
    object_event(&mut ctl, &mut f, dispatcher, 0x7A);
    let (mut ctl2, mut f2, j) = setup();
    ctl2.records[j].extra.a2.q4.palace_open = true;
    f2.pos.insert(dispatcher, (30, 40, RoomId(3)));
    object_event(&mut ctl2, &mut f2, dispatcher, 0x7A);
    assert!(f.log.is_empty() && f2.log.is_empty());
    // Both positions fail: nothing stored.
    let (mut ctl, mut f, i) = setup();
    f.pos.insert(dispatcher, (30, 40, RoomId(3)));
    object_event(&mut ctl, &mut f, dispatcher, 0x7A);
    assert_eq!(f.log.len(), 2);
    assert!(!x4(&ctl, i).blocker_made);
    // Init 30: mode := +0x40, GUID stored, mode 2 → collision freed.
    let (mut ctl, mut f, i) = setup();
    f.objects.insert(BLOCKER, (0x51, 318, 0));
    q4::blocker_init(&mut ctl, &mut f, BLOCKER);
    assert_eq!(f.log, ["mode 81 0"]);
    assert_eq!(x4(&ctl, i).blocker_guid, 0x51);
    ctl.records[i].extra.a2.q4.blocker_mode = 2;
    f.log.clear();
    q4::blocker_init(&mut ctl, &mut f, BLOCKER);
    assert_eq!(f.log, ["mode 81 2", "free collision 81"]);
    // §10 `0x0059AEC0`: blocker open.
    assert!(q4::blocker_open(&ctl));
}

// Covers: specs/world/quests-act2.md §6.9
#[test]
fn sanctuary_portal() {
    let (mut ctl, mut f, i) = setup();
    f.objects.insert(PORTAL, (0x52, 298, 0));
    // Operate in the Sanctuary: cellar 1, sanctuary 2; again: no change.
    q4::portal_operate(&mut ctl, &mut f, 74);
    let x = x4(&ctl, i);
    assert_eq!((x.portal_mode_sanctuary, x.portal_mode_cellar), (2, 1));
    q4::portal_operate(&mut ctl, &mut f, 54);
    let x = x4(&ctl, i);
    assert_eq!((x.portal_mode_sanctuary, x.portal_mode_cellar), (2, 1));
    // Init in the cellar: 1 → 2 with the end-animation event at frame +
    // 16 + 1.
    f.unit_levels.insert(PORTAL, 54);
    f.frame = 10;
    q4::portal_init(&mut ctl, &mut f, PORTAL);
    assert_eq!(f.log, ["mode 82 2", "event1 82 27"]);
    assert_eq!(x4(&ctl, i).portal_mode_cellar, 2);
    // Init in the Sanctuary (2): the mode only.
    f.unit_levels.insert(PORTAL, 74);
    f.log.clear();
    q4::portal_init(&mut ctl, &mut f, PORTAL);
    assert_eq!(f.log, ["mode 82 2"]);
    // Another level: nothing.
    f.unit_levels.insert(PORTAL, 40);
    f.log.clear();
    q4::portal_init(&mut ctl, &mut f, PORTAL);
    assert!(f.log.is_empty());
    // Operate from the cellar first.
    let (mut ctl, mut f, i) = setup();
    q4::portal_operate(&mut ctl, &mut f, 54);
    let x = x4(&ctl, i);
    assert_eq!((x.portal_mode_sanctuary, x.portal_mode_cellar), (1, 2));
}

// Covers: specs/world/quests-act2.md §6.11
#[test]
fn game_start() {
    let start = args(event::PLAYER_STARTED_GAME, Some(P1), 0, 0);
    let opened = |ctl: &QuestControl, i| {
        let x = x4(ctl, i);
        x.palace_open && x.blocker_mode == 2
    };
    // 12.0: game 12.13, palace open, stop.
    let (mut ctl, mut f, i) = setup();
    f.p(P1).quests.flags[0].set(12, 0);
    ev(&mut ctl, &mut f, i, start);
    assert!(ctl.game.get(12, 13) && opened(&ctl, i));
    assert_eq!(ctl.records[i].state, 0);
    // Nothing: palace closed.
    let (mut ctl, mut f, i) = setup();
    ev(&mut ctl, &mut f, i, start);
    assert!(!opened(&ctl, i) && ctl.records[i].state == 0);
    // 11.0 or 10.0 alone: palace open, no state.
    for slot in [11, 10] {
        let (mut ctl, mut f, i) = setup();
        f.p(P1).quests.flags[0].set(slot, 0);
        ev(&mut ctl, &mut f, i, start);
        assert!(opened(&ctl, i) && ctl.records[i].state == 0, "slot {slot}");
    }
    // `hst ` alone: status 1, state 1, palace closed.
    let (mut ctl, mut f, i) = setup();
    f.p(P1).items.push(*b"hst ");
    ev(&mut ctl, &mut f, i, start);
    let r = &ctl.records[i];
    assert_eq!((r.status, r.state), (1, 1));
    assert!(!opened(&ctl, i));
    // The progress bits (status, state).
    for (b, want) in [(5, (4, 4)), (4, (3, 4)), (3, (3, 3)), (2, (2, 2))] {
        let (mut ctl, mut f, i) = setup();
        f.p(P1).quests.flags[0].set(12, b);
        f.p(P1).items.push(*b"hst ");
        ev(&mut ctl, &mut f, i, start);
        let r = &ctl.records[i];
        assert_eq!((r.status, r.state), want, "bit {b}");
        assert!(opened(&ctl, i));
        assert!(f.sent.is_empty());
    }
    // Event 10.
    let (mut ctl, mut f, i) = setup();
    ctl.records[i].guids.add(1);
    ev(
        &mut ctl,
        &mut f,
        i,
        args(event::PLAYER_LEAVES_GAME, Some(P1), 0, 0),
    );
    assert!(ctl.records[i].guids.0.is_empty());
}

// Covers: specs/world/quests-act2.md §4.9, §6.2
#[test]
fn arcane_hook() {
    // Intro: open the palace (blocker was neutral).
    let (mut ctl, mut f, i) = setup();
    ctl.records[i].not_intro = false;
    q4::arcane_hook(&mut ctl, &mut f);
    let x = x4(&ctl, i);
    assert!(x.palace_open && x.blocker_was_neutral && x.blocker_mode == 2);
    assert_eq!(ctl.records[i].state, 0);
    // Not-intro: state 0 → 1, status 0 → status 1 to all.
    let (mut ctl, mut f, i) = setup();
    q4::arcane_hook(&mut ctl, &mut f);
    assert_eq!((ctl.records[i].state, ctl.records[i].status), (1, 1));
    assert_eq!(f.sent, [(P1, hex("5d 0b 00 01 0000"))]);
    assert!(!x4(&ctl, i).palace_open);
    // Past both: nothing.
    f.sent.clear();
    ctl.records[i].state = 3;
    q4::arcane_hook(&mut ctl, &mut f);
    assert_eq!(ctl.records[i].state, 3);
    assert!(f.sent.is_empty());
}

// Covers: specs/world/quests-act2.md §10
#[test]
fn palace_hooks() {
    let (mut ctl, mut f, i) = setup();
    // `0x0059B6E0`.
    assert!(!q4::guard_moved(&mut ctl));
    ctl.records[i].extra.a2.q4.guard_moved = true;
    assert!(q4::guard_moved(&mut ctl));
    assert!(x4(&ctl, i).guard_moved2);
    assert!(!q4::guard_moved(&mut ctl));
    // `0x0059B8B0`.
    assert!(!q4::guard_at_end(&ctl));
    ctl.records[i].extra.a2.q4.blocker_mode = 2;
    assert!(q4::guard_at_end(&ctl));
    ctl.records[i].extra.a2.q4.guard_pos = true;
    assert!(!q4::guard_at_end(&ctl));
    // `0x0059B8F0`.
    ctl.records[i].extra.a2.q4.guard_x = 50;
    ctl.records[i].extra.a2.q4.guard_y = 60;
    assert_eq!(q4::guard_target(&ctl), Some((50, 60)));
    ctl.records[i].extra.a2.q4.guard_pos2 = true;
    assert_eq!(q4::guard_target(&ctl), Some((50, 56)));
    // `0x0059AEC0`.
    assert!(q4::blocker_open(&ctl));
    ctl.records[i].extra.a2.q4.blocker_mode = 0;
    assert!(!q4::blocker_open(&ctl));
    // `0x0059B820`: P2 within 30 of the blocker, P1 has 14.1.
    f.objects.insert(BLOCKER, (0x51, 318, 0));
    ctl.records[i].extra.a2.q4.blocker_guid = 0x51;
    add_player(&mut f, P2, 40);
    f.distances.insert((P1, BLOCKER), 5);
    f.distances.insert((P2, BLOCKER), 31);
    f.p(P1).quests.flags[0].set(14, 1);
    q4::jerhyn_near_blocker(&mut ctl, &mut f);
    assert!(!x4(&ctl, i).near_blocker);
    f.distances.insert((P2, BLOCKER), 30);
    q4::jerhyn_near_blocker(&mut ctl, &mut f);
    assert!(x4(&ctl, i).near_blocker && x4(&ctl, i).near_guid == 2);
    // `0x0059B6C0` / `0x0059B6D0`: bare `ret`.
    q4::jerhyn_class_hook();
    q4::guard_class_hook();
    // Without chain 11.
    ctl.records.remove(i);
    assert!(q4::guard_at_end(&ctl));
    assert!(!q4::guard_moved(&mut ctl) && !q4::blocker_open(&ctl));
    assert_eq!(q4::guard_target(&ctl), None);
}
