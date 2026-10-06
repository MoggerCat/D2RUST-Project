// Spec: specs/sim/pathing.md
//! Tests written to kill surviving mutants of `cargo mutants` on
//! `crates/d2-sim/src/path/walk/` (METHODS M08; record:
//! `docs/handoff/mutants-path.md`). Each asserts what pathing.md states;
//! the mutants it kills are named in the test's comment. Fakes: the
//! walk tests' `tests/fake.rs`, included again here.

// The fakes of `tests/` are private to that module; this file stays
// separate (parallel sessions edit `tests/`), so it includes them again.
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "tests/fake.rs"]
mod fake;

use fake::{FakeUnit, FakeUnits, FakeWorld, ROOM};

use super::find::{compute, refresh_point, reset_type, set_type};
use super::seams::{flag, PathInfo, Point, TargetUnit, WalkError, WalkPath, WalkUnits};
use super::tables::PathTables;
use crate::rng::Seed;
use crate::units::{RoomId, UnitId, UnitType};

const P: UnitId = UnitId(1);
const M: UnitId = UnitId(2);
const T: UnitId = UnitId(3);

fn tables() -> PathTables {
    PathTables::embedded()
}

fn pts(v: &[(i32, i32)]) -> Vec<Point> {
    v.iter().map(|&(x, y)| Point::new(x, y)).collect()
}

/// A world of `w`×`h` with the player at (x, y) (as the walk tests).
fn setup(w: i32, h: i32, x: i32, y: i32) -> (PathTables, FakeWorld, Units) {
    let t = tables();
    let mut world = FakeWorld::new(w, h);
    world.add_player(&t, P, x, y);
    let mut units = FakeUnits::with_player(P);
    units.unit(P).pos = Point::new(x, y);
    (
        t,
        world,
        Units {
            f: units,
            lead: None,
            door: None,
            in_town: true,
        },
    )
}

/// `FakeUnits` plus the seams it leaves at their defaults: target lead,
/// door orientation, monster-in-town.
struct Units {
    f: FakeUnits,
    /// Offset the lead seam adds to the target (`None`: unspecified).
    lead: Option<Point>,
    door: Option<bool>,
    in_town: bool,
}

impl Units {
    fn add(&mut self, id: UnitId, ty: UnitType, x: i32, y: i32, size: i32) {
        let u = FakeUnit {
            ty,
            guid: id.0,
            pos: Point::new(x, y),
            size,
            ..FakeUnit::player()
        };
        self.f.units.insert(id, u);
    }
}

impl WalkUnits for Units {
    fn unit_type(&self, unit: UnitId) -> UnitType {
        self.f.unit_type(unit)
    }
    fn mode(&self, unit: UnitId) -> u32 {
        self.f.mode(unit)
    }
    fn seed(&mut self, unit: UnitId) -> &mut Seed {
        self.f.seed(unit)
    }
    fn position(&self, unit: UnitId) -> Point {
        self.f.position(unit)
    }
    fn unit_size(&self, unit: UnitId) -> i32 {
        self.f.unit_size(unit)
    }
    fn door_orientation(&self, _unit: UnitId) -> Option<bool> {
        self.door
    }
    fn monster_can_be_in_town(&self, _unit: UnitId) -> bool {
        self.in_town
    }
    fn target_lead(&self, _unit: UnitId, target: Point, _lead: u8) -> Option<Point> {
        self.lead
            .map(|d| Point::new(target.x + d.x, target.y + d.y))
    }
    fn other_path_function(&mut self, _path: &mut WalkPath, _info: &PathInfo) -> i32 {
        0
    }
}

/// Computes `unit`'s path to a point (town access 0).
fn to_point(
    t: &PathTables,
    w: &mut FakeWorld,
    u: &mut Units,
    unit: UnitId,
    target: Point,
) -> (i32, WalkPath) {
    let mut path = w.paths[&unit].clone();
    path.target = target;
    path.target_unit = None;
    let n = compute(t, w, u, &mut path, unit, false).unwrap();
    (n, path)
}

/// Computes the player's path to the unit `id`.
fn to_unit(t: &PathTables, w: &mut FakeWorld, u: &mut Units, id: UnitId) -> (i32, WalkPath) {
    let mut path = w.paths[&P].clone();
    path.target_unit = Some(TargetUnit {
        unit: id,
        ty: u.unit_type(id),
        guid: id.0,
    });
    let n = compute(t, w, u, &mut path, P, false).unwrap();
    (n, path)
}

// ---- §2 set type / reset ----------------------------------------------

// Kills the `set_type` mutants of the player-type-2 test, the 0x2000 /
// 0x4000 and 0x8000 / 0x10000 conditions, the flags merge and the
// asserts.
// Covers: specs/sim/pathing.md §2
#[test]
fn set_type_rules() {
    let t = tables();
    // A player never takes type 2; a monster does.
    let mut p = WalkPath::zeroed(P);
    assert!(set_type(&t, &mut p, UnitType::Player, 2).is_err());
    let mut m = WalkPath::zeroed(M);
    assert_eq!(set_type(&t, &mut m, UnitType::Monster, 2), Ok(()));
    assert_eq!(m.path_type, 2);

    // Type 1 (0x1900, no 0x2000 / 0x8000): previous type and saved
    // velocity stay; non-type flag bits stay.
    let mut p = WalkPath::zeroed(P);
    set_type(&t, &mut p, UnitType::Player, 7).unwrap();
    p.flags |= flag::ACTIVE | flag::OUTSIDE_ROOM;
    p.velocity = 0x800;
    set_type(&t, &mut p, UnitType::Player, 1).unwrap();
    assert_eq!((p.prev_type, p.saved_velocity), (0, 0));
    assert_eq!(p.flags, 0x1900 | flag::ACTIVE | flag::OUTSIDE_ROOM);

    // Type 9 (0x1E800: 0x2000 and 0x8000) on a path without 0x4000 /
    // 0x10000: previous type := 1, saved velocity := velocity.
    set_type(&t, &mut p, UnitType::Player, 9).unwrap();
    assert_eq!((p.path_type, p.prev_type, p.saved_velocity), (9, 1, 0x800));
    assert_eq!(p.flags, 0x1E800 | flag::ACTIVE | flag::OUTSIDE_ROOM);
    // Again, now with 0x4000 and 0x10000: both stay.
    p.velocity = 0x900;
    set_type(&t, &mut p, UnitType::Player, 9).unwrap();
    assert_eq!((p.prev_type, p.saved_velocity), (1, 0x800));

    // A previous type of 8 or 11 is a fatal assert.
    for prev in [8, 11] {
        let mut q = WalkPath::zeroed(P);
        q.prev_type = prev;
        assert!(set_type(&t, &mut q, UnitType::Player, 7).is_err());
    }
    // Type 4 with max distance ≥ 78 is fatal; below 78, or another type
    // at 78, is not.
    let mut q = WalkPath::zeroed(M);
    q.max_distance = 78;
    assert!(set_type(&t, &mut q, UnitType::Monster, 4).is_err());
    let mut q = WalkPath::zeroed(M);
    q.max_distance = 77;
    assert_eq!(set_type(&t, &mut q, UnitType::Monster, 4), Ok(()));
    let mut q = WalkPath::zeroed(P);
    q.max_distance = 78;
    assert_eq!(set_type(&t, &mut q, UnitType::Player, 7), Ok(()));
}

