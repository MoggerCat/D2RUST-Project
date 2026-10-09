// Spec: specs/render/unit-composite.md (§8 "Creators" site 0x004C8726, timed arc 0x004DA5B0), specs/skills/bodies-2b.md (§8.10, §8.11)
//! The client motion of skill moves in the play preview (REC-275):
//! Leap's arc and Whirlwind's spin.
//!
//! - **Leap arc** (`unit-composite.md` §8): `0x004C8670(unit, point)` gives
//!   the unit a motion record whose timed arc (`0x004DA5B0`, height 0)
//!   lifts the draw by `oz` pixels and lands it after `n` updates. The
//!   record is stepped once per server tick with the spec's
//!   [`MotionRecord::update`]; the draw adds `(ox, oy + oz)`
//!   ([`MotionRecord::draw_offset`], fed through `ViewSource::unit_offset`).
//! - **Whirlwind spin** (`bodies-2b.md` §8.11 step 4.1): the server runs the
//!   player's animation again from frame 3 on each do; the view loops the
//!   skill's mode from frame 3 ([`spin_frame`]).
//!
//! Which client event starts the arc (the callers `0x004C8D58`,
//! `0x004C902F` are not traced) and the path speed `s` are d2rs-own,
//! unverified: PROVISIONAL (REC-275).

use std::collections::BTreeMap;

use bevy::prelude::*;

use crate::bridge::mirror::bridge_frame;
use crate::bridge::world::{ClientWorld, ModeRequest, SkillRow, UnitKey};
use crate::bridge::BridgeResource;
use crate::rules::unit_composite::{motion, MotionRecord};

use super::walk::{preview_walk_frame, PreviewWalk};
use super::WorldViewState;

/// `srvdofunc` of Whirlwind, Leap and Leap Attack (`bodies-2b.md` §8.11,
/// `bodies-2.md` §4.7).
const WHIRLWIND: i16 = 76;
const LEAP: i16 = 77;
const LEAP_ATTACK: i16 = 78;

/// Frame where Whirlwind's repeated animation restarts
/// (`bodies-2b.md` §8.11 step 4.1, `0x00553DC0(game, unit, 3)`).
pub const SPIN_FIRST_FRAME: usize = 3;

/// `g` of `0x004C8726`: `-270·d + 8,462` for `d < 27`, else
/// `-72·d + 1,224` (table `0x006DAE70`), at least 500.
pub fn leap_gravity(d: i32) -> i32 {
    let g = if d < 27 {
        8462 - 270 * d
    } else {
        1224 - 72 * d
    };
    g.max(500)
}

/// The timed arc `0x004DA5B0` on `rec` (flag 2; ticks := max(`n`, 1);
/// x = y = 0; z := `h` ≪; vx = vy = 0; az := -0x1000 when 0; vz :=
/// (-`h`·2048 - az·`n`²/2) / `n`, C division).
pub fn timed_arc(rec: &mut MotionRecord, height: i32, n: i32) {
    let n = n.max(1);
    rec.flags |= motion::TIMED;
    rec.ticks_left = n;
    rec.pos = [0, 0, height.wrapping_shl(11)];
    rec.vel[0] = 0;
    rec.vel[1] = 0;
    if rec.acc[2] == 0 {
        rec.acc[2] = -0x1000;
    }
    let az = i64::from(rec.acc[2]);
    let n64 = i64::from(n);
    let vz = (-i64::from(height) * 2048 - az * n64 * n64 / 2) / n64;
    rec.vel[2] = vz as i32;
}

/// The leap's motion record (`0x004C8726`): `d` the distance to the point
/// in sub-tiles (at least 1), `s` the unit's path speed (`velocity >> 8`).
/// `None` when `s` is 0 (no record is created).
pub fn leap_record(d: i32, s: i32) -> Option<MotionRecord> {
    if s == 0 {
        return None;
    }
    let d = d.max(1);
    // `n := (d << 16) / (s << 12)`, "-1 when > 1" read as: one less when
    // above 1 (PROVISIONAL, REC-275).
    let q = (i64::from(d) << 16) / (i64::from(s) << 12);
    let n = if q > 1 { q - 1 } else { q } as i32;
    let mut rec = MotionRecord::default();
    // A fresh record is zeroed; `az := -g` (unshifted).
    rec.acc[2] = -leap_gravity(d);
    timed_arc(&mut rec, 0, n);
    Some(rec)
}

