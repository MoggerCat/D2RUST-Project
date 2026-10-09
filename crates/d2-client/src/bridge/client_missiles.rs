// Spec: specs/missiles/client.md (§C1–§C4 the client create `0x004CD540`; §C6–§C10, §C13), specs/missiles/client-bodies.md (§B1, §B2, §B3 r1, r2, §B4 3, 4, 6, 25, 49, §B5 r1–r6), specs/missiles/missiles.md (§R2.1 the create record, §R4 r6), specs/sim/pathing.md (§9.4, §9.6), specs/sim/path-placement.md (§4, §6 r3), specs/render/lighting.md (§8 missile row)
//! Client missile creation: the client create `0x004CD540` fills a
//! client-only type-3 unit in set C (`client/model.md` §2 r1) from a
//! 0x5C-byte create record ([`CreateRecord`]) and its `missiles` row
//! ([`ClientMissileRow`]), and gives it its light (§C4 r26). The missile's
//! own fields (frames, animation, velocity, target, skill, owner, pierce
//! count) are kept in [`ClientMissile`] beside the set-C unit.
//!
//! Each client update runs [`update`] on every set-C missile (§C6): the
//! light flicker, the row's client function (here and in `bodies`), the
//! end `0x004D2D70` ([`end`], §C9) and the default removal (§C10 r1).
//!
//! The path is the straight missile path (`missiles.md` §R4.3, §R4.1):
//! built at the create, stepped by the default step through the
//! movement and cell walk of `sim/pathing.md` §9.4, §9.6 over the client
//! DRLG (the missile move of `sim/path-placement.md` §6 r3), which ends
//! the missile on a missile wall (§C7 r5, r10).
//!
//! The motion record (`render/unit-composite.md` §8) is made by the
//! create (§C3 r19, r21), updated before each dispatch and ends a landed
//! arc (§C7 r3). The town tests (§C6 r4, §C7 r8) read the active room of
//! the missile's sub-tile.
//!
//! Unit hits (§C7 r12) search the saved steps of the walk with the
//! collide tests of §C8 and end the missile on the unit (§C9 r2, r4–r8);
//! the client hit functions are in `hits` (§B6–§B7, §B12).
//!
//! Not modelled (each named where it would run): the init callback (§C4
//! r27), sounds (r28, §C9 r4.4, r6: audio), the umod callback (r29), the
//! client event hooks (§C9 r4.2), the second pass (§C7 r13), the hit
//! functions not in `hits` (read as returning non-zero, PROVISIONAL
//! REC-452), and the client functions not listed in [`update_with`]'s
//! dispatch (the missile is then left as it is). The aim nudge (§C2 r8)
//! reads the owner's direction from the record
//! ([`CreateRecord::owner_dir64`]; none given is a handler error).

use std::collections::BTreeMap;

use d2_sim::rng::Seed;

use super::dispatch::HandlerError;
use super::world::{ClientWorld, UnitKey, MISSILE, MONSTER, PLAYER};
use crate::rules::lighting::records::{unit_light_pos, LightKind, Owner};
use crate::rules::unit_composite::MotionRecord;
use d2_sim::path::collision::{find_room, size_value};
use d2_sim::path::footprint::missile_move;
use d2_sim::path::CollisionRooms;
use d2_sim::units::RoomId;

#[path = "client_missiles_bodies.rs"]
mod bodies;
#[path = "client_missiles_hits.rs"]
mod hits;

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
    /// `CltSubMissile1`–`3` (i16) and `CltParam1`–`5` (i32)
    /// (`client-bodies.md` §B1 S1–S3, P1–P3; `client-bodies-2.md` §B8
    /// P4, P5).
    pub clt_sub: [i16; 3],
    pub clt_param: [i32; 5],
    /// `CltCalc1` (+0x84): the offset of its formula in the `misscode`
    /// buffer (`client-bodies.md` §B4 function 3).
    pub clt_calc1: u32,
    /// `Town` (flags bit 10, §C6 r4) and `CltSrcTown` (§C7 r8).
    pub town: bool,
    pub clt_src_town: u8,
    /// `Size` (+0x18A), the missile's unit size (`sim/path-placement.md`
    /// §3).
    pub size: u8,
    /// `RandStart` (the disc's frame offset, `client-bodies.md` §B3 r3).
    pub rand_start: i32,
    /// `ProgSound` (i16; function 47, `client-bodies-2.md` §B11).
    pub prog_sound: i16,
    /// The server columns `Param1`, `Param2` (+0x38, +0x3C; function 58
    /// reads `Param1`, `client-bodies-2.md` Edge case 7).
    pub param: [i32; 2],
    /// `ClientCol`, `CollideKill`, `CollideFriend`, `NextHit`,
    /// `NextDelay`, `CanDestroy` (§C7 r12, §C8, §C9).
    pub client_col: bool,
    pub collide_kill: bool,
    pub collide_friend: bool,
    pub next_hit: bool,
    pub next_delay: u8,
    pub can_destroy: bool,
    /// `CltHitSubMissile1`–`4` (i16) and `cHitPar1`–`3` (i32), the hit
    /// functions' H1–H4 and c1–c3 (`client-bodies.md` §B1).
    pub clt_hit_sub: [i16; 4],
    pub c_hit_par: [i32; 3],
    /// The server column `HitSubMissile1` (i16; hit 26 gates on it,
    /// `client-bodies-2.md` Edge case 3).
    pub hit_sub1_server: i16,
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
    /// +0x24 arc height (flag 0x100, §C3 r21).
    pub arc_height: i32,
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
    /// Missile data +0x04 / +0x06 (`client-bodies.md` §B1 d04, d06; i16):
    /// written only by client skill functions, 0 otherwise.
    pub d04: i16,
    pub d06: i16,
    /// The path new-step flag (path +0x34 bit 3, `client-bodies.md` §B2):
    /// the last path step entered a new sub-tile.
    pub new_step: bool,
    /// The client motion record (gfx +0x30, `render/unit-composite.md`
    /// §8): created and restarted by the create (§C3 r19), a timed arc
    /// with flag 0x100 (r21), updated before each dispatch (§C6).
    pub motion: MotionRecord,
    /// The path's room (path +0x1C; `sim/pathing.md` §9.6 r9), its
    /// collided mask (+0x54, the OR of the last step's missile moves),
    /// the saved steps of the last cell walk (+0x1D4 / +0x1D8, at most 10)
    /// and a point count of 0 (`stopped`: the step entered no room,
    /// §9.6 r8).
    pub room: Option<RoomId>,
    pub collided: u16,
    pub steps: [(i32, i32); 10],
    pub step_count: u8,
    pub stopped: bool,
}

