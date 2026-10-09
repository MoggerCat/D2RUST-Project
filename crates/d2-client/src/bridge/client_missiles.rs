// Spec: specs/missiles/client.md (§C1–§C4 the client create `0x004CD540`), specs/missiles/missiles.md (§R2.1 the create record), specs/render/lighting.md (§8 missile row)
//! Client missile creation: the client create `0x004CD540` fills a
//! client-only type-3 unit in set C (`client/model.md` §2 r1) from a
//! 0x5C-byte create record ([`CreateRecord`]) and its `missiles` row
//! ([`ClientMissileRow`]), and gives it its light (§C4 r26). The missile's
//! own fields (frames, animation, velocity, target, skill, owner, pierce
//! count) are kept in [`ClientMissile`] beside the set-C unit.
//!
//! Not modelled here (each named where it would run): the path and the
//! motion record (§C3 r14–r15, r17, r19, r21: the model has no path for
//! set-C units; velocity and target are kept), the init callback (r27),
//! the sounds (r28: audio), the umod callback (r29), the per-update
//! dispatch and the client functions (§C6–§C13), the removals (§C10).

use std::collections::BTreeMap;

use d2_sim::rng::Seed;

use super::dispatch::HandlerError;
use super::world::{ClientWorld, UnitKey, MISSILE, MONSTER, PLAYER};
use crate::rules::lighting::records::{unit_light_pos, LightKind, Owner};

/// The `missiles` columns the client create reads (§C2–§C4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClientMissileRow {
    pub vel: i32,
    pub vel_lev: i32,
    pub max_vel: i32,
    pub accel: i16,
    /// `Range`, `LevRange` (signed i16 columns, §C3 r11).
    pub range: i16,
    pub lev_range: i16,
    pub sub_loop: u8,
    pub sub_start: u8,
    pub sub_stop: u8,
    pub activate: i32,
    pub init_steps: u8,
    pub anim_len: u8,
    pub anim_speed: u8,
    /// `Light` and `Red`, `Green`, `Blue` (`render/lighting.md` §8).
    pub light: u8,
    pub rgb: (u8, u8, u8),
    pub can_slow: bool,
    pub pierce: bool,
    pub last_collide: bool,
    /// `pCltDoFunc` (§C6 r5).
    pub clt_do_func: u16,
}

/// The create record (`missiles.md` §R2.1, 0x5C bytes) as the client
/// create reads it (§C4 last paragraph: flags 1, 2, 4, 8, 0x10, 0x20,
/// 0x100, 0x200, 0x400, 0x800, 0x2000, 0x4000, 0x8000).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CreateRecord {
    pub flags: u32,
    /// +0x04 owner, +0x08 origin, +0x0C target unit.
    pub owner: Option<UnitKey>,
    pub origin: Option<UnitKey>,
    pub target: Option<UnitKey>,
    /// +0x10.
    pub class: u32,
    /// +0x14, +0x18 (flag 1).
    pub x: i32,
    pub y: i32,
    /// +0x1C, +0x20 (flag 2 relative, flag 0x20 absolute).
    pub tx: i32,
    pub ty: i32,
    /// +0x28 velocity (flag 4).
    pub velocity: i32,
    /// +0x2C skill, +0x30 level.
    pub skill: i32,
    pub level: i32,
    /// +0x34 loops (flag 8).
    pub loops: i32,
    /// +0x40 start frame (flag 0x200), +0x44 activate frames (flag
    /// 0x800), +0x4C range (flag 0x8000).
    pub start_frame: i32,
    pub activate: i32,
    pub range: i32,
    /// +0x50 light byte (§C4 r26).
    pub light: u8,
}