// Kills the `reset_type` mutants: velocity restore on 0x8000, player →
// type 7, others with 0x2000 → previous type, others without → nothing.
// Covers: specs/sim/pathing.md §1.5 r1
#[test]
fn reset_type_rules() {
    let t = tables();
    // Player on type 9 (velocity saved 0x800, changed since).
    let mut p = WalkPath::zeroed(P);
    set_type(&t, &mut p, UnitType::Player, 7).unwrap();
    p.velocity = 0x800;
    set_type(&t, &mut p, UnitType::Player, 9).unwrap();
    p.velocity = 0x1000;
    reset_type(&t, &mut p, UnitType::Player).unwrap();
    assert_eq!((p.path_type, p.velocity), (7, 0x800));
    // Monster on type 9 from type 2: back to type 2.
    let mut m = WalkPath::zeroed(M);
    set_type(&t, &mut m, UnitType::Monster, 2).unwrap();
    m.velocity = 0x600;
    set_type(&t, &mut m, UnitType::Monster, 9).unwrap();
    m.velocity = 0x1000;
    reset_type(&t, &mut m, UnitType::Monster).unwrap();
    assert_eq!((m.path_type, m.velocity), (2, 0x600));
    // Monster on type 2 (no 0x2000, no 0x8000): unchanged.
    let mut m = WalkPath::zeroed(M);
    set_type(&t, &mut m, UnitType::Monster, 2).unwrap();
    m.prev_type = 1;
    m.velocity = 0x700;
    m.saved_velocity = 0x100;
    let before = m.clone();
    reset_type(&t, &mut m, UnitType::Monster).unwrap();
    assert_eq!(m, before);
}

// ---- §3 compute -------------------------------------------------------

// Kills the `refresh_point` mutants: the lead applies only to a player
// or monster target and only when the lead byte ≠ 0.
// Covers: specs/sim/pathing.md §9.5 r3
#[test]
fn refresh_point_lead_only_for_players_and_monsters() {
    let (_, _, mut u) = setup(40, 40, 10, 10);
    u.add(M, UnitType::Monster, 20, 20, 2);
    u.add(T, UnitType::Object, 30, 30, 1);
    u.lead = Some(Point::new(2, 1));
    let mut path = WalkPath::zeroed(P);
    path.lead = 3;
    assert_eq!(refresh_point(&u, &path, M), Point::new(22, 21));
    assert_eq!(refresh_point(&u, &path, P), Point::new(12, 11));
    assert_eq!(refresh_point(&u, &path, T), Point::new(30, 30));
    path.lead = 0;
    assert_eq!(refresh_point(&u, &path, M), Point::new(20, 20));
}

// Kills the deleted player/monster arm and `lead != 0 → ==` in compute's
// target refresh.
// Covers: specs/sim/pathing.md §3 r4
#[test]
fn compute_target_lead() {
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    u.add(M, UnitType::Monster, 20, 20, 0);
    u.lead = Some(Point::new(2, 1));
    let mut path = w.paths[&P].clone();
    path.lead = 3;
    w.paths.insert(P, path);
    let (_, p) = to_unit(&t, &mut w, &mut u, M);
    assert_eq!(p.final_target, Point::new(22, 21));
    let mut path = w.paths[&P].clone();
    path.lead = 0;
    w.paths.insert(P, path);
    let (_, p) = to_unit(&t, &mut w, &mut u, M);
    assert_eq!(p.final_target, Point::new(20, 20));
}

// Kills the deleted object arm and the door shift mutants: orientation
// set → y ± 2, else x ± 2; − when the unit's coordinate is smaller.
// Covers: specs/sim/pathing.md §3 r4
#[test]
fn compute_door_target_shift() {
    let cases = [
        (true, (20, 10), (20, 18)),
        (true, (20, 30), (20, 22)),
        (true, (10, 20), (20, 22)),
        (false, (10, 20), (18, 20)),
        (false, (30, 20), (22, 20)),
        (false, (20, 10), (22, 20)),
    ];
    for (orient, (sx, sy), (ex, ey)) in cases {
        let (t, mut w, mut u) = setup(40, 40, sx, sy);
        u.add(T, UnitType::Object, 20, 20, 0);
        u.door = Some(orient);
        let (_, p) = to_unit(&t, &mut w, &mut u, T);
        assert_eq!(p.final_target, Point::new(ex, ey), "{orient} ({sx}, {sy})");
    }
}

// Kills the deleted item arm (slack 2) and `index < count → <=` of step
// 10. Item target T = (13, 11), start (10, 10): the ray's first test
// (11, 10) collides (wall (12, 10) in its plus), so P = start, and
// dist(P, T) = dist8_path[3 + 8] + 1 = 2 ≤ slack 2 → points = [start].
// Straight: L = start → A*: every probe around T is walled → 0; toward
// again → [start]. Step 10 drops the point equal to the position → no
// point remains → step 11: result 0.
// Covers: specs/sim/pathing.md §3 r4, §3 r10, §5.2 r2, §6 r3, §7 r1
#[test]
fn compute_item_slack_and_no_point_left() {
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    w.wall(12, 10);
    for (dx, dy) in [
        (-2, -2),
        (-2, 2),
        (2, -2),
        (2, 2),
        (-2, 0),
        (0, -2),
        (2, 0),
        (0, 2),
    ] {
        w.wall(13 + dx, 11 + dy);
    }
    u.add(T, UnitType::Item, 13, 11, 1);
    let mut path = w.paths[&P].clone();
    path.flags |= flag::ACTIVE;
    w.paths.insert(P, path);
    let (n, p) = to_unit(&t, &mut w, &mut u, T);
    assert_eq!((n, p.index, p.count), (0, 0, 0));
    assert_eq!(p.flags & flag::ACTIVE, 0);

    // The same ray with T = (16, 12) and a wall at (15, 11): P = (13, 11),
    // dist (3, 1) = 2 ≤ slack 2 → points = [P].
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    w.wall(15, 11);
    u.add(T, UnitType::Item, 16, 12, 1);
    let (n, p) = to_unit(&t, &mut w, &mut u, T);
    assert_eq!(n, 1);
    assert_eq!(p.live_points(), pts(&[(13, 11)]).as_slice());
}

// Kills the range-check mutants (`> → ==`, `> → >=`, `- → /`): 100
// sub-tiles per axis is in range, 101 is not.
// Covers: specs/sim/pathing.md §3 r5, §edge-cases-original-bugs r10
#[test]
fn compute_range_is_100_per_axis() {
    for (target, want) in [((5, 105), 1), ((5, 106), 0), ((105, 5), 1), ((106, 5), 0)] {
        let (t, mut w, mut u) = setup(120, 120, 5, 5);
        let (n, p) = to_point(&t, &mut w, &mut u, P, Point::new(target.0, target.1));
        assert_eq!(n, want, "{target:?}");
        assert_eq!(p.count, want);
    }
}