/// m's sub-tile (the path position, `client-bodies.md` §B1 (x, y)).
fn cell_of(m: &ClientMissile) -> (i32, i32) {
    ((m.pos.0 >> 16) as i32, (m.pos.1 >> 16) as i32)
}

/// Re-path `0x00649970(path, m, 0)` toward m's target point (a target
/// unit is dropped by the point setter `0x00648AD0`): the straight path
/// of §R4.3; no point → the direction vector is 0 (no step).
fn repath(m: &mut ClientMissile) {
    repath_to(m, m.target_point);
}

/// Re-path toward `t` (the target unit's position or the target point).
fn repath_to(m: &mut ClientMissile, t: (i32, i32)) {
    match straight_path(m.pos, t) {
        Some((vec, dir)) if t != cell_of(m) => {
            m.dir_vec = vec;
            m.direction = dir & 63;
        }
        _ => m.dir_vec = (0, 0),
    }
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
    m.room = w.drlg.as_ref().and_then(|d| drlg_room_at(&d.drlg, x, y));
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
    // r19: the motion record created (zeroed) and restarted.
    m.motion.restart();
    // r21: the timed arc (height +0x24, n = total frames), one motion
    // update, direction := (tx + ty) & 63 (path flag 0x40: the facing
    // stays, `sim/pathing.md` §8.5).
    if rec.flags & flag::ARC != 0 {
        m.motion.timed_arc(rec.arc_height, m.total);
        motion_update(&mut m.motion)?;
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

/// A create made by the client missile `parent` (its bodies, §C9 r4.5):
/// [`create`] with the record's owner direction, when the record names
/// none, read as the parent missile's direction.
///
/// PROVISIONAL (REC-543): the aim nudge (§C2 r8) reads the owner's
/// direction `0x00620100`, which the model does not hold (unit
/// directions live in the view's poses); a child created by a missile
/// takes its parent's direction instead. Found on the user's rows (a
/// child with velocity aimed at its own start, `app_client_drlg`
/// `the_users_client_missiles_run_their_functions`).
fn create_from(
    w: &mut ClientWorld,
    env: &Env,
    parent: UnitKey,
    rec: &CreateRecord,
) -> Result<Option<UnitKey>, HandlerError> {
    let mut rec = *rec;
    if rec.owner_dir64.is_none() {
        rec.owner_dir64 = w.objclient.missiles.get(&parent).map(|m| m.direction);
    }
    create(w, env.rows, &rec, env.lights)
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
pub const FN_SCATTER: u16 = 4;
pub const FN_MIST: u16 = 25;
pub const FN_SPAWN_FACING: u16 = 49;
pub const FN_WALL_MAKER: u16 = 6;
pub const FN_BLOOD: u16 = 2;
pub const FN_SUB_AT_STEP: u16 = 3;
pub const FN_HEIGHT_WINDOW: u16 = 59;
pub const FN_SPIRIT: u16 = 65;
pub const FN_GUIDED: u16 = 7;
pub const FN_METEOR: u16 = 9;
pub const FN_CURSE_CENTRE: u16 = 17;
pub const FN_SPEAR_TRAIL: u16 = 18;
pub const FN_WANDER_MAKER: u16 = 27;
pub const FN_DIABLO_APPEARS: u16 = 37;
pub const FN_FRAME_ZERO: u16 = 39;
pub const FN_JAVELIN_TRAIL: u16 = 46;
pub const FN_RECYCLER: u16 = 51;
pub const FN_WAKE_MAKER: u16 = 52;
pub const FN_MON_BLIZZARD: u16 = 10;
pub const FN_BLIZZARD: u16 = 13;
pub const FN_FROZEN_ORB: u16 = 19;
pub const FN_ORB_NOVA: u16 = 20;
pub const FN_DISTRACTION: u16 = 44;
pub const FN_DISTRACTION_FOG: u16 = 45;
pub const FN_MOLTEN_BOULDER: u16 = 47;
pub const FN_ERUPTION: u16 = 48;
pub const FN_TIGER_FURY: u16 = 53;
pub const FN_CHAOS_ICE: u16 = 58;
pub const FN_SUC_FIREBALL: u16 = 68;

/// The per-update dispatch `0x004D2C70` (§C6) of the set-C missile `key`.
pub fn update(
    w: &mut ClientWorld,
    rows: &[ClientMissileRow],
    key: UnitKey,
    lights: bool,
) -> Result<(), HandlerError> {
    update_with(
        w,
        &Env {
            rows,
            lights,
            skills: None,
            monsters: &[],
        },
        key,
    )
}

/// What the client missile code reads beside the model: the `missiles`
/// rows, the light quality `[0x0072A348]` ≠ 0 (§C4 r26) and the skills
/// tables with the formula buffers (`ModelInputs::skill_tables`; the
/// missiles evaluator `0x0064B7C0` and the skill evaluator `0x00646CA0`
/// of the bodies; `None`: a body that evaluates is a handler error).
#[derive(Clone, Copy)]
pub struct Env<'a> {
    pub rows: &'a [ClientMissileRow],
    pub lights: bool,
    pub skills: Option<&'a super::passive::Tables>,
    /// The client `monstats` rows (`ClientTables::monsters`): monster
    /// sizes and set-ups for the distance and hostility tests.
    pub monsters: &'a [Option<super::world::MonsterClass>],
}

/// [`update`] with the skills tables ([`Env`]).
pub fn update_with(w: &mut ClientWorld, env: &Env, key: UnitKey) -> Result<(), HandlerError> {
    let Some(class) = w.objclient.set_c.get(&key).map(|u| u.class) else {
        return Ok(());
    };
    // The motion update runs before the dispatch (§C6,
    // `render/unit-composite.md` §8).
    if let Some(m) = w.objclient.missiles.get_mut(&key) {
        motion_update(&mut m.motion)?;
    }
    // r1.
    let Some(row) = env.rows.get(class as usize).copied() else {
        remove(w, key);
        return Ok(());
    };
    // r2.
    if row.flicker != 0 {
        flicker(w, key, &row);
    }
    // r3–r4: a row without `Town` in a town room → default removal.
    if !row.town && in_town(w, key) {
        remove(w, key);
        return Ok(());
    }
    // r5.
    match row.clt_do_func {
        FN_DEFAULT_STEP => default_step(w, env, key, &row),
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
                default_step(w, env, key, &row)
            }
        }
        FN_DEN_LIGHT => {
            // §C13 23: frames left < 100 → 500; step (never expires).
            if let Some(m) = w.objclient.missiles.get_mut(&key) {
                if m.current < 100 {
                    m.current = 500;
                }
            }
            default_step(w, env, key, &row)
        }
        FN_TRAIL => trail(w, env, key, &row),
        FN_SCATTER => {
            // §B4 4: scatter(m, P1, P2, P3, S1), then step.
            let [p1, p2, p3, ..] = row.clt_param;
            scatter(w, env, key, p1, p2, p3, i32::from(row.clt_sub[0]))?;
            default_step(w, env, key, &row)
        }
        FN_SPAWN_FACING => {
            // §B4 49: no row or S1 < 0 → remove; elapsed mod max(P1, 1)
            // = 0 → spawn facing(m, S1); step.
            let s1 = row.clt_sub[0];
            if s1 < 0 {
                remove(w, key);
                return Ok(());
            }
            let m = w.objclient.missiles.get(&key).copied().unwrap_or_default();
            if (m.total - m.current) % row.clt_param[0].max(1) == 0 {
                if let Some(x) = spawn(w, env, key, s1 as u32)? {
                    if let Some(xm) = w.objclient.missiles.get_mut(&x) {
                        xm.direction = m.direction;
                    }
                }
            }
            default_step(w, env, key, &row)
        }
        FN_MIST => mist(w, env, key, &row),
        FN_WALL_MAKER => wall_maker(w, env, key, &row),
        FN_SUB_LOOP_FIRE => {
            fire_frames(w, key, &row);
            default_step(w, env, key, &row)
        }
        FN_FOLLOW_OWNER => follow_owner(w, env, key),
        FN_ORBIT_EVEN | FN_ORBIT => {
            let m = w.objclient.missiles.get(&key).copied().unwrap_or_default();
            if row.clt_do_func == FN_ORBIT || (m.total - m.current) % 2 == 0 {
                orbit(w, key);
            }
            default_step(w, env, key, &row)
        }
        FN_BLOOD => bodies::blood(w, env, key, &row),
        FN_SUB_AT_STEP => bodies::sub_at_new_step(w, env, key, &row),
        FN_HEIGHT_WINDOW => bodies::height_window(w, env, key, &row),
        FN_SPIRIT => bodies::spirit(w, env, key, &row),
        FN_GUIDED => bodies::guided(w, env, key, &row),
        FN_METEOR => bodies::meteor(w, env, key, &row),
        FN_CURSE_CENTRE => bodies::curse_centre(w, env, key, &row),
        FN_SPEAR_TRAIL => bodies::spear_trail(w, env, key, &row),
        FN_WANDER_MAKER => bodies::wander_maker(w, env, key, &row),
        FN_FRAME_ZERO => bodies::frame_zero(w, env, key, &row),
        FN_JAVELIN_TRAIL | FN_WAKE_MAKER => bodies::pair_trail(w, env, key, &row),
        FN_RECYCLER => bodies::recycler(w, env, key, &row),
        FN_MON_BLIZZARD => bodies::mon_blizzard(w, env, key, &row),
        FN_BLIZZARD => bodies::blizzard(w, env, key, &row),
        FN_FROZEN_ORB => bodies::frozen_orb(w, env, key, &row),
        FN_ORB_NOVA => bodies::orb_nova(w, env, key, &row),
        FN_DISTRACTION => bodies::distraction(w, env, key, &row),
        FN_DISTRACTION_FOG => {
            // `client-bodies-2.md` §B11 45: S1 < 0 → remove; scatter(m,
            // P1, P2, P3, S1); step.
            let s1 = row.clt_sub[0];
            if s1 < 0 {
                remove(w, key);
                return Ok(());
            }
            let [p1, p2, p3, ..] = row.clt_param;
            scatter(w, env, key, p1, p2, p3, i32::from(s1))?;
            default_step(w, env, key, &row)
        }
        FN_MOLTEN_BOULDER => bodies::molten_boulder(w, env, key, &row),
        FN_ERUPTION => bodies::eruption(w, env, key, &row),
        FN_TIGER_FURY => bodies::tiger_fury(w, env, key, &row),
        FN_CHAOS_ICE => bodies::chaos_ice(w, env, key, &row),
        FN_SUC_FIREBALL => bodies::suc_fireball(w, env, key, &row),
        // §C13 37: frames left 150 → the shake (`render/camera.md` §8,
        // row `q-fix-shake-starts`), 50 → sound 4,638 (audio); both not
        // modelled; every branch steps.
        FN_DIABLO_APPEARS => default_step(w, env, key, &row),
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
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
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
        if let Some(child) = create_from(w, env, key, &rec)? {
            let crow = env.rows.get(s1 as usize).copied().unwrap_or_default();
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
    default_step(w, env, key, row)
}

/// Scatter `0x004CE140(m, chance, count, spread, c)` (`client-bodies.md`
/// §B3 r2): c out of range → nothing; frames left ≠ 0 and rnd(chance) ≠
/// 0 → nothing (left 0: no draw); else `count` creates (flags 0x20,
/// owner O, origin m, class c, m's skill and level), each aimed at (x +
/// dx + σ(dx)·spread, y + dy + σ(dy)·spread), dx, dy := rnd(2·spread) −
/// spread, σ(v) = −1 for v < 0, else +1.
#[allow(clippy::too_many_arguments)]
fn scatter(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    chance: i32,
    count: i32,
    spread: i32,
    c: i32,
) -> Result<(), HandlerError> {
    if c < 0 || c as usize >= env.rows.len() {
        return Ok(());
    }
    let m = w.objclient.missiles.get(&key).copied().unwrap_or_default();
    if m.current != 0 && rnd(w, key, chance) != 0 {
        return Ok(());
    }
    let (x, y) = ((m.pos.0 >> 16) as i32, (m.pos.1 >> 16) as i32);
    let sigma = |v: i32| if v < 0 { -1 } else { 1 };
    for _ in 0..count {
        let dx = rnd(w, key, 2 * spread) - spread;
        let dy = rnd(w, key, 2 * spread) - spread;
        let rec = CreateRecord {
            flags: flag::TARGET_ABSOLUTE,
            owner: m.owner,
            origin: Some(key),
            class: c as u32,
            tx: x + dx + sigma(dx) * spread,
            ty: y + dy + sigma(dy) * spread,
            skill: m.skill,
            level: m.level,
            ..CreateRecord::default()
        };
        create_from(w, env, key, &rec)?;
    }
    Ok(())
}

/// spawn(c) `0x004CDBA0(m, c, 0, 0, skill, level)` (`client-bodies.md`
/// §B1, `client.md` §C5): flags 0x20, origin m, owner m's owner (none →
/// none), target (0, 0) absolute.
fn spawn(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    c: u32,
) -> Result<Option<UnitKey>, HandlerError> {
    let m = w.objclient.missiles.get(&key).copied().unwrap_or_default();
    let Some(owner) = m.owner else {
        return Ok(None);
    };
    let rec = CreateRecord {
        flags: flag::TARGET_ABSOLUTE,
        owner: Some(owner),
        origin: Some(key),
        class: c,
        skill: m.skill,
        level: m.level,
        ..CreateRecord::default()
    };
    create_from(w, env, key, &rec)
}

/// Function 25 `0x004D4DC0` (`client-bodies.md` §B4, `towermist`): S1 <
/// 0 → remove; elapsed mod max(P3, 1) = 0 → u, v := rnd(2·P1 + 1) − P1,
/// sx, sy := x / y + rnd(2·P2 + 1) − P2 (in this order); a create with
/// flags 0x21, owner O, start (sx, sy), target (sx + u, sy + v), class
/// S1, skill, level. Step.
fn mist(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    let s1 = row.clt_sub[0];
    if s1 < 0 {
        remove(w, key);
        return Ok(());
    }
    let [p1, p2, p3, ..] = row.clt_param;
    let m = w.objclient.missiles.get(&key).copied().unwrap_or_default();
    if (m.total - m.current) % p3.max(1) == 0 {
        let (x, y) = ((m.pos.0 >> 16) as i32, (m.pos.1 >> 16) as i32);
        let u = rnd(w, key, 2 * p1 + 1) - p1;
        let v = rnd(w, key, 2 * p1 + 1) - p1;
        let sx = x + rnd(w, key, 2 * p2 + 1) - p2;
        let sy = y + rnd(w, key, 2 * p2 + 1) - p2;
        let rec = CreateRecord {
            flags: flag::POSITION | flag::TARGET_ABSOLUTE,
            owner: m.owner,
            class: s1 as u32,
            x: sx,
            y: sy,
            tx: sx + u,
            ty: sy + v,
            skill: m.skill,
            level: m.level,
            ..CreateRecord::default()
        };
        create_from(w, env, key, &rec)?;
    }
    default_step(w, env, key, row)
}

/// Function 6 `0x004D3630` (`client-bodies.md` §B4, `firewallmaker`):
/// the last path step entered no new sub-tile → step. S1 < 0 or out of
/// range → remove. Else a create with flags 0x21 (0x8021 with frames :=
/// S1's `Range` when that is ≠ 0), owner O, start = target = (x, y), m's
/// skill and level, of a class picked on m's seed (S2 and S3 ≥ 0: rnd(3)
/// 0 → S2, 1 → S3, 2 → S1; only S2: rnd(2) 0 → S2, else S1; else S1),
/// without a light (flag 0x4000) when P1 = 0 or rnd(P1) ≠ 0; the child
/// takes m's precise position. Step.
fn wall_maker(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
) -> Result<(), HandlerError> {
    let m = w.objclient.missiles.get(&key).copied().unwrap_or_default();
    if !m.new_step {
        return default_step(w, env, key, row);
    }
    let [s1, s2, s3] = row.clt_sub;
    if s1 < 0 || s1 as usize >= env.rows.len() {
        remove(w, key);
        return Ok(());
    }
    let (x, y) = ((m.pos.0 >> 16) as i32, (m.pos.1 >> 16) as i32);
    let mut rec = CreateRecord {
        flags: flag::POSITION | flag::TARGET_ABSOLUTE,
        owner: m.owner,
        x,
        y,
        tx: x,
        ty: y,
        skill: m.skill,
        level: m.level,
        ..CreateRecord::default()
    };
    let range = i32::from(env.rows[s1 as usize].range);
    if range != 0 {
        rec.flags |= flag::RANGE;
        rec.range = range;
    }
    let c = if s2 >= 0 && s3 >= 0 {
        match rnd(w, key, 3) {
            0 => s2,
            1 => s3,
            _ => s1,
        }
    } else if s2 >= 0 {
        if rnd(w, key, 2) == 0 {
            s2
        } else {
            s1
        }
    } else {
        s1
    };
    rec.class = c as u32;
    let p1 = row.clt_param[0];
    if p1 == 0 || rnd(w, key, p1) != 0 {
        rec.flags |= flag::NO_LIGHT;
    }
    if let Some(child) = create_from(w, env, key, &rec)? {
        if let Some(cm) = w.objclient.missiles.get_mut(&child) {
            cm.pos = m.pos;
        }
    }
    default_step(w, env, key, row)
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
fn follow_owner(w: &mut ClientWorld, env: &Env, key: UnitKey) -> Result<(), HandlerError> {
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
        end_with(w, env, key, None, false)?;
        return Ok(());
    }
    let (ox, oy) = o.cell();
    let room = w
        .drlg
        .as_ref()
        .and_then(|d| drlg_room_at(&d.drlg, i32::from(ox), i32::from(oy)));
    if let Some(u) = w.objclient.set_c.get_mut(&key) {
        u.position = Some((ox, oy));
    }
    if let Some(m) = w.objclient.missiles.get_mut(&key) {
        m.room = room;
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
    let (x, y) = cell_of(m);
    m.target_point = (x + (y - m.d2c), y - (x - m.d28));
    repath(m);
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
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
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
    // r3: the motion record reports "not moving" (a flag-0x100 arc that
    // landed) → end(none, 1).
    if !m.motion.moving() {
        end_with(w, env, key, None, true)?;
        return Ok(());
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
        path_step(w, env, key);
        let m = w.objclient.missiles.get(&key).expect("checked above");
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
        end_with(w, env, key, None, false)?;
        return Ok(());
    }
    // r7.
    if !active {
        return Ok(());
    }
    // r8: `CltSrcTown` T ≠ 0 and (no owner, or the owner's room is a
    // town room): frames left ≥ T → T, and without `LoopAnim` with
    // `AnimLen` − T > 0, frame := (`AnimLen` − T) << 8.
    let t = i32::from(row.clt_src_town);
    if t != 0 {
        let owner = w.objclient.missiles.get(&key).and_then(|m| m.owner);
        if owner.is_none_or(|o| super::modes::in_town(w, o)) {
            let m = w.objclient.missiles.get_mut(&key).expect("checked above");
            if m.current >= t {
                m.current = t;
                let rest = i32::from(row.anim_len) - t;
                if !row.loop_anim && rest > 0 {
                    m.frame = rest << 8;
                }
            }
        }
    }
    // r9.
    if row.collide_type == 0 {
        return Ok(());
    }
    // r10: the collision word under the missile; & 5 → end(none, 1).
    let word = collision_under(w, env, key);
    if word & 5 != 0 {
        end_with(w, env, key, None, true)?;
        return Ok(());
    }
    // r11.
    if row.collide_type == 6 {
        return Ok(());
    }
    // r12: `ClientCol` and a unit bit in the word: the unit search on
    // each saved step of the walk, in order; a living unit found ends
    // the missile on it, a dead one moves the search to the next point.
    if row.client_col && word & 0xFFFF != 0 {
        let m = w
            .objclient
            .missiles
            .get(&key)
            .copied()
            .expect("checked above");
        for &(x, y) in &m.steps[..usize::from(m.step_count)] {
            let found = unit_at_point(w, env, key, *row, m.room, x, y)?;
            if let Some(u) = found.filter(|&u| !is_dead(w, u)) {
                end_with(w, env, key, Some(u), false)?;
                return Ok(());
            }
        }
    }
    // r13: the second pass of a local player's missile (`render/camera.md`
    // §9) is not modelled.
    Ok(())
}

/// The client act's active rooms as read-only [`CollisionRooms`]
/// (`sim/path-placement.md` §4): a missile's footprint mask is 0
/// (§3 table), so its moves stamp nothing and `grid_mut` answers none.
/// A room's grid is its copy with the unit footprints
/// ([`stamp_unit_footprints`]) when it has one.
struct MissileRooms<'a>(
    &'a d2_sim::drlg::Drlg,
    &'a BTreeMap<RoomId, d2_sim::drlg::CollisionGrid>,
);

impl CollisionRooms for MissileRooms<'_> {
    fn subtile_rect(&self, room: RoomId) -> Option<d2_sim::drlg::TileRect> {
        let r = self.0.drlg_room_of(room)?;
        self.0.active_room(r).map(|a| a.subtiles)
    }
    fn adjacent_count(&self, room: RoomId) -> usize {
        self.0
            .drlg_room_of(room)
            .and_then(|r| self.0.active_room(r))
            .map_or(0, |a| a.adjacency.len())
    }
    fn adjacent(&self, room: RoomId, i: usize) -> Option<RoomId> {
        let r = self.0.drlg_room_of(room)?;
        let n = *self.0.active_room(r)?.adjacency.get(i)?;
        self.0.active_room(n).map(|a| a.id)
    }
    fn grid(&self, room: RoomId) -> Option<&d2_sim::drlg::CollisionGrid> {
        if let Some(g) = self.1.get(&room) {
            return Some(g);
        }
        let r = self.0.drlg_room_of(room)?;
        self.0.active_room(r).map(|a| &a.collision)
    }
    fn grid_mut(&mut self, _: RoomId) -> Option<&mut d2_sim::drlg::CollisionGrid> {
        None
    }
}

/// [`CollisionRooms`] writing into copies of the client act's grids.
struct FootRooms<'a> {
    drlg: &'a d2_sim::drlg::Drlg,
    grids: BTreeMap<RoomId, d2_sim::drlg::CollisionGrid>,
}