/// Record flags (§R2.1).
pub mod flag {
    pub const POSITION: u32 = 0x1;
    pub const TARGET_RELATIVE: u32 = 0x2;
    pub const VELOCITY: u32 = 0x4;
    pub const LOOPS: u32 = 0x8;
    pub const VELOCITY_FIXED: u32 = 0x10;
    pub const TARGET_ABSOLUTE: u32 = 0x20;
    pub const ARC: u32 = 0x100;
    pub const START_FRAME: u32 = 0x200;
    pub const FRAMES_FROM_DISTANCE: u32 = 0x400;
    pub const ACTIVATE: u32 = 0x800;
    pub const RANDOM_DIRECTION: u32 = 0x2000;
    pub const NO_LIGHT: u32 = 0x4000;
    pub const RANGE: u32 = 0x8000;
}

/// A client missile's own fields (§C1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClientMissile {
    /// Missile data: activate frame (+0x08), skill (+0x0A), level
    /// (+0x0C), total (+0x0E), current = frames left (+0x10).
    pub activate: i32,
    pub skill: i32,
    pub level: i32,
    pub total: i32,
    pub current: i32,
    /// Animation frame, length, speed (+0x44, +0x48, +0x4C; 8.8).
    pub frame: i32,
    pub anim_len: i32,
    pub anim_speed: i32,
    /// Path: velocity (8.8 sub-tiles), target point, target unit,
    /// direction (0…63), acceleration, max velocity.
    pub velocity: i32,
    pub target_point: (i32, i32),
    pub target_unit: Option<UnitKey>,
    pub direction: u8,
    pub accel: i16,
    pub max_vel: i32,
    /// The last-collided unit (§C3 r17, with `LastCollide`).
    pub last_collided: Option<UnitKey>,
    /// Owner (+0x94 / +0x98, flag-ex 0x400).
    pub owner: Option<UnitKey>,
    /// Stat 328, the pierce count (§C4 r24).
    pub pierce: u32,
}

/// The client missiles of set C, by key.
pub type ClientMissiles = BTreeMap<UnitKey, ClientMissile>;

/// Flag-ex 0x40000: not drawn while in `InitSteps` (§C1, §C3 r16).
pub const FLAG_EX_NOT_DRAWN: u32 = 0x40000;