// Kills `|| → &&` of step 5 and `&= → |=` of step 11: a target (0, 0)
// ends the compute with flag 0x20 cleared and every other flag kept.
// Covers: specs/sim/pathing.md §3 r5, §3 r11, §3 r12
#[test]
fn compute_zero_target_clears_active_only() {
    let (t, mut w, mut u) = setup(40, 40, 3, 3);
    let mut path = w.paths[&P].clone();
    path.flags |= flag::ACTIVE;
    w.paths.insert(P, path);
    let (n, p) = to_point(&t, &mut w, &mut u, P, Point::new(0, 0));
    assert_eq!((n, p.index, p.count), (0, 0, 0));
    assert_eq!(p.flags, 0x21900);
}

// Kills `&= → |=` and `delete !` of step 12 (`Some(false)`): W5, where
// the preparation reaches the start, clears only flag 0x20.
// Covers: specs/sim/pathing.md §3 r12
#[test]
fn compute_no_point_clears_active_only() {
    let (t, mut w, mut u) = setup(40, 40, 13, 10);
    for y in 5..=15 {
        w.wall(15, y);
    }
    let mut path = w.paths[&P].clone();
    path.flags |= flag::ACTIVE;
    w.paths.insert(P, path);
    let (n, p) = to_point(&t, &mut w, &mut u, P, Point::new(15, 10));
    assert_eq!(n, 0);
    assert_eq!(p.flags, 0x21900);
}

/// A monster path as `path-placement.md` §2.4 allocates it (pattern 1,
/// footprint 0x100, move mask 0x3C01, type 2, max distance 14).
fn add_monster(t: &PathTables, w: &mut FakeWorld, u: &mut Units, x: i32, y: i32) {
    let mut p = WalkPath::zeroed(M);
    p.size = 2;
    p.pattern = 1;
    p.precise_x = ((x as u32) << 16) | 0x8000;
    p.precise_y = ((y as u32) << 16) | 0x8000;
    p.velocity = 0x800;
    p.room = Some(ROOM);
    p.footprint_mask = 0x100;
    p.move_mask = 0x3C01;
    set_type(t, &mut p, UnitType::Monster, 2).unwrap();
    p.max_distance = 14;
    w.stamp(1, Point::new(x, y), 0x100, true);
    w.paths.insert(M, p);
    u.add(M, UnitType::Monster, x, y, 2);
}

// Kills the town-access mutants of step 5: only a monster that cannot be
// in town, with town access 0, is refused a town target room.
// Covers: specs/sim/pathing.md §3 r5
#[test]
fn compute_town_access() {
    let target = Point::new(15, 10);
    // (town room, monster may be in town, town access) → count.
    for (town, may, access, want) in [
        (true, false, false, 0),
        (true, false, true, 1),
        (true, true, false, 1),
        (false, false, false, 1),
    ] {
        let (t, mut w, mut u) = setup(40, 40, 10, 30);
        add_monster(&t, &mut w, &mut u, 10, 10);
        w.town = town;
        u.in_town = may;
        let mut path = w.paths[&M].clone();
        path.target = target;
        let n = compute(&t, &mut w, &mut u, &mut path, M, access).unwrap();
        assert_eq!(n, want, "town {town} may {may} access {access}");
    }
    // A player is never refused.
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    w.town = true;
    u.in_town = false;
    assert_eq!(to_point(&t, &mut w, &mut u, P, target).0, 1);
}

// Kills the step-6 mutants: the target unit's footprint is removed while
// computing only with a non-zero size and path flag 0x800; it is put
// back after.
// Covers: specs/sim/pathing.md §3 r6, §3 r9
#[test]
fn compute_removes_target_footprint() {
    // Monster target at (20, 10): its NO_PATH marker collides with the
    // player's move mask unless removed.
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    add_monster(&t, &mut w, &mut u, 20, 10);
    let (n, p) = to_unit(&t, &mut w, &mut u, M);
    assert_eq!(p.final_target, Point::new(20, 10));
    assert_eq!(n, 1);
    assert_eq!(w.value(Point::new(20, 10)) & 0x1100, 0x1100);
    // Size 0: not removed → the target collides and is prepared.
    u.f.unit(M).size = 0;
    let (_, p) = to_unit(&t, &mut w, &mut u, M);
    assert_ne!(p.final_target, Point::new(20, 10));
    // Flag 0x800 clear: not removed either.
    u.f.unit(M).size = 2;
    let mut path = w.paths[&P].clone();
    path.flags &= !flag::REMOVE_TARGET_FOOTPRINT;
    w.paths.insert(P, path);
    let (_, p) = to_unit(&t, &mut w, &mut u, M);
    assert_ne!(p.final_target, Point::new(20, 10));
}

// Kills `& → |` / `& → ^` of step 7: without flag 0x1000 a blocked
// target is not prepared (W4's grid; prepared it is (13, 10)).
// Covers: specs/sim/pathing.md §3 r7
#[test]
fn compute_prepares_only_with_flag_0x1000() {
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    for y in 5..=15 {
        w.wall(15, y);
    }
    let mut path = w.paths[&P].clone();
    path.flags &= !flag::PREPARE_TARGET;
    w.paths.insert(P, path);
    let (_, p) = to_point(&t, &mut w, &mut u, P, Point::new(15, 10));
    assert_eq!(p.final_target, Point::new(15, 10));
}

// Kills `&& → ||` of step 10: with flag 0x10 set the target stays (W3's
// grid: the only point is (28, 20)).
// Covers: specs/sim/pathing.md §3 r10
#[test]
fn compute_keep_target_flag() {
    let (t, mut w, mut u) = setup(60, 60, 20, 20);
    for y in 0..=49 {
        w.wall(30, y);
    }
    let mut path = w.paths[&P].clone();
    path.flags |= flag::KEEP_TARGET;
    w.paths.insert(P, path);
    let (n, p) = to_point(&t, &mut w, &mut u, P, Point::new(40, 20));
    assert_eq!(n, 1);
    assert_eq!(p.live_points(), pts(&[(28, 20)]).as_slice());
    assert_eq!(p.target, Point::new(40, 20));
}

// Kills the `room_exit_flag` mutants: flag 0x1 when a point lies outside
// the path room's sub-tile rect (half-open), every other flag kept.
// Covers: specs/sim/pathing.md §3 r10
#[test]
fn compute_room_exit_flag() {
    for ((x, y), out) in [
        ((10, 20), false),
        ((9, 20), true),
        ((20, 10), false),
        ((20, 9), true),
        ((29, 20), false),
        ((30, 20), true),
        ((20, 29), false),
        ((20, 30), true),
    ] {
        let (t, mut w, mut u) = setup(40, 40, 20, 20);
        w.room0 = (10, 10, 20, 20);
        w.rooms.insert(RoomId(1), (0, 0, 40, 40));
        let (n, p) = to_point(&t, &mut w, &mut u, P, Point::new(x, y));
        assert_eq!(n, 1, "({x}, {y})");
        let want = 0x21900 | flag::ACTIVE | if out { flag::OUTSIDE_ROOM } else { 0 };
        assert_eq!(p.flags, want, "({x}, {y})");
    }
}

