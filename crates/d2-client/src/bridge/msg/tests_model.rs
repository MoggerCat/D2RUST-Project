// Spec: specs/client/model.md
//! Test vectors of `client/model.md` (recorded bytes from
//! `traces/raw/20261006-022633-packets.jsonl` unless marked synthetic).

use super::super::check::{check, Checked};
use super::super::world::{
    update_order, ActLoad, ClientUnit, ClientWorld, ModelInputs, MonsterClass, RoomSight, UnitKey,
    MISSILE, MONSTER, OBJECT, PLAYER,
};
use super::support::{hex, Model};

const AC_1_6: &str = "ac 06 00 00 00 9a 00 1a 12 b9 11 80 0e 01";
const STOP_1_6: &str = "6d 06 00 00 00 1a 12 b9 11 80";
const M6: UnitKey = UnitKey::new(MONSTER, 6);
const P1: UnitKey = UnitKey::new(PLAYER, 1);

/// A model whose tables have `monstats` rows 0..=154, each with a
/// `monstats2` row and no components.
fn with_monsters() -> Model {
    let mut m = Model::default();
    m.inputs.tables.monsters = vec![Some(MonsterClass::default()); 155];
    m
}

/// 0x59 for player 1 ("werwer", class 1) at (x, y).
fn assign_player(x: u16, y: u16) -> Vec<u8> {
    let mut b = hex("59 01 00 00 00 01 77 65 72 77 65 72");
    b.resize(0x16, 0);
    b.extend_from_slice(&x.to_le_bytes());
    b.extend_from_slice(&y.to_le_bytes());
    b
}

// Covers: specs/client/model.md §7 r1, §7 r2, §7 r3
#[test]
fn session_flags_and_load_successful() {
    let mut m = Model::default();
    m.hex("01 00 04 00 10 00 01 00");
    assert_eq!(
        (m.w.difficulty, m.w.game_flags, m.w.expansion, m.w.ladder),
        (0, 0x0010_0004, 1, 0)
    );
    let before = m.w.clone();
    m.hex("00");
    assert_eq!(m.w, before);
    m.hex("02");
    assert_eq!(m.w.outgoing, vec![vec![0x6B]]);
    assert!(m.log.rejected.is_empty());
    assert_eq!(m.log.handled, 3);
}

// Covers: specs/client/model.md §3 r1, §3 r2, §1 r1, §edge-cases-original-bugs
#[test]
fn local_player_from_0x0b() {
    let mut m = Model::default();
    m.recv(&assign_player(0, 0)).hex("0b 00 01 00 00 00");
    assert_eq!(m.w.local_player, Some(P1));
    // Randomness rule 2: one step on {1, 666}.
    assert_eq!(m.unit(P1).seed, Some((0x6AC6_935F, 0)));
    // An unknown key: unchanged.
    m.hex("0b 00 09 00 00 00");
    assert_eq!(m.w.local_player, Some(P1));
    // Cleared only by freeing that unit; a replacing add frees it.
    m.recv(&assign_player(0, 0));
    assert_eq!(m.w.local_player, None);
    m.hex("0b 00 01 00 00 00").hex("0a 00 01 00 00 00");
    assert_eq!(m.w.local_player, None);
    assert!(m.w.units.is_empty());
}

// Covers: specs/client/model.md §7 r4, §9 r1, §9 r2, §9 r3, §9 r4
#[test]
fn load_act_and_rooms_in_sight() {
    let mut m = Model::default();
    // Without an act: fatal 0x58A / 0x59E (handler errors), no change.
    m.hex("07 a0 03 88 03 01").hex("08 d0 03 60 04 01");
    assert_eq!(
        m.rejected(),
        [
            (0x07, "fatal assert 0x58A".to_owned()),
            (0x08, "fatal assert 0x59E".to_owned())
        ]
    );
    assert!(m.w.rooms_in_sight.is_empty());
    m.hex("03 00 c4 88 38 10 01 00 61 d1 e0 9f");
    assert_eq!(
        m.w.act,
        Some(ActLoad {
            act: 0,
            init_seed: 0x1038_88C4,
            town_level: 1,
            f8: 0x9FE0_D161
        })
    );
    m.hex("07 a0 03 88 03 01").hex("08 d0 03 60 04 01");
    assert_eq!(
        m.w.rooms_in_sight,
        [
            RoomSight {
                show: true,
                level: 1,
                x: 0x3A0,
                y: 0x388
            },
            RoomSight {
                show: false,
                level: 1,
                x: 0x3D0,
                y: 0x460
            }
        ]
    );
}

