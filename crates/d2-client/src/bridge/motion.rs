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
//! - each server tick the track steps in a straight line by the path
//!   velocity × 16 in 16.16 sub-tiles (`sim/pathing.md` §9.4 r2.1, base
//!   0x400, as [`super::predict::Speeds::step`]) and writes the reached
//!   cell to the model, so hover, clicks and the draw follow. The
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
            let keep = self
                .tracks
                .get(key)
                .filter(|t| t.request == r && t.cell == cell)
                .copied();
            next.insert(
                *key,
                keep.unwrap_or(Track {
                    request: r,
                    goal,
                    at: centre(cell),
                    step,
                    cell,
                }),
            );
        }
        self.tracks = next;
    }

    fn step(&mut self, world: &mut ClientWorld) {
        for (key, t) in self.tracks.iter_mut() {
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
            let s = t.step.min(room);
            t.at = (t.at.0 + dx * s / len, t.at.1 + dy * s / len);
            let cell = ((t.at.0 >> 16) as u16, (t.at.1 >> 16) as u16);
            if cell != t.cell {
                if let Some(u) = world.units.get_mut(key) {
                    u.position = Some(cell);
                }
                t.cell = cell;
            }
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
}