// Kills the deleted A* arm of `run_function`: type 1 runs A* (W2's grid
// and A* points).
// Covers: specs/sim/pathing.md §2, §7 r6
#[test]
fn compute_type_1_runs_astar() {
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    for y in 5..=15 {
        w.wall(15, y);
    }
    let mut path = w.paths[&P].clone();
    set_type(&t, &mut path, UnitType::Player, 1).unwrap();
    w.paths.insert(P, path);
    let (n, p) = to_point(&t, &mut w, &mut u, P, Point::new(20, 10));
    assert_eq!(n, 6);
    assert_eq!(
        p.live_points(),
        pts(&[(13, 13), (13, 15), (15, 17), (17, 15), (17, 13), (20, 10)]).as_slice()
    );
}

// Kills the deleted `4 | 10 | 14 | 17` arm: type 17 has no function and
// is fatal when computed.
// Covers: specs/sim/pathing.md §2
#[test]
fn compute_type_17_is_fatal() {
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    let mut path = w.paths[&P].clone();
    set_type(&t, &mut path, UnitType::Player, 17).unwrap();
    path.target = Point::new(20, 10);
    assert!(matches!(
        compute(&t, &mut w, &mut u, &mut path, P, false),
        Err(WalkError::Fatal(_))
    ));
}

// ---- §4 preparation and push ------------------------------------------

/// The player's prepared target toward `target` with walls `walls`.
fn prepared(walls: &[(i32, i32)], target: (i32, i32)) -> Point {
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    for &(x, y) in walls {
        w.wall(x, y);
    }
    to_point(&t, &mut w, &mut u, P, Point::new(target.0, target.1))
        .1
        .final_target
}

// Kills the `push` mutants and `owner == player → !=` of `prepare`.
// Target (13, 10) walled with (11, 10), (12, 10): o = 2, altdir (4, 2,
// 6). Pass 1: p0 (12, 10), p1 (13, 11), p2 (13, 9) collide; d0 :=
// altdir[o((12, 10) → start)] = 4. Pass 2: p0 (11, 10) collides, p1
// (13, 12) is free. Push: (dx, dy) = (3, 2), ox = snap9[61] = 1, oy =
// snap9[69] = 0, c = 3 → (14, 12), (15, 12), c = 5 stops.
// Covers: specs/sim/pathing.md §4 r2, §4 r3, §4 r4
#[test]
fn push_moves_the_prepared_target() {
    assert_eq!(
        prepared(&[(11, 10), (12, 10), (13, 10)], (13, 10)),
        Point::new(15, 12)
    );
}

// Kills `< 5 → <=` on both axes and the `&& → ||` of the push test: at
// |dx| = 5 or |dy| = 5 there is no push.
// Covers: specs/sim/pathing.md §4 r4
#[test]
fn push_needs_both_axes_below_5() {
    // (15, 10) walled with (13, 10), (14, 10): found p1 (15, 12), dx = 5.
    assert_eq!(
        prepared(&[(13, 10), (14, 10), (15, 10)], (15, 10)),
        Point::new(15, 12)
    );
    // (10, 15) walled with (10, 13), (10, 14): o = 10, altdir (6, 0, 4);
    // found p1 (12, 15), dy = 5.
    assert_eq!(
        prepared(&[(10, 13), (10, 14), (10, 15)], (10, 15)),
        Point::new(12, 15)
    );
}

// Kills `d2 != 255 → ==` of `prepare`: the third probe finds (13, 8)
// in pass 2 (p1's (13, 12) is walled); push (3, −2): ox = snap9[25] = 1,
// oy = snap9[65] = 0 → (15, 8).
// Covers: specs/sim/pathing.md §4 r2, §4 r4
#[test]
fn third_probe_finds_the_target() {
    assert_eq!(
        prepared(&[(11, 10), (12, 10), (13, 10), (13, 12)], (13, 10)),
        Point::new(15, 8)
    );
}

// ---- §5 toward --------------------------------------------------------

/// A monster's type-2 (toward) path from (10, 10) to `target` with walls;
/// the player stands out of the way.
fn monster_toward(walls: &[(i32, i32)], target: (i32, i32)) -> (i32, Vec<Point>) {
    let (t, mut w, mut u) = setup(40, 40, 35, 35);
    add_monster(&t, &mut w, &mut u, 10, 10);
    for &(x, y) in walls {
        w.wall(x, y);
    }
    let (n, p) = to_point(&t, &mut w, &mut u, M, Point::new(target.0, target.1));
    (n, p.live_points().to_vec())
}

// Kills `steps += 1 → *=` and `(tail || !turned) → &&` of §5.2 step 5:
// the walk reaches the target on a straight step (no turn in the last
// iteration), so the target is appended. Ray (10, 10) → (20, 12) stops
// before (12, 10) (wall (13, 10) in its plus): P = (11, 10). Greedy:
// 1 → (12, 11) (turn: P again, edge case 3), 1 → (13, 12), 0 → (14, 12)
// (turn: (13, 12)), 0 … → (20, 12) = target, last step straight.
// Covers: specs/sim/pathing.md §5.2 r3, §5.2 r4, §5.2 r5
#[test]
fn toward_appends_target_after_straight_run() {
    let (n, p) = monster_toward(&[(13, 10)], (20, 12));
    assert_eq!(n, 4);
    assert_eq!(p, pts(&[(11, 10), (11, 10), (13, 12), (20, 12)]));
    // Target (20, 10): from (12, 11) every candidate collides → tail with
    // one step taken → (12, 11) appended.
    let (n, p) = monster_toward(&[(13, 10)], (20, 10));
    assert_eq!(n, 3);
    assert_eq!(p, pts(&[(11, 10), (11, 10), (12, 11)]));
}

// Kills the mutants of the reverse test `prev ≠ 255 && (d − 4) & 7 =
// prev` (§5.2 step 4.2): at (17, 18) the free candidate 5 is the reverse
// of prev 1 → tail, (16, 19)… The walk: P = (13, 14) (ray blocked),
// greedy 1 to (17, 18) (P again on the turn), 3 to (16, 19), then 7…
// Covers: specs/sim/pathing.md §5.2 r4, §5.2 r5
#[test]
fn toward_reverse_step_ends_the_walk() {
    let (n, p) = monster_toward(&[(13, 16), (15, 18)], (16, 18));
    assert_eq!(n, 4);
    assert_eq!(p, pts(&[(13, 14), (13, 14), (17, 18), (16, 19)]));
}

// Kills `k == 2 → !=` of §5.2 step 4.2: t2 = 255 is a fatal assert.
// Covers: specs/sim/pathing.md §5.2 r4
#[test]
fn toward_testdir_t2_255_is_fatal() {
    let mut t = tables();
    // Every row: t0 and t1 blocked below, t2 = 255.
    for row in t.testdir.iter_mut() {
        row[2] = 255;
    }
    let (_, mut w, mut u) = setup(40, 40, 35, 35);
    add_monster(&t, &mut w, &mut u, 10, 10);
    // P = (11, 10); from there 0 and 1 (row 22: 0, 1, 255) collide.
    for (x, y) in [(13, 10), (12, 12)] {
        w.wall(x, y);
    }
    let mut path = w.paths[&M].clone();
    path.target = Point::new(20, 10);
    assert!(matches!(
        compute(&t, &mut w, &mut u, &mut path, M, false),
        Err(WalkError::Fatal(_))
    ));
}

