// Spec: specs/client/msg-units.md (§4 rule 1: 0x67, 0x68), specs/client/model.md (§8 rule 1, open question 1), specs/sim/pathing.md (§9.4 r2.1); preview fills: docs/PLAN.md decisions D1–D2
//! Provisional monster motion between messages (first playable preview).
//!
//! A monster's S→C 0x67 (`MonsterMove`, to a point) or 0x68
//! (`MonsterMoveToTarget`, to a unit) puts it in walk (2) or run (15)
//! through the mode request (`model.md` §8 rule 1, [`super::modes`]),
//! but the model moves a unit only when a message places it (`model.md`
//! §3 r3; the monster mode machine `0x004AFF60` with its client path is
//! open question 1). Until that is specified, this module stands in:
//!
//! - a monster in walk / run whose last request is a move code of the
//!   server's mode table (walk 1 / 0, run 23 / 24 to a point / a unit,
//!   `sim/intents-events.md` §7.4) gets a track from its cell toward the
//!   request's point, or the target unit's cell;
//! - from the tick after the request (the server path's first step) the
//!   track steps each server tick by the velocity vector of
//!   `sim/pathing.md` §9.4 r2.1 ((m · direction vector) >> 12 per axis,
//!   the §8.3 direction vector toward the goal, m = path velocity ·
//!   0x400 >> 6), starting at the cell centre; toward a point it stops
//!   where the next step would pass the goal's centre (no snap); it
//!   writes the reached cell and the precise position to the model, so
//!   hover, clicks and the draw follow (measured against the 1.14d state
//!   and draws of the Rogue Encampment NPCs). The
//!   message's velocity field is stat 67 `velocitypercent`, not a path
//!   velocity (`seams/movement-prediction.md` §2.4 r3): the path velocity
//!   is the server's own computation (`sim/pathing.md` §8.1 r2,
//!   `path::walk::velocity::mode_velocity`) on the class's `monstats`
//!   `Velocity` and that percent; a class without the column does not
//!   move;
//! - any other request (stop 0x6D, attack, hit, death) or a placement by
//!   a message ends the track.
//!
//! d2rs-own, unverified. PROVISIONAL (client/model.md OQ1): straight line
//! instead of the client path, no collision; the server stays the
//! authority (its next placement or stop wins).

use std::collections::BTreeMap;

use d2_sim::monsters::mode_message::{mode, MODE_ROWS};
use d2_sim::path::walk::velocity::{mode_velocity, STAT_VELOCITYPERCENT};
use d2_sim::path::walk::WalkUnits;
use d2_sim::rng::Seed;
use d2_sim::units::{UnitId, UnitType};

use super::world::{ClientWorld, ModeRequest, MonsterClass, UnitKey, MONSTER};

/// One monster as the server's velocity computation reads it: class,
/// mode, the stat-67 total the message carried and the class's
/// `monstats` `Velocity` / `npc`.
struct Mover {
    class: u32,
    mode: u32,
    percent: i32,
    velocity: i32,
    npc: bool,
    seed: Seed,
}

