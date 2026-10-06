//! Unit tests from pathing.md's test vectors and edge cases, on the
//! fakes of [`fake`].

mod fake;
mod gaps;
mod messages;

use fake::{FakeUnits, FakeWorld, ROOM};

use super::find::compute;
use super::geom::{direction_vector, octant, path_distance, ray_test, Ray};
use super::request::{handle_message, interrupt_check, mode_check, request, Outcome, WalkTarget};
use super::seams::{flag, Point, UsedSkill, WalkPath};
use super::step::{Step, Walk};
use super::tables::{PathTables, TableError, PATH_TABLES_TSV, PATH_TABLE_ROWS};
use super::velocity::{mode_velocity, run_velocity_bonus};
use crate::game::Game;
use crate::units::{ClientId, RoomId, UnitId, UnitType};

const P: UnitId = UnitId(1);

fn tables() -> PathTables {
    PathTables::embedded()
}

/// A world of `w`×`h` with the player at (x, y).
fn setup(w: i32, h: i32, x: i32, y: i32) -> (PathTables, FakeWorld, FakeUnits) {
    let t = tables();
    let mut world = FakeWorld::new(w, h);
    world.add_player(&t, P, x, y);
    let mut units = FakeUnits::with_player(P);
    units.unit(P).pos = Point::new(x, y);
    (t, world, units)
}

/// Computes a type-7 path to `target` (no velocity change, as §1.5 step 3
/// runs before the mode set).
fn compute_to(
    t: &PathTables,
    world: &mut FakeWorld,
    units: &mut FakeUnits,
    target: Point,
) -> (i32, WalkPath) {
    let mut path = world.paths[&P].clone();
    path.target = target;
    path.target_unit = None;
    let n = compute(t, world, units, &mut path, P, false).unwrap();
    (n, path)
}

fn pts(v: &[(i32, i32)]) -> Vec<Point> {
    v.iter().map(|&(x, y)| Point::new(x, y)).collect()
}

// ---- tables ---------------------------------------------------------

// Covers: specs/sim/pathing.md §2
#[test]
fn tables_parse_and_match_spec_values() {
    let t = tables();
    assert_eq!(PATH_TABLES_TSV.lines().count() - 1, PATH_TABLE_ROWS);
    // §2 type table: flags of types 0, 1, 7, 8, 11 and offsets of 5, 6, 12.
    assert_eq!(t.pathtype_flags[0], 0x21900);
    assert_eq!(t.pathtype_flags[1], 0x1900);
    assert_eq!(t.pathtype_flags[7], 0x21900);
    assert_eq!(t.pathtype_flags[8], 0x1E600);
    assert_eq!(t.pathtype_flags[11], 0x1E604);
    assert_eq!(t.pathtype_diroff[5], 2);
    assert_eq!(t.pathtype_diroff[6], -2);
    assert_eq!(t.pathtype_diroff[12], -4);
    // §5.1 rule 2 steps.
    assert_eq!(
        t.dir8_toward,
        [
            (1, 0),
            (1, 1),
            (0, 1),
            (-1, 1),
            (-1, 0),
            (-1, -1),
            (0, -1),
            (1, -1)
        ]
    );
    assert_eq!(t.dir8_target, t.dir8_toward);
    assert_eq!(t.dist8_unit, t.dist8_path);
    assert_eq!(t.tan[127].x, 2896);
    assert_eq!(t.animstat[4].base, 150);
    assert_eq!(t.animstat[4].stat, 96);
}

#[test]
fn tables_parse_rejects_perturbations() {
    // M08: one changed value is seen exactly there.
    let changed = PATH_TABLES_TSV.replacen("snap9\t40\t0\t", "snap9\t40\t7\t", 1);
    assert_ne!(changed, PATH_TABLES_TSV);
    let t2 = PathTables::parse(&changed).unwrap();
    let t = tables();
    assert_eq!(t2.snap9[40], 7);
    let mut back = t2.clone();
    back.snap9[40] = t.snap9[40];
    assert_eq!(back, t);
    // A dropped row, a repeated row, an unknown table, a bad header.
    let dropped: String = PATH_TABLES_TSV
        .lines()
        .filter(|l| !l.starts_with("tan\t5\t"))
        .map(|l| format!("{l}\n"))
        .collect();
    assert_eq!(PathTables::parse(&dropped), Err(TableError::Missing("tan")));
    let dup = format!("{PATH_TABLES_TSV}tan\t5\t1\t2\t3\t\t\t0x0\n");
    assert!(matches!(
        PathTables::parse(&dup),
        Err(TableError::Row(_, "repeated row"))
    ));
    let unknown = format!("{PATH_TABLES_TSV}nope\t0\t1\t\t\t\t\t0x0\n");
    assert!(matches!(
        PathTables::parse(&unknown),
        Err(TableError::Row(_, "unknown table"))
    ));
    assert_eq!(PathTables::parse("x\n"), Err(TableError::Header(1)));
}

// ---- helpers §5.1 ---------------------------------------------------