// ---- §6 straight ------------------------------------------------------

// Kills `n > 0 → >=` of straight: toward returns 0 (the player is boxed
// in), A* finds nothing → result 0.
// Covers: specs/sim/pathing.md §6 r2, §6 r4
#[test]
fn straight_with_no_toward_point() {
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    for (dx, dy) in [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ] {
        w.wall(10 + dx, 10 + dy);
    }
    let (n, p) = to_point(&t, &mut w, &mut u, P, Point::new(20, 10));
    assert_eq!((n, p.count), (0, 0));
}

// Kills the radius mutants of straight (`dx² + dy² ≤ 324`): at d² = 400
// (on either axis) A* does not run and toward's point stays.
// Covers: specs/sim/pathing.md §6 r3, §6 r4
#[test]
fn straight_radius_on_both_axes() {
    // Wall x = 15, y 5..15 (W2): P = (13, 10); greedy blocked at once.
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    for y in 5..=15 {
        w.wall(15, y);
    }
    let (n, p) = to_point(&t, &mut w, &mut u, P, Point::new(30, 10));
    assert_eq!(n, 1);
    assert_eq!(p.live_points(), pts(&[(13, 10)]).as_slice());
    // The same rotated: wall y = 15, x 5..15; target (10, 30).
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    for x in 5..=15 {
        w.wall(x, 15);
    }
    let (n, p) = to_point(&t, &mut w, &mut u, P, Point::new(10, 30));
    assert_eq!(n, 1);
    assert_eq!(p.live_points(), pts(&[(10, 13)]).as_slice());
}

// ---- §7 A* ------------------------------------------------------------

/// A type-1 (A*) player path from (10, 10) on a 30×30 grid with walls.
fn astar_path(walls: &[(i32, i32)], target: (i32, i32)) -> (i32, Vec<Point>) {
    let (t, mut w, mut u) = setup(30, 30, 10, 10);
    for &(x, y) in walls {
        w.wall(x, y);
    }
    let mut path = w.paths[&P].clone();
    set_type(&t, &mut path, UnitType::Player, 1).unwrap();
    w.paths.insert(P, path);
    let (n, p) = to_point(&t, &mut w, &mut u, P, Point::new(target.0, target.1));
    (n, p.live_points().to_vec())
}

// Kills the A* mutants of the best-node rule (`h <`, `h =`, `g > g(best)
// + 5`), the open / closed branches, the children and the propagation.
// Expected points: the rules of §7 run by hand-checked model (scratch
// `astar.py`, recorded in docs/handoff/mutants-path.md).
// Covers: specs/sim/pathing.md §7 r2, §7 r3, §7 r4, §7 r5, §7 r6
#[test]
fn astar_best_node_and_propagation_a() {
    let walls = [
        (6, 14),
        (6, 25),
        (7, 14),
        (9, 17),
        (9, 18),
        (11, 24),
        (12, 18),
        (12, 19),
        (12, 20),
        (12, 21),
        (12, 22),
        (13, 8),
        (13, 9),
        (13, 10),
        (13, 11),
        (13, 12),
        (13, 13),
        (13, 14),
        (13, 24),
        (14, 17),
        (14, 24),
        (15, 17),
        (15, 24),
        (16, 17),
        (16, 24),
        (17, 17),
        (17, 24),
        (18, 17),
        (19, 17),
        (20, 17),
        (21, 17),
    ];
    let (n, p) = astar_path(&walls, (15, 19));
    assert_eq!(n, 2);
    assert_eq!(p, pts(&[(10, 13), (13, 16)]));
}

// Covers: specs/sim/pathing.md §7 r3, §7 r5, §7 r6
#[test]
fn astar_open_update_neighbour_order_and_steps() {
    let walls = [
        (5, 18),
        (6, 18),
        (6, 23),
        (6, 24),
        (7, 18),
        (13, 12),
        (14, 12),
        (15, 7),
        (15, 12),
        (16, 7),
        (16, 12),
        (17, 7),
        (17, 12),
        (18, 7),
        (18, 12),
        (18, 14),
        (19, 7),
        (19, 14),
        (20, 7),
        (20, 14),
        (21, 7),
        (21, 14),
        (21, 19),
        (21, 20),
        (21, 21),
        (21, 22),
        (21, 23),
        (22, 14),
        (23, 14),
        (25, 9),
        (26, 9),
        (27, 9),
    ];
    let (n, p) = astar_path(&walls, (23, 22));
    assert_eq!(n, 9);
    assert_eq!(
        p,
        pts(&[
            (11, 11),
            (11, 12),
            (13, 14),
            (15, 14),
            (19, 18),
            (20, 18),
            (21, 17),
            (23, 19),
            (23, 22)
        ])
    );
}

// Covers: specs/sim/pathing.md §7 r5, §7 r6
#[test]
fn astar_best_node_and_propagation_b() {
    let walls = [
        (7, 17),
        (13, 8),
        (13, 15),
        (13, 16),
        (13, 17),
        (13, 18),
        (16, 7),
        (17, 7),
        (17, 13),
        (18, 7),
        (18, 20),
        (19, 7),
        (19, 14),
        (19, 15),
        (19, 16),
        (19, 17),
        (19, 18),
        (19, 19),
        (19, 20),
        (19, 21),
        (19, 25),
        (20, 13),
        (20, 20),
        (20, 25),
        (21, 13),
        (21, 16),
        (21, 20),
        (22, 13),
        (22, 16),
        (22, 20),
        (23, 13),
        (23, 16),
        (24, 13),
        (24, 16),
        (25, 13),
        (25, 14),
        (25, 16),
        (26, 13),
        (26, 14),
        (26, 16),
        (27, 14),
        (27, 16),
    ];
    let (n, p) = astar_path(&walls, (24, 18));
    assert_eq!(n, 3);
    assert_eq!(p, pts(&[(16, 10), (17, 11), (24, 11)]));
}

// Covers: specs/sim/pathing.md §7 r5, §7 r6
#[test]
fn astar_propagation_updates_f() {
    let walls = [
        (11, 8),
        (12, 8),
        (16, 20),
        (16, 21),
        (16, 22),
        (16, 23),
        (16, 24),
        (16, 25),
        (21, 20),
        (21, 21),
        (21, 22),
        (21, 23),
        (23, 25),
    ];
    let (n, p) = astar_path(&walls, (18, 23));
    assert_eq!(n, 8);
    assert_eq!(
        p,
        pts(&[
            (13, 13),
            (14, 13),
            (15, 14),
            (15, 16),
            (17, 18),
            (17, 19),
            (18, 20),
            (18, 23)
        ])
    );
}

