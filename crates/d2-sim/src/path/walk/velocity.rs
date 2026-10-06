// Spec: specs/sim/pathing.md §8.1 (velocity), §8.2 (run stat list), §8.4 (velocity and direction toward the next point)
//! Velocity of a mode and the per-point aim. Integer arithmetic only.

use super::geom::{direction_vector, set_facing};
use super::seams::{count, index, WalkUnits};
use crate::path::coords::to_fp16_center;
use crate::path::record::{flags, DynamicPath, PATH_POINTS};
use crate::path::tables::PathTables;
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
    // Columns a (by passive skill), b (velocity modifier).
    if row[1] != 0 {
        return true;
    }
    if row[0] != 0 {
        if let Some(s) = u.used_skill(unit) {
            return s.skill_flags & 0x1 != 0 && s.skill_flags & 0x1000 == 0;
        }
    }
    false
}

/// The velocity half of `0x00623F50` (§8.1 rules 1–2, 4) for a player or
/// monster in `mode`. `None`: a mode without the velocity modifier (and
/// not knockback); the routine then sets only the animation rate and the
/// velocity keeps its previous value (§8.1 rule 4).
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
    // animstat row 4: (has base, base, stat).
    let [_, scale_base, scale_stat] = t.animstat[4];
    let raw = u.item_stat(unit, scale_stat as u16);
    let f = if raw != 0 {
        scale_base.wrapping_mul(raw) / (scale_base + raw)
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

/// Velocity setter `0x00648690` (§8.1 rule 3): +0x38 := 15 only when the
/// value differs; velocity and max velocity (+0x84) always.
pub fn set_velocity(path: &mut DynamicPath, v: i32) {
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
pub fn aim(t: &PathTables, path: &mut DynamicPath, ty: UnitType) {
    loop {
        let i = index(path).clamp(0, PATH_POINTS as i32 - 1) as usize;
        let p = path.point(i);
        if (to_fp16_center(p.x), to_fp16_center(p.y)) != (path.precise_x, path.precise_y) {
            break;
        }
        if index(path) >= count(path) - 1 {
            path.dir_vec_x = 0;
            path.dir_vec_y = 0;
            path.vel_vec_x = 0;
            path.vel_vec_y = 0;
            path.velocity = 0;
            path.cur_point = path.point_count;
            return;
        }
        path.cur_point += 1;
    }
    let i = index(path).clamp(0, PATH_POINTS as i32 - 1) as usize;
    let p = path.point(i);
    let (v, mut d) = direction_vector(
        t,
        (path.precise_x, path.precise_y),
        (to_fp16_center(p.x), to_fp16_center(p.y)),
    );
    if path.flags & flags::FACE_AWAY != 0 {
        d = d.wrapping_sub(32) & 63;
    }
    (path.dir_vec_x, path.dir_vec_y) = v;
    path.vel_vec_x = v.0.wrapping_mul(path.velocity) >> 8;
    path.vel_vec_y = v.1.wrapping_mul(path.velocity) >> 8;
    set_facing(t, path, ty, d as i32);
}