// Covers: specs/client/model.md §7 r5, §7 r6, §7 r7
#[test]
fn load_complete_unload_and_exit() {
    let mut m = Model::default();
    // No local player: fatal 0x527.
    m.hex("04");
    assert_eq!(m.rejected(), [(0x04, "fatal assert 0x527".to_owned())]);
    assert!(!m.w.in_game);
    // A local player created at (0, 0) has no room: still fatal.
    m.recv(&assign_player(0, 0))
        .hex("0b 00 01 00 00 00")
        .hex("04");
    assert_eq!(m.log.rejected.len(), 2);
    // Placed by 0x15: in game.
    m.hex("15 00 01 00 00 00 41 12 c4 11 01").hex("04");
    assert!(m.w.in_game && !m.w.unloaded);
    m.hex("05");
    assert!(!m.w.in_game && m.w.unloaded);
    m.hex("06");
    assert!(m.w.exit_requested);
    assert_eq!(m.log.rejected.len(), 2);
}

// Covers: specs/client/model.md §4 r1, §4 r5, §8 r1, §8 r3
#[test]
fn unit_message_is_queued_then_applied_in_the_update_pass() {
    let mut m = with_monsters();
    m.recv(&[hex(AC_1_6), hex(STOP_1_6)].concat());
    assert_eq!(m.log.queued, 1);
    let u = m.unit(M6);
    assert_eq!(u.queue, vec![hex(STOP_1_6)]);
    // Not applied at receive.
    assert_eq!(u.last_mode_request, None);
    assert_eq!(u.stat(328), 0x23D3);
    assert_eq!(m.drain(), 1);
    let u = m.unit(M6);
    assert!(u.queue.is_empty());
    assert_eq!(u.stat(328), 0x23D4);
    assert_eq!(
        u.last_mode_request.map(|r| (r.code, r.record)),
        Some((7, [0x121A, 0x11B9, 0x80, 2, 0, 4, 0]))
    );
    assert_eq!(m.log.drained, 1);
}

// Covers: specs/client/model.md §4 r6, §2 r5, §2 r2
#[test]
fn queue_follows_the_unit_table() {
    // The unit message before the add: dropped at receive.
    let mut m = with_monsters();
    m.recv(&[hex(STOP_1_6), hex(AC_1_6)].concat());
    assert_eq!(m.log.dropped.get(&0x6D), Some(&1));
    assert!(m.unit(M6).queue.is_empty());
    assert_eq!(m.drain(), 0);
    // Removed in the same receive: the queue goes with the unit.
    m.recv(&[hex(STOP_1_6), hex("0a 01 06 00 00 00")].concat());
    assert!(m.w.units.is_empty());
    assert_eq!(m.drain(), 0);
    // A message for a key that is not in the set addresses nothing,
    // even if a unit with that GUID has another type.
    let mut m = with_monsters();
    m.hex(AC_1_6);
    m.hex("0e 02 06 00 00 00 03 00 02 00 00 00");
    assert_eq!(m.log.dropped.get(&0x0E), Some(&1));
}

// Covers: specs/client/model.md §2 r4, §2 r3, §2 r8
#[test]
fn adding_an_existing_key_replaces_the_unit() {
    let mut m = with_monsters();
    m.recv(&[hex(AC_1_6), hex(STOP_1_6)].concat());
    assert_eq!(m.unit(M6).queue.len(), 1);
    // The second add (another class) replaces the first: its queue is
    // dropped.
    m.hex("ac 06 00 00 00 05 00 1a 12 b9 11 80 0e 01");
    let u = m.unit(M6);
    assert_eq!((u.class, u.queue.len()), (5, 0));
    assert_eq!(m.w.units.len(), 1);
}

