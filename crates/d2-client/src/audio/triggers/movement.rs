// Spec: specs/audio/triggers.md §5 (footsteps), §6 (monster idle voices)
//! Per-update unit sounds of `Unit\UnitSnd.cpp`: footsteps `0x004CAF60`,
//! the monster `Neutral` voice `0x004CB460`, `Init` `0x004CC380` and
//! `Flee` `0x004CB950`. Per unit and update the order is: neutral, then
//! footsteps (Randomness).

use super::modes::skill_voice;
use super::{class_record, sid, uniform_gap, Ctx, TriggerError, Unit, UnitSound, MONSTER, PLAYER};

/// Whether the per-update code calls the footstep rule for this unit and
/// mode (§5 r1). Players: modes with movement entry 1 (`0x00711E00`):
/// 2 WL, 3 RN, 6 TW, 19 KB. Monsters: 2 WL, 15 RN; class 15 (`foulcrow1`)
/// also 1; class 110 (`vulture1`) only 8.
pub fn footstep_called(unit_type: u8, class: i32, mode: u8) -> bool {
    match unit_type {
        PLAYER => matches!(mode, 2 | 3 | 6 | 19),
        MONSTER => match class {
            110 => mode == 8,
            15 => matches!(mode, 1 | 2 | 15),
            _ => matches!(mode, 2 | 15),
        },
        _ => false,
    }
}

/// Circular distance `0x004E4180` of `x` and `o` modulo `step` (§5 r4):
/// both reduced mod step (masked when step is a power of two), then the
/// minimum of |b − a|, |b − a − step|, |a − b − step|.
pub fn circular_distance(x: u32, o: u32, step: u32) -> u32 {
    let red = |v: u32| {
        if step.is_power_of_two() {
            v & (step - 1)
        } else {
            v % step
        }
    };
    let (a, b, st) = (red(x) as i64, red(o) as i64, step as i64);
    (b - a)
        .abs()
        .min((b - a - st).abs())
        .min((a - b - st).abs()) as u32
}

/// Floor lookup result for the material rule (§5 r8, `0x004CADB0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Floor {
    /// U has no room: k = 0.
    NoRoom,
    /// No tile found in U's room or the rooms near it: the default.
    NotFound,
    /// A tile was found; its DT1 material flags (`formats/dt1.md` +0x06).
    Tile(u16),
}

/// Footstep material k (§5 r8). `material1` = `soundenviron.Material 1`
/// of the current environment.
pub fn footstep_material(material1: i32, floor: Floor) -> u8 {
    let default = if (1..=6).contains(&material1) {
        material1 as u8
    } else {
        1
    };
    match floor {
        Floor::NoRoom => 0,
        Floor::NotFound => default,
        Floor::Tile(f) => {
            for (bit, k) in [
                (0x20, 1),
                (0x08, 2),
                (0x10, 3),
                (0x40, 4),
                (0x80, 6),
                (0x400, 5),
            ] {
                if f & bit != 0 {
                    return k;
                }
            }
            default
        }
    }
}