/// Whirlwind's drawn frame: the skill mode's frames `3..frames` looped
/// (`ticks` advanced at the 8.8 animation `rate`); a mode of 3 frames or
/// fewer loops from its first frame.
pub fn spin_frame(ticks: u64, rate: u32, frames: usize) -> usize {
    let frames = frames.max(1);
    let first = if frames > SPIN_FIRST_FRAME {
        SPIN_FIRST_FRAME
    } else {
        0
    };
    let span = (frames - first) as u64;
    first + ((ticks.wrapping_mul(u64::from(rate)) >> 8) % span) as usize
}

/// What the local player's skill move shows now.
#[derive(Resource, Debug, Default)]
pub struct SkillMotion {
    /// The request the last start was made for.
    seen: Option<(UnitKey, ModeRequest)>,
    seen_ticks: u64,
    leaps: BTreeMap<UnitKey, MotionRecord>,
    /// Whirlwind: ticks left and the skill's mode.
    spins: BTreeMap<UnitKey, (i32, u32)>,
}

/// The sub-tile a skill mode request aims at: code 0x15 a point
/// (record 2, 3), code 0x16 a unit (type, GUID) and its cell. The skill is
/// record 0 (`bridge::modes::skill_mode`). Record 2, 3 of code 0x16 is
/// d2rs-own, unverified (PROVISIONAL, REC-275).
fn request_point(world: &ClientWorld, r: &ModeRequest) -> Option<(i32, i32)> {
    match r.code {
        0x15 => Some((r.record[2], r.record[3])),
        0x16 => {
            let key = UnitKey::new(u8::try_from(r.record[2]).ok()?, r.record[3] as u32);
            let (x, y) = world.units.get(&key)?.position?;
            Some((i32::from(x), i32::from(y)))
        }
        _ => None,
    }
}

/// `0x006417F0`: max(|dx|, |dy|) + min(|dx|, |dy|) / 2.
fn distance(a: (i32, i32), b: (i32, i32)) -> i32 {
    let (dx, dy) = ((a.0 - b.0).abs(), (a.1 - b.1).abs());
    dx.max(dy) + dx.min(dy) / 2
}

impl SkillMotion {
    /// One bridge frame. `row_of` is the client skills table, `speed` the
    /// unit's path speed (`velocity >> 8`; d2rs-own, unverified: the run
    /// velocity), `ticked` whether the server ticked. `cell` is the local
    /// player's own path cell when the client walks it
    /// ([`crate::bridge::predict::Predict::cell`]); the model's position
    /// otherwise.
    pub fn frame(
        &mut self,
        world: &ClientWorld,
        row_of: impl Fn(u16) -> Option<SkillRow>,
        speed: i32,
        cell: Option<(u16, u16)>,
    ) {
        let ticked = world.server_ticks != self.seen_ticks;
        self.seen_ticks = world.server_ticks;
        let Some(local) = world.local() else {
            self.leaps.clear();
            self.spins.clear();
            return;
        };
        let key = local.key;
        let req = local.last_mode_request;
        if self.seen != req.map(|r| (key, r)) {
            self.seen = req.map(|r| (key, r));
            // `0x004C8670` measures from the client unit's position, its
            // own path cell (`unit-composite.md` §8, creator
            // `0x004C8726`): the server sends a walking player nothing
            // (`sim/pathing.md` §10 rule 2), so the model's position is
            // the last placement.
            if let (Some(r), Some(at)) = (req, cell.or(local.position)) {
                self.start(
                    world,
                    &row_of,
                    key,
                    &r,
                    (i32::from(at.0), i32::from(at.1)),
                    speed,
                );
            }
        }
        if ticked {
            self.leaps
                .retain(|_, rec| rec.update(false, None).is_ok() && rec.flags & motion::DONE == 0);
            self.spins.retain(|_, (n, _)| {
                *n -= 1;
                *n > 0
            });
        }
    }

    fn start(
        &mut self,
        world: &ClientWorld,
        row_of: &impl Fn(u16) -> Option<SkillRow>,
        key: UnitKey,
        r: &ModeRequest,
        at: (i32, i32),
        speed: i32,
    ) {
        self.leaps.remove(&key);
        self.spins.remove(&key);
        let Some(row) = u16::try_from(r.record[0]).ok().and_then(row_of) else {
            return;
        };
        let Some(to) = request_point(world, r) else {
            return;
        };
        let d = distance(at, to);
        match row.srvdofunc {
            LEAP | LEAP_ATTACK => {
                if let Some(rec) = leap_record(d, speed) {
                    self.leaps.insert(key, rec);
                }
            }
            WHIRLWIND if speed > 0 => {
                let n = ((i64::from(d.max(1)) << 16) / (i64::from(speed) << 12)).max(1);
                self.spins.insert(key, (n as i32, u32::from(row.anim)));
            }
            _ => {}
        }
    }

