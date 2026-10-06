// Spec: specs/sim/pathing.md §8.1 (velocity), §8.2 (run stat list), §8.4 (velocity and direction toward the next point)
//! Velocity of a mode and the per-point aim. Integer arithmetic only.

use super::geom::{centre, direction_vector, set_facing};
use super::seams::{flag, WalkPath, WalkUnits};
use super::tables::PathTables;
use crate::units::{UnitId, UnitType};

/// Stat 67 `velocitypercent`.
pub const STAT_VELOCITYPERCENT: u16 = 67;
/// Stat 96 `item_fastermovevelocity`.
pub const STAT_FASTERMOVE: u16 = 96;
/// Knockback velocity (`0x00623F50`).
pub const KNOCKBACK_VELOCITY: i32 = 0x1000;
/// Velocity percent floor.
pub const VELOCITY_PERCENT_FLOOR: i32 = 25;
/// First monster class of `velmod_monster_x`.
pub const MONSTER_CLASS_X: u32 = 410;

/// Whether the mode has the velocity modifier (`0x006214A0`, §8.1 rule 2).
fn has_modifier<U: WalkUnits + ?Sized>(t: &PathTables, u: &U, unit: UnitId, mode: u32) -> bool {
    let ty = u.unit_type(unit);
    let row = match ty {
        UnitType::Player => t.velmod_player.get(mode as usize),
        UnitType::Monster => {
            if u.monstats_velocity(unit).1 {
                return mode == 2 || mode == 15;
            }
            if u.class(unit) < MONSTER_CLASS_X {
                t.velmod_monster.get(mode as usize)
            } else {
                t.velmod_monster_x.get(mode as usize)
            }
        }
        _ => None,
    };
    let Some(row) = row else { return false };
    if row.b != 0 {
        return true;
    }
    if row.a != 0 {
        if let Some(s) = u.used_skill(unit) {
            return s.skill_flags & 0x1 != 0 && s.skill_flags & 0x1000 == 0;
        }
    }
    false
}

/// The velocity half of `0x00623F50` (§8.1 rules 1–2) for a player or
/// monster in `mode`. `None`: the spec gives no rule for a mode without
/// the modifier (velocity unchanged; TODO(spec: pathing.md §8.1, the
/// velocity of modes without the modifier)).
pub fn mode_velocity<U: WalkUnits + ?Sized>(
    t: &PathTables,
    u: &U,
    unit: UnitId,
    mode: u32,
) -> Option<i32> {
    let ty = u.unit_type(unit);
    if (ty == UnitType::Player && mode == 19) || (ty == UnitType::Monster && mode == 13) {
        return Some(KNOCKBACK_VELOCITY);
    }
    if !has_modifier(t, u, unit, mode) {
        return None;
    }
    let scale = t.animstat[4];
    let raw = u.item_stat(unit, scale.stat as u16);
    let f = if raw != 0 {
        scale.base.wrapping_mul(raw) / (scale.base + raw)
    } else {
        0
    };
    let p = (f + u.stat(unit, STAT_VELOCITYPERCENT)).max(VELOCITY_PERCENT_FLOOR);
    let base = match ty {
        UnitType::Player => u.charstats_velocity(unit).0 * 256,
        _ => u.monstats_velocity(unit).0 * 256,
    };
    Some(base.wrapping_mul(p) / 100)
}

/// Velocity setter `0x00648690` (§8.1 rule 3).
pub fn set_velocity(path: &mut WalkPath, v: i32) {
    if v != path.velocity {
        path.field_38 = 15;
    }
    path.velocity = v;
    path.max_velocity = v;
}

/// Stat 67 of the run stat list (§8.2); `None` when `WalkVelocity` is 0.
pub fn run_velocity_bonus(walk: i32, run: i32) -> Option<i32> {
    if walk == 0 {
        None
    } else {
        Some(100 * run / walk - 100)
    }
}

/// Velocity and direction toward the next point (`0x0064FE40`, §8.4).
pub fn aim(t: &PathTables, path: &mut WalkPath, ty: UnitType) {
    loop {
        let i = path.index.clamp(0, super::seams::MAX_POINTS as i32 - 1) as usize;
        let p = path.points[i];
        if (centre(p.x), centre(p.y)) != (path.precise_x, path.precise_y) {
            break;
        }
        if path.index >= path.count - 1 {
            path.dir_vec = (0, 0);
            path.vel_vec = (0, 0);
            path.velocity = 0;
            path.index = path.count;
            return;
        }
        path.index += 1;
    }
    let i = path.index.clamp(0, super::seams::MAX_POINTS as i32 - 1) as usize;
    let p = path.points[i];
    let (v, mut d) = direction_vector(
        t,
        (path.precise_x, path.precise_y),
        (centre(p.x), centre(p.y)),
    );
    if path.flags & flag::FACE_AWAY != 0 {
        d = d.wrapping_sub(32) & 63;
    }
    path.dir_vec = v;
    path.vel_vec = (
        v.0.wrapping_mul(path.velocity) >> 8,
        v.1.wrapping_mul(path.velocity) >> 8,
    );
    set_facing(t, path, ty, d as i32);
}
