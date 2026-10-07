// Spec: specs/client/msg-units.md
//! Test vectors of `client/msg-units.md`: "B" =
//! `traces/raw/20261006-022633-packets.jsonl`, "A" = `-015956-…`, seq
//! numbers in the comments; synthetic where marked.

use super::super::world::{
    ClientUnit, KindData, MonsterClass, ObjectData, StatSend, UnitKey, MISSILE, MONSTER, OBJECT,
    PLAYER,
};
use super::support::{hex, Model};
use super::units::queued;

const P1: UnitKey = UnitKey::new(PLAYER, 1);

fn with_monsters() -> Model {
    let mut m = Model::default();
    m.inputs.tables.monsters = vec![Some(MonsterClass::default()); 155];
    m
}

/// The local player (0, 1) at `pos`, mode `mode`.
fn local(m: &mut Model, pos: (u16, u16), mode: u32) {
    let u = m.put(P1);
    u.position = Some(pos);
    u.mode = mode;
    m.w.local_player = Some(P1);
}

fn request(m: &Model, k: UnitKey) -> Option<(u8, [i32; 7])> {
    m.unit(k).last_mode_request.map(|r| (r.code, r.record))
}

// Covers: specs/client/msg-units.md §1.1 r1, §1.1 r2, §1.1 r3, §1.1 r4, §1.1 r5
#[test]
fn assign_player_b102() {
    let mut m = Model::default();
    let mut b = hex("59 01 00 00 00 01 77 65 72 77 65 72 00");
    b.resize(26, 0);
    m.recv(&b);
    let u = m.unit(P1);
    assert_eq!((u.class, u.position, u.mode), (1, None, 5));
    assert_eq!(
        u.stats.iter().map(|(&k, &v)| (k, v)).collect::<Vec<_>>(),
        [(67, 100), (68, 100), (69, 100)]
    );
    assert_eq!(u.seed, Some((0x6AC6_935F, 0)));
    let KindData::Player(p) = &u.kind else {
        panic!("player data");
    };
    assert_eq!(&p.name[..7], b"werwer\0");
}

// Covers: specs/client/msg-units.md §1.2 r1, §1.2 r2, §1.2 r3, §1.2 r5
#[test]
fn assign_monster_b157() {
    let mut m = with_monsters();
    m.hex("ac 06 00 00 00 9a 00 1a 12 b9 11 80 0e 01");
    let u = m.unit(UnitKey::new(MONSTER, 6));
    assert_eq!(
        (u.class, u.position, u.mode),
        (154, Some((0x121A, 0x11B9)), 1)
    );
    assert_eq!(
        u.stats.iter().map(|(&k, &v)| (k, v)).collect::<Vec<_>>(),
        [(6, 0x8000), (7, 0x8000), (328, 0x23D3)]
    );
    let KindData::Monster(d) = &u.kind else {
        panic!("monster data");
    };
    assert_eq!(
        (d.flags, d.components, d.value, d.v31, &d.stat_list),
        (0, [0; 16], -1, None, &None)
    );
    // No `monstats` / `monstats2` row: nothing is created.
    let mut m = Model::default();
    m.hex("ac 06 00 00 00 9a 00 1a 12 b9 11 80 0e 01");
    assert!(m.w.units.is_empty());
    assert!(m.log.rejected.is_empty());
}

