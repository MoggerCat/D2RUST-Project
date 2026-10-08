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

use crate::rules::unit_composite::{motion, MotionRecord};

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

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: specs/render/unit-composite.md §8
    #[test]
    fn gravity_follows_the_two_branches_and_the_floor() {
        assert_eq!(leap_gravity(10), 5762);
        assert_eq!(leap_gravity(26), 8462 - 270 * 26);
        assert_eq!(leap_gravity(27), 500.max(1224 - 72 * 27));
        assert_eq!(leap_gravity(100), 500);
    }

    // Covers: specs/render/unit-composite.md §8
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

    // Covers: specs/render/unit-composite.md §8
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