impl WalkUnits for Mover {
    fn unit_type(&self, _: UnitId) -> UnitType {
        UnitType::Monster
    }
    fn class(&self, _: UnitId) -> u32 {
        self.class
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
    fn monstats_velocity(&self, _: UnitId) -> (i32, bool) {
        (self.velocity, self.npc)
    }
}

/// The 16.16 distance of one server tick of a monster of `class` in
/// `mode` at velocity percent `percent` (module doc): the server's path
/// velocity (`sim/pathing.md` §8.1 r2) · 0x400 >> 6 (§9.4 r2.1). 0 when
/// the class has no `monstats` row or the mode no velocity.
pub fn monster_step(monsters: &[Option<MonsterClass>], class: u32, mode: u32, percent: i32) -> i64 {
    let Some((row, t)) = monsters
        .get(class as usize)
        .copied()
        .flatten()
        .zip(super::predict::path_tables())
    else {
        return 0;
    };
    let Some(setup) = row.setup else {
        return 0;
    };
    let u = Mover {
        class,
        mode,
        percent,
        velocity: i32::from(setup.velocity),
        npc: row.npc,
        seed: Seed::default(),
    };
    let v = mode_velocity(t, &u, UnitId(0), mode).unwrap_or(0);
    (i64::from(v) * 0x400) >> 6
}

/// Where a monster walks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Goal {
    Point(u16, u16),
    Unit(UnitKey),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Track {
    /// The request the track was made from.
    request: ModeRequest,
    goal: Goal,
    /// Precise position, 16.16 sub-tiles.
    at: (i64, i64),
    /// 16.16 distance per server tick.
    step: i64,
    /// The cell last written to the model.
    cell: (u16, u16),
    /// Made in this frame's refresh: the server path moves first on the
    /// tick after the request (`sim/pathing.md` §9.4, event 0 of the
    /// next tick), so a new track does not step yet.
    fresh: bool,
    /// The unit's request count when the track was made (a new request,
    /// even one equal to the last, starts a fresh track).
    requests: u32,
}

/// The tracks of the walking monsters (module doc).
#[derive(Clone, Debug, Default)]
pub struct MonsterMotion {
    tracks: BTreeMap<UnitKey, Track>,
    seen_ticks: u64,
}

/// The goal of a monster's move request: the to-point / to-unit codes of
/// the walk and run rows of the server's mode table.
fn goal_of(r: &ModeRequest) -> Option<Goal> {
    let walk = MODE_ROWS[mode::WL as usize];
    let run = MODE_ROWS[mode::RN as usize];
    if r.code == walk.code_to_point || r.code == run.code_to_point {
        return Some(Goal::Point(r.record[0] as u16, r.record[1] as u16));
    }
    if r.code == walk.code_to_unit || r.code == run.code_to_unit {
        return Some(Goal::Unit(UnitKey::new(
            r.record[0] as u8,
            r.record[1] as u32,
        )));
    }
    None
}

fn centre(c: (u16, u16)) -> (i64, i64) {
    (
        (i64::from(c.0) << 16) | 0x8000,
        (i64::from(c.1) << 16) | 0x8000,
    )
}

impl MonsterMotion {
    /// The number of monsters with a track.
    pub fn walking(&self) -> usize {
        self.tracks.len()
    }

    /// One bridge frame: refresh the tracks from the model (speeds from
    /// the `monstats` rows `monsters`), then step them once if the server
    /// ticked since the last frame.
    pub fn frame(&mut self, world: &mut ClientWorld, monsters: &[Option<MonsterClass>]) {
        let ticked = world.server_ticks != self.seen_ticks;
        self.seen_ticks = world.server_ticks;
        self.refresh(world, monsters);
        if ticked {
            self.step(world);
        }
        for t in self.tracks.values_mut() {
            t.fresh = false;
        }
    }

    fn refresh(&mut self, world: &ClientWorld, monsters: &[Option<MonsterClass>]) {
        let mut next = BTreeMap::new();
        for (key, u) in &world.units {
            if key.unit_type != MONSTER || !matches!(u.mode, mode::WL | mode::RN) {
                continue;
            }
            let (Some(r), Some(cell)) = (u.last_mode_request, u.position) else {
                continue;
            };
            let Some(goal) = goal_of(&r) else {
                continue;
            };
            let step = monster_step(monsters, u.class, u.mode, r.record[4]);
            let old = self.tracks.get(key);
            let keep = old
                .filter(|t| t.request == r && t.cell == cell && t.requests == u.mode_requests)
                .copied();
            // Only a new request waits for the next tick; a track remade
            // from a placement steps at once.
            let fresh = old.is_none_or(|t| t.requests != u.mode_requests);
            next.insert(
                *key,
                keep.unwrap_or(Track {
                    request: r,
                    goal,
                    // A walk starts at the cell centre (recorded: Warriv's
                    // x fraction 32016 at the end of a walk, 32768 on the
                    // tick of the next request).
                    at: centre(cell),
                    step,
                    cell,
                    fresh,
                    requests: u.mode_requests,
                }),
            );
        }
        self.tracks = next;
    }