// Covers: specs/sim/pathing.md §5.1 r1, §5.1 r3
#[test]
fn octant_and_path_distance() {
    let t = tables();
    let o = |dx, dy| octant(Point::new(0, 0), Point::new(dx, dy));
    assert_eq!(o(0, 0), 12);
    assert_eq!(o(7, 0), 22); // horizontal, dx clamped to 2
    assert_eq!(o(-5, 0), 2);
    assert_eq!(o(-1, 5), 9); // vertical branch, dx < 0
    assert_eq!(o(-1, -5), 5);
    assert_eq!(o(1, 1), 18);
    assert_eq!(o(2, 5), 14); // vertical, dx := 2 & 1 = 0
    assert_eq!(o(1, 5), 19); // vertical, dx := 1
    assert_eq!(o(-3, -3), 0);
    // Distance: table below 8, else 2·max + min.
    let d = |dx, dy| path_distance(&t, Point::new(0, 0), Point::new(dx, dy));
    assert_eq!(d(1, 1), 0); // dist8_path[9] = −1 → 0
    assert_eq!(d(7, 0), 9);
    assert_eq!(d(8, 3), 19);
}

// Covers: specs/sim/pathing.md §5.1 r4, §edge-cases-original-bugs r4
#[test]
fn ray_test_returns_cell_before_block() {
    let t = tables();
    let mut w = FakeWorld::new(40, 40);
    for y in 5..=15 {
        w.wall(15, y);
    }
    let _ = t;
    let r = ray_test(
        &w,
        Some(ROOM),
        1,
        0x1C09,
        Point::new(10, 10),
        Point::new(20, 10),
    );
    assert_eq!(r, Ray::Blocked(Point::new(13, 10)));
    let r = ray_test(
        &w,
        Some(ROOM),
        1,
        0x1C09,
        Point::new(10, 10),
        Point::new(13, 30),
    );
    assert_eq!(r, Ray::Clear);
    // Edge case 4: after a minor-axis step with err = 0 the new cell is
    // not tested. (10,10)→(13,11): Nx 4, Ny 2; the first minor step
    // leaves err = 0 at (11,11), which is never tested (point pattern).
    let mut w2 = FakeWorld::new(40, 40);
    w2.wall(11, 11);
    let r = ray_test(
        &w2,
        Some(ROOM),
        0,
        0x1,
        Point::new(10, 10),
        Point::new(13, 11),
    );
    assert_eq!(r, Ray::Clear);
    // With err > 0 after the minor step the cell is tested: (10,10)→(12,11)
    // (Nx 3, Ny 2) tests (11,11).
    let r = ray_test(
        &w2,
        Some(ROOM),
        0,
        0x1,
        Point::new(10, 10),
        Point::new(12, 11),
    );
    assert_eq!(r, Ray::Blocked(Point::new(10, 10)));
}

// ---- path vectors W1–W9 ---------------------------------------------

// Covers: specs/sim/pathing.md §3 r8, §3 r10, §5.2 r1, §6 r2
#[test]
fn w1_clear_ray() {
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    let (n, p) = compute_to(&t, &mut w, &mut u, Point::new(20, 15));
    assert_eq!(n, 1);
    assert_eq!(p.live_points(), pts(&[(20, 15)]).as_slice());
    assert_eq!(p.final_target, Point::new(20, 15));
    assert_ne!(p.flags & flag::ACTIVE, 0);
    assert_eq!(p.flags & flag::OUTSIDE_ROOM, 0);
}

fn w2_world(x: i32, y: i32) -> (PathTables, FakeWorld, FakeUnits) {
    let (t, mut w, u) = setup(40, 40, x, y);
    for yy in 5..=15 {
        w.wall(15, yy);
    }
    (t, w, u)
}

// Covers: specs/sim/pathing.md §6 r3, §7 r2, §7 r3, §7 r4, §7 r5, §7 r6
#[test]
fn w2_astar_around_wall() {
    let (t, mut w, mut u) = w2_world(10, 10);
    let (n, p) = compute_to(&t, &mut w, &mut u, Point::new(20, 10));
    assert_eq!(
        p.live_points(),
        pts(&[(13, 13), (13, 15), (15, 17), (17, 15), (17, 13), (20, 10)]).as_slice()
    );
    assert_eq!(n, 6);
}

// Covers: specs/sim/pathing.md §6 r4, §5.2 r4, §5.2 r5
#[test]
fn w3_no_astar_beyond_radius() {
    let (t, mut w, mut u) = setup(60, 60, 20, 20);
    for y in 0..=49 {
        w.wall(30, y);
    }
    let (n, p) = compute_to(&t, &mut w, &mut u, Point::new(40, 20));
    assert_eq!(n, 1);
    assert_eq!(p.live_points(), pts(&[(28, 20)]).as_slice());
    // §3 step 10: no target unit, flag 0x10 clear → target := last point.
    assert_eq!(p.target, Point::new(28, 20));
}

// Covers: specs/sim/pathing.md §3 r7, §4 r1, §4 r2, §4 r3, §4 r4
#[test]
fn w4_blocked_target_prepared() {
    let (t, mut w, mut u) = w2_world(10, 10);
    let (n, p) = compute_to(&t, &mut w, &mut u, Point::new(15, 10));
    assert_eq!(n, 1);
    assert_eq!(p.final_target, Point::new(13, 10));
    assert_eq!(p.live_points(), pts(&[(13, 10)]).as_slice());
}

