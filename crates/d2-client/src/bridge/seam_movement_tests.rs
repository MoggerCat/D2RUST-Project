// Spec: specs/seams/movement-prediction.md
//! Contract tests of the movement seam: each test calls the server's
//! builder or computation and the client's reader or prediction on the
//! same value (`specs/seams/movement-prediction.md` §2). Synthetic values,
//! no game files.

use d2_server::adapters::handlers::walk::{form, Form, WALK_IDS};
use d2_sim::path::coords::{subtile_of, to_fp16_center};
use d2_sim::path::tables::PathTables;
use d2_sim::path::walk::geom::direction_vector;
use d2_sim::path::walk::messages::{player_move, reassign_player};
use d2_sim::path::walk::request::message_request;
use d2_sim::path::walk::resync::{resync_distance, MESSAGE_LEN};
use d2_sim::path::walk::step::STEP_BASE;
use d2_sim::path::walk::velocity::{mode_velocity, run_velocity_bonus, STAT_VELOCITYPERCENT};
use d2_sim::path::walk::{Point, WalkUnits};
use d2_sim::rng::Seed;
use d2_sim::units::{UnitId, UnitType};

use super::msg::support::Model;
use super::predict::{cell_centre, facing, walk_of, Predict, Speeds, Walk, WalkTo};
use super::world::{UnitKey, PLAYER};

const P1: UnitKey = UnitKey::new(PLAYER, 1);

/// One player unit for the server's velocity computation.
struct Mover {
    ty: UnitType,
    mode: u32,
    percent: i32,
    charstats: (i32, i32, i32),
    seed: Seed,
}

impl WalkUnits for Mover {
    fn unit_type(&self, _: UnitId) -> UnitType {
        self.ty
    }
    fn frame(&self) -> i32 {
        0
    }
    fn mode(&self, _: UnitId) -> u32 {
        self.mode
    }
    fn stat(&self, _: UnitId, stat: u16) -> i32 {
        if stat == STAT_VELOCITYPERCENT {
            self.percent
        } else {
            0
        }
    }
    fn seed(&mut self, _: UnitId) -> &mut Seed {
        &mut self.seed
    }
    fn charstats_velocity(&self, _: UnitId) -> (i32, i32, i32) {
        self.charstats
    }
}

fn tables() -> PathTables {
    PathTables::spec().expect("spec path tables parse")
}

/// The local player (0, 1) at `pos`, mode `mode`.
fn local(m: &mut Model, pos: (u16, u16), mode: u32) {
    let u = m.put(P1);
    u.position = Some(pos);
    u.mode = mode;
    m.w.local_player = Some(P1);
}

// Covers: specs/seams/movement-prediction.md §1 r1
#[test]
fn cell_centre_and_cell_agree_with_the_server() {
    for c in [0u16, 1, 99, 0x1234, 25_000, u16::MAX] {
        let (x, y) = cell_centre((c, c));
        assert_eq!(x, to_fp16_center(i32::from(c)));
        assert_eq!(y, to_fp16_center(i32::from(c)));
        assert_eq!(subtile_of(x), i32::from(c));
    }
    // The prediction's cell of a precise position is the server's cell.
    let mut p = Predict::new();
    let mut m = Model::default();
    local(&mut m, (4825, 5636), 1);
    p.observe(&m.w);
    let (px, py) = p.position().unwrap();
    assert_eq!(
        p.cell(),
        Some((subtile_of(px) as u16, subtile_of(py) as u16))
    );
    assert_eq!(p.cell(), Some((4825, 5636)));
}

// Covers: specs/seams/movement-prediction.md §2.1 r1
#[test]
fn walk_intents_read_the_same_on_both_sides() {
    // Point forms.
    for (id, run) in [(0x01u8, false), (0x03, true)] {
        let msg = [id, 100, 0, 200, 0];
        assert_eq!(
            walk_of(&msg),
            Some(Walk {
                to: WalkTo::Point(100, 200),
                run
            })
        );
        assert_eq!(form(id), Some(Form::Point));
        let (mode, unit_form) = message_request(id).unwrap();
        assert!(!unit_form);
        assert_eq!(mode, if run { 3 } else { 2 });
    }
    // Unit forms: type u32 @1, GUID u32 @5.
    for (id, run) in [(0x02u8, false), (0x04, true)] {
        let mut msg = vec![id];
        msg.extend_from_slice(&1u32.to_le_bytes());
        msg.extend_from_slice(&0x0102_0304u32.to_le_bytes());
        assert_eq!(
            walk_of(&msg),
            Some(Walk {
                to: WalkTo::Unit(UnitKey::new(1, 0x0102_0304)),
                run
            })
        );
        assert_eq!(form(id), Some(Form::Unit));
        let (mode, unit_form) = message_request(id).unwrap();
        assert!(unit_form);
        assert_eq!(mode, if run { 3 } else { 2 });
    }
    // The server's table names the same modes.
    for &(id, _, _, f, mode) in WALK_IDS {
        assert_eq!(message_request(id), Some((mode, f == Form::Unit)));
    }
}