/// Packs `(value, bits)` fields low bits first (`model.md` §10).
fn pack(fields: &[(u32, u32)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    for &(v, n) in fields {
        for i in 0..n {
            if pos / 8 == out.len() {
                out.push(0);
            }
            out[pos / 8] |= (((v >> i) & 1) as u8) << (pos % 8);
            pos += 1;
        }
    }
    out
}

// Covers: specs/client/msg-units.md §1.2 r1, §1.2 r4
#[test]
fn assign_monster_bit_stream_synthetic() {
    let mut m = Model::default();
    let mut class = MonsterClass::default();
    class.components[0] = 2; // 1 bit
    class.components[1] = 5; // bit length of 4 = 3 bits
    class.components[2] = 1; // 1 bit
    m.inputs.tables.monsters = vec![Some(class)];
    m.inputs.tables.stats = vec![StatSend::default(); 3];
    // Stat 2: 6 bits, signed, 2 param bits.
    m.inputs.tables.stats[2] = StatSend {
        bits: 6,
        param_bits: 2,
        signed: true,
    };
    let mut comps = vec![(1, 1), (4, 3), (1, 1)];
    comps.extend(std::iter::repeat_n((0, 1), 13));
    let mut fields = vec![(3, 4), (1, 1)];
    fields.extend(comps);
    fields.extend([
        (1, 1),       // has type flags
        (0, 1),       // champion
        (1, 1),       // unique → 8
        (1, 1),       // superunique → 2
        (0, 1),       // minion
        (1, 1),       // ghostly → 0x40
        (0xFFFE, 16), // hcIdx
        (0x21, 8),
        (0x07, 8),
        (0, 8),       // umods end
        (0x1234, 16), // name seed
        (1, 1),       // has value
        (0xDEAD_BEEF, 32),
        (1, 1), // 31-bit value follows
        (0x5555, 31),
        (1, 1), // stat list follows
        (2, 9),
        (3, 2),     // param
        (0x3E, 6),  // value −2
        (0x1FF, 9), // end
    ]);
    let stream = pack(&fields);
    let mut b = hex("ac 09 00 00 00 00 00 0a 00 0b 00 10");
    b.push((0xD + stream.len()) as u8);
    b.extend(stream);
    m.recv(&b);
    assert!(m.log.rejected.is_empty(), "{:?}", m.rejected());
    let u = m.unit(UnitKey::new(MONSTER, 9));
    assert_eq!(u.mode, 3);
    assert_eq!(u.stat(6), 0x1000);
    assert_eq!(u.stat(328), 21);
    let KindData::Monster(d) = &u.kind else {
        panic!("monster data");
    };
    assert_eq!(&d.components[..4], &[1, 4, 1, 0]);
    assert_eq!(d.flags, 8 | 2 | 0x40);
    assert_eq!(d.hc_idx, 0xFFFE);
    assert_eq!(d.umods, [0x21, 7, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(d.name_seed, 0x1234);
    assert_eq!(d.value, 0xDEAD_BEEF_u32 as i32);
    assert_eq!(d.v31, Some(0x5555));
    assert_eq!(
        d.stat_list.clone().unwrap().into_iter().collect::<Vec<_>>(),
        [((2, 3), -2)]
    );
}

// Covers: specs/client/msg-units.md §1.3 r1, §1.3 r2, §1.3 r3, §1.3 r4
#[test]
fn assign_object_b160() {
    let mut m = Model::default();
    m.hex("51 02 0d 00 00 00 25 00 14 12 c0 11 02 00");
    let u = m.unit(UnitKey::new(OBJECT, 13));
    assert_eq!(
        (u.class, u.position, u.mode, &u.kind),
        (
            37,
            Some((0x1214, 0x11C0)),
            2,
            &KindData::Object(ObjectData {
                interact: 0,
                ..ObjectData::default()
            })
        )
    );
    // Type 1: fatal 0x202.
    m.hex("51 01 0d 00 00 00 25 00 14 12 c0 11 02 00");
    assert_eq!(m.rejected(), [(0x51, "fatal assert 0x202".to_owned())]);
}

// Covers: specs/client/msg-units.md §2 r1, §2 r2
#[test]
fn remove_unit_a7228_a215606() {
    let mut m = Model::default();
    m.put(UnitKey::new(OBJECT, 12));
    m.put(UnitKey::new(MONSTER, 0x19));
    m.hex("0a 02 0c 00 00 00").hex("0a 01 19 00 00 00");
    assert!(m.w.units.is_empty());
    // A key not in the set: nothing.
    m.hex("0a 01 19 00 00 00");
    assert!(m.log.rejected.is_empty());
}

// Covers: specs/client/msg-units.md §3 r1, §3 r2, §3 r4, §3 r5
#[test]
fn reassign_player_b154() {
    let mut m = Model::default();
    m.put(P1).mode = 5;
    m.hex("15 00 01 00 00 00 41 12 c4 11 01");
    assert_eq!(m.unit(P1).position, Some((0x1241, 0x11C4)));
    // Not in the set: nothing.
    m.hex("15 00 02 00 00 00 41 12 c4 11 01");
    assert_eq!(m.w.units.len(), 1);
    // A dead monster stays.
    let k = UnitKey::new(MONSTER, 4);
    let u = m.put(k);
    u.mode = 0xC;
    u.position = Some((5, 5));
    m.hex("15 01 04 00 00 00 41 12 c4 11 00");
    assert_eq!(m.unit(k).position, Some((5, 5)));
    assert!(m.log.rejected.is_empty());
}

/// Receives `msg` for a unit already in the model, drains, returns the
/// request.
fn drained(m: &mut Model, msg: &str) -> Option<(u8, [i32; 7])> {
    let b = hex(msg);
    m.recv(&b);
    assert_eq!(m.drain(), 1, "{msg}");
    let k = super::super::world::addressed_unit(&b).unwrap();
    request(m, k)
}

// Covers: specs/client/msg-units.md §4 r1, §4 r2, §4 r3, §4 r4
#[test]
fn queued_messages_recorded() {
    let mut m = Model::default();
    let k13 = UnitKey::new(MONSTER, 0x13);
    let u = m.put(k13);
    u.mode = 1;
    u.position = Some((0x1294, 0x1555));
    // A 213893.
    assert_eq!(
        drained(&mut m, "67 13 00 00 00 01 95 12 55 15 01 00 0d 4b 00 05"),
        Some((1, [0x1295, 0x1555, 1, 0x0D, 0x004B, 5, 0]))
    );
    // A 218973.
    assert_eq!(
        drained(&mut m, "69 13 00 00 00 06 94 12 55 15 2f 06"),
        Some((6, [0x1294, 0x1555, 0x2F, 2, 0, 4, 6]))
    );
    // A 221956: check at the unit's own point (kept), then the request.
    assert_eq!(
        drained(&mut m, "6c 13 00 00 00 10 00 01 00 00 00 00 94 12 55 15"),
        Some((0x10, [0, 1, 0, 2, 0, 4, 0]))
    );
    assert_eq!(m.unit(k13).server_point, (0x1294, 0x1555));
    // A 220507: 0x0C addresses (type, GUID).
    assert_eq!(
        drained(&mut m, "0c 01 13 00 00 00 13 06 0f"),
        Some((0x13, [6, 0x0F, 0, 0, 0, 0, 0]))
    );
    // B 161: an object.
    let k = UnitKey::new(OBJECT, 13);
    m.put(k);
    assert_eq!(
        drained(&mut m, "0e 02 0d 00 00 00 03 00 02 00 00 00"),
        Some((3, [0, 2, 0, 0, 0, 0, 0]))
    );
    // B 25465: the local player; the roster is not in the model.
    local(&mut m, (0x12E4, 0x138E), 1);
    assert_eq!(
        drained(&mut m, "0d 00 01 00 00 00 13 e4 12 8e 13 11 57"),
        Some((0x13, [0x12E4, 0x138E, 0x11, 0, 0, 0, 0]))
    );
    // 0x6D adds 1 to stat 328 between the check and the request.
    m.unit_mut_stat(k13, 328, 9);
    drained(&mut m, "6d 13 00 00 00 94 12 55 15 80");
    assert_eq!(m.unit(k13).stat(328), 10);
    assert!(m.log.rejected.is_empty(), "{:?}", m.rejected());
}

// Covers: specs/client/msg-units.md §4 r1
#[test]
fn every_queued_row_reads_its_layout() {
    // Synthetic: each id, bytes 0..n, the unit (1, 0x04030201) /
    // (1 at +1, 0x05040302 at +2) without a position check reaching a
    // correction (check points 0 or equal).
    let cases: &[(&str, u8, [i32; 7])] = &[
        (
            "4c 01 02 03 04 05 06 07 08 09 0a 0b 0c 0d 0e 0f",
            0x16,
            [0x0706, -1, 9, 0x0D0C_0B0A, 8, 0, 0],
        ),
        (
            "4d 01 02 03 04 05 06 07 08 09 0a 0b 0c 0d 0e 0f 10",
            0x15,
            [0x0908_0706, -1, 0x0C0B, 0x0E0D, 0x0A, 0, 0],
        ),
        (
            "68 01 02 03 04 05 00 00 00 00 0a 0b 0c 0d 0e 0f 10 11 12 ff 14",
            5,
            [0x0A, 0x0E0D_0C0B, 0x0F, 0x11, -238, 0x14, 0x10],
        ),
        (
            "6a 01 02 03 04 05 06 07 08 09 0a 0b",
            5,
            [6, 0x0A09_0807, 0x0B, 2, 0, 4, 0],
        ),
        (
            "6b 01 02 03 04 05 06 07 08 09 0a 0b 00 00 00 00",
            5,
            [0x0706, 0x0908, 0x0A, 2, 0, 4, 0x0B],
        ),
        (
            "0f 01 02 03 04 05 06 07 08 09 0a 0b 00 00 00 00",
            6,
            [0x0807, 0x0A09, 0x0B, 0, 0, 0, 0],
        ),
        (
            "10 01 02 03 04 05 06 07 08 09 0a 0b 00 00 00 00",
            6,
            [7, 0x0B0A_0908, 0, 0, 0, 0, 0],
        ),
    ];
    for (msg, code, record) in cases {
        let mut m = Model::default();
        let b = hex(msg);
        let k = super::super::world::addressed_unit(&b).unwrap();
        m.put(k).mode = 1;
        m.recv(&b);
        assert_eq!(m.drain(), 1, "{msg}");
        assert_eq!(request(&m, k), Some((*code, *record)), "{msg}");
    }
    // A missile's request does nothing (`model.md` §8 rule 1).
    let mut m = Model::default();
    let k = UnitKey::new(MISSILE, 2);
    m.put(k);
    let b = hex("0e 03 02 00 00 00 03 00 02 00 00 00");
    let msg = super::super::dispatch::UnitMessage {
        id: 0x0E,
        bytes: &b,
        unit: k,
        inputs: &m.inputs,
        out: &Default::default(),
    };
    let mut w = m.w.clone();
    queued(&mut w, &msg).unwrap();
    assert_eq!(w.units[&k], ClientUnit::new(k));
}

// Covers: specs/client/msg-units.md §5 r1, §5 r2, §5 r3
#[test]
fn vitals_b11145_b15933_b140_b32694_b12323() {
    let mut m = Model::default();
    // No local player: nothing.
    m.hex("96 4a 80 8e 89 c0 09 00 00");
    assert!(m.w.units.is_empty() && m.log.rejected.is_empty());
    // B 11145: at the stated point, no correction.
    local(&mut m, (0x131D, 0x1381), 2);
    m.hex("96 4a 80 8e 89 c0 09 00 00");
    let u = m.unit(P1);
    assert_eq!((u.stat(10), u.server_point), (0x4A00, (0x131D, 0x1381)));
    assert!(m.w.outgoing.is_empty());
    // B 15933: (0x1318, 0x1387), target (0x131B, 0x1383): dy 0xFC → −4.
    local(&mut m, (0x1318, 0x1387), 2);
    m.hex("96 48 00 8c 89 c3 89 01 7e");
    let u = m.unit(P1);
    assert_eq!((u.stat(10), u.server_point), (0x4800, (0x1318, 0x1387)));
    // B 140: x = 0, no check.
    local(&mut m, (0x10, 0x10), 2);
    m.hex("95 24 00 0c 80 12 00 00 00 00 00 00 00");
    let u = m.unit(P1);
    assert_eq!(
        (u.stat(6), u.stat(8), u.stat(10), u.server_point),
        (0x2400, 0x1800, 0x4A00, (0, 0))
    );
    // B 32694.
    local(&mut m, (0x12DD, 0x1391), 2);
    m.hex("95 1c 80 0a 80 12 a0 5b 22 72 c2 bf 00");
    let u = m.unit(P1);
    assert_eq!(
        (u.stat(6), u.stat(8), u.stat(10), u.server_point),
        (0x1C00, 0x1500, 0x4A00, (0x12DD, 0x1391))
    );
    // B 12323.
    local(&mut m, (0x131D, 0x1381), 2);
    m.hex("18 24 80 0c 80 12 80 0c e8 98 08 9c 00 00 00");
    let u = m.unit(P1);
    assert_eq!(
        (u.stat(6), u.stat(8), u.stat(10), u.stat(74), u.stat(26)),
        (0x2400, 0x1900, 0x4A00, 0x64, 0)
    );
    assert_eq!(u.server_point, (0x131D, 0x1381));
    assert!(m.log.rejected.is_empty() && m.w.outgoing.is_empty());
}

// Covers: specs/client/msg-units.md §5 r3, §edge-cases-original-bugs
#[test]
fn vitals_correction_and_dx_0x80() {
    // Synthetic: the local player in mode 1 (T 3) at (100, 100); 0x96
    // states (110, 100) with dx 0x80: the target is x + 128 (d2 ≥ d1),
    // so the player asks with its own position.
    let mut m = Model::default();
    local(&mut m, (100, 100), 1);
    let mut b = vec![0x96];
    b.extend(pack(&[(0x10, 15), (110, 16), (100, 16), (0x80, 8), (0, 8)]));
    m.recv(&b);
    assert_eq!(m.w.outgoing, vec![hex("5f 64 00 64 00")]);
    // dx 0xFF → −1.
    let mut m = Model::default();
    local(&mut m, (104, 100), 1);
    let mut b = vec![0x96];
    b.extend(pack(&[(0x10, 15), (108, 16), (100, 16), (0xFF, 8), (0, 8)]));
    // (108, 100) from (104, 100): 4 > 3, d1 = 16, target (107, 100):
    // d2 = 9 < 16 → accepted; y = cy → visible: no correction.
    m.recv(&b);
    assert!(m.w.outgoing.is_empty());
    assert_eq!(m.unit(P1).position, Some((104, 100)));
    // A dead local player: stamina set, no check.
    let mut m = Model::default();
    local(&mut m, (100, 100), 0x11);
    let mut b = vec![0x96];
    b.extend(pack(&[(0x10, 15), (150, 16), (100, 16), (0, 8), (0, 8)]));
    m.recv(&b);
    assert_eq!(m.unit(P1).stat(10), 0x1000);
    assert!(m.w.outgoing.is_empty());
}

impl Model {
    fn unit_mut_stat(&mut self, k: UnitKey, stat: u16, v: i32) {
        self.w.units.get_mut(&k).unwrap().stats.insert(stat, v);
    }
}
