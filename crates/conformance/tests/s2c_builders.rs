// Spec: specs/sim/intents-events.md §3; specs/world/npc.md §7.3, §8.1, §9; specs/world/waypoints.md §5.3, §7 r7; specs/world/cube.md §1; specs/sim/pathing.md §10
//! One maker per S→C id (HANDOFF §2 step 7k, J13): `d2-sim`'s byte
//! builders produce exactly the bytes of the `d2_proto::s2c` types, and
//! the client parser reads them back as the same values. `d2-sim` may not
//! depend on `d2-proto` (depcheck), so the two makers are tied here.
//!
//! Inputs: every combination of the edge values of each field, then a
//! fixed pseudo-random sweep.

use d2_proto::s2c::{
    parse, Message, NpcTransaction, OpenUi, PlayerStop, TradeAction, Unknown9B, WaypointMenu,
};
use d2_sim::path::walk::messages::player_stop;
use d2_sim::world::cube::trade_action;
use d2_sim::world::npc::{resurrect_message, service_result, transaction};
use d2_sim::world::waypoints::{arrival_message, menu_message};

const U8S: [u8; 5] = [0, 1, 0x7F, 0x80, 0xFF];
const U16S: [u16; 5] = [0, 1, 0x7FFF, 0x8000, 0xFFFF];
const U32S: [u32; 6] = [0, 1, 0x1122_3344, 0x7FFF_FFFF, 0x8000_0000, u32::MAX];

/// Fixed xorshift sweep (test input only).
fn sweep(n: usize) -> impl Iterator<Item = u32> {
    let mut s = 0x2545_F491_u32;
    (0..n).map(move |_| {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        s
    })
}

fn same(sim: &[u8], proto: &[u8], parsed: Message, want: Message) {
    assert_eq!(sim, proto, "sim bytes vs d2_proto::s2c encode");
    assert_eq!(parsed, want, "parse of the sim bytes");
}

#[test]
fn npc_transaction_0x2a_one_maker() {
    let check = |kind: u8, code: u8, guid: u32, gold: u32| {
        let sim = transaction(kind, code, guid, gold);
        let m = NpcTransaction {
            kind,
            code,
            guid,
            gold,
        };
        same(
            &sim,
            &m.encode(),
            parse(&sim).unwrap(),
            Message::NpcTransaction(m),
        );
    };
    for &kind in &U8S {
        for &code in &U8S {
            for &guid in &U32S {
                for &gold in &U32S {
                    check(kind, code, guid, gold);
                }
            }
        }
    }
    let r: Vec<u32> = sweep(4000).collect();
    for c in r.chunks(4) {
        check(c[0] as u8, c[1] as u8, c[2], c[3]);
    }
}

#[test]
fn service_result_0x58_one_maker() {
    let check = |npc_guid: u32, result: u8| {
        let sim = service_result(npc_guid, result);
        let m = OpenUi {
            npc_guid,
            result,
            effect: 0,
        };
        same(&sim, &m.encode(), parse(&sim).unwrap(), Message::OpenUi(m));
    };
    for &g in &U32S {
        for &r in &U8S {
            check(g, r);
        }
    }
    let r: Vec<u32> = sweep(2000).collect();
    for c in r.chunks(2) {
        check(c[0], c[1] as u8);
    }
}

#[test]
fn resurrect_0x9b_one_maker() {
    let sim = resurrect_message();
    let m = Unknown9B { f1: 0xFFFF, f3: 0 };
    same(
        &sim,
        &m.encode(),
        parse(&sim).unwrap(),
        Message::Unknown9B(m),
    );
}

#[test]
fn waypoint_menu_0x63_one_maker() {
    let check = |guid: u32, record: [u8; 16]| {
        let sim = menu_message(guid, &record);
        let m = WaypointMenu {
            object_guid: guid,
            record,
        };
        same(
            &sim,
            &m.encode(),
            parse(&sim).unwrap(),
            Message::WaypointMenu(m),
        );
    };
    for &g in &U32S {
        for &b in &U8S {
            check(g, [b; 16]);
        }
    }
    let r: Vec<u32> = sweep(5 * 500).collect();
    for c in r.chunks(5) {
        let mut rec = [0u8; 16];
        for (i, w) in c[1..].iter().enumerate() {
            rec[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
        check(c[0], rec);
    }
}

#[test]
fn waypoint_arrival_0x0d_one_maker() {
    // §7 rule 7: unit type 0, GUID, 1, x + 3, y + 3 (as u16), 0, 0.
    let check = |guid: u32, x: i32, y: i32| {
        let sim = arrival_message(guid, x, y);
        let m = PlayerStop {
            unit_type: 0,
            unit_guid: guid,
            f6: 1,
            x: x.wrapping_add(3) as u16,
            y: y.wrapping_add(3) as u16,
            f11: 0,
            f12: 0,
        };
        same(
            &sim,
            &m.encode(),
            parse(&sim).unwrap(),
            Message::PlayerStop(m),
        );
    };
    let coords = [-4, -3, 0, 0x1320, 0xFFFC, 0xFFFD, 0x1_0000, i32::MAX - 3];
    for &g in &U32S {
        for &x in &coords {
            for &y in &coords {
                check(g, x, y);
            }
        }
    }
    let r: Vec<u32> = sweep(3000).collect();
    for c in r.chunks(3) {
        // Keep x + 3 inside i32, as the caller's coordinates are.
        check(c[0], (c[1] >> 1) as i32 - 3, (c[2] >> 1) as i32 - 3);
    }
}

#[test]
fn player_stop_0x0d_one_maker() {
    let check = |ty: u8, guid: u32, a: u8, x: u16, y: u16, b: u8, life: u8| {
        let sim = player_stop(ty, guid, a, x, y, b, life);
        let m = PlayerStop {
            unit_type: ty,
            unit_guid: guid,
            f6: a,
            x,
            y,
            f11: b,
            f12: life,
        };
        same(
            &sim,
            &m.encode(),
            parse(&sim).unwrap(),
            Message::PlayerStop(m),
        );
    };
    for &v in &U8S {
        for &g in &U32S {
            for &c in &U16S {
                check(v, g, v, c, c, v, v);
            }
        }
    }
    let r: Vec<u32> = sweep(3000).collect();
    for c in r.chunks(3) {
        let [a, b, l, t] = c[2].to_le_bytes();
        check(t, c[0], a, c[1] as u16, (c[1] >> 16) as u16, b, l);
    }
}

#[test]
fn cube_trade_action_0x77_one_maker() {
    for action in 0..=u8::MAX {
        let sim = trade_action(action);
        let m = TradeAction { action };
        same(
            &sim,
            &m.encode(),
            parse(&sim).unwrap(),
            Message::TradeAction(m),
        );
    }
}