// Kills `n == 0 || n ≥ 78 → &&` of §7 rule 6: a corridor whose every
// step turns gives 78 outputs → result 0; one step shorter (77) is a
// path. Pattern 0 (a size-0 unit), so the corridor is one cell wide.
// Covers: specs/sim/pathing.md §7 r6
#[test]
fn astar_78_outputs_is_no_path() {
    for (steps, want) in [(78, 0), (77, 77)] {
        let (t, mut w, mut u) = setup(100, 60, 10, 10);
        // Wall everything, then open the staircase (1,0), (1,1), (1,0)…
        for y in 0..60 {
            for x in 0..100 {
                w.wall(x, y);
            }
        }
        let mut cells = vec![(10, 10)];
        for k in 0..steps {
            let (x, y) = *cells.last().unwrap();
            cells.push(if k % 2 == 0 {
                (x + 1, y)
            } else {
                (x + 1, y + 1)
            });
        }
        for &(x, y) in &cells {
            w.grid[(y * 100 + x) as usize] = 0;
        }
        let mut path = w.paths[&P].clone();
        set_type(&t, &mut path, UnitType::Player, 1).unwrap();
        path.pattern = 0;
        w.paths.insert(P, path);
        let (x, y) = *cells.last().unwrap();
        let (n, _) = to_point(&t, &mut w, &mut u, P, Point::new(x, y));
        assert_eq!(n, want, "{steps} steps");
    }
}

// Kills the deleted `-` of the target probes and `&& → ||` / `+ → *` of
// §7 rule 1: with a target unit, A* runs when one probe is free (each
// probe in turn), and returns 0 only when all eight collide; without a
// target unit the probes are not tested.
// Covers: specs/sim/pathing.md §7 r1
#[test]
fn astar_target_probes() {
    const PROBES: [(i32, i32); 8] = [
        (-2, -2),
        (-2, 2),
        (2, -2),
        (2, 2),
        (-2, 0),
        (0, -2),
        (2, 0),
        (0, 2),
    ];
    // Target unit (monster, size 0) at (20, 20); the start (10, 10).
    let run = |free: Option<usize>, with_unit: bool| {
        let (t, mut w, mut u) = setup(40, 40, 10, 10);
        for (i, (dx, dy)) in PROBES.iter().enumerate() {
            if Some(i) != free {
                w.wall(20 + dx, 20 + dy);
            }
        }
        u.add(M, UnitType::Monster, 20, 20, 0);
        let mut path = w.paths[&P].clone();
        set_type(&t, &mut path, UnitType::Player, 1).unwrap();
        w.paths.insert(P, path);
        if with_unit {
            to_unit(&t, &mut w, &mut u, M).0
        } else {
            to_point(&t, &mut w, &mut u, P, Point::new(20, 20)).0
        }
    };
    assert_eq!(run(None, true), 0);
    for i in 0..PROBES.len() {
        assert_ne!(run(Some(i), true), 0, "probe {i} free");
    }
    assert_ne!(run(None, false), 0);
}

// ---- §5.1 helpers -----------------------------------------------------

// Kills `dx < 0 → <=`, `dy < −1 → <=` and the deleted `−1` of the
// octant: the §5.1 rule 1 values for every (dx, dy) in −4..4.
// Covers: specs/sim/pathing.md §5.1 r1
#[test]
fn octant_table() {
    // Rows dy = −4..4, columns dx = −4..4 (§5.1 rule 1, worked by a
    // model of the rule text; "7 + clamp(dy)" clamps to [−2, 2] as
    // "dy < −1 gives o = 5" implies).
    const O: [[usize; 9]; 9] = [
        [0, 0, 5, 5, 10, 15, 10, 20, 20],
        [0, 0, 0, 5, 10, 15, 20, 20, 20],
        [1, 0, 0, 5, 10, 15, 20, 20, 21],
        [1, 1, 1, 6, 11, 16, 21, 21, 21],
        [2, 2, 2, 7, 12, 17, 22, 22, 22],
        [3, 3, 3, 8, 13, 18, 23, 23, 23],
        [2, 4, 4, 9, 14, 19, 24, 24, 22],
        [4, 4, 4, 9, 14, 19, 24, 24, 24],
        [4, 4, 9, 9, 14, 19, 14, 24, 24],
    ];
    for (r, row) in O.iter().enumerate() {
        for (c, &want) in row.iter().enumerate() {
            let q = Point::new(c as i32 - 4, r as i32 - 4);
            assert_eq!(
                super::geom::octant(Point::new(0, 0), q),
                want,
                "({}, {})",
                q.x,
                q.y
            );
        }
    }
}

// Kills `ay < 8 → <=` and `v < 0 → ==` / `<=` of the path distance.
// Covers: specs/sim/pathing.md §5.1 r3
#[test]
fn path_distance_rules() {
    let t = tables();
    let d = |x: i32, y: i32| super::geom::path_distance(&t, Point::new(0, 0), Point::new(x, y));
    // dist8_path[3] = 0 → 1; [1] = −1 → 0; [1 + 8] = −1 → 0.
    assert_eq!((d(3, 0), d(1, 0), d(1, 1)), (1, 0, 0));
    // Either axis ≥ 8: 2·max + min.
    assert_eq!((d(0, 8), d(8, 0), d(8, 3), d(3, 9)), (16, 16, 19, 21));
}

// ---- §9.5 unit distance -----------------------------------------------

// Kills the `unit_distance` mutants: the table branch (both Δ < 8, both
// sizes < 4; size 3 subtracts 1; a size < 2 adds 1) and the box branch.
// Values: §9.5 by a model of the rule text over `dist8_unit`.
// Covers: specs/sim/pathing.md §9.5
#[test]
fn unit_distance_rules() {
    let t = tables();
    // (a, size a, b, size b, distance).
    let cases = [
        ((14, 10), 2, (10, 10), 2, 2),
        ((13, 10), 2, (10, 10), 2, 0),
        ((14, 10), 1, (10, 10), 2, 3),
        ((14, 10), 2, (10, 10), 1, 3),
        ((14, 10), 3, (10, 10), 2, 1),
        ((14, 10), 2, (10, 10), 3, 1),
        ((11, 10), 3, (10, 10), 3, 0),
        ((14, 10), 1, (10, 10), 3, 2),
        ((18, 10), 2, (10, 10), 2, 12),
        ((14, 13), 4, (10, 10), 2, 2),
        ((14, 13), 2, (10, 10), 4, 2),
        ((10, 18), 1, (10, 10), 1, 16),
        ((16, 11), 2, (10, 10), 2, 6),
        ((11, 16), 2, (10, 10), 2, 6),
        ((10, 10), 2, (16, 13), 2, 7),
        ((10, 18), 2, (10, 10), 2, 12),
        ((19, 13), 2, (10, 10), 3, 15),
        ((14, 10), 1, (10, 10), 1, 3),
    ];
    for ((ax, ay), sa, (bx, by), sb, want) in cases {
        let d = super::geom::unit_distance(&t, Point::new(ax, ay), sa, Point::new(bx, by), sb);
        assert_eq!(d, want, "({ax}, {ay}) {sa} ({bx}, {by}) {sb}");
    }
}

// ---- §5.1 rule 4 ray test ---------------------------------------------

/// A probe that records every tested cell and blocks the cells listed.
struct Recorder {
    blocked: Vec<Point>,
    tested: std::cell::RefCell<Vec<Point>>,
}

impl super::geom::Probe for Recorder {
    fn collides(&self, _room: Option<RoomId>, p: Point, _pattern: u8, _mask: u16) -> bool {
        self.tested.borrow_mut().push(p);
        self.blocked.contains(&p)
    }
}

