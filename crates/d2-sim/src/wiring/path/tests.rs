// Spec: specs/sim/pathing.md §1, §8.1, §9 (vector M1); specs/sim/path-placement.md §2.4, §2.5, §5, §8, §10, §11; specs/world/waypoints.md §7 (integration of the wired path seams)
//! The path provider on the wired sim: the action fixture (act 0's DRLG,
//! level 2 as rooms A and B, streamed) with `enable_paths`. A player's
//! walk request moves it through the real timer queue, sub-tile by
//! sub-tile (`pathing.md` vector M1 translated to the fixture's rooms),
//! across a room edge with the room recache; the waypoint warp places
//! the player in the destination's spawn room; allocation stamps
//! footprints and the coarse free-box search avoids them.

use std::sync::Arc;

use d2_data::tables::{Levels, Objects};

use crate::drlg::collision::bits;
use crate::drlg::TileRect;
use crate::path::coords::Point;
use crate::path::walk::Outcome;
use crate::path::{CollisionRooms, UnitPath};
use crate::skills::fake::blank;
use crate::tick::events::event;
use crate::units::{UnitId, UnitType};
use crate::wiring::action::tests::{Fx, LEVEL};
use crate::world::waypoints::{ArrivalList, WaypointData, NO_WAYPOINT};

/// `velocitypercent` (`pathing.md` §8.1), at its creation value 100
/// (`combat/vitals.md` §1).
const STAT_VELOCITY: u16 = 67;

/// The fixture with the path provider on and charstats `WalkVelocity` 6
/// for class 0 (vector V1: velocity 0x600).
fn fx_with(rooms: &[(u32, TileRect)]) -> Fx {
    let mut fx = Fx::with_rooms(rooms);
    let h = fx.sim.hooks();
    h.enable_paths().expect("embedded tables");
    Arc::make_mut(&mut h.tables).combat.charstats[0].walkvelocity = 6;
    fx
}

fn fx() -> Fx {
    fx_with(&[
        (LEVEL, TileRect::new(0, 0, 8, 8)),
        (LEVEL, TileRect::new(8, 0, 8, 8)),
    ])
}

fn player(fx: &mut Fx, x: i32, y: i32) -> UnitId {
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, x, y);
    fx.stats(p, &[(STAT_VELOCITY, 100)]);
    p
}

fn path(fx: &mut Fx, u: UnitId) -> crate::path::DynamicPath {
    fx.sim
        .hooks()
        .paths
        .as_ref()
        .unwrap()
        .dynamic(u)
        .unwrap()
        .clone()
}

fn cell(fx: &mut Fx, x: i32, y: i32) -> u16 {
    let r = fx.a;
    crate::path::collision::point_value(&fx.sim.hooks().drlg, Some(r), x, y, 0xFFFF)
}

fn walk(fx: &mut Fx, p: UnitId, x: i32, y: i32) -> Option<Outcome> {
    fx.sim
        .with(&mut fx.game, |g, v| {
            super::walk::walk_message(v, g, p, 0x01, x as u32, y as u32)
        })
        .1
}

/// Ticks until the player's event 0 stops, recording (precise x, precise
/// y, mode) after each tick.
fn run(fx: &mut Fx, p: UnitId, max: usize) -> Vec<(u32, u32, u32)> {
    let mut out = Vec::new();
    for _ in 0..max {
        fx.tick();
        let d = path(fx, p);
        let mode = fx.sim.sys.units.get(p).unwrap().mode;
        out.push((d.precise_x, d.precise_y, mode));
        if mode != 2 {
            break;
        }
    }
    out
}

// Covers: specs/sim/path-placement.md §2.4 r1, §2.4 r2, §2.4 r3, §2.4 r4, §2.4 r5, §2.5
#[test]
fn allocation_gives_the_unit_its_path_and_footprint() {
    let mut fx = fx();
    let p = player(&mut fx, 26, 10);
    let d = path(&mut fx, p);
    assert_eq!((d.x(), d.y(), d.room), (26, 10, Some(fx.a)));
    assert_eq!((d.precise_x, d.precise_y), (0x1A8000, 0xA8000));
    assert_eq!((d.foot_mask, d.move_mask, d.path_type), (0x80, 0x1C09, 7));
    assert_eq!((d.unit_size, d.pattern, d.velocity), (2, 1, 0x800));
    // Pattern 1 (plus) with the NO_PATH marker on the centre.
    assert_eq!(cell(&mut fx, 26, 10), 0x80 | bits::NO_PATH);
    for (x, y) in [(25, 10), (27, 10), (26, 9), (26, 11)] {
        assert_eq!(cell(&mut fx, x, y), 0x80, "({x}, {y})");
    }
    assert_eq!(fx.sim.hooks().path_position(p), (26, 10));
    // Removal frees the record and the footprint.
    fx.sim.with(&mut fx.game, |g, v| v.remove(g, p));
    assert_eq!(cell(&mut fx, 26, 10), 0);
    assert!(fx.sim.hooks().paths.as_ref().unwrap().record(p).is_none());
    fx.assert_clean();
}