// Covers: specs/sim/pathing.md §3 r11, §3 r12, §4 r3
#[test]
fn w5_preparation_reaches_start() {
    let (t, mut w, mut u) = w2_world(13, 10);
    let (n, p) = compute_to(&t, &mut w, &mut u, Point::new(15, 10));
    assert_eq!(n, 0);
    assert_eq!((p.index, p.count), (0, 0));
    assert_eq!(p.flags & flag::ACTIVE, 0);
    // Through the request: count 0 → neutral start.
    let mut g = Game::new();
    let o = request(
        &t,
        &mut w,
        &mut u,
        &mut g,
        P,
        None,
        2,
        WalkTarget::Point(Point::new(15, 10)),
        false,
    )
    .unwrap();
    assert_eq!(o, Outcome::Neutral);
    assert_eq!(u.units[&P].mode, 1);
}

// Covers: specs/sim/pathing.md §4 r2, §4 r4
#[test]
fn w6_push_stops_at_collision() {
    let (t, mut w, mut u) = setup(40, 40, 14, 25);
    for x in 10..=20 {
        w.wall(x, 20);
    }
    let (n, p) = compute_to(&t, &mut w, &mut u, Point::new(15, 20));
    assert_eq!(n, 1);
    assert_eq!(p.final_target, Point::new(15, 22));
    assert_eq!(p.live_points(), pts(&[(15, 22)]).as_slice());
}

// Covers: specs/sim/pathing.md §5.2 r2, §5.2 r3, §5.2 r4, §5.2 r5, §5.2 r6, §edge-cases-original-bugs r3
#[test]
fn w9_greedy_walk_with_duplicate_first_corner() {
    let (t, mut w, mut u) = setup(80, 80, 10, 10);
    for y in 9..=11 {
        w.wall(15, y);
    }
    let (n, p) = compute_to(&t, &mut w, &mut u, Point::new(34, 22));
    let expect = pts(&[
        (14, 12),
        (14, 12),
        (15, 13),
        (17, 13),
        (18, 14),
        (19, 14),
        (20, 15),
        (21, 15),
        (22, 16),
        (23, 16),
        (24, 17),
        (25, 17),
        (26, 18),
        (27, 18),
        (28, 19),
        (29, 19),
        (30, 20),
        (31, 20),
        (32, 21),
        (33, 21),
    ]);
    assert_eq!(p.live_points(), expect.as_slice());
    assert_eq!(n, 20);
}

// Covers: specs/sim/pathing.md §3 r5, §edge-cases-original-bugs r10
#[test]
fn far_target_clears_path() {
    let (t, mut w, mut u) = setup(300, 40, 10, 10);
    let (n, p) = compute_to(&t, &mut w, &mut u, Point::new(10 + 101, 10));
    assert_eq!(n, 0);
    assert_eq!((p.index, p.count), (0, 0));
    let (n, _) = compute_to(&t, &mut w, &mut u, Point::new(10 + 100, 10));
    assert_eq!(n, 1);
}

// Covers: specs/sim/pathing.md §3 r1
#[test]
fn compute_owner_mismatch_is_fatal() {
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    let mut p = w.paths[&P].clone();
    p.owner = UnitId(9);
    p.target = Point::new(20, 10);
    assert!(compute(&t, &mut w, &mut u, &mut p, P, false).is_err());
}

// Covers: specs/sim/pathing.md §3 r2, §3 r4
#[test]
fn target_unit_position_refreshes_target() {
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    let mut m = fake::FakeUnit::player();
    m.ty = UnitType::Monster;
    m.guid = 77;
    m.pos = Point::new(20, 12);
    m.size = 0;
    u.units.insert(UnitId(2), m);
    let mut g = Game::new();
    let o = request(
        &t,
        &mut w,
        &mut u,
        &mut g,
        P,
        None,
        2,
        WalkTarget::Unit {
            ty: UnitType::Monster,
            guid: 77,
        },
        false,
    )
    .unwrap();
    assert_eq!(o, Outcome::Moving(1));
    let p = &w.paths[&P];
    assert_eq!(p.target, Point::new(20, 12));
    assert_eq!(p.live_points(), pts(&[(20, 12)]).as_slice());
    assert!(p.target_unit.is_some());
    // A target unit at (0, y): result 0, nothing reset.
    u.unit(UnitId(2)).pos = Point::new(0, 12);
    let mut p2 = w.paths[&P].clone();
    let before = p2.clone();
    assert_eq!(compute(&t, &mut w, &mut u, &mut p2, P, false).unwrap(), 0);
    assert_eq!(p2.count, before.count);
    // Unknown GUID: a log line, nothing else.
    let o = request(
        &t,
        &mut w,
        &mut u,
        &mut g,
        P,
        None,
        2,
        WalkTarget::Unit {
            ty: UnitType::Monster,
            guid: 5,
        },
        false,
    )
    .unwrap();
    assert_eq!(o, Outcome::NoTargetUnit);
}

// ---- direction vectors D1–D4 ----------------------------------------

// Covers: specs/sim/pathing.md §8.3 r1, §8.3 r2, §8.3 r3
#[test]
fn d_direction_vectors() {
    let t = tables();
    let fp = |x: i32| (x as u32) << 16;
    let dv =
        |x: i32, y: i32| direction_vector(&t, (fp(1000), fp(1000)), (fp(1000 + x), fp(1000 + y)));
    assert_eq!(dv(10, 0), ((4096, 0), 56));
    assert_eq!(dv(0, -10), ((0, -4096), 40));
    assert_eq!(dv(5, 5), ((2896, 2896), 0));
    assert_eq!(dv(3, 1), ((3888, 1286), 59));
}

// ---- velocity V1–V3 -------------------------------------------------