// Kills the `ray_test` mutants (signs, the three branches, the error
// steps, the minor-step test only when err > 0): from (10, 10) to each
// end, the cells tested in order on a clear ray, and for each of them
// blocked alone, the returned cell. Values: §5.1 rule 4 by a model of the
// rule text.
// Covers: specs/sim/pathing.md §5.1 r4, §edge-cases-original-bugs r4
#[test]
fn ray_test_cells_and_blocks() {
    use super::geom::{ray_test, Ray};
    fn p(x: i32, y: i32) -> Point {
        Point::new(x, y)
    }
    #[allow(clippy::type_complexity)]
    let cases: [(Point, &[Point], &[Point]); 21] = [
        (
            p(15, 12),
            &[p(11, 10), p(12, 11), p(13, 11), p(14, 12), p(15, 12)],
            &[p(10, 10), p(11, 11), p(12, 11), p(13, 12), p(14, 12)],
        ),
        (
            p(5, 12),
            &[p(9, 10), p(8, 11), p(7, 11), p(6, 12), p(5, 12)],
            &[p(10, 10), p(9, 11), p(8, 11), p(7, 12), p(6, 12)],
        ),
        (
            p(15, 8),
            &[p(11, 10), p(12, 9), p(13, 9), p(14, 8), p(15, 8)],
            &[p(10, 10), p(11, 9), p(12, 9), p(13, 8), p(14, 8)],
        ),
        (
            p(5, 8),
            &[p(9, 10), p(8, 9), p(7, 9), p(6, 8), p(5, 8)],
            &[p(10, 10), p(9, 9), p(8, 9), p(7, 8), p(6, 8)],
        ),
        (
            p(12, 15),
            &[p(10, 11), p(11, 12), p(11, 13), p(12, 14), p(12, 15)],
            &[p(10, 10), p(11, 11), p(11, 12), p(12, 13), p(12, 14)],
        ),
        (
            p(8, 15),
            &[p(10, 11), p(9, 12), p(9, 13), p(8, 14), p(8, 15)],
            &[p(10, 10), p(9, 11), p(9, 12), p(8, 13), p(8, 14)],
        ),
        (
            p(12, 5),
            &[p(10, 9), p(11, 8), p(11, 7), p(12, 6), p(12, 5)],
            &[p(10, 10), p(11, 9), p(11, 8), p(12, 7), p(12, 6)],
        ),
        (
            p(8, 5),
            &[p(10, 9), p(9, 8), p(9, 7), p(8, 6), p(8, 5)],
            &[p(10, 10), p(9, 9), p(9, 8), p(8, 7), p(8, 6)],
        ),
        (
            p(13, 13),
            &[p(11, 11), p(12, 12), p(13, 13)],
            &[p(10, 10), p(11, 11), p(12, 12)],
        ),
        (
            p(7, 7),
            &[p(9, 9), p(8, 8), p(7, 7)],
            &[p(10, 10), p(9, 9), p(8, 8)],
        ),
        (
            p(13, 7),
            &[p(11, 9), p(12, 8), p(13, 7)],
            &[p(10, 10), p(11, 9), p(12, 8)],
        ),
        (
            p(7, 13),
            &[p(9, 11), p(8, 12), p(7, 13)],
            &[p(10, 10), p(9, 11), p(8, 12)],
        ),
        (
            p(14, 10),
            &[p(11, 10), p(12, 10), p(13, 10), p(14, 10)],
            &[p(10, 10), p(11, 10), p(12, 10), p(13, 10)],
        ),
        (
            p(10, 14),
            &[p(10, 11), p(10, 12), p(10, 13), p(10, 14)],
            &[p(10, 10), p(10, 11), p(10, 12), p(10, 13)],
        ),
        (
            p(6, 10),
            &[p(9, 10), p(8, 10), p(7, 10), p(6, 10)],
            &[p(10, 10), p(9, 10), p(8, 10), p(7, 10)],
        ),
        (
            p(10, 6),
            &[p(10, 9), p(10, 8), p(10, 7), p(10, 6)],
            &[p(10, 10), p(10, 9), p(10, 8), p(10, 7)],
        ),
        (p(10, 10), &[], &[]),
        (
            p(16, 13),
            &[
                p(11, 10),
                p(11, 11),
                p(12, 11),
                p(13, 11),
                p(13, 12),
                p(14, 12),
                p(15, 12),
                p(15, 13),
                p(16, 13),
            ],
            &[
                p(10, 10),
                p(10, 10),
                p(11, 11),
                p(12, 11),
                p(12, 11),
                p(13, 12),
                p(14, 12),
                p(14, 12),
                p(15, 13),
            ],
        ),
        (
            p(13, 16),
            &[
                p(10, 11),
                p(11, 11),
                p(11, 12),
                p(11, 13),
                p(12, 13),
                p(12, 14),
                p(12, 15),
                p(13, 15),
                p(13, 16),
            ],
            &[
                p(10, 10),
                p(10, 10),
                p(11, 11),
                p(11, 12),
                p(11, 12),
                p(12, 13),
                p(12, 14),
                p(12, 14),
                p(13, 15),
            ],
        ),
        (
            p(14, 11),
            &[p(11, 10), p(12, 10), p(12, 11), p(13, 11), p(14, 11)],
            &[p(10, 10), p(11, 10), p(11, 10), p(12, 11), p(13, 11)],
        ),
        (
            p(11, 14),
            &[p(10, 11), p(10, 12), p(11, 12), p(11, 13), p(11, 14)],
            &[p(10, 10), p(10, 11), p(10, 11), p(11, 12), p(11, 13)],
        ),
    ];
    let s = p(10, 10);
    for (e, tested, blocks) in cases {
        let r = Recorder {
            blocked: Vec::new(),
            tested: Default::default(),
        };
        assert_eq!(ray_test(&r, None, 1, 1, s, e), Ray::Clear, "{e:?}");
        assert_eq!(r.tested.borrow().as_slice(), tested, "{e:?}");
        for (&c, &want) in tested.iter().zip(blocks) {
            let r = Recorder {
                blocked: vec![c],
                tested: Default::default(),
            };
            assert_eq!(
                ray_test(&r, None, 1, 1, s, e),
                Ray::Blocked(want),
                "{e:?} {c:?}"
            );
        }
    }
}

// ---- §8.3 direction vector, §8.5 facing -------------------------------

// Kills the `direction_vector` mutants of the tx < sx and ty < sy
// branches: vectors and directions around the circle (D1–D4 plus the
// mirrored cases). Values: §8.3 by a model of the rule text over `tan`.
// Covers: specs/sim/pathing.md §8.3
#[test]
fn direction_vector_all_quadrants() {
    let t = tables();
    let c = super::geom::centre;
    let cases = [
        ((10, 0), (4096, 0), 56),
        ((0, -10), (0, -4096), 40),
        ((5, 5), (2896, 2896), 0),
        ((3, 1), (3888, 1286), 59),
        ((-10, 0), (-4096, 0), 23),
        ((0, 10), (0, 4096), 7),
        ((-5, 5), (-2896, 2896), 15),
        ((-5, -5), (-2896, -2896), 32),
        ((5, -5), (2896, -2896), 47),
        ((-3, 1), (-3888, 1286), 20),
        ((-3, -1), (-3888, -1286), 27),
        ((3, -1), (3888, -1286), 52),
        ((1, 3), (1286, 3888), 4),
        ((-1, 3), (-1286, 3888), 11),
        ((-1, -3), (-1286, -3888), 36),
        ((1, -3), (1286, -3888), 43),
        ((7, 2), (3940, 1117), 58),
        ((-2, 7), (-1117, 3940), 10),
    ];
    for ((dx, dy), v, d) in cases {
        let got = super::geom::direction_vector(&t, (c(20), c(20)), (c(20 + dx), c(20 + dy)));
        assert_eq!(got, (v, d), "({dx}, {dy})");
    }
}