// Covers: specs/sim/pathing.md §1.1, §1.5 r6, §8.1 r2, §9.1, §9.2 r4, §9.2 r6, §9.4 r2, §9.6 r8, §9.7
#[test]
fn walk_request_moves_the_player_sub_tile_by_sub_tile() {
    // Vector M1 moved to (26, 10) → (31, 10): +0x6000 per tick for 13
    // ticks, the 14th lands on the target centre and stops; the step runs
    // from the tick's timer pass (event 0 every tick, `tick.md` §3 step
    // 4) and the stop starts neutral (mode 1).
    let mut fx = fx();
    let p = player(&mut fx, 26, 10);
    assert_eq!(walk(&mut fx, p, 31, 10), Some(Outcome::Moving(1)));
    assert_eq!(fx.sim.sys.units.get(p).unwrap().mode, 2);
    assert_eq!(path(&mut fx, p).velocity, 0x600);
    assert!(fx.timers(p).iter().any(|&(e, _)| e == event::MODE_CHANGE));
    let ticks = run(&mut fx, p, 20);
    assert_eq!(ticks.len(), 14);
    for (k, &(x, y, m)) in ticks.iter().take(13).enumerate() {
        assert_eq!(x, 0x1AE000 + k as u32 * 0x6000, "tick {}", k + 1);
        assert_eq!((y, m), (0xA8000, 2), "tick {}", k + 1);
    }
    assert_eq!(ticks[13], (0x1F8000, 0xA8000, 1));
    let d = path(&mut fx, p);
    assert_eq!((d.cur_point, d.point_count, d.flags & 0x20), (0, 0, 0));
    // The footprint moved with the unit; no event 0 is left.
    assert_eq!(cell(&mut fx, 26, 10), 0);
    assert_eq!(cell(&mut fx, 31, 10), 0x80 | bits::NO_PATH);
    assert!(!fx.timers(p).iter().any(|&(e, _)| e == event::MODE_CHANGE));
    fx.assert_clean();
}

// Covers: specs/sim/pathing.md §9.6 r9
#[test]
fn walking_across_a_room_edge_changes_the_room() {
    // Room A is sub-tiles x 0..40, B 40..80: the step into x = 40 moves
    // the player from A's unit list to B's (`0x0064FAD0`).
    let mut fx = fx();
    let (a, b) = (fx.a, fx.b);
    let p = player(&mut fx, 37, 10);
    assert_eq!(walk(&mut fx, p, 43, 10), Some(Outcome::Moving(1)));
    let ticks = run(&mut fx, p, 30);
    // 6 sub-tiles at 0x6000: 16 ticks, the last one reaching the point.
    assert_eq!(ticks.len(), 16);
    assert_eq!(ticks[15], (0x2B8000, 0xA8000, 1));
    let d = path(&mut fx, p);
    assert_eq!((d.room, d.prev_room), (Some(b), Some(a)));
    assert_eq!(fx.game.lists.unit(p).unwrap().room(), Some(b));
    assert!(fx.game.lists.room_units(b).contains(&p));
    assert!(!fx.game.lists.room_units(a).contains(&p));
    // The room-change messages ran (flag 0x2 cleared).
    assert_eq!(d.flags & 0x2, 0);
    fx.assert_clean();
}

// Covers: specs/sim/pathing.md §9.6 r4, §9.6 r5
#[test]
fn a_wall_on_the_way_changes_the_walk() {
    // M08: the same request with a wall in the target cell's row ends
    // elsewhere (the walk check reports the grid it runs on).
    let mut fx = fx();
    let p = player(&mut fx, 26, 10);
    let a = fx.a;
    *fx.sim
        .hooks()
        .drlg
        .grid_mut(a)
        .unwrap()
        .get_mut(29, 10)
        .unwrap() |= bits::WALL;
    let o = walk(&mut fx, p, 31, 10);
    let ticks = run(&mut fx, p, 40);
    assert_ne!(
        (o, ticks.last().copied()),
        (Some(Outcome::Moving(1)), Some((0x1F8000, 0xA8000, 1)))
    );
    // Wherever it went, the player never stands on the wall.
    let d = path(&mut fx, p);
    assert_ne!((d.x(), d.y()), (29, 10));
    fx.assert_clean();
}