    fn step(&mut self, world: &mut ClientWorld) {
        for (key, t) in self.tracks.iter_mut() {
            if t.fresh {
                continue;
            }
            let goal = match t.goal {
                Goal::Point(x, y) => Some((x, y)),
                Goal::Unit(u) => world.units.get(&u).and_then(|u| u.position),
            };
            let Some(goal) = goal else { continue };
            let (gx, gy) = centre(goal);
            let (dx, dy) = (gx - t.at.0, gy - t.at.1);
            let len = dx.abs().max(dy.abs());
            if len == 0 || t.step == 0 {
                continue;
            }
            // A unit goal: stop one sub-tile short (the target's cell is
            // taken).
            let room = if matches!(t.goal, Goal::Unit(_)) {
                (len - 0x10000).max(0)
            } else {
                len
            };
            // `sim/pathing.md` §9.4 r2.1: the velocity vector is (m ·
            // direction vector) >> 12 per axis, the direction vector of
            // §8.3 toward the goal (a straight line: no client path).
            let Some(tables) = super::predict::path_tables() else {
                continue;
            };
            let ((vx, vy), _) = d2_sim::path::walk::geom::direction_vector(
                tables,
                (t.at.0 as u32, t.at.1 as u32),
                (gx as u32, gy as u32),
            );
            let (mx, my) = (
                (t.step * i64::from(vx)) >> 12,
                (t.step * i64::from(vy)) >> 12,
            );
            let reach = mx.abs().max(my.abs());
            t.at = match t.goal {
                // A point: full steps while the next one does not pass
                // the goal's centre; then the path stops where it is (no
                // snap: recorded Warriv at rest 376 short of the centre
                // on both axes, (32392, 33144)).
                Goal::Point(..) if dx.abs() < mx.abs() && dy.abs() < my.abs() => t.at,
                Goal::Point(..) if dx == 0 && dy == 0 => t.at,
                Goal::Point(..) => (t.at.0 + mx, t.at.1 + my),
                // A unit: one sub-tile short of it.
                Goal::Unit(_) if room == 0 => t.at,
                Goal::Unit(_) if room <= reach => {
                    (t.at.0 + dx * room / len, t.at.1 + dy * room / len)
                }
                Goal::Unit(_) => (t.at.0 + mx, t.at.1 + my),
            };
            let cell = ((t.at.0 >> 16) as u16, (t.at.1 >> 16) as u16);
            if let Some(u) = world.units.get_mut(key) {
                u.precise = Some((t.at.0 as u32, t.at.1 as u32));
                if cell != t.cell {
                    u.position = Some(cell);
                }
            }
            t.cell = cell;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::world::{ClientUnit, MonsterSetup, PLAYER};

    /// `monstats` rows: class 0 with `Velocity` 16 (path velocity 0x1000
    /// at 100 %: one sub-tile a tick).
    fn classes() -> Vec<Option<MonsterClass>> {
        vec![Some(MonsterClass {
            setup: Some(MonsterSetup {
                velocity: 16,
                ..Default::default()
            }),
            ..Default::default()
        })]
    }

    fn monster(w: &mut ClientWorld, guid: u32, at: (u16, u16), mode: u32, r: ModeRequest) {
        let k = UnitKey::new(MONSTER, guid);
        let mut u = ClientUnit::new(k);
        u.position = Some(at);
        u.mode = mode;
        u.last_mode_request = Some(r);
        w.units.insert(k, u);
    }

    /// A move request; `percent` is the message's stat 67.
    fn request(code: u8, a: i32, b: i32, percent: i32) -> ModeRequest {
        ModeRequest {
            code,
            record: [a, b, 0, 0, percent, 0, 0],
        }
    }

    fn tick(m: &mut MonsterMotion, w: &mut ClientWorld) {
        w.server_ticks += 1;
        m.frame(w, &classes());
    }

    #[test]
    fn a_walking_monster_steps_to_its_point_and_stops() {
        let mut w = ClientWorld::default();
        // Walk to (110, 100) at 100 %: one sub-tile a tick.
        monster(&mut w, 5, (100, 100), mode::WL, request(1, 110, 100, 100));
        let mut m = MonsterMotion::default();
        m.frame(&mut w, &classes());
        assert_eq!(m.walking(), 1);
        for i in 1..=3 {
            tick(&mut m, &mut w);
            assert_eq!(
                w.units[&UnitKey::new(MONSTER, 5)].position,
                Some((100 + i, 100))
            );
        }
        for _ in 0..20 {
            tick(&mut m, &mut w);
        }
        assert_eq!(
            w.units[&UnitKey::new(MONSTER, 5)].position,
            Some((110, 100))
        );
    }

    #[test]
    fn a_monster_walks_to_a_unit_and_stops_next_to_it() {
        let mut w = ClientWorld::default();
        let p = UnitKey::new(PLAYER, 1);
        let mut pu = ClientUnit::new(p);
        pu.position = Some((100, 108));
        w.units.insert(p, pu);
        // Run (15) to the player, code 24, at 200 %.
        monster(&mut w, 5, (100, 100), mode::RN, request(24, 0, 1, 200));
        let mut m = MonsterMotion::default();
        m.frame(&mut w, &classes());
        for _ in 0..20 {
            tick(&mut m, &mut w);
        }
        assert_eq!(
            w.units[&UnitKey::new(MONSTER, 5)].position,
            Some((100, 107))
        );
    }

    #[test]
    fn other_modes_and_placements_end_the_track() {
        let mut w = ClientWorld::default();
        let k = UnitKey::new(MONSTER, 5);
        monster(&mut w, 5, (100, 100), mode::WL, request(1, 120, 100, 100));
        let mut m = MonsterMotion::default();
        m.frame(&mut w, &classes());
        tick(&mut m, &mut w);
        // A server placement (position check) moves it elsewhere: snap.
        w.units.get_mut(&k).unwrap().position = Some((50, 100));
        tick(&mut m, &mut w);
        assert_eq!(
            w.units[&k].position,
            Some((51, 100)),
            "steps from the placement"
        );
        // Neutral: no track, no motion.
        w.units.get_mut(&k).unwrap().mode = mode::NU;
        tick(&mut m, &mut w);
        assert_eq!(m.walking(), 0);
        assert_eq!(w.units[&k].position, Some((51, 100)));
    }

    // Covers: specs/sim/pathing.md §9.4
    #[test]
    fn a_diagonal_npc_walk_follows_the_recorded_server_positions() {
        // Warriv's second walk at the Rogue Encampment arrival (1.14d
        // state, Wine): from the centre of (4868, 4233) toward (4869, 4232)
        // at monstats Velocity 3 (npc) and 75 %: 6516 per axis a tick from
        // the tick after the request, resting at (4869 + 32392, 4232 +
        // 33144) in 16.16, 376 short of the centre.
        let rows = vec![Some(MonsterClass {
            npc: true,
            setup: Some(MonsterSetup {
                velocity: 3,
                ..Default::default()
            }),
            ..Default::default()
        })];
        let mut w = ClientWorld::default();
        let k = UnitKey::new(MONSTER, 7);
        monster(
            &mut w,
            7,
            (4868, 4233),
            mode::WL,
            request(1, 4869, 4232, 75),
        );
        w.units.get_mut(&k).unwrap().mode_requests = 1;
        let mut m = MonsterMotion::default();
        w.server_ticks += 1;
        m.frame(&mut w, &rows); // the request's tick: no step yet
        let p = |w: &ClientWorld| w.units[&k].precise.unwrap();
        let x0 = (4868u32 << 16) | 0x8000;
        let y0 = (4233u32 << 16) | 0x8000;
        assert_eq!(w.units[&k].precise, None);
        for n in 1..=10u32 {
            w.server_ticks += 1;
            m.frame(&mut w, &rows);
            assert_eq!(p(&w), (x0 + 6516 * n, y0 - 6516 * n), "step {n}");
        }
        assert_eq!(p(&w), ((4869 << 16) + 32392, (4232 << 16) + 33144));
        w.server_ticks += 1;
        m.frame(&mut w, &rows);
        assert_eq!(p(&w), ((4869 << 16) + 32392, (4232 << 16) + 33144));
        assert_eq!(w.units[&k].position, Some((4869, 4232)));
    }
}
