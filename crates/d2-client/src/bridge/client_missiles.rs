// Spec: specs/missiles/client.md (§C1–§C4 the client create `0x004CD540`; §C6, §C7, §C9, §C10, §C13), specs/missiles/client-bodies.md (§B1, §B5 r1, r3), specs/missiles/missiles.md (§R2.1 the create record), specs/render/lighting.md (§8 missile row)
//! Client missile creation: the client create `0x004CD540` fills a
//! client-only type-3 unit in set C (`client/model.md` §2 r1) from a
//! 0x5C-byte create record ([`CreateRecord`]) and its `missiles` row
//! ([`ClientMissileRow`]), and gives it its light (§C4 r26). The missile's
//! own fields (frames, animation, velocity, target, skill, owner, pierce
//! count) are kept in [`ClientMissile`] beside the set-C unit.
//!
//! Each client update runs [`update`] on every set-C missile (§C6): the
//! light flicker, the row's client function (bodies 1, 11, 23 here), the
//! end `0x004D2D70` ([`end`], §C9) and the default removal (§C10 r1).
//!
//! The path is the straight missile path (`missiles.md` §R4.3, §R4.1):
//! built at the create, stepped by the default step, which ends the
//! missile on a wall (§C7 r5, r10; PROVISIONAL REC-451: no cell walk).
//!
//! Not modelled (each named where it would run): the motion record (§C3
//! r19, r21; §C7 r3), unit hits (§C7 r12), the init callback (§C4
//! r27), sounds (r28, §C9 r4.4, r6: audio), the umod callback (r29), the
//! town tests (§C6 r4, §C7 r8: no town flag in the client level rows),
//! the second pass (§C7 r13), the client hit functions (§C9 r4.3: a
//! handler error when a row names one), and every client function but
//! 1, 5, 8, 11, 23, 43, 60 and 63 (the missile is then left as it is). The aim nudge
//! (§C2 r8) reads the owner's direction from the record
//! ([`CreateRecord::owner_dir64`]; none given is a handler error).

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
    /// `LoopAnim` (§C7 r4).
    pub loop_anim: bool,
    /// `Flicker` (§C6 r2, `render/lighting.md` §8).
    pub flicker: u8,
    /// `CollideType` (§C1 mode, §C7 r9).
    pub collide_type: u8,
    /// `AlwaysExplode`, `ExplosionMissile` (i16), `pCltHitFunc` (i16)
    /// (§C9).
    pub always_explode: bool,
    pub explosion_missile: i16,
    pub clt_hit_func: i16,
    /// `CltSubMissile1`–`3` (i16) and `CltParam1`–`3` (i32)
    /// (`client-bodies.md` §B1 S1–S3, P1–P3).
    pub clt_sub: [i16; 3],
    pub clt_param: [i32; 3],
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
    /// The owner's direction (`0x00620100`, 0…63) for the aim nudge
    /// (§C2 r8), as the caller reads it from the unit's drawn pose
    /// (`world_view::UnitPose::dir64`); the model holds no client path.
    pub owner_dir64: Option<u8>,
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
    /// Unit flag 0x10000 (set by functions 2 and 11, §C13).
    pub flat: bool,
    /// The path's precise position (16.16 sub-tiles), direction vector
    /// (path +0x6A, +0x6E; 0 when the path has no point) and
    /// acceleration counter (+0x8C) (`missiles.md` §R4.1, §R4.3).
    pub pos: (u32, u32),
    pub dir_vec: (i32, i32),
    pub accel_counter: i32,
    /// Missile data +0x28 / +0x2C (`client-bodies.md` §B1 d28, d2C): set
    /// by the creator unless a body writes them.
    pub d28: i32,
    pub d2c: i32,
    /// The path new-step flag (path +0x34 bit 3, `client-bodies.md` §B2):
    /// the last path step entered a new sub-tile.
    pub new_step: bool,
}

/// The path tables of `sim/pathing.md` (the direction-vector `tan`
/// table, §8.3).
fn path_tables() -> Option<&'static d2_sim::path::tables::PathTables> {
    static T: std::sync::OnceLock<Option<d2_sim::path::tables::PathTables>> =
        std::sync::OnceLock::new();
    T.get_or_init(|| d2_sim::path::tables::PathTables::spec().ok())
        .as_ref()
}