/// Waypoint tables: level 1 (town) has waypoint index 0, the fixture's
/// level 2 index 1; one object class with operate function 23.
fn waypoint_data() -> WaypointData {
    let mut levels = vec![blank::<Levels>(); 150];
    for l in &mut levels {
        l.waypoint = NO_WAYPOINT;
    }
    levels[1].waypoint = 0;
    levels[LEVEL as usize].waypoint = 1;
    let mut o: Objects = blank();
    o.operatefn = 23;
    o.initfn = 17;
    o.framecnt1 = 10 << 8;
    WaypointData::new(&levels, &[o])
}

/// Travel from level 2 (room A) to level 1 (one room, C).
fn travel(fx: &mut Fx) -> (UnitId, Option<crate::units::RoomId>) {
    let a = fx.a;
    let o = fx.spawn(UnitType::Object, 0, a, 20, 20);
    fx.sim.sys.units.get_mut(o).unwrap().mode = 0;
    let p = player(fx, 22, 20);
    let wd = waypoint_data();
    let guid = fx.game.lists.unit(o).unwrap().guid;
    let mut arrivals = ArrivalList(Vec::new());
    let spawn = fx.sim.waypoints(&mut fx.game, |w| {
        use crate::world::waypoints::WaypointWorld;
        let d = w.difficulty();
        w.records(p).unwrap().get_mut(d).set(0).unwrap();
        wd.travel(w, &mut arrivals, p, guid, 1).unwrap();
        // The room the arrival test compares with (one room: the same
        // whatever the search draws).
        w.spawn_room(1, crate::world::waypoints::tile_code(1))
    });
    (p, spawn)
}

// Covers: specs/sim/path-placement.md §6 r4, §10 r3, §10 r4, §10 r6, §11 r1, §11 r2, §11 r3, §11 r4; specs/world/waypoints.md §7 r5, §7 r7
#[test]
fn waypoint_warp_places_the_player_in_the_spawn_room() {
    // The e2e step-6 condition in d2-sim terms: after the same-act warp
    // the player's room is the destination's spawn room, so rule 7's
    // arrival runs (mode request 2 at its own position → no path →
    // neutral; in town: 5).
    let mut fx = fx_with(&[
        (LEVEL, TileRect::new(0, 0, 8, 8)),
        (LEVEL, TileRect::new(8, 0, 8, 8)),
        (1, TileRect::new(0, 16, 8, 8)),
    ]);
    let (p, spawn) = travel(&mut fx);
    let room = fx.game.lists.unit(p).unwrap().room();
    assert!(room.is_some());
    assert_eq!(room, spawn);
    assert_ne!(room, Some(fx.a));
    let d = path(&mut fx, p);
    assert_eq!(d.room, room);
    let (x, y) = (d.x(), d.y());
    let rect = fx.sim.hooks().drlg.subtile_rect(room.unwrap()).unwrap();
    assert!(rect.contains(x, y), "({x}, {y}) in {rect:?}");
    // The old footprint is cleared (0x4 is the fixture's barrier
    // column). The stamp at the destination is looked up from the
    // destination room (§6 rule 4: the forced move's room2), so a warp
    // to a room not adjacent to the old one still stamps there.
    assert_eq!(cell(&mut fx, 22, 20), bits::MISSILE_BARRIER);
    let r = room.unwrap();
    let v = crate::path::collision::point_value(&fx.sim.hooks().drlg, Some(r), x, y, 0xFFFF);
    assert_eq!(v & 0x80, 0x80);
    // §10 rule 6: flags 2 bit 0x10000, event 14 at f + 50.
    let f = fx.game.frame;
    assert_ne!(fx.sim.sys.units.get(p).unwrap().flags2 & 0x10000, 0);
    assert!(fx.timers(p).contains(&(14, f + 50)));
    // Rule 7: neutral in town.
    assert_eq!(fx.sim.sys.units.get(p).unwrap().mode, 5);
    fx.assert_clean();
}

#[test]
fn without_the_provider_the_warp_stays_pending() {
    // M08 for the warp: the same travel with the provider off leaves the
    // player where it was (the e2e step-6 stop).
    let mut fx = Fx::with_rooms(&[
        (LEVEL, TileRect::new(0, 0, 8, 8)),
        (LEVEL, TileRect::new(8, 0, 8, 8)),
        (1, TileRect::new(0, 16, 8, 8)),
    ]);
    let (p, spawn) = travel(&mut fx);
    assert_eq!(fx.game.lists.unit(p).unwrap().room(), Some(fx.a));
    assert_ne!(Some(fx.a), spawn);
}