/// The client create `0x004CD540` (§C2–§C4). `Ok(None)`: the create
/// returned none. `lights` is the light-quality flag `[0x0072A348]` ≠ 0
/// (high quality, §C4 r26).
pub fn create(
    w: &mut ClientWorld,
    rows: &[ClientMissileRow],
    rec: &CreateRecord,
    lights: bool,
) -> Result<Option<UnitKey>, HandlerError> {
    // r1.
    let Some(row) = rows.get(rec.class as usize).copied() else {
        return Ok(None);
    };
    // r2.
    let (x, y) = if rec.flags & flag::POSITION != 0 {
        (rec.x, rec.y)
    } else {
        let Some(o) = rec.origin.and_then(|k| w.units.get(&k)) else {
            return Ok(None);
        };
        let (x, y) = o.cell();
        (i32::from(x), i32::from(y))
    };
    // r3: the room containing (x, y) searched from the owner's room (no
    // owner: the local player's), else the client DRLG lookup.
    let from = match rec.owner {
        Some(o) => Some(o),
        None => w.local_player,
    };
    if let Some(from) = from {
        if w.active_rooms.is_some() {
            let start = w.unit_room(from).copied();
            let found = start
                .and_then(|r| w.cell_lookup(&r, x, y))
                .or_else(|| point_room(w, x, y));
            if found.is_none() {
                return Ok(None);
            }
        }
    }
    // r4.
    let (tx, ty) = if rec.flags & flag::TARGET_RELATIVE != 0 {
        (x + rec.tx, y + rec.ty)
    } else if rec.flags & flag::TARGET_ABSOLUTE != 0 {
        (rec.tx, rec.ty)
    } else {
        (x, y)
    };
    // r5.
    let mut v = if rec.flags & flag::VELOCITY == 0 {
        row.vel
            .wrapping_add(row.vel_lev.wrapping_mul(rec.level) / 8)
            .wrapping_shl(8)
    } else if rec.flags & flag::VELOCITY_FIXED != 0 {
        rec.velocity
    } else {
        rec.velocity.wrapping_shl(8)
    };
    // r6.
    let mut s = 100;
    if let Some(o) = rec.owner.and_then(|k| w.units.get(&k)) {
        if row.can_slow && o.states.contains(&87) {
            if let Some(list) = o.state_lists.get(&87) {
                s = list.get(&(161, 0)).copied().unwrap_or(0);
                v = d2_sim::combat::pct(v, s, 100);
            }
        }
    }
    // r7.
    if v != 0 {
        v = if v <= 0x100000 {
            (i64::from(v) * 75 / 100) as i32
        } else {
            (v / 100) * 75
        };
    }
    // r8: the aim. Owner none: nothing.
    let target = rec.target;
    if v != 0 {
        if let Some(owner) = rec.owner {
            let o_cell = w.units.get(&owner).map(|u| u.cell());
            let nudge = match target {
                None => (tx, ty) == (x, y),
                Some(t) if t != owner => {
                    let t_cell = w.units.get(&t).map(|u| u.cell());
                    t_cell.is_some() && t_cell == o_cell
                }
                Some(_) => false,
            };
            if nudge {
                // `0x004C51E0` reads the owner's direction (`0x00620100`),
                // which the model does not hold.
                return Err(HandlerError::Invalid(
                    "missiles/client.md §C2 r8: the aim nudge needs the owner's direction",
                ));
            }
            let aim = match target.and_then(|t| w.units.get(&t)) {
                Some(t) => {
                    let (ax, ay) = t.cell();
                    (i32::from(ax), i32::from(ay))
                }
                None => (tx, ty),
            };
            if (aim.0 - x).abs() >= 100 || (aim.1 - y).abs() >= 100 {
                return Ok(None);
            }
        }
    }
    // r9: allocate in set C (GUID counter, client room seed step).
    let (Ok(ux), Ok(uy)) = (u16::try_from(x), u16::try_from(y)) else {
        return Ok(None);
    };
    let Some(key) = super::objects::create_client_unit(w, MISSILE, rec.class, ux, uy) else {
        return Ok(None);
    };
    let mut m = ClientMissile {
        // r10.
        anim_len: i32::from(row.anim_len) << 8,
        anim_speed: (i32::from(row.anim_speed) << 4).clamp(-0x8000, 0x7FFF),
        skill: rec.skill,
        level: rec.level,
        accel: row.accel,
        max_vel: row.max_vel << 8,
        ..ClientMissile::default()
    };
    // r11.
    let mut f = if rec.flags & flag::RANGE != 0 {
        rec.range
    } else {
        let mut f = i32::from(row.range) + i32::from(row.lev_range) * rec.level;
        if rec.flags & flag::LOOPS != 0 && row.sub_loop != 0 {
            f += (i32::from(row.sub_stop) - i32::from(row.sub_start)) * rec.loops;
        }
        f
    };
    if s != 0 && s != 100 {
        f += d2_sim::combat::pct(f, 100, s);
    }
    // r12.
    if rec.flags & flag::START_FRAME != 0 {
        f -= rec.start_frame;
        m.frame = rec.start_frame << 8;
    }
    // r13.
    let f = f.clamp(-0x8000, 0x7FFF);
    m.total = f;
    m.current = f;
    m.activate = f - if rec.flags & flag::ACTIVATE != 0 {
        rec.activate
    } else {
        row.activate
    };
    // r15 (the path itself is not modelled): velocity and target.
    m.target_unit = target;
    m.target_point = (tx, ty);
    m.velocity = v;
    // r17.
    if row.last_collide {
        m.last_collided = rec.owner;
    }
    let unit = w.objclient.set_c.get_mut(&key).expect("just created");
    // r16.
    if row.init_steps != 0 {
        unit.flag_ex |= FLAG_EX_NOT_DRAWN;
    }
    // r18: direction := rnd(64) on the missile's seed.
    if rec.flags & flag::RANDOM_DIRECTION != 0 {
        if let Some((lo, hi)) = unit.seed {
            let mut seed = Seed::new(lo, hi);
            m.direction = seed.roll(64) as u8;
            unit.seed = Some((seed.lo, seed.hi));
        }
    }
    // r20: frames from the distance (`sim/pathing.md`: max + ⌊min / 2⌋).
    if rec.flags & flag::FRAMES_FROM_DISTANCE != 0 {
        let (dx, dy) = ((tx - x).abs(), (ty - y).abs());
        let d = (dx.max(dy) + dx.min(dy) / 2).max(1);
        let vv = (v as u32).wrapping_shl(4);
        if let Some(n) = ((d as u32) << 16).checked_div(vv) {
            m.total = n as i32;
            m.current = n as i32;
        }
    }
    // r21: direction := (tx + ty) & 63 (the timed arc of the motion
    // record is not modelled).
    if rec.flags & flag::ARC != 0 {
        m.direction = ((tx + ty) & 63) as u8;
    }
    // r23–r24.
    if let Some(owner) = rec.owner {
        m.owner = Some(owner);
        unit.flag_ex |= 0x400;
        m.pierce = pierce_count(w, &row, owner);
    }
    w.objclient.missiles.insert(key, m);
    // r26: the light (`render/lighting.md` §8 missile row).
    missile_light(w, key, &row, rec, lights);
    Ok(Some(key))
}