/// The straight missile path compute (`missiles.md` §R4.3, type 4,
/// `0x006492F0`) from the precise start `pos` toward the target point
/// (cell centre): no point when the target is 100 or more away on an
/// axis or has a 0 coordinate; else the direction vector and direction
/// of `sim/pathing.md` §8.3 (§8.4 r2).
fn straight_path(pos: (u32, u32), target: (i32, i32)) -> Option<((i32, i32), u8)> {
    let (sx, sy) = ((pos.0 >> 16) as i32, (pos.1 >> 16) as i32);
    if (target.0 - sx).abs() > 99 || (target.1 - sy).abs() > 99 || target.0 == 0 || target.1 == 0 {
        return None;
    }
    let centre = |c: i32| ((c as u32) << 16) | 0x8000;
    let t = path_tables()?;
    Some(d2_sim::path::walk::geom::direction_vector(
        t,
        pos,
        (centre(target.0), centre(target.1)),
    ))
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
        let Some(o) = rec
            .origin
            .and_then(|k| w.units.get(&k).or_else(|| w.objclient.set_c.get(&k)))
        else {
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
    let mut target = rec.target;
    let (mut tx, mut ty) = (tx, ty);
    if v != 0 {
        if let Some(owner) = rec.owner {
            let o_cell = w.units.get(&owner).map(|u| u.cell());
            let nudge = match target {
                None => (tx, ty) == (x, y),
                Some(t) if t != owner => {
                    let t_cell = w.units.get(&t).map(|u| u.cell());
                    let same = t_cell.is_some() && t_cell == o_cell;
                    if same {
                        target = None;
                    }
                    same
                }
                Some(_) => false,
            };
            if nudge {
                // `0x004C51E0`: d := owner direction >> 3; (tx, ty) :=
                // owner position + (DX[d], DY[d]).
                const DX: [i32; 8] = [0, -1, -2, -1, 0, 1, 2, 1];
                const DY: [i32; 8] = [2, 1, 0, -1, -2, -1, 0, 1];
                let dir = rec.owner_dir64.ok_or(HandlerError::Invalid(
                    "missiles/client.md §C2 r8: the aim nudge needs the owner's direction",
                ))?;
                let d = usize::from((dir & 63) >> 3);
                let (ox, oy) = o_cell.unwrap_or_default();
                (tx, ty) = (i32::from(ox) + DX[d], i32::from(oy) + DY[d]);
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
    // r15: velocity, target; v ≠ 0 → the path built toward the target
    // (§R4.3: a target unit's position, else the point).
    m.target_unit = target;
    m.target_point = (tx, ty);
    m.velocity = v;
    m.pos = (((x as u32) << 16) | 0x8000, ((y as u32) << 16) | 0x8000);
    if v != 0 {
        let aim = match target.and_then(|t| w.units.get(&t)) {
            Some(t) => {
                let (ax, ay) = t.cell();
                (i32::from(ax), i32::from(ay))
            }
            None => (tx, ty),
        };
        if aim == (x, y) {
            // `pathing.md` §8.4 r1: the point is the position (and the
            // last): vectors and velocity 0.
            m.velocity = 0;
        } else if let Some((vec, dir)) = straight_path(m.pos, aim) {
            m.dir_vec = vec;
            // `pathing.md` §8.5: a missile without path flag 0x40 faces d.
            m.direction = dir & 63;
        }
    }
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

/// The client functions the model runs (§C12): the default step (1),
/// the bodies of §C13 named here and of `client-bodies.md` §B5 r1, r3.
pub const FN_DEFAULT_STEP: u16 = 1;
pub const FN_FLAT_AT_END: u16 = 11;
pub const FN_DEN_LIGHT: u16 = 23;
pub const FN_SUB_LOOP_FIRE: u16 = 5;
pub const FN_FOLLOW_OWNER: u16 = 43;
pub const FN_ORBIT_EVEN: u16 = 60;
pub const FN_ORBIT: u16 = 63;
pub const FN_TRAIL: u16 = 8;

/// The per-update dispatch `0x004D2C70` (§C6) of the set-C missile `key`.
pub fn update(
    w: &mut ClientWorld,
    rows: &[ClientMissileRow],
    key: UnitKey,
    lights: bool,
) -> Result<(), HandlerError> {
    let Some(class) = w.objclient.set_c.get(&key).map(|u| u.class) else {
        return Ok(());
    };
    // r1.
    let Some(row) = rows.get(class as usize).copied() else {
        remove(w, key);
        return Ok(());
    };
    // r2.
    if row.flicker != 0 {
        flicker(w, key, &row);
    }
    // r5 (r3–r4: the room and the town test are not modelled).
    match row.clt_do_func {
        FN_DEFAULT_STEP => default_step(w, rows, key, &row, lights),
        FN_FLAT_AT_END => {
            // §C13 11: at the animation end flag 0x10000 and the light
            // removed; else step. No countdown at the end.
            let m = w.objclient.missiles.get(&key).copied().unwrap_or_default();
            if m.frame + m.anim_speed >= m.anim_len {
                if let Some(m) = w.objclient.missiles.get_mut(&key) {
                    m.flat = true;
                }
                remove_light(w, key);
                Ok(())
            } else {
                default_step(w, rows, key, &row, lights)
            }
        }
        FN_DEN_LIGHT => {
            // §C13 23: frames left < 100 → 500; step (never expires).
            if let Some(m) = w.objclient.missiles.get_mut(&key) {
                if m.current < 100 {
                    m.current = 500;
                }
            }
            default_step(w, rows, key, &row, lights)
        }
        FN_TRAIL => trail(w, rows, key, &row, lights),
        FN_SUB_LOOP_FIRE => {
            fire_frames(w, key, &row);
            default_step(w, rows, key, &row, lights)
        }
        FN_FOLLOW_OWNER => follow_owner(w, rows, key, lights),
        FN_ORBIT_EVEN | FN_ORBIT => {
            let m = w.objclient.missiles.get(&key).copied().unwrap_or_default();
            if row.clt_do_func == FN_ORBIT || (m.total - m.current) % 2 == 0 {
                orbit(w, key);
            }
            default_step(w, rows, key, &row, lights)
        }
        // f ≤ 0: never stepped (§C6 r5); other functions: not modelled.
        _ => Ok(()),
    }
}

/// Function 8 `0x004D38D0` (`client.md` §C13, trails): S1 < 0 → remove.
/// When elapsed ≥ `InitSteps` and the last path step entered a new
/// sub-tile (§B2): S1 created with flags 1 at m's position, m's owner,
/// skill and level, then its init callback `0x004CC870` (m): the child's
/// P2 > 0 → rnd(P2) on **m's** seed (the child's motion z, − ⌊P2 / 2⌋;
/// the motion record is not modelled, the draw is made); the child's
/// direction := m's; its frame := rnd(`AnimLen`) on the child's seed <<
/// 8. Then step.
fn trail(
    w: &mut ClientWorld,
    rows: &[ClientMissileRow],
    key: UnitKey,
    row: &ClientMissileRow,
    lights: bool,
) -> Result<(), HandlerError> {
    let s1 = row.clt_sub[0];
    if s1 < 0 {
        remove(w, key);
        return Ok(());
    }
    let m = w.objclient.missiles.get(&key).copied().unwrap_or_default();
    if m.total - m.current >= i32::from(row.init_steps) && m.new_step {
        let rec = CreateRecord {
            flags: flag::POSITION,
            owner: m.owner,
            class: s1 as u32,
            x: (m.pos.0 >> 16) as i32,
            y: (m.pos.1 >> 16) as i32,
            skill: m.skill,
            level: m.level,
            ..CreateRecord::default()
        };
        if let Some(child) = create(w, rows, &rec, lights)? {
            let crow = rows.get(s1 as usize).copied().unwrap_or_default();
            let p2 = crow.clt_param[1];
            if p2 > 0 {
                rnd(w, key, p2);
            }
            let frame = rnd(w, child, i32::from(crow.anim_len)) << 8;
            if let Some(c) = w.objclient.missiles.get_mut(&child) {
                c.direction = m.direction;
                c.frame = frame;
            }
        }
    }
    default_step(w, rows, key, row, lights)
}

/// Function 5 `0x004D3540` (`client-bodies.md` §B5 r1) before its step:
/// f := frame >> 8, A := `SubStart`, Z := `SubStop`; f = A − 1 → frame
/// := (f + rnd(Z − A)) << 8; else left = A → frame := max(A − 3, 0) <<
/// 8; else left < A → frame := max(f − 2, 0) << 8.
fn fire_frames(w: &mut ClientWorld, key: UnitKey, row: &ClientMissileRow) {
    let Some(m) = w.objclient.missiles.get(&key).copied() else {
        return;
    };
    let (f, a, z) = (
        m.frame >> 8,
        i32::from(row.sub_start),
        i32::from(row.sub_stop),
    );
    let frame = if f == a - 1 {
        let r = rnd(w, key, z - a);
        Some((f + r) << 8)
    } else if m.current == a {
        Some((a - 3).max(0) << 8)
    } else if m.current < a {
        Some((f - 2).max(0) << 8)
    } else {
        None
    };
    if let (Some(frame), Some(m)) = (frame, w.objclient.missiles.get_mut(&key)) {
        m.frame = frame;
    }
}

/// `rnd(n)` on the missile's unit seed (`client-bodies.md` §B1: n < 1 →
/// 0, no draw).
fn rnd(w: &mut ClientWorld, key: UnitKey, n: i32) -> i32 {
    if n < 1 {
        return 0;
    }
    let Some(u) = w.objclient.set_c.get_mut(&key) else {
        return 0;
    };
    let Some((lo, hi)) = u.seed else {
        return 0;
    };
    let mut seed = Seed::new(lo, hi);
    let r = seed.roll(n) as i32;
    u.seed = Some((seed.lo, seed.hi));
    r
}

/// Function 43 `0x004D3070` (`client.md` §C13): no owner → remove; an
/// owner of type ≤ 1 that is dead → end(none, 0); else m takes the
/// owner's position and advances its animation with wrap; no step, no
/// countdown.
fn follow_owner(
    w: &mut ClientWorld,
    rows: &[ClientMissileRow],
    key: UnitKey,
    lights: bool,
) -> Result<(), HandlerError> {
    let owner = w
        .objclient
        .missiles
        .get(&key)
        .and_then(|m| m.owner)
        .and_then(|o| w.units.get(&o));
    let Some(o) = owner else {
        remove(w, key);
        return Ok(());
    };
    if o.key.unit_type <= MONSTER && o.is_dead() {
        end(w, rows, key, false, lights)?;
        return Ok(());
    }
    let (ox, oy) = o.cell();
    if let Some(u) = w.objclient.set_c.get_mut(&key) {
        u.position = Some((ox, oy));
    }
    if let Some(m) = w.objclient.missiles.get_mut(&key) {
        m.pos = (
            (u32::from(ox) << 16) | 0x8000,
            (u32::from(oy) << 16) | 0x8000,
        );
        m.frame += m.anim_speed;
        if m.anim_len > 0 && m.frame >= m.anim_len {
            m.frame -= m.anim_len;
        }
    }
    Ok(())
}

/// Functions 60 / 63 (`client-bodies.md` §B5 r3), the orbit: target
/// point := (x + (y − d2C), y − (x − d28)), a quarter turn around (d28,
/// d2C), and the path re-built toward it.
fn orbit(w: &mut ClientWorld, key: UnitKey) {
    let Some(m) = w.objclient.missiles.get_mut(&key) else {
        return;
    };
    let (x, y) = ((m.pos.0 >> 16) as i32, (m.pos.1 >> 16) as i32);
    let t = (x + (y - m.d2c), y - (x - m.d28));
    m.target_point = t;
    match straight_path(m.pos, t) {
        Some((vec, dir)) if t != (x, y) => {
            m.dir_vec = vec;
            m.direction = dir & 63;
        }
        _ => m.dir_vec = (0, 0),
    }
}

/// The light flicker `0x004CD1C0` (`render/lighting.md` §8): when the
/// frame has bits 0x300 clear, the missile has a light and its radius ≥
/// `Light`: target := `Light` + rnd(`Flicker`) on the missile's seed.
fn flicker(w: &mut ClientWorld, key: UnitKey, row: &ClientMissileRow) {
    let frame = w.objclient.missiles.get(&key).map_or(0, |m| m.frame);
    if frame & 0x300 != 0 {
        return;
    }
    let Some(id) = light_of(w, key) else {
        return;
    };
    if w.lights.radius(id).unwrap_or(0) < i32::from(row.light) {
        return;
    }
    let Some(u) = w.objclient.set_c.get_mut(&key) else {
        return;
    };
    let Some((lo, hi)) = u.seed else {
        return;
    };
    let mut seed = Seed::new(lo, hi);
    let r = seed.roll(i32::from(row.flicker)) as i32;
    u.seed = Some((seed.lo, seed.hi));
    w.lights.set_target(id, i32::from(row.light) + r);
}

/// The default step `0x004D30C0` (§C7) without the path (module doc).
fn default_step(
    w: &mut ClientWorld,
    rows: &[ClientMissileRow],
    key: UnitKey,
    row: &ClientMissileRow,
    lights: bool,
) -> Result<(), HandlerError> {
    let Some(m) = w.objclient.missiles.get_mut(&key) else {
        return Ok(());
    };
    // r1.
    let active = m.current <= m.activate;
    // r2.
    if active && m.total - m.current > i32::from(row.init_steps) {
        if let Some(u) = w.objclient.set_c.get_mut(&key) {
            u.flag_ex &= !FLAG_EX_NOT_DRAWN;
        }
    }
    let m = w.objclient.missiles.get_mut(&key).expect("checked above");
    // r4.
    if active {
        if !row.loop_anim {
            if m.frame + m.anim_speed < m.anim_len {
                m.frame += m.anim_speed;
            }
        } else if row.sub_loop != 0
            && m.current > i32::from(row.anim_len) - i32::from(row.sub_stop) + 1
        {
            m.frame += m.anim_speed;
            if m.frame >= i32::from(row.sub_stop) << 8 {
                m.frame += (i32::from(row.sub_start) - i32::from(row.sub_stop)) << 8;
            }
        } else {
            m.frame += m.anim_speed;
            if m.frame >= m.anim_len {
                m.frame -= m.anim_len;
            }
        }
    }
    // r5: one path step.
    if m.velocity != 0 {
        path_step(m);
        let cell = (m.pos.0 >> 16, m.pos.1 >> 16);
        if let (Some(u), Ok(cx), Ok(cy)) = (
            w.objclient.set_c.get_mut(&key),
            u16::try_from(cell.0),
            u16::try_from(cell.1),
        ) {
            u.position = Some((cx, cy));
        }
    }
    let m = w.objclient.missiles.get_mut(&key).expect("checked above");
    // r6.
    m.current -= 1;
    if m.current < 1 {
        end(w, rows, key, false, lights)?;
        return Ok(());
    }
    // r7.
    if !active {
        return Ok(());
    }
    // r9 (r8, the town clamp, is not modelled).
    if row.collide_type == 0 {
        return Ok(());
    }
    // r10: the collision word under the missile; & 5 → end(none, 1).
    let at = w.objclient.set_c.get(&key).and_then(|u| u.position);
    if let Some((x, y)) = at {
        if collision_word(w, i32::from(x), i32::from(y)) & 5 != 0 {
            end(w, rows, key, true, lights)?;
        }
    }
    // r11–r13: unit hits and the second pass are not modelled.
    Ok(())
}

/// One path step of a missile (`missiles.md` §R4.1; `sim/pathing.md`
/// §9.4 rule 2.1, base 0x400): the acceleration counter, then the
/// position += ((velocity × 0x400) >> 6) × direction vector >> 12.
///
/// PROVISIONAL (REC-451): the step's cell walk (§9.4 rules 2.3–2.4, the
/// footprint move of a missile) is not run; the position moves by the
/// step vector and the wall test of §C7 r10 reads the cell it lands on.
fn path_step(m: &mut ClientMissile) {
    // §B2 r1: cleared first on every call.
    m.new_step = false;
    if m.dir_vec == (0, 0) {
        return;
    }
    if m.accel != 0 {
        m.accel_counter += 1;
        if m.accel_counter > 4 {
            m.velocity = (m.velocity + i32::from(m.accel)).clamp(0, m.max_vel);
            if m.velocity == m.max_vel {
                m.accel = 0;
            }
            m.accel_counter = 0;
        }
    }
    let k = 0x400i32.wrapping_mul(m.velocity) >> 6;
    let vx = k.wrapping_mul(m.dir_vec.0) >> 12;
    let vy = k.wrapping_mul(m.dir_vec.1) >> 12;
    let old = (m.pos.0 >> 16, m.pos.1 >> 16);
    m.pos = (
        m.pos.0.wrapping_add_signed(vx),
        m.pos.1.wrapping_add_signed(vy),
    );
    // §B2 r2 (without the cell walk, REC-451): a new sub-tile entered.
    m.new_step = (m.pos.0 >> 16, m.pos.1 >> 16) != old;
}

/// The collision word under a sub-tile (`0x00648EB0`) from the client
/// DRLG. PROVISIONAL (REC-451): a sub-tile in no room reads 0x27 (the
/// point test's no-room answer, `render/lighting.md` §4 r2); no client
/// DRLG reads 0.
fn collision_word(w: &ClientWorld, x: i32, y: i32) -> u32 {
    match w.drlg.as_ref() {
        Some(d) => d.drlg.collision_at(x, y).map_or(0x27, u32::from),
        None => 0,
    }
}

/// The end `0x004D2D70(m, none, forced)` (§C9) of a missile that hit no
/// unit. Returns the explosion missile made, if any.
pub fn end(
    w: &mut ClientWorld,
    rows: &[ClientMissileRow],
    key: UnitKey,
    forced: bool,
    lights: bool,
) -> Result<Option<UnitKey>, HandlerError> {
    let Some(class) = w.objclient.set_c.get(&key).map(|u| u.class) else {
        return Ok(None);
    };
    let row = rows.get(class as usize).copied().unwrap_or_default();
    let m = w.objclient.missiles.get(&key).copied().unwrap_or_default();
    let mut x = None;
    // r3: no unit, not forced, no `AlwaysExplode` → r5.
    if forced || row.always_explode {
        // r4.3.
        if row.clt_hit_func > 0 && row.clt_hit_func < 81 {
            return Err(HandlerError::Invalid(
                "missiles/client.md §C9 r4.3: client hit functions are not modelled",
            ));
        }
        // r4.5: `0x004CDBA0(m, E, 0, 0, skill, level)`: flags 0x20, the
        // owner m's owner (none → none), origin m.
        if row.explosion_missile >= 0 {
            if let Some(owner) = m.owner {
                let rec = CreateRecord {
                    flags: flag::TARGET_ABSOLUTE,
                    owner: Some(owner),
                    origin: Some(key),
                    class: row.explosion_missile as u32,
                    skill: m.skill,
                    level: m.level,
                    ..CreateRecord::default()
                };
                x = create(w, rows, &rec, lights)?;
                if let Some(xk) = x {
                    // X's direction: rnd(64) on X's seed for 146
                    // `spidergoo`, else m's (the motion copy is not
                    // modelled).
                    let dir = if row.explosion_missile == 146 {
                        let u = w.objclient.set_c.get_mut(&xk).expect("just made");
                        u.seed.map(|(lo, hi)| {
                            let mut s = Seed::new(lo, hi);
                            let d = s.roll(64) as u8;
                            u.seed = Some((s.lo, s.hi));
                            d
                        })
                    } else {
                        Some(m.direction)
                    };
                    if let (Some(d), Some(xm)) = (dir, w.objclient.missiles.get_mut(&xk)) {
                        xm.direction = d;
                    }
                }
            }
        }
    }
    // r7: m's light dies (`0x00474470`, `render/lighting.md` §6.2 r6).
    if let Some(id) = light_of(w, key) {
        let _ = w.lights.die(id);
    }
    // r8 (r = 3): m removed from set C.
    super::objects::remove_client_unit(w, key);
    Ok(x)
}

/// The default removal `0x004CD390` (§C10 r1): the light removed, the
/// unit removed.
pub fn remove(w: &mut ClientWorld, key: UnitKey) {
    remove_light(w, key);
    super::objects::remove_client_unit(w, key);
}

fn light_owner(key: UnitKey) -> Owner {
    Owner {
        unit_type: u32::from(MISSILE),
        guid: key.guid,
        client_only: true,
    }
}

fn light_of(w: &ClientWorld, key: UnitKey) -> Option<crate::rules::lighting::records::LightId> {
    let owner = light_owner(key);
    w.lights
        .iter()
        .find(|(_, r)| r.owner() == Some(owner))
        .map(|(id, _)| id)
}

fn remove_light(w: &mut ClientWorld, key: UnitKey) {
    if let Some(id) = light_of(w, key) {
        let _ = w.lights.remove(id);
    }
}

#[cfg(test)]
#[path = "client_missiles_tests.rs"]
mod tests;