impl CollisionRooms for FootRooms<'_> {
    fn subtile_rect(&self, room: RoomId) -> Option<d2_sim::drlg::TileRect> {
        MissileRooms(self.drlg, &self.grids).subtile_rect(room)
    }
    fn adjacent_count(&self, room: RoomId) -> usize {
        MissileRooms(self.drlg, &self.grids).adjacent_count(room)
    }
    fn adjacent(&self, room: RoomId, i: usize) -> Option<RoomId> {
        MissileRooms(self.drlg, &self.grids).adjacent(room, i)
    }
    fn grid(&self, room: RoomId) -> Option<&d2_sim::drlg::CollisionGrid> {
        if let Some(g) = self.grids.get(&room) {
            return Some(g);
        }
        let r = self.drlg.drlg_room_of(room)?;
        self.drlg.active_room(r).map(|a| &a.collision)
    }
    fn grid_mut(&mut self, room: RoomId) -> Option<&mut d2_sim::drlg::CollisionGrid> {
        if !self.grids.contains_key(&room) {
            let r = self.drlg.drlg_room_of(room)?;
            let g = self.drlg.active_room(r)?.collision.clone();
            self.grids.insert(room, g);
        }
        self.grids.get_mut(&room)
    }
}

/// The unit footprints of the client grids as the client missiles read
/// them, rebuilt once per client update before the set-C missile walk:
/// every living player and monster of the model at its sub-tile, with
/// its pattern (`sim/path-placement.md` §3: size 0 → 0, 1 and 2 → 1, 3 →
/// 2, others → 1; a monster that can be in town (`npc` or `inTown`)
/// without `interact` 1 → 3, 2 → 4; players size 2, monsters `monstats2`
/// `SizeX`) and footprint mask (player 0x80, monster 0x100; a dying or
/// dead monster none, `client/msg-units.md` §3 r2), stamped
/// (`0x0064EA90`, §5.1) on copies of the rooms' grids.
///
/// PROVISIONAL (REC-546): the 1.14d client stamps these on its grids as
/// its units move (`sim/pathing.md` §13.3 r5); the model's grids carry
/// none (the local player's walk prediction reads them without other
/// units, `client/model.md` REC-277 (d)), so the missiles read a copy
/// stamped at the units' model positions once per update.
pub fn stamp_unit_footprints(w: &mut ClientWorld, monsters: &[Option<super::world::MonsterClass>]) {
    let Some(d) = w.drlg.as_ref() else {
        w.objclient.unit_grids.clear();
        return;
    };
    let mut rooms = FootRooms {
        drlg: &d.drlg,
        grids: BTreeMap::new(),
    };
    for (k, u) in &w.units {
        if u.is_dead() {
            continue;
        }
        let (size, town_npc, mask) = match k.unit_type {
            PLAYER => (2, false, 0x80),
            MONSTER => {
                let c = monsters.get(u.class as usize).and_then(|c| c.as_ref());
                let size = c.map_or(0, |c| i32::from(c.size_x));
                let town = c.is_some_and(|c| (c.npc || c.in_town) && !c.interact);
                (size, town, 0x100)
            }
            _ => continue,
        };
        let Some((x, y)) = u.position.map(|(x, y)| (i32::from(x), i32::from(y))) else {
            continue;
        };
        let pattern = match (size, town_npc) {
            (0, _) => 0,
            (3, false) => 2,
            (3, true) => 4,
            (_, true) => 3,
            _ => 1,
        };
        let room = drlg_room_at(&d.drlg, x, y);
        d2_sim::path::footprint::stamp_pattern(&mut rooms, room, x, y, pattern, mask);
    }
    let grids = rooms.grids;
    w.objclient.unit_grids = grids;
}