// Covers: specs/sim/path-placement.md §8 r1, §8 r2, §8 r3
#[test]
fn coarse_free_box_avoids_a_monster_footprint() {
    // Pass 1 tries (X0 − 1, Y0 − 1) with a 3 × 3 box (n = 1); a size-2
    // monster (pattern 1, NO_PATH 0x1000 on its centre, in mask 0x3C01)
    // stamped at (X0, Y0) blocks it, pass 2's first offset (−2, −2) is
    // free. Without the monster pass 1 wins (M08).
    let mut fx = fx();
    let a = fx.a;
    {
        let t = Arc::make_mut(&mut fx.sim.hooks().tables);
        let ex = usize::from(t.combat.monstats[0].monstatsex);
        t.combat.monstats2[ex].sizex = 2;
    }
    let search = |fx: &mut Fx| {
        let mut pt = Point::new(30, 10);
        let r = super::place::coarse_free_box(&fx.sim.hooks().drlg, a, &mut pt, 1, 0x3C01);
        (r, pt)
    };
    assert_eq!(search(&mut fx), (Some(a), Point::new(29, 9)));
    let m = fx.spawn(UnitType::Monster, 0, a, 30, 10);
    match fx.sim.hooks().paths.as_ref().unwrap().record(m) {
        Some(UnitPath::Dynamic(d)) => {
            assert_eq!((d.foot_mask, d.pattern, d.x(), d.y()), (0x100, 1, 30, 10))
        }
        other => panic!("monster path {other:?}"),
    }
    assert_ne!(cell(&mut fx, 30, 10) & 0x100, 0);
    assert_eq!(search(&mut fx), (Some(a), Point::new(28, 8)));
    fx.assert_clean();
}

/// A monster of class 0 allocated in `mode` at (x, y) of room A.
fn spawn_monster_in_mode(fx: &mut Fx, mode: u32, x: i32, y: i32) -> UnitId {
    let req = crate::units::lifecycle::AllocRequest {
        ty: UnitType::Monster,
        class: 0,
        room: Some(fx.a),
        add: true,
        fixed_guid: None,
        mode,
        allied: false,
    };
    fx.sim
        .with(&mut fx.game, |g, v| v.allocate(g, &req, x, y))
        .expect("allocated")
}

// Covers: specs/sim/units.md §3.1 r8; specs/sim/path-placement.md §5.3 r3
#[test]
fn a_monster_allocated_dead_carries_the_corpse_mask() {
    let mut fx = fx();
    {
        let t = Arc::make_mut(&mut fx.sim.hooks().tables);
        let ex = usize::from(t.combat.monstats[0].monstatsex);
        t.combat.monstats2[ex].sizex = 2;
    }
    // Mode 12 (dead), `deadCol` clear: pattern 5, mask 0x8000, the live
    // mask 0x100 and its NO_PATH marker are gone.
    let m = spawn_monster_in_mode(&mut fx, 12, 30, 10);
    let d = path(&mut fx, m);
    assert_eq!((d.pattern, d.foot_mask), (5, 0x8000));
    assert_eq!(cell(&mut fx, 30, 10) & 0x8000, 0x8000);
    assert_eq!(cell(&mut fx, 30, 10) & (0x100 | bits::NO_PATH), 0);
    assert_eq!(cell(&mut fx, 29, 10) & 0x100, 0);
    // Mode 0 likewise; a live mode keeps the live footprint.
    let m0 = spawn_monster_in_mode(&mut fx, 0, 20, 10);
    assert_eq!(path(&mut fx, m0).foot_mask, 0x8000);
    let live = spawn_monster_in_mode(&mut fx, 1, 10, 10);
    assert_eq!(path(&mut fx, live).foot_mask, 0x100);
    assert_ne!(cell(&mut fx, 10, 10) & 0x100, 0);
    // `deadCol` set: the settings are skipped.
    {
        let t = Arc::make_mut(&mut fx.sim.hooks().tables);
        let ex = usize::from(t.combat.monstats[0].monstatsex);
        t.combat.monstats2[ex].deadcol = true;
    }
    let keep = spawn_monster_in_mode(&mut fx, 12, 24, 10);
    assert_eq!(path(&mut fx, keep).foot_mask, 0x100);
    assert_ne!(cell(&mut fx, 24, 10) & 0x100, 0);
}