// Covers: specs/client/model.md §5 r3, §5 r4, §2 r1
#[test]
fn drain_order() {
    let mut w = ClientWorld::default();
    for k in [
        UnitKey::new(MONSTER, 5),
        UnitKey::new(MONSTER, 0x85),
        UnitKey::new(MONSTER, 0x105),
        UnitKey::new(MONSTER, 6),
        UnitKey::new(OBJECT, 1),
        UnitKey::new(PLAYER, 9),
        UnitKey::new(MISSILE, 0x80),
        UnitKey::new(MISSILE, 0),
        UnitKey::new(5, 1),
    ] {
        w.units.insert(k, ClientUnit::new(k));
    }
    let guids: Vec<(u8, u32)> = update_order(&w.units)
        .into_iter()
        .map(|k| (k.unit_type, k.guid))
        .collect();
    assert_eq!(
        guids,
        [
            (MISSILE, 0x80),
            (MISSILE, 0),
            (PLAYER, 9),
            (MONSTER, 0x105),
            (MONSTER, 0x85),
            (MONSTER, 5),
            (MONSTER, 6),
            (OBJECT, 1),
        ]
    );
}

// Covers: specs/client/model.md §4 r2, §4 r3, §4 r4
#[test]
fn pre_steps_and_one_byte_unit_ids_change_nothing() {
    let mut m = Model::default();
    let before = m.w.clone();
    // 0x6E–0x72: no addressed unit, no queue entry, no effect.
    m.recv(&hex("6e 6f 70 71 72"));
    assert_eq!(m.w, before);
    assert_eq!(m.log.dropped.values().sum::<u64>(), 5);
    // The largest unit-handler message (0x68, 21 bytes) fits a 26-byte
    // slot; d2rs queues whole messages.
    assert_eq!(
        d2_proto::transport::server_message(0x68).unwrap().size,
        d2_proto::schema::SizeRule::Fixed(21)
    );
}

fn monster_at(w: &mut ClientWorld, mode: u32, x: u16, y: u16) -> UnitKey {
    let k = UnitKey::new(MONSTER, 3);
    let mut u = ClientUnit::new(k);
    u.mode = mode;
    u.position = Some((x, y));
    w.units.insert(k, u);
    k
}

fn hidden(_: &ClientUnit, _: i32, _: i32) -> bool {
    false
}

fn shown(_: &ClientUnit, _: i32, _: i32) -> bool {
    true
}