// V1–V3 state stat 67 as the modifier on top of its base: players start
// with velocitypercent 100 (`combat/vitals.md` §1 table), so the formula
// p = f + stat 67 gives V1 and V2 with stat 67 = 100 and 150. V3's
// printed result (p = 67, 1029) drops that base; the formula gives
// p = 17 + 150 = 167 (spec question, docs/handoff/impl-walk.md).
// Covers: specs/sim/pathing.md §8.1 r1, §8.1 r2, §8.2
#[test]
fn v_velocities() {
    let t = tables();
    let mut u = FakeUnits::with_player(P);
    assert_eq!(u.units[&P].stats[&67], 100);
    assert_eq!(mode_velocity(&t, &u, P, 2), Some(0x600));
    assert_eq!(mode_velocity(&t, &u, P, 19), Some(0x1000));
    assert_eq!(run_velocity_bonus(6, 9), Some(50));
    assert_eq!(run_velocity_bonus(0, 9), None);
    u.unit(P).stats.insert(67, 150);
    assert_eq!(mode_velocity(&t, &u, P, 3), Some(0x900));
    u.unit(P).item_stats.insert(96, 20);
    // f = 150·20/170 = 17.
    assert_eq!(mode_velocity(&t, &u, P, 3), Some(0x600 * 167 / 100));
    // Floor 25 %.
    u.unit(P).item_stats.clear();
    u.unit(P).stats.insert(67, 10);
    assert_eq!(mode_velocity(&t, &u, P, 2), Some(0x600 * 25 / 100));
    // Neutral has no modifier: no rule (seam).
    assert_eq!(mode_velocity(&t, &u, P, 1), None);
}

// ---- movement M1, M2 ------------------------------------------------

fn run_ticks(
    t: &PathTables,
    w: &mut FakeWorld,
    u: &mut FakeUnits,
    g: &mut Game,
    max: usize,
) -> Vec<(u32, u32, Step)> {
    let mut out = Vec::new();
    for _ in 0..max {
        let s = Walk {
            t,
            w: &mut *w,
            u: &mut *u,
        }
        .player_event0(g, P)
        .unwrap();
        let p = &w.paths[&P];
        out.push((p.precise_x, p.precise_y, s));
        if s == Step::Stopped {
            break;
        }
    }
    out
}

// Covers: specs/sim/pathing.md §1.1, §1.2 r5, §1.5 r1, §1.5 r2, §1.5 r3, §1.5 r4, §1.5 r6, §8.4 r1, §8.4 r2, §9.2 r4, §9.2 r6, §9.3, §9.4 r1, §9.4 r2, §9.4 r3, §9.5 r2, §9.6 r3, §9.6 r4, §9.6 r7, §9.6 r8, §9.7, §edge-cases-original-bugs r6
#[test]
fn m1_walk_per_tick() {
    let (t, mut w, mut u) = setup(200, 200, 100, 100);
    let mut g = Game::new();
    let (r, o) = handle_message(&t, &mut w, &mut u, &mut g, P, 0x01, 105, 100).unwrap();
    assert_eq!((r, o), (0, Some(Outcome::Moving(1))));
    assert_eq!(u.units[&P].mode, 2);
    assert_eq!(w.paths[&P].velocity, 0x600);
    let ticks = run_ticks(&t, &mut w, &mut u, &mut g, 20);
    assert_eq!(ticks.len(), 14);
    for (k, &(x, y, s)) in ticks.iter().take(13).enumerate() {
        assert_eq!(x, 0x64E000 + k as u32 * 0x6000, "tick {}", k + 1);
        assert_eq!(y, 0x648000);
        assert_eq!(s, Step::Moving);
    }
    assert_eq!(ticks[13], (0x698000, 0x648000, Step::Stopped));
    let p = &w.paths[&P];
    assert_eq!((p.index, p.count, p.flags & flag::ACTIVE), (0, 0, 0));
    assert_eq!(u.units[&P].mode, 1);
    // The footprint moved with the unit: the old centre is clear, the
    // new one carries the player marker.
    assert_eq!(w.value(Point::new(100, 100)) & 0x1000, 0);
    assert_eq!(w.value(Point::new(105, 100)) & 0x1080, 0x1080);
}

// Covers: specs/sim/pathing.md §8.2, §9.2 r3, §9.4 r2, §9.6 r3, §9.6 r5
#[test]
fn m2_run_per_tick() {
    let (t, mut w, mut u) = setup(200, 200, 100, 100);
    let mut g = Game::new();
    let (_, o) = handle_message(&t, &mut w, &mut u, &mut g, P, 0x03, 103, 101).unwrap();
    assert_eq!(o, Some(Outcome::Moving(1)));
    assert_eq!(u.units[&P].mode, 3);
    assert!(u.log.contains(&"run list 50".to_string()));
    assert_eq!(w.paths[&P].velocity, 0x900);
    let ticks = run_ticks(&t, &mut w, &mut u, &mut g, 20);
    let expect = [
        (0x6508B0, 0x64AD36),
        (0x659160, 0x64DA6C),
        (0x661A10, 0x6507A2),
        (0x66A2C0, 0x6534D8),
        (0x672B1F, 0x656301),
        (0x678000, 0x658000),
    ];
    assert_eq!(ticks.len(), 6);
    for (k, &(x, y, _)) in ticks.iter().enumerate() {
        assert_eq!((x, y), expect[k], "tick {}", k + 1);
    }
    assert_eq!(ticks[5].2, Step::Stopped);
    // Five run-drain ticks of 40 (S1) before the stop tick's drain too.
    assert_eq!(u.units[&P].stats[&10], (100 << 8) - 6 * 40);
}