    /// The draw offsets `(ox, oy + oz)` of the units in an arc.
    pub fn offsets(&self) -> BTreeMap<UnitKey, (i32, i32)> {
        self.leaps
            .iter()
            .map(|(k, r)| (*k, r.draw_offset()))
            .collect()
    }

    /// The unit whirling now and the mode it shows.
    pub fn spinning(&self) -> Option<(UnitKey, u32)> {
        self.spins.iter().next().map(|(k, (_, m))| (*k, *m))
    }
}

/// Steps [`SkillMotion`] after the walk prediction and hands the offsets
/// to the feed and the spin to the unit art.
pub fn skill_motion_frame(
    bridge: Res<BridgeResource>,
    walk: Res<PreviewWalk>,
    mut motion: ResMut<SkillMotion>,
    mut state: ResMut<WorldViewState>,
) {
    let speed = walk.speeds.map_or(0, |s| i32::from(s.run));
    let world = bridge.0.world();
    let cell = walk
        .predict
        .cell()
        .filter(|_| walk.predict.player() == world.local().map(|u| u.key));
    motion.frame(world, |id| bridge.0.skill_row(id), speed, cell);
    state.feed.set_motion_offsets(motion.offsets());
    if let Some(art) = &walk.art {
        let mut art = art.write().unwrap_or_else(|e| e.into_inner());
        let spin = motion.spinning();
        art.spin = spin.map(|(k, _)| k);
        if let Some(pose) = spin {
            art.pose_mode = Some(pose);
        }
    }
}

/// Adds [`SkillMotion`] and [`skill_motion_frame`] after the walk frame.
pub fn add_skill_motion(app: &mut App) {
    app.init_resource::<SkillMotion>().add_systems(
        PreUpdate,
        skill_motion_frame
            .after(preview_walk_frame)
            .after(bridge_frame)
            .before(crate::bridge::mirror::mirror_units)
            .run_if(resource_exists::<BridgeResource>)
            .run_if(resource_exists::<PreviewWalk>)
            .run_if(resource_exists::<WorldViewState>),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: specs/render/unit-composite.md §8 r1, §8 r2, §8 r6
    #[test]
    fn gravity_follows_the_two_branches_and_the_floor() {
        assert_eq!(leap_gravity(10), 5762);
        assert_eq!(leap_gravity(26), 8462 - 270 * 26);
        assert_eq!(leap_gravity(27), 500);
        assert_eq!(leap_gravity(100), 500);
    }

    // Covers: specs/render/unit-composite.md §8 r1, §8 r2, §8 r6
    #[test]
    fn timed_arc_starts_at_the_height_and_lands_after_n_updates() {
        let mut r = MotionRecord::default();
        timed_arc(&mut r, 0, 4);
        assert_eq!(r.acc[2], -0x1000);
        assert_eq!(r.ticks_left, 4);
        // vz = -az·n²/2 / n = 0x1000·16/2/4.
        assert_eq!(r.vel[2], 0x1000 * 4 / 2);
        let mut rec = r;
        let mut oz = Vec::new();
        for _ in 0..6 {
            rec.update(false, None).unwrap();
            oz.push(rec.offset[2]);
        }
        assert_eq!(rec.flags & motion::DONE, motion::DONE);
        assert!(oz.iter().any(|&z| z < 0), "the unit rises: {oz:?}");
    }

    // Covers: specs/render/unit-composite.md §8 r1, §8 r2, §8 r6
    #[test]
    fn leap_record_needs_a_speed() {
        assert_eq!(leap_record(10, 0), None);
        let r = leap_record(10, 6).unwrap();
        // (10 << 16) / (6 << 12) = 26, minus one.
        assert_eq!(r.ticks_left, 25);
        assert_eq!(r.acc[2], -5762);
    }

    // Covers: specs/skills/bodies-2b.md §8.11
    #[test]
    fn spin_loops_from_frame_three() {
        let f: Vec<usize> = (0..8).map(|t| spin_frame(t, 256, 6)).collect();
        assert_eq!(f, [3, 4, 5, 3, 4, 5, 3, 4]);
        assert_eq!(spin_frame(5, 256, 2), 1);
    }
}