/// The active room of the client act whose sub-tile rect holds (x, y)
/// (the path's room at the create, `sim/path-placement.md` §2.4).
fn drlg_room_at(d: &d2_sim::drlg::Drlg, x: i32, y: i32) -> Option<RoomId> {
    d.active_rooms()
        .into_iter()
        .find(|&(_, r)| d.active_room(r).is_some_and(|a| a.subtiles.contains(x, y)))
        .map(|(id, _)| id)
}

fn cell16(p: u32) -> i32 {
    (p >> 16) as i32
}

fn centre16(c: i32) -> u32 {
    ((c as u32) << 16) | 0x8000
}

/// One path step of a missile, the movement `0x00650840(m, 0x400)`
/// (`sim/pathing.md` §9.4) with the one step and cell walk of §9.6:
///
/// 1. The new-step flag and the collided mask cleared (§9.4 r1; §B2 r1).
/// 2. A stopped path (point count 0, rule 9.6.8) or a zero direction
///    vector: the movement reset (§9.7: position := its cell centre).
/// 3. The acceleration counter (§9.4 r2.1); velocity vector := ((0x400 ×
///    velocity) >> 6) × direction vector >> 12.
/// 4. In another cell (§9.6 r4–r6): the cell walk in steps halved to at
///    most 0x10000 per axis; each cell change is the missile move
///    (`sim/path-placement.md` §6 r3: the size query at the new cell with
///    the move mask, ORed into the collided mask), refused when the
///    result has 0x1 or 0x4 → Q := the centre of the last free cell; each
///    entered cell is a saved step (at most 10, then the walk stops); the
///    new-step flag is set when the walk ends with a saved step and was
///    not refused (§B2 r2).
/// 5. Set position (§9.6 r8): a new cell in no room → point count 0,
///    position unchanged; else the room recache (r9) from the path's
///    room.
///
/// With no client DRLG the step moves without the walk.
fn path_step(w: &mut ClientWorld, env: &Env, key: UnitKey) {
    let size = w
        .objclient
        .set_c
        .get(&key)
        .and_then(|u| env.rows.get(u.class as usize))
        .map_or(0, |r| i32::from(r.size));
    let move_mask = w
        .objclient
        .set_c
        .get(&key)
        .and_then(|u| env.rows.get(u.class as usize))
        .map_or(0, |r| move_mask(r.collide_type));
    let drlg = w.drlg.as_ref().map(|d| &d.drlg);
    let unit_grids = &w.objclient.unit_grids;
    let Some(m) = w.objclient.missiles.get_mut(&key) else {
        return;
    };
    // 1.
    m.new_step = false;
    m.collided = 0;
    m.step_count = 0;
    // 2.
    if m.stopped || m.dir_vec == (0, 0) {
        m.pos = (centre16(cell16(m.pos.0)), centre16(cell16(m.pos.1)));
        return;
    }
    // 3.
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
    let d = (
        k.wrapping_mul(m.dir_vec.0) >> 12,
        k.wrapping_mul(m.dir_vec.1) >> 12,
    );
    if d == (0, 0) {
        m.pos = (centre16(cell16(m.pos.0)), centre16(cell16(m.pos.1)));
        return;
    }
    let end = (
        m.pos.0.wrapping_add_signed(d.0),
        m.pos.1.wrapping_add_signed(d.1),
    );
    let old = (cell16(m.pos.0), cell16(m.pos.1));
    let end_cell = (cell16(end.0), cell16(end.1));
    let Some(drlg) = drlg else {
        m.pos = end;
        m.new_step = end_cell != old;
        return;
    };
    let mut rooms = MissileRooms(drlg, unit_grids);
    // 4.
    let mut q = end;
    if end_cell != old {
        let (mut sx, mut sy) = d;
        while sx.abs() > 0x10000 || sy.abs() > 0x10000 {
            sx >>= 1;
            sy >>= 1;
        }
        let mut c = m.pos;
        let mut blocked = false;
        while (cell16(c.0), cell16(c.1)) != end_cell {
            let n = (c.0.wrapping_add_signed(sx), c.1.wrapping_add_signed(sy));
            let (cc, nc) = ((cell16(c.0), cell16(c.1)), (cell16(n.0), cell16(n.1)));
            if cc != nc {
                let r = missile_move(&mut rooms, m.room, cc, nc, size, 0, move_mask);
                m.collided |= r;
                if r & 0x5 != 0 {
                    blocked = true;
                    q = (centre16(cc.0), centre16(cc.1));
                    break;
                }
                m.steps[usize::from(m.step_count)] = nc;
                m.step_count += 1;
                if m.step_count >= 10 {
                    break;
                }
            }
            c = n;
        }
        m.new_step = !blocked && m.step_count > 0;
    }
    // 5.
    let qc = (cell16(q.0), cell16(q.1));
    let room = if m.room.is_some_and(|r| {
        rooms
            .subtile_rect(r)
            .is_some_and(|rect| rect.contains(qc.0, qc.1))
    }) {
        m.room
    } else {
        find_room(&rooms, m.room, qc.0, qc.1)
    };
    match room {
        None => m.stopped = true,
        Some(r) => {
            m.room = Some(r);
            m.pos = q;
        }
    }
}