// Covers: specs/sim/pathing.md §9.6 r4, §9.6 r5, §9.6 r6, §edge-cases-original-bugs r7
#[test]
fn blocked_cell_stops_at_last_free_centre() {
    let (t, mut w, mut u) = setup(200, 200, 100, 100);
    let mut g = Game::new();
    request(
        &t,
        &mut w,
        &mut u,
        &mut g,
        P,
        None,
        2,
        WalkTarget::Point(Point::new(110, 100)),
        false,
    )
    .unwrap();
    // A wall appears after the path was computed: the plus at 103 hits it.
    w.wall(104, 100);
    let ticks = run_ticks(&t, &mut w, &mut u, &mut g, 40);
    let last = *ticks.last().unwrap();
    assert_eq!(last.2, Step::Stopped);
    assert_eq!((last.0, last.1), (0x668000, 0x648000)); // centre of 102
    assert_eq!(w.paths[&P].count, 0);
    assert_eq!(u.units[&P].mode, 1);
    // Footprint stays on 102.
    assert_eq!(w.value(Point::new(102, 100)) & 0x1000, 0x1000);
}

// ---- stamina S1, S2 -------------------------------------------------

// Covers: specs/sim/pathing.md §9.9 r1, §9.9 r2, §9.9 r3
#[test]
fn s_run_drain() {
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    let mut g = Game::new();
    let p = w.paths[&P].clone();
    u.walk_velocity = (6, 9, 20);
    u.unit(P).stats.insert(10, 1000);
    assert!(!Walk {
        t: &t,
        w: &mut w,
        u: &mut u
    }
    .run_drain(&mut g, P, &p));
    assert_eq!(u.units[&P].stats[&10], 960);
    u.unit(P).torso_speed = Some(10);
    u.unit(P).item_stats.insert(154, 25);
    assert!(!Walk {
        t: &t,
        w: &mut w,
        u: &mut u
    }
    .run_drain(&mut g, P, &p));
    assert_eq!(u.units[&P].stats[&10], 900);
    // Exhausted: stamina := 0.
    u.unit(P).stats.insert(10, 30);
    assert!(Walk {
        t: &t,
        w: &mut w,
        u: &mut u
    }
    .run_drain(&mut g, P, &p));
    assert_eq!(u.units[&P].stats[&10], 0);
    // Town: no drain.
    w.town = true;
    u.unit(P).stats.insert(10, 30);
    assert!(!Walk {
        t: &t,
        w: &mut w,
        u: &mut u
    }
    .run_drain(&mut g, P, &p));
    assert_eq!(u.units[&P].stats[&10], 30);
}

// Covers: specs/sim/pathing.md §1.5 r2, §edge-cases-original-bugs r9
#[test]
fn run_without_stamina_is_walk_and_town_walk() {
    let (t, mut w, mut u) = setup(200, 200, 100, 100);
    let mut g = Game::new();
    u.unit(P).stats.insert(10, 0);
    request(
        &t,
        &mut w,
        &mut u,
        &mut g,
        P,
        None,
        3,
        WalkTarget::Point(Point::new(105, 100)),
        false,
    )
    .unwrap();
    assert_eq!(u.units[&P].mode, 2);
    w.town = true;
    request(
        &t,
        &mut w,
        &mut u,
        &mut g,
        P,
        None,
        2,
        WalkTarget::Point(Point::new(108, 100)),
        false,
    )
    .unwrap();
    assert_eq!(u.units[&P].mode, 6);
}

// ---- mode and interrupt checks §1.3, §1.4 ---------------------------

// Covers: specs/sim/pathing.md §1.3 r1, §1.3 r2, §1.3 r3
#[test]
fn mode_check_rules() {
    let mut u = FakeUnits::with_player(P);
    let mut g = Game::new();
    g.frame = 100;
    u.unit(P).cursor = true;
    assert!(!mode_check(&u, &g, P, 2));
    assert!(mode_check(&u, &g, P, 17));
    assert!(mode_check(&u, &g, P, 0));
    u.unit(P).cursor = false;
    for m in [0, 1, 5, 17] {
        u.unit(P).mode = 0;
        assert!(mode_check(&u, &g, P, m));
    }
    for cur in [0, 4, 9, 17] {
        u.unit(P).mode = cur;
        assert!(!mode_check(&u, &g, P, 2));
    }
    // Attack: frame ≤ E + 5.
    u.unit(P).mode = 7;
    u.type1_expire = 94;
    assert!(!mode_check(&u, &g, P, 2));
    assert!(mode_check(&u, &g, P, 4));
    u.type1_expire = 95;
    assert!(mode_check(&u, &g, P, 2));
    u.unit(P).mode = 12;
    u.type1_expire = 0;
    assert!(!mode_check(&u, &g, P, 4));
    // S1 Amazon never, S3 Druid never.
    u.unit(P).mode = 13;
    assert!(!mode_check(&u, &g, P, 2));
    u.unit(P).class = 1;
    assert!(mode_check(&u, &g, P, 2));
    u.unit(P).mode = 15;
    u.unit(P).class = 5;
    assert!(!mode_check(&u, &g, P, 2));
    // SQ: SeqInput > 0.
    u.unit(P).mode = 18;
    assert!(!mode_check(&u, &g, P, 2));
    u.unit(P).used_skill = Some(UsedSkill {
        seq_input: 1,
        ..UsedSkill::default()
    });
    assert!(mode_check(&u, &g, P, 2));
    for cur in [1, 2, 3, 5, 6, 14, 16, 19, 25] {
        u.unit(P).mode = cur;
        assert!(mode_check(&u, &g, P, 2));
    }
}