/// Footstep `0x004CAF60(U)` (§5 r2–r7), material `k` from
/// [`footstep_material`].
///
/// TODO(spec: audio/triggers.md §5 r4): the signedness of `f ± s` before
/// the reduction is not stated; u32 wrapping is used (exact when step is
/// a power of two).
pub fn footstep(cx: &mut Ctx, u: &Unit, us: &mut UnitSound, k: u8) -> Result<(), TriggerError> {
    // r2.
    let s = u.speed as u32;
    if s == 0 {
        return Ok(());
    }
    let (mut n, mut o, mut layer, mut p) = (2u32, 0u32, 0i32, 100u32);
    let monster = u.unit_type() == MONSTER;
    if monster {
        if let Some(r) = u.monsounds {
            n = r.fscnt;
            o = r.fsoff.wrapping_mul(256);
            layer = sid(r.footsteplayer);
            p = r.fsprb;
        }
    }
    if n == 0 {
        return Ok(());
    }
    // r3.
    let frames = u.frame_count;
    let f = u.frame;
    let period = frames / n.wrapping_mul(s).max(1);
    let elapsed = cx.c.wrapping_sub(us.last_footstep);
    if n > 1 && elapsed < 2 * period / 3 {
        return Ok(());
    }
    // r4.
    let step = frames / n;
    if step == 0 {
        return Err(TriggerError::FootstepStep { frames, count: n });
    }
    let d = |x: u32| circular_distance(x, o, step);
    if !(d(f) < d(f.wrapping_add(s)) && d(f) < d(f.wrapping_sub(s))) {
        return Ok(());
    }
    // r5.
    let mut id = if monster {
        u.monsounds.map_or(0, |r| sid(r.footstep))
    } else {
        class_record(u.class)?.footstep_base
    };
    if id == 0 {
        return Ok(());
    }
    if (2..=6).contains(&k) {
        id += 4 * (k as i32 - 1);
    }
    let running = if monster { u.mode == 15 } else { u.mode == 3 };
    if running {
        id += 24;
    }
    // r6.
    let h = cx.req(id, Some(u.key), 0);
    if h != 0 {
        let mut v = if u.is_local { 255 } else { 200 };
        if elapsed > 3 * period / 2 {
            v = v * 160 / 255;
        }
        cx.s.set_volume(h, v);
    }
    // r7.
    if layer != 0 && cx.s.roll(100) < p {
        cx.req(layer, Some(u.key), 0);
    }
    us.last_footstep = cx.c;
    Ok(())
}

/// Town levels (`0x0061AB00`, §6 r1.4).
pub const TOWN_LEVELS: [i32; 5] = [1, 40, 75, 103, 109];

/// `Neutral` idle voice (`0x004CB460` on monsters, §6 r1).
pub fn neutral(cx: &mut Ctx, u: &Unit, us: &mut UnitSound) {
    let b = u.base_class;
    let mode_ok =
        matches!(u.mode, 1 | 2 | 15) || (b == 110 && u.mode == 8) || (b == 136 && u.mode == 11);
    if !mode_ok {
        return;
    }
    let Some(r) = u.monsounds else {
        return;
    };
    let neutral = sid(r.neutral);
    if neutral == 0 {
        return;
    }
    // 1. `0x004CA900` (false while the sound system is off).
    let in_group = cx.s.sound_on() && {
        let base = cx.s.group_base(neutral);
        cx.s.unit_requests(u.key)
            .iter()
            .any(|&(_, id)| cx.s.group_base(id) == base)
    };
    let c = cx.c;
    let ok = !in_group
        && c.wrapping_sub(cx.g.last_idle_any) >= cx.g.idle_gap
        && c.wrapping_sub(us.last_voice) >= r.neutime
        && c.wrapping_sub(us.last_idle) >= r.neutime
        && !u.in_town
        && u.near_local;
    // 6.
    if !ok || cx.s.roll(15) != 0 {
        return;
    }
    cx.req(neutral, Some(u.key), 0);
    cx.g.last_idle_any = c;
    us.last_idle = c;
    cx.g.idle_gap = uniform_gap(cx);
}

/// `Init` voice (`0x004CC380`, §6 r2). TODO(spec: audio/triggers.md open
/// question 11): who calls it (assign or first sight) is the caller's.
pub fn init_voice(cx: &mut Ctx, u: &Unit, us: &mut UnitSound) {
    let Some(r) = u.monsounds else {
        return;
    };
    let init = sid(r.init);
    if init != 0 && !u.dying && us.last_idle == 0 {
        cx.req(init, Some(u.key), 0);
        cx.g.last_idle_any = cx.c;
        us.last_idle = cx.c;
        cx.g.idle_gap = uniform_gap(cx);
    } else if u.mode == 8 {
        skill_voice(cx, u, us, r, 8);
    }
}

/// §6 r4: flee voice gap (C).
pub const FLEE_GAP: u32 = 4;

/// `Flee` voice (event 17, `0x004CB950`, §6 r4).
pub fn flee(cx: &mut Ctx, u: &Unit, us: &mut UnitSound) {
    let Some(r) = u.monsounds else {
        return;
    };
    if u.unit_type() != MONSTER || cx.c.wrapping_sub(cx.g.last_voice_any) < FLEE_GAP {
        return;
    }
    let d = 6 + cx.s.roll(3);
    cx.req(sid(r.flee), Some(u.key), d);
    cx.voiced(us);
}