// Covers: specs/client/model.md §6 r1, §6 r2, §6 r3, §6 r4, §6 r5, §6 r6, §6 r7, §6 r8, §6 text
#[test]
fn position_check_vectors() {
    let none = ModelInputs::default();
    let shown = ModelInputs {
        visible: Some(shown),
        ..ModelInputs::default()
    };
    let hidden = ModelInputs {
        visible: Some(hidden),
        ..ModelInputs::default()
    };

    // The local player at (100, 100), mode 1: 4 > 3 → 0x5F with its own
    // position, not moved.
    let mut w = ClientWorld::default();
    let mut u = ClientUnit::new(P1);
    u.mode = 1;
    u.position = Some((100, 100));
    w.units.insert(P1, u);
    w.local_player = Some(P1);
    assert_eq!(
        check(&mut w, &none, P1, 104, 100, 0, 0, 0).unwrap(),
        Checked::Asked
    );
    assert_eq!(w.outgoing, vec![hex("5f 64 00 64 00")]);
    assert_eq!(w.units[&P1].position, Some((100, 100)));
    assert_eq!(w.units[&P1].server_point, (104, 100));

    // A monster, mode 1 (T 15): 10 ≤ 15 and y = cy (visible): nothing.
    let mut w = ClientWorld::default();
    let k = monster_at(&mut w, 1, 100, 100);
    assert_eq!(
        check(&mut w, &none, k, 110, 100, 0, 0, 0).unwrap(),
        Checked::Kept
    );
    assert_eq!(w.units[&k].server_point, (110, 100));
    assert_eq!(w.units[&k].position, Some((100, 100)));

    // Within tolerance, both points not visible: teleport.
    assert_eq!(
        check(&mut w, &hidden, k, 110, 105, 0, 0, 0).unwrap(),
        Checked::Moved
    );
    assert_eq!(w.units[&k].position, Some((110, 105)));
    // Visible: kept.
    let k = monster_at(&mut w, 1, 100, 100);
    assert_eq!(
        check(&mut w, &shown, k, 110, 105, 0, 0, 0).unwrap(),
        Checked::Kept
    );
    // No predicate yet: a loud error, not a guess.
    assert!(check(&mut w, &none, k, 110, 105, 0, 0, 0).is_err());

    // Mode 4 (T 5): 6 > 5 → teleport.
    let k = monster_at(&mut w, 4, 100, 100);
    assert_eq!(
        check(&mut w, &none, k, 106, 100, 0, 0, 0).unwrap(),
        Checked::Moved
    );
    assert_eq!(w.units[&k].position, Some((106, 100)));
    // Mode 7 (T 7): 7 is within.
    let k = monster_at(&mut w, 7, 100, 100);
    assert_eq!(
        check(&mut w, &none, k, 107, 100, 0, 0, 0).unwrap(),
        Checked::Kept
    );

    // Kind 0, tx > 0: d1 < 100, d2 < d1 → accepted, then the
    // visibility rule (visible: kept).
    let k = monster_at(&mut w, 4, 100, 100);
    // (x, y) = (106, 104): 6 > 5, d1 = 36 + 16 = 52; target (103, 103):
    // d2 = 9 + 9 = 18.
    assert_eq!(
        check(&mut w, &shown, k, 106, 104, 0, 103, 103).unwrap(),
        Checked::Kept
    );
    assert_eq!(w.units[&k].position, Some((100, 100)));
    // Accepted, then not visible: corrected.
    assert_eq!(
        check(&mut w, &hidden, k, 106, 104, 0, 103, 103).unwrap(),
        Checked::Moved
    );
    // d2 ≥ d1: corrected.
    let k = monster_at(&mut w, 4, 100, 100);
    assert_eq!(
        check(&mut w, &shown, k, 106, 104, 0, 110, 110).unwrap(),
        Checked::Moved
    );
    // d1 ≥ 100: corrected.
    let k = monster_at(&mut w, 4, 100, 100);
    assert_eq!(
        check(&mut w, &shown, k, 110, 100, 0, 101, 100).unwrap(),
        Checked::Moved
    );
    // Kind 1 (T 10) and kind 2 (T 0).
    let k = monster_at(&mut w, 4, 100, 100);
    assert_eq!(
        check(&mut w, &none, k, 110, 100, 1, 0, 0).unwrap(),
        Checked::Kept
    );
    assert_eq!(
        check(&mut w, &none, k, 101, 100, 2, 0, 0).unwrap(),
        Checked::Moved
    );

    // x = 0: nothing; a dead monster (mode 0xC): nothing.
    let k = monster_at(&mut w, 1, 100, 100);
    assert_eq!(
        check(&mut w, &none, k, 0, 150, 0, 0, 0).unwrap(),
        Checked::Skipped
    );
    assert_eq!(w.units[&k].server_point, (0, 0));
    let k = monster_at(&mut w, 0xC, 100, 100);
    assert_eq!(
        check(&mut w, &none, k, 150, 150, 0, 0, 0).unwrap(),
        Checked::Skipped
    );
    assert_eq!(w.units[&k].position, Some((100, 100)));
}

// Covers: specs/client/model.md §2 r6, §1 r2
#[test]
fn creation_common_fields() {
    let mut m = with_monsters();
    // A player at (0, 0): not placed, seed {1, 666} stepped once.
    m.recv(&assign_player(0, 0));
    let u = m.unit(P1);
    assert_eq!((u.position, u.seed), (None, Some((0x6AC6_935F, 0))));
    // At another point the seed comes from the client room (not in the
    // model yet): unknown, not guessed.
    m.recv(&assign_player(0x1241, 0x11C4));
    assert_eq!(m.unit(P1).seed, None);
    assert_eq!(m.unit(P1).position, Some((0x1241, 0x11C4)));
    // A monster at (0, 0): {0, 666}.
    m.hex("ac 07 00 00 00 9a 00 00 00 00 00 80 0e 01");
    assert_eq!(m.unit(UnitKey::new(MONSTER, 7)).seed, Some((0, 666)));
}