// Covers: specs/sim/pathing.md §1.4 r1, §1.4 r2, §1.4 r3, §1.4 r4, §1.4 r5, §1.4 r6
#[test]
fn interrupt_check_rules() {
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    let mut g = Game::new();
    g.frame = 100;
    u.unit(P).mode = 2;
    // Rule 1.
    u.unit(P).states = vec![54];
    assert!(!interrupt_check(&t, &mut w, &mut u, &mut g, P, 2, None).unwrap());
    u.unit(P).states.clear();
    // Rule 2: no used skill.
    assert!(interrupt_check(&t, &mut w, &mut u, &mut g, P, 2, None).unwrap());
    // Rule 3: srvdofunc 67 with the same skill.
    u.unit(P).used_skill = Some(UsedSkill {
        id: 9,
        srvdofunc: 67,
        ..UsedSkill::default()
    });
    assert!(interrupt_check(&t, &mut w, &mut u, &mut g, P, 2, Some(9)).unwrap());
    // Rule 4: interrupt clear, not neutral, m = 2 → refuse.
    assert!(!interrupt_check(&t, &mut w, &mut u, &mut g, P, 2, None).unwrap());
    // Neutral → re-enter neutral (mode 1 start) and allow.
    u.unit(P).mode = 1;
    assert!(interrupt_check(&t, &mut w, &mut u, &mut g, P, 2, None).unwrap());
    assert!(u.log.contains(&"start 1".to_string()));
    // Rule 5: interrupt set, concentration with stat 164 = 100: one draw,
    // r < 100 → rule 6; mode 2 → refuse.
    u.unit(P).mode = 2;
    u.unit(P).used_skill = Some(UsedSkill {
        id: 9,
        interrupt: true,
        ..UsedSkill::default()
    });
    u.unit(P).states = vec![42];
    u.unit(P).state_stats.insert((42, 164), 100);
    let before = u.units[&P].seed;
    assert!(!interrupt_check(&t, &mut w, &mut u, &mut g, P, 2, None).unwrap());
    let mut s = before;
    s.roll(100);
    assert_eq!(u.units[&P].seed, s);
    // v = 0: no rule 6, allow.
    u.unit(P).state_stats.insert((42, 164), 0);
    assert!(interrupt_check(&t, &mut w, &mut u, &mut g, P, 2, None).unwrap());
    // State 15 → rule 6 → refuse in mode 2; no draw.
    u.unit(P).states = vec![15];
    let before = u.units[&P].seed;
    assert!(!interrupt_check(&t, &mut w, &mut u, &mut g, P, 2, None).unwrap());
    assert_eq!(u.units[&P].seed, before);
    // No state: allow.
    u.unit(P).states.clear();
    assert!(interrupt_check(&t, &mut w, &mut u, &mut g, P, 2, None).unwrap());
}

// Covers: specs/sim/pathing.md §1.2 r2, §1.2 r3
#[test]
fn refused_request_changes_nothing() {
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    let mut g = Game::new();
    u.unit(P).mode = 4; // GH
    let before = w.paths[&P].clone();
    let o = request(
        &t,
        &mut w,
        &mut u,
        &mut g,
        P,
        None,
        2,
        WalkTarget::Point(Point::new(20, 10)),
        false,
    )
    .unwrap();
    assert_eq!(o, Outcome::ModeRefused);
    assert_eq!(w.paths[&P], before);
    u.unit(P).mode = 2;
    u.unit(P).states = vec![54];
    let o = request(
        &t,
        &mut w,
        &mut u,
        &mut g,
        P,
        None,
        2,
        WalkTarget::Point(Point::new(20, 10)),
        false,
    )
    .unwrap();
    assert_eq!(o, Outcome::InterruptRefused);
}

// Covers: specs/sim/pathing.md §1.5, §edge-cases-original-bugs r8
#[test]
fn request_while_moving_keeps_fraction() {
    let (t, mut w, mut u) = setup(200, 200, 100, 100);
    let mut g = Game::new();
    request(
        &t,
        &mut w,
        &mut u,
        &mut g,
        P,
        None,
        2,
        WalkTarget::Point(Point::new(110, 100)),
        false,
    )
    .unwrap();
    run_ticks(&t, &mut w, &mut u, &mut g, 3);
    let x = w.paths[&P].precise_x;
    assert_eq!(x & 0xFFFF, (0x648000u32 + 3 * 0x6000) & 0xFFFF);
    request(
        &t,
        &mut w,
        &mut u,
        &mut g,
        P,
        None,
        2,
        WalkTarget::Point(Point::new(101, 106)),
        false,
    )
    .unwrap();
    let p = &w.paths[&P];
    assert_eq!(p.precise_x, x);
    assert_eq!(p.live_points(), pts(&[(101, 106)]).as_slice());
}

// ---- room change §9.6 r9, §9.8 --------------------------------------