/// The move-test masks of the client collide table (§C8), by
/// `CollideType`.
const MOVE_MASKS: [u16; 9] = [0, 0x84, 0x104, 0x184, 0, 0x104, 4, 0x40, 0x185];

fn move_mask(collide_type: u8) -> u16 {
    MOVE_MASKS
        .get(usize::from(collide_type))
        .copied()
        .unwrap_or(0)
}

/// The point test `0x0064CB30(room, x, y, mask)` from `room`
/// (`sim/path-placement.md` §4 r1–r2: no room → 0x27 unmasked) over the
/// client DRLG; no client DRLG → 0.
fn point_value_from(w: &ClientWorld, room: Option<RoomId>, x: i32, y: i32, mask: u16) -> u32 {
    w.drlg.as_ref().map_or(0, |d| {
        u32::from(d2_sim::path::collision::point_value(
            &MissileRooms(&d.drlg, &w.objclient.unit_grids),
            room,
            x,
            y,
            mask,
        ))
    })
}

/// The collision word under the missile (`0x00648EB0`, `missiles.md`
/// §R4 r6): with path velocity 0 the size query (`Size`) at the path
/// position with all bits, from the path's room (no room → 0x27);
/// otherwise the collided mask the last step cached. No client DRLG →
/// 0.
fn collision_under(w: &ClientWorld, env: &Env, key: UnitKey) -> u32 {
    let (Some(d), Some(m)) = (w.drlg.as_ref(), w.objclient.missiles.get(&key)) else {
        return 0;
    };
    if m.velocity != 0 {
        return u32::from(m.collided);
    }
    let size = w
        .objclient
        .set_c
        .get(&key)
        .and_then(|u| env.rows.get(u.class as usize))
        .map_or(0, |r| i32::from(r.size));
    let (x, y) = cell_of(m);
    u32::from(size_value(
        &MissileRooms(&d.drlg, &w.objclient.unit_grids),
        m.room,
        x,
        y,
        size,
        0xFFFF,
    ))
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
    let env = Env {
        rows,
        lights,
        skills: None,
        monsters: &[],
    };
    end_with(w, &env, key, None, forced)
}