// Kills the deleted missile arm of `set_facing`: a missile with path
// flag 0x40 keeps its facing; without it the direction is set.
// Covers: specs/sim/pathing.md §8.5
#[test]
fn facing_of_missiles() {
    let t = tables();
    let mut p = WalkPath::zeroed(M);
    p.flags = 0x40;
    p.direction = 5;
    let before = p.clone();
    super::geom::set_facing(&t, &mut p, UnitType::Missile, 10);
    assert_eq!(p, before);
    p.flags = 0;
    super::geom::set_facing(&t, &mut p, UnitType::Missile, 10 + 64);
    assert_eq!((p.direction, p.new_direction, p.turn_step), (10, 0, 0));
}

// ---- §1.1 messages ----------------------------------------------------

// Kills the deleted 0x02 / 0x04 arms: walk / run to a unit.
// Covers: specs/sim/pathing.md §1.1
#[test]
fn message_ids_to_mode_and_form() {
    use super::request::message_request;
    assert_eq!(message_request(0x01), Some((2, false)));
    assert_eq!(message_request(0x02), Some((2, true)));
    assert_eq!(message_request(0x03), Some((3, false)));
    assert_eq!(message_request(0x04), Some((3, true)));
    assert_eq!(message_request(0x05), None);
}

// ---- §1.2–§1.4 requests -----------------------------------------------

use super::request::{interrupt_check, mode_check, request, Outcome, WalkTarget};
use super::seams::UsedSkill;
use crate::game::Game;

fn skill(seq_input: i32, interrupt: bool) -> UsedSkill {
    UsedSkill {
        id: 7,
        seq_input,
        interrupt,
        ..UsedSkill::default()
    }
}

// Kills `&& → ||` of §1.2 step 4: only the unit form clears the queued
// action.
// Covers: specs/sim/pathing.md §1.2 r4
#[test]
fn request_clears_queued_action_only_in_unit_form() {
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    let mut g = Game::new();
    request(
        &t,
        &mut w,
        &mut u.f,
        &mut g,
        P,
        None,
        2,
        WalkTarget::Point(Point::new(20, 10)),
        false,
    )
    .unwrap();
    assert!(!u.f.log.iter().any(|l| l == "clear queued"));
    let (t, mut w, mut u) = setup(40, 40, 10, 10);
    u.add(M, UnitType::Monster, 20, 10, 0);
    let target = WalkTarget::Unit {
        ty: UnitType::Monster,
        guid: M.0,
    };
    request(&t, &mut w, &mut u.f, &mut g, P, None, 2, target, false).unwrap();
    assert!(u.f.log.iter().any(|l| l == "clear queued"));
}

// Kills the mutants of the knockback test of §1.2 step 5: only a
// knockback request (19) on a unit already in mode 19 does nothing.
// Covers: specs/sim/pathing.md §1.2 r5
#[test]
fn request_knockback_on_knocked_back_unit() {
    let run = |m: u32, target_mode: u32| {
        let (t, mut w, mut u) = setup(40, 40, 10, 10);
        u.add(M, UnitType::Monster, 20, 10, 0);
        u.f.unit(M).mode = target_mode;
        let mut g = Game::new();
        let target = WalkTarget::Unit {
            ty: UnitType::Monster,
            guid: M.0,
        };
        request(&t, &mut w, &mut u.f, &mut g, P, None, m, target, false).unwrap()
    };
    assert_eq!(run(19, 19), Outcome::KnockbackIgnored);
    assert_ne!(run(19, 1), Outcome::KnockbackIgnored);
    assert_ne!(run(2, 19), Outcome::KnockbackIgnored);
}

// Kills the `SeqInput > 0` guard mutants of §1.3 (current mode 18).
// Covers: specs/sim/pathing.md §1.3 r3
#[test]
fn mode_check_sequence_mode() {
    let (_, _, mut u) = setup(40, 40, 10, 10);
    let mut g = Game::new();
    g.frame = 100;
    u.f.type1_expire = 50;
    u.f.unit(P).mode = 18;
    u.f.unit(P).used_skill = Some(skill(0, true));
    assert!(!mode_check(&u.f, &g, P, 2));
    u.f.unit(P).used_skill = Some(skill(1, true));
    assert!(mode_check(&u.f, &g, P, 2));
}

// Kills `|| → &&` of §1.4 rule 1: mode 17 (DD) alone refuses.
// Covers: specs/sim/pathing.md §1.4 r1
#[test]
fn interrupt_check_dead_modes() {
    for cur in [0, 17] {
        let (t, mut w, mut u) = setup(40, 40, 10, 10);
        let mut g = Game::new();
        u.f.unit(P).mode = cur;
        assert_eq!(
            interrupt_check(&t, &mut w, &mut u.f, &mut g, P, 2, None),
            Ok(false),
            "{cur}"
        );
    }
}

// Kills the mutants of §1.4 rule 4's last test: without the interrupt
// flag (current mode not 1) only m ∈ {7, 8, 10, 11, 13, 18} with frame ≤
// E + 5 is allowed.
// Covers: specs/sim/pathing.md §1.4 r4
#[test]
fn interrupt_check_without_interrupt_flag() {
    let check = |m: u32, frame: i32| {
        let (t, mut w, mut u) = setup(40, 40, 10, 10);
        let mut g = Game::new();
        g.frame = frame;
        u.f.type1_expire = 10;
        u.f.unit(P).mode = 2;
        u.f.unit(P).used_skill = Some(skill(0, false));
        interrupt_check(&t, &mut w, &mut u.f, &mut g, P, m, None).unwrap()
    };
    assert!(check(7, 15));
    assert!(!check(7, 16));
    assert!(!check(2, 15));
}

// Kills `r < v → <=` of §1.4 rule 5: a roll equal to v does not go to
// rule 6 (allow); one below v does (current mode 2: refuse).
// Covers: specs/sim/pathing.md §1.4 r5, §1.4 r6
#[test]
fn interrupt_check_concentration_roll_boundary() {
    let check = |dv: i64| {
        let (t, mut w, mut u) = setup(40, 40, 10, 10);
        let mut g = Game::new();
        u.f.unit(P).mode = 2;
        u.f.unit(P).used_skill = Some(skill(0, true));
        u.f.unit(P).states.push(42);
        let mut seed = u.f.unit(P).seed;
        let r = seed.roll(100);
        u.f.unit(P)
            .state_stats
            .insert((42, 164), (r as i64 + dv) as i32);
        interrupt_check(&t, &mut w, &mut u.f, &mut g, P, 2, None).unwrap()
    };
    assert!(check(0));
    assert!(!check(1));
}