// Covers: specs/sim/pathing.md §9.6 r9, §9.8
#[test]
fn room_change_sends_merge_of_client_arrays() {
    let (t, mut w, mut u) = setup(40, 20, 18, 10);
    w.room0 = (0, 0, 20, 20);
    let r1 = RoomId(1);
    w.rooms.insert(r1, (20, 0, 20, 20));
    w.clients.insert(ROOM, vec![ClientId(1), ClientId(2)]);
    w.clients.insert(r1, vec![ClientId(2), ClientId(3)]);
    u.client_players.insert(ClientId(1), P);
    u.client_players.insert(ClientId(3), UnitId(9));
    let mut g = Game::new();
    request(
        &t,
        &mut w,
        &mut u,
        &mut g,
        P,
        None,
        2,
        WalkTarget::Point(Point::new(24, 10)),
        false,
    )
    .unwrap();
    assert_ne!(w.paths[&P].flags & flag::OUTSIDE_ROOM, 0);
    run_ticks(&t, &mut w, &mut u, &mut g, 30);
    let p = &w.paths[&P];
    assert_eq!(p.room, Some(r1));
    assert_eq!(p.prev_room, Some(ROOM));
    assert_eq!(p.flags & flag::ROOM_CHANGED, 0);
    assert!(w.log.contains(&"leave 1 0".to_string()));
    assert!(w.log.contains(&"insert 1 1".to_string()));
    // Client 1 (the unit's own) gets no removal; 2 is in both; 3 gets the add.
    let msgs: Vec<&String> = u
        .log
        .iter()
        .filter(|l| l.starts_with("add") || l.starts_with("remove"))
        .collect();
    assert_eq!(msgs, vec!["add 1 to 3"]);
}

// ---- re-path §9.10 --------------------------------------------------

// Covers: specs/sim/pathing.md §9.10, §9.5 r2
#[test]
fn repath_without_budget_stops() {
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    let mut g = Game::new();
    let mut p = w.paths[&P].clone();
    u.repath_budget = 0;
    let r = Walk {
        t: &t,
        w: &mut w,
        u: &mut u,
    }
    .repath(&mut g, P, &mut p, false)
    .unwrap();
    assert_eq!(r, 0);
    // With budget: queue, unit flag, budget −= index, compute.
    u.repath_budget = 1;
    p.target = Point::new(20, 10);
    p.distance_budget = 5;
    p.index = 2;
    let r = Walk {
        t: &t,
        w: &mut w,
        u: &mut u,
    }
    .repath(&mut g, P, &mut p, false)
    .unwrap();
    assert_eq!(r, 1);
    assert_eq!(p.distance_budget, 3);
    assert!(w.log.contains(&"queue 1".to_string()));
}

// ---- facing §8.3 r4, §8.5 -------------------------------------------

// Covers: specs/sim/pathing.md §8.3 r4, §8.4 r3, §8.5
#[test]
fn facing_turn_step_and_face_away() {
    use super::geom::set_facing;
    use super::velocity::aim;
    let t = tables();
    let mut p = WalkPath::zeroed(P);
    p.direction = 10;
    set_facing(&t, &mut p, UnitType::Player, 20);
    assert_eq!((p.direction, p.new_direction, p.turn_step), (10, 20, 4)); // dirdiff[10]
    p.turn_step = 0;
    set_facing(&t, &mut p, UnitType::Player, 20 + 64);
    assert_eq!(p.turn_step, 0); // same new direction: unchanged
    set_facing(&t, &mut p, UnitType::Item, 33);
    assert_eq!(p.direction, 33);
    p.flags = 0x40;
    set_facing(&t, &mut p, UnitType::Missile, 5);
    assert_eq!(p.direction, 33);
    p.flags = 0;
    set_facing(&t, &mut p, UnitType::Missile, 5);
    assert_eq!(p.direction, 5);
    // Flag 0x200: the computed direction − 32. (0,0)→(10,0) is 56.
    let mut p = WalkPath::zeroed(P);
    p.precise_x = super::geom::centre(100);
    p.precise_y = super::geom::centre(100);
    p.velocity = 0x600;
    p.count = 1;
    p.points[0] = Point::new(110, 100);
    aim(&t, &mut p, UnitType::Player);
    assert_eq!(p.new_direction, 56);
    assert_eq!(p.vel_vec, (0x6000, 0));
    p.flags = flag::FACE_AWAY;
    aim(&t, &mut p, UnitType::Player);
    assert_eq!(p.new_direction, 24);
}

// ---- target unit §9.2 r1, §9.5 r3, §1.2 r4 -------------------------