// Covers: specs/seams/movement-prediction.md §2.2 r1, §2.2 r2
#[test]
fn the_resync_point_is_the_clients_own_cell() {
    // No prediction: the check asks with the model's cell.
    let mut m = Model::default();
    local(&mut m, (100, 100), 1);
    let mut b = vec![0x96];
    b.extend(pack(&[(0x10, 15), (150, 16), (100, 16), (0, 8), (0, 8)]));
    m.recv(&b);
    assert_eq!(m.w.outgoing.len(), 1);
    let msg = &m.w.outgoing[0];
    assert_eq!(msg.len(), MESSAGE_LEN);
    assert_eq!(msg[0], 0x5F);
    let x = i32::from(u16::from_le_bytes([msg[1], msg[2]]));
    let y = i32::from(u16::from_le_bytes([msg[3], msg[4]]));
    assert_eq!((x, y), (100, 100));
    // The server measures it against its own cell (150, 100).
    assert_eq!(resync_distance(Point::new(150, 100), Point::new(x, y)), 50);
}

// Covers: specs/seams/movement-prediction.md §2.3 r1, §2.3 r2
#[test]
fn a_server_placement_places_the_model_and_the_prediction() {
    let mut m = Model::default();
    local(&mut m, (100, 100), 1);
    let mut p = Predict::new();
    p.observe(&m.w);
    m.recv(&reassign_player(0, 1, 120, 130, 0));
    assert_eq!(m.unit(P1).position, Some((120, 130)));
    p.observe(&m.w);
    assert_eq!(
        p.position(),
        Some((to_fp16_center(120), to_fp16_center(130)))
    );
}

// Covers: specs/seams/movement-prediction.md §2.4 r1
#[test]
fn another_players_move_states_the_server_cell_and_target() {
    let other = UnitKey::new(PLAYER, 7);
    let mut m = Model::default();
    local(&mut m, (100, 100), 1);
    let o = m.put(other);
    o.position = Some((90, 96));
    // Mode 1 (neutral): mode 0 is dead and skips the check.
    o.mode = 1;
    m.recv(&player_move(0, 7, 1, 140, 150, 95, 96));
    let n = m.drain();
    assert!(m.rejected().is_empty(), "{:?}", m.rejected());
    assert_eq!(n, 1);
    let u = m.unit(other);
    assert_eq!(u.server_point, (95, 96));
    let r = u.last_mode_request.expect("the move's mode request");
    assert_eq!(r.code, 1);
    assert_eq!((r.record[0], r.record[1]), (140, 150));
}

// Covers: specs/seams/movement-prediction.md §2.6 r1, §1 r3
#[test]
fn the_predicted_tick_distance_is_the_servers() {
    let t = tables();
    for (walk, run) in [(6u8, 9u8), (6, 6), (5, 8), (4, 9)] {
        let speeds = Speeds { walk, run };
        let bonus = run_velocity_bonus(i32::from(walk), i32::from(run)).unwrap();
        for (mode, running) in [(2u32, false), (6, false), (3, true)] {
            let u = Mover {
                ty: UnitType::Player,
                mode,
                percent: 100 + if running { bonus } else { 0 },
                charstats: (i32::from(walk), i32::from(run), 0),
                seed: Seed::default(),
            };
            let v = mode_velocity(&t, &u, UnitId(1), mode).expect("a walk mode has a velocity");
            let server = i64::from(STEP_BASE.wrapping_mul(v) >> 6);
            assert_eq!(
                speeds.step(running),
                server,
                "walk {walk} run {run} mode {mode}"
            );
        }
    }
}

// Covers: specs/seams/movement-prediction.md §2.7 r1
#[test]
fn the_predicted_facing_is_the_servers_direction() {
    let t = tables();
    let from = (to_fp16_center(100), to_fp16_center(100));
    for (dx, dy) in [(5, 0), (0, 5), (-5, 3), (7, -2), (-3, -3), (1, 9)] {
        let to = (to_fp16_center(100 + dx), to_fp16_center(100 + dy));
        assert_eq!(facing(from, to), Some(direction_vector(&t, from, to).1));
    }
}

/// Packs `(value, bits)` fields low bits first (`client/model.md` §10).
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