/// The room of a point through the client DRLG (`0x00619DA0`).
fn point_room(w: &ClientWorld, x: i32, y: i32) -> Option<super::world::ActiveRoom> {
    let rooms = w.active_rooms.as_deref()?;
    super::world::room_of_point(rooms, x, y).copied()
}

/// The pierce count `0x004CD420` (§C4 r24) of a missile owned by `o`.
fn pierce_count(w: &ClientWorld, row: &ClientMissileRow, o: UnitKey) -> u32 {
    if !row.pierce || !matches!(o.unit_type, PLAYER | MONSTER) {
        return 0;
    }
    let p = w.total(o, 166, 0).wrapping_add(w.total(o, 156, 0));
    if p == 0 {
        return 0;
    }
    let (p, c) = if o.unit_type == PLAYER && w.local_player != Some(o) {
        (100, 0)
    } else {
        (p, w.units.get(&o).map_or(0, |u| u.stat(328)))
    };
    let mut seed = Seed::init_low(c as u32);
    let mut n = 0;
    for _ in 0..4 {
        let lo = seed.step();
        if ((lo % 100) as i32) < p {
            n += 1;
        } else {
            break;
        }
    }
    n
}

/// §C4 r26: low quality or flag 0x4000 → the unit's light removed; else
/// `Light` ≠ 0 or record byte +0x50 ≠ 0 → removed and created
/// (`render/lighting.md` §8 missile row: kind 1, radius `Light`, the
/// row's colour; §8 r5: radius 0 creates nothing).
fn missile_light(
    w: &mut ClientWorld,
    key: UnitKey,
    row: &ClientMissileRow,
    rec: &CreateRecord,
    lights: bool,
) {
    let owner = Owner {
        unit_type: u32::from(MISSILE),
        guid: key.guid,
        client_only: true,
    };
    let old = w
        .lights
        .iter()
        .find(|(_, r)| r.owner() == Some(owner))
        .map(|(id, _)| id);
    if !lights || rec.flags & flag::NO_LIGHT != 0 {
        if let Some(id) = old {
            let _ = w.lights.remove(id);
        }
        return;
    }
    if row.light == 0 && rec.light == 0 {
        return;
    }
    if let Some(id) = old {
        let _ = w.lights.remove(id);
    }
    let Some(u) = w.objclient.set_c.get(&key) else {
        return;
    };
    let (x, y) = u.cell();
    let (r, g, b) = row.rgb;
    w.lights.create(
        Some(owner),
        (
            unit_light_pos(i32::from(x) << 16),
            unit_light_pos(i32::from(y) << 16),
        ),
        LightKind::Plain,
        i32::from(row.light),
        crate::rules::lighting::sources::SOURCE_INTENSITY,
        r,
        g,
        b,
    );
}

#[cfg(test)]
#[path = "client_missiles_tests.rs"]
mod tests;