// Covers: specs/sim/pathing.md §1.2 r1, §1.2 r4, §9.2 r1, §9.5 r3
#[test]
fn target_unit_check_and_stop_distance() {
    let (t, mut w, mut u) = setup(200, 200, 100, 100);
    let mut m = fake::FakeUnit::player();
    m.ty = UnitType::Monster;
    m.guid = 77;
    m.pos = Point::new(110, 100);
    m.size = 2;
    u.units.insert(UnitId(2), m);
    let mut g = Game::new();
    let o = request(
        &t,
        &mut w,
        &mut u,
        &mut g,
        P,
        None,
        2,
        WalkTarget::Unit {
            ty: UnitType::Monster,
            guid: 77,
        },
        false,
    )
    .unwrap();
    assert_eq!(o, Outcome::Moving(1));
    assert!(u.log.contains(&"clear queued".to_string()));
    // Stop distance 5: the unit stops once the unit distance is ≤ 5.
    w.paths.get_mut(&P).unwrap().stop_distance = 5;
    let ticks = run_ticks(&t, &mut w, &mut u, &mut g, 40);
    assert_eq!(ticks.last().unwrap().2, Step::Stopped);
    let cell = w.paths[&P].cell();
    let d = super::geom::unit_distance(&t, cell, 2, Point::new(110, 100), 2);
    assert!(d <= 5, "stopped at {cell:?}, distance {d}");
    assert!(cell.x < 109);
    // Target check: a GUID that now names another unit drops the target.
    let mut p = w.paths[&P].clone();
    p.target_unit = Some(super::seams::TargetUnit {
        unit: UnitId(3),
        ty: UnitType::Monster,
        guid: 77,
    });
    Walk {
        t: &t,
        w: &mut w,
        u: &mut u,
    }
    .target_check(&mut p);
    assert_eq!(p.target_unit, None);
    // An item in mode 1 or 2 (equipped, belt) is dropped too.
    let mut it = fake::FakeUnit::player();
    it.ty = UnitType::Item;
    it.guid = 5;
    it.mode = 1;
    u.units.insert(UnitId(4), it);
    p.target_unit = Some(super::seams::TargetUnit {
        unit: UnitId(4),
        ty: UnitType::Item,
        guid: 5,
    });
    Walk {
        t: &t,
        w: &mut w,
        u: &mut u,
    }
    .target_check(&mut p);
    assert_eq!(p.target_unit, None);
    u.unit(UnitId(4)).mode = 3;
    p.target_unit = Some(super::seams::TargetUnit {
        unit: UnitId(4),
        ty: UnitType::Item,
        guid: 5,
    });
    Walk {
        t: &t,
        w: &mut w,
        u: &mut u,
    }
    .target_check(&mut p);
    assert!(p.target_unit.is_some());
}

// ---- cell walk limit, edge case 5 ----------------------------------

// Edge case 5 needs a step over 10 sub-tiles; §9.4 rule 2.1 computes
// (m · direction) in 32 bits, which wraps above 8 sub-tiles per tick on an
// axis, so the cell walk is driven directly with a 12-cell Δ here.
// Covers: specs/sim/pathing.md §9.6 r5, §edge-cases-original-bugs r5
#[test]
fn cell_walk_stops_testing_after_ten_cells() {
    let (t, mut w, mut u) = setup(200, 200, 100, 100);
    let mut p = w.paths[&P].clone();
    // A wall at 113 touches the plus at 112: cells 101..110 are tested,
    // 111 and 112 are not.
    w.wall(113, 100);
    let r = Walk {
        t: &t,
        w: &mut w,
        u: &mut u,
    }
    .cell_walk(P, &mut p, (12 << 16, 0))
    .unwrap();
    assert_eq!(r, Ok((super::geom::centre(112), super::geom::centre(100))));
    assert_eq!(p.saved_count, 10);
    assert_eq!(p.saved_steps[0], Point::new(101, 100));
    assert_eq!(p.saved_steps[9], Point::new(110, 100));
    assert_ne!(p.flags & flag::CROSSED, 0);
    // The footprint stopped at the last tested cell.
    assert_eq!(w.value(Point::new(110, 100)) & 0x1000, 0x1000);
    // A wall inside the first ten cells blocks: Q = centre of the last
    // free cell.
    let (t, mut w, mut u) = setup(200, 200, 100, 100);
    let mut p = w.paths[&P].clone();
    w.wall(106, 100);
    let r = Walk {
        t: &t,
        w: &mut w,
        u: &mut u,
    }
    .cell_walk(P, &mut p, (12 << 16, 0))
    .unwrap();
    assert_eq!(r, Err((super::geom::centre(104), super::geom::centre(100))));
    assert_eq!(p.collided, 0x1);
}

// ---- next-position check §5.1 r5, start (0,0) §3 r3 ----------------

// Covers: specs/sim/pathing.md §3 r3, §3 r6, §3 r9, §5.1 r2, §5.1 r5, §6 r1
#[test]
fn zero_velocity_probe_and_origin_start() {
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    // Velocity 0: P := position, blocked; greedy walk from the start.
    w.paths.get_mut(&P).unwrap().velocity = 0;
    let (n, p) = compute_to(&t, &mut w, &mut u, Point::new(13, 10));
    assert_eq!((n, p.live_points()), (1, pts(&[(13, 10)]).as_slice()));
    // Greedy diagonal then straight: (10,10)→(14,12) gives the corner
    // where the direction changes and the tail end.
    // Greedy from the start to (14,12): octants 22, 24, 23, 18 turn at
    // every step, so each corner is a point and the target itself is not.
    let (n, p) = compute_to(&t, &mut w, &mut u, Point::new(14, 12));
    assert_eq!(
        (n, p.live_points()),
        (3, pts(&[(11, 10), (12, 11), (13, 11)]).as_slice())
    );
    // The unit's own footprint is back after the compute (steps 6, 9).
    assert_eq!(w.value(Point::new(10, 10)) & 0x1080, 0x1080);
    // A unit at (0, 0): no path.
    let (t, mut w, mut u) = setup(40, 40, 0, 0);
    let (n, p) = compute_to(&t, &mut w, &mut u, Point::new(5, 5));
    assert_eq!((n, p.count), (0, 0));
}