/// [`end`] with the skills tables ([`Env`]).
pub fn end_with(
    w: &mut ClientWorld,
    env: &Env,
    key: UnitKey,
    unit: Option<UnitKey>,
    forced: bool,
) -> Result<Option<UnitKey>, HandlerError> {
    let Some(class) = w.objclient.set_c.get(&key).map(|u| u.class) else {
        return Ok(None);
    };
    let row = env.rows.get(class as usize).copied().unwrap_or_default();
    let m = w.objclient.missiles.get(&key).copied().unwrap_or_default();
    // r1.
    let owner = m.owner;
    let mut r = 3;
    // r2: the hit tests.
    if let Some(u) = unit {
        if row.next_hit && just_hit(w, u) {
            return Ok(None);
        }
        if row.last_collide {
            if m.last_collided == Some(u) {
                return Ok(None);
            }
            if let Some(m) = w.objclient.missiles.get_mut(&key) {
                m.last_collided = Some(u);
            }
        }
        if !flags_4_8(w, u) {
            return Ok(None);
        }
        if let Some(o) = owner {
            if !super::combat::hostile_between(w, env.monsters, o, u) && !row.collide_friend {
                return Ok(None);
            }
            let piercing = w
                .units
                .get(&o)
                .is_some_and(|ou| ou.states.contains(&69) || w.total(o, 156, 0) != 0);
            if row.pierce && o.unit_type <= MONSTER && piercing {
                // `0x004CD310`.
                let mm = w.objclient.missiles.get_mut(&key).expect("checked above");
                if mm.pierce != 0 {
                    mm.pierce -= 1;
                    r = 2;
                }
            }
        }
    }
    let mut x = None;
    // r3: no unit, not forced, no `AlwaysExplode` → r5.
    if unit.is_some() || forced || row.always_explode {
        // r4.1: U gets state 86 for `NextDelay` frames.
        if let (true, Some(u)) = (row.next_hit, unit) {
            w.objclient.just_hit.insert(u, i32::from(row.next_delay));
        }
        // r4.2: U's client event hooks of kind 0 (`0x004DC210`): not
        // modelled (no client event hook list in the model).
        // r4.3: the client hit function; 0 → the missile stays.
        match hits::call(w, env, key, unit, row.clt_hit_func) {
            Ok(true) => {}
            Ok(false) => return Ok(None),
            Err(e) => {
                // A body the model cannot run: the error is reported and
                // the end goes on as for a non-zero result without the
                // explosion (r7–r8), so the missile does not stay to
                // fail again on every update (PROVISIONAL REC-547).
                if let Some(id) = light_of(w, key) {
                    let _ = w.lights.die(id);
                }
                super::objects::remove_client_unit(w, key);
                return Err(e);
            }
        }
        // r4.4: `HitSound` (audio, not modelled).
        // r4.5: `0x004CDBA0(m, E, 0, 0, skill, level)`: flags 0x20, the
        // owner m's owner (none → none), origin m.
        if row.explosion_missile >= 0 {
            if let Some(owner) = owner {
                let rec = CreateRecord {
                    flags: flag::TARGET_ABSOLUTE,
                    owner: Some(owner),
                    origin: Some(key),
                    class: row.explosion_missile as u32,
                    skill: m.skill,
                    level: m.level,
                    ..CreateRecord::default()
                };
                x = create_from(w, env, key, &rec)?;
                if let Some(xk) = x {
                    // X's motion position := m's (x, y, z), done,
                    // restart. PROVISIONAL (REC-541; Open question 10):
                    // the getters' values are copied as stored.
                    if let Some(xm) = w.objclient.missiles.get_mut(&xk) {
                        xm.motion.pos = m.motion.pos;
                        xm.motion.done();
                        xm.motion.restart();
                    }
                    // X's direction: rnd(64) on X's seed for 146
                    // `spidergoo`, else m's.
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
    // r5: a unit hit without `CollideKill`: the missile flies on.
    if unit.is_some() && !row.collide_kill {
        return Ok(x);
    }
    // r6: `TravelSound` stop (audio, not modelled).
    // r7: m's light dies (`0x00474470`, `render/lighting.md` §6.2 r6).
    if let Some(id) = light_of(w, key) {
        let _ = w.lights.die(id);
    }
    // r8: r & 1 (r = 3) → m removed from set C; a piercing hit (r = 2)
    // keeps it (Edge case 2).
    if r & 1 != 0 {
        super::objects::remove_client_unit(w, key);
    }
    Ok(x)
}

/// U is in state 86 `justhit` (`0x00639DF0`): the server's state, or the
/// client's own state list of §C9 r4.1 while its frames run
/// ([`super::objects::ClientObjects::just_hit`]).
fn just_hit(w: &ClientWorld, u: UnitKey) -> bool {
    w.units.get(&u).is_some_and(|x| x.states.contains(&86)) || w.objclient.just_hit.contains_key(&u)
}

/// U's unit flags +0xC4 0x4 and 0x8, read by the collide test (§C8) and
/// the hit tests (§C9 r2.4). PROVISIONAL (REC-544; `client/msg-ui.md`
/// OQ2: the model holds no flag word): a player or monster has both
/// while it is not dead (the death clears 0x4, `client/msg-units.md`
/// §4 r6); a missile has 0x4 with `CanDestroy` (§C1).
fn flags_4_8(w: &ClientWorld, u: UnitKey) -> bool {
    match u.unit_type {
        PLAYER | MONSTER => w.units.get(&u).is_some_and(|x| !x.is_dead()),
        _ => false,
    }
}

/// The dead test `0x00464820` (`client/msg-units.md` §4 r6) of a model
/// unit; units outside the model (set C) are not dead.
fn is_dead(w: &ClientWorld, u: UnitKey) -> bool {
    w.units.get(&u).is_some_and(|x| x.is_dead())
}

/// The unit at a point `0x00641CB0(room, x, y, accept, m, size)`
/// (`sim/path-placement.md` §4 r6) of a client missile, with the collide
/// test of its `CollideType` (§C8): the rooms of the missile's room's
/// adjacency array in order, each room's unit list from its head; players
/// in mode 0 / 17, monsters in mode 0 / 12, objects, items and tiles
/// skipped; a candidate of size s (≤ 0 skipped, above 3 read as 3) whose
/// shape meets the query shape of the missile's `Size`
/// (`shapes_overlap`) and that the test accepts is returned. `Size`
/// outside 1…3 or no room → none.
fn unit_at_point(
    w: &ClientWorld,
    env: &Env,
    key: UnitKey,
    row: ClientMissileRow,
    room: Option<RoomId>,
    x: i32,
    y: i32,
) -> Result<Option<UnitKey>, HandlerError> {
    let r = i32::from(row.size);
    let (Some(d), Some(room)) = (w.drlg.as_ref(), room) else {
        return Ok(None);
    };
    if !(1..=3).contains(&r) {
        return Ok(None);
    }
    if row.collide_type == 4 {
        return Err(HandlerError::Invalid(
            "missiles/client.md §C8: CollideType 4 passes a null test to the unit search",
        ));
    }
    let Some(adjacency) = d
        .drlg
        .drlg_room_of(room)
        .and_then(|dr| d.drlg.active_room(dr))
        .map(|a| a.adjacency.clone())
    else {
        return Ok(None);
    };
    for dr in adjacency {
        for &u in w.room_units.list(dr) {
            let mode = w.units.get(&u).map_or(0, |x| x.mode);
            match u.unit_type {
                PLAYER if mode == 0 || mode == 17 => continue,
                MONSTER if mode == 0 || mode == 12 => continue,
                PLAYER | MONSTER | MISSILE => {}
                _ => continue,
            }
            let s = unit_size(env, w, u);
            if s <= 0 {
                continue;
            }
            let Some((ux, uy)) = w.units.get(&u).map(|x| x.cell()) else {
                continue;
            };
            let (dx, dy) = ((x - i32::from(ux)).abs(), (y - i32::from(uy)).abs());
            if d2_sim::path::collision::shapes_overlap(r, s.min(3), dx, dy)
                && collide_test(w, env, key, &row, u)
            {
                return Ok(Some(u));
            }
        }
    }
    Ok(None)
}

/// The collide test of the missile's `CollideType` (§C8 table): the
/// common test `0x004CCF40` (U flags 0x8 and 0x4; not (`NextHit` and U in
/// state 86); not (`LastCollide` and U the last-collided); no owner, or
/// the owner hostile to U, or `CollideFriend`) for players (modes 1, 3,
/// 8) and monsters (2, 3, 5, 8); mode 1 accepts monsters whose alignment
/// (state 105's list, stat 172) is 2 without it; mode 7 accepts missiles
/// whose row has `CanDestroy`.
fn collide_test(
    w: &ClientWorld,
    env: &Env,
    key: UnitKey,
    row: &ClientMissileRow,
    u: UnitKey,
) -> bool {
    let common = || {
        let Some(m) = w.objclient.missiles.get(&key) else {
            return false;
        };
        flags_4_8(w, u)
            && !(row.next_hit && just_hit(w, u))
            && !(row.last_collide && m.last_collided == Some(u))
            && (m
                .owner
                .is_none_or(|o| super::combat::hostile_between(w, env.monsters, o, u))
                || row.collide_friend)
    };
    match (row.collide_type, u.unit_type) {
        (1 | 3 | 8, PLAYER) => common(),
        (2 | 3 | 5 | 8, MONSTER) => common(),
        (1, MONSTER) => {
            w.units
                .get(&u)
                .and_then(|x| x.state_lists.get(&105))
                .and_then(|l| l.get(&(172, 0)))
                == Some(&2)
        }
        (7, MISSILE) => w
            .units
            .get(&u)
            .and_then(|x| env.rows.get(x.class as usize))
            .is_some_and(|r| r.can_destroy),
        _ => false,
    }
}

/// The client state lists of §C9 r4.1 count their frames down, one per
/// client update, before the set-C missile walk
/// (PROVISIONAL REC-545: `0x006251F0(0, 2, NextDelay, …)`, Open question
/// 11): an entry at 0 is removed.
pub fn tick_just_hit(w: &mut ClientWorld) {
    w.objclient.just_hit.retain(|_, n| {
        *n -= 1;
        *n > 0
    });
}

/// The town test of a missile's room (§C6 r3–r4, `client-bodies.md`
/// §B5 r2): the active room containing its sub-tile (set-C units are in
/// no room list), its level a town level (`0x0061AB00`,
/// `sim/stat-lists.md` OQ2); no client DRLG or no room → not in town.
fn in_town(w: &ClientWorld, key: UnitKey) -> bool {
    let Some(m) = w.objclient.missiles.get(&key) else {
        return false;
    };
    let (x, y) = cell_of(m);
    let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) else {
        return false;
    };
    w.room_at(x, y)
        .is_some_and(|r| d2_sim::drlg::is_town(u32::from(r.level)))
}

/// The unit size `0x00620510` (`sim/path-placement.md` §3) of a client
/// unit the missile code measures: player 2, monster `monstats2`
/// `SizeX`, missile `Size`, item 1; objects and tiles 0 (the object rows
/// are not in [`Env`]; no modelled body measures one).
fn unit_size(env: &Env, w: &ClientWorld, k: UnitKey) -> i32 {
    use super::world::ITEM;
    match k.unit_type {
        PLAYER => 2,
        MONSTER => w
            .units
            .get(&k)
            .and_then(|u| env.monsters.get(u.class as usize))
            .and_then(|c| c.as_ref())
            .map_or(0, |c| i32::from(c.size_x)),
        MISSILE => w
            .objclient
            .set_c
            .get(&k)
            .or_else(|| w.units.get(&k))
            .and_then(|u| env.rows.get(u.class as usize))
            .map_or(0, |r| i32::from(r.size)),
        ITEM => 1,
        _ => 0,
    }
}

/// The motion update `0x004DA350` of a missile's record
/// (`render/unit-composite.md` §8 r1–r6); no follow record is made by
/// the modelled creators, so the linked unit is none.
fn motion_update(r: &mut MotionRecord) -> Result<(), HandlerError> {
    r.update(true, None).map_err(|_| {
        HandlerError::Invalid("render/unit-composite.md §8 r4: a missile follows a non-monster")
    })
}

/// One advance of the missile's seed (`client-bodies.md` §B1 "seed
/// step"): the new low word lo'.
fn seed_step(w: &mut ClientWorld, key: UnitKey) -> u32 {
    let Some(u) = w.objclient.set_c.get_mut(&key) else {
        return 0;
    };
    let Some((lo, hi)) = u.seed else {
        return 0;
    };
    let mut seed = Seed::new(lo, hi);
    let lo = seed.step();
    u.seed = Some((seed.lo, seed.hi));
    lo
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

#[cfg(test)]
#[path = "client_missiles_bodies_tests.rs"]
mod bodies_tests;

#[cfg(test)]
#[path = "client_missiles_hits_tests.rs"]
mod hits_tests;
