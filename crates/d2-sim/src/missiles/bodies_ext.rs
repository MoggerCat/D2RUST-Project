// Spec: specs/missiles/bodies.md §1–§30
//! The server-do bodies 6, 9, 11, 14, 17, 22, 23/24, 26, 28, 31, 34, 35
//! and the server-hit bodies 2, 3, 7, 8, 9, 10, 14, 15, 16, 17, 18, 19,
//! 26, 27, 36, 38, 44, 45, 52, 56, 58 (`srvdo.tsv` / `srvhit.tsv` status
//! `spec'd-here`, `missiles/bodies.md`), with their helpers: the plague
//! ring `0x005A9370`, the meteor scatter `0x005AAA90`, `nova`
//! `0x0056D4E0`, the `zigzag` init callback `0x005AC040`, `fire_disc`
//! `0x005A9530`, the bone spirit re-aim `0x005AA5B0` / `0x005AA460`, the
//! Cairn Stones portal `0x005A9930` and `missile_at` `0x0056EDE0`
//! (`skills/bodies.md` §6.13). What other specs own goes through
//! [`super::seams::MissileBodies`].

use crate::combat::DamageRecord;
use crate::game::Game;
use crate::rng::Seed;
use crate::units::{UnitId, UnitType};

use super::bodies::{
    area_damage, elem_roll, i16v, next_unit, pos, skill_level, srv_do_3, unit_distance,
};
use super::catalogue::{create_with_collision_check, run_srv_hit, srv_hit_in_range};
use super::create::{create_missile, MissileParams};
use super::flight::default_flight;
use super::hit::{damage_roll, fill_damage, hit_handler};
use super::seams::{SkillCalc, SkillField};
use super::{clamp_frame, param_flags as pf, stat, Ctx, MissileRow, MissileWorld, Unhandled};

// ---------------------------------------------------------------- shared

/// A missiles.txt `Param` / `sHitPar` cell as the i32 the bodies read.
pub(super) fn par(v: u32) -> i32 {
    v as i32
}

/// Frames left (`0x0064A380`).
pub(super) fn left<W: MissileWorld + ?Sized>(cx: &Ctx<'_, W>, m: UnitId) -> i32 {
    cx.store.get(m).map_or(0, |d| i32::from(d.current))
}

/// Elapsed frames (`0x0064A3B0`).
pub(super) fn elapsed<W: MissileWorld + ?Sized>(cx: &Ctx<'_, W>, m: UnitId) -> i32 {
    cx.store.get(m).map_or(0, |d| d.elapsed())
}

/// Data +0x28 / +0x2C (`0x0064A730` / `0x0064A780`).
pub(super) fn data<W: MissileWorld + ?Sized>(cx: &Ctx<'_, W>, m: UnitId) -> (i32, i32) {
    cx.store.get(m).map_or((0, 0), |d| d.target)
}

/// Data +0x28 := `v` (`0x0064A710`).
pub(super) fn set_d28<W: MissileWorld + ?Sized>(cx: &mut Ctx<'_, W>, m: UnitId, v: i32) {
    if let Some(d) = cx.store.get_mut(m) {
        d.target.0 = v;
    }
}

/// Data +0x2C := `v` (`0x0064A760`).
pub(super) fn set_d2c<W: MissileWorld + ?Sized>(cx: &mut Ctx<'_, W>, m: UnitId, v: i32) {
    if let Some(d) = cx.store.get_mut(m) {
        d.target.1 = v;
    }
}

/// The missile's room.
pub(super) fn room_of(game: &Game, m: UnitId) -> Option<crate::units::RoomId> {
    game.lists.unit(m).and_then(|e| e.room())
}

/// The unit's type.
pub(super) fn type_of(game: &Game, u: UnitId) -> Option<UnitType> {
    game.lists.unit(u).map(|e| e.ty)
}

/// `max(eval(owner, k.<calc>, k, L), lo)`.
pub(super) fn calc_max<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    owner: Option<UnitId>,
    k: i32,
    calc: SkillCalc,
    level: i32,
    lo: i32,
) -> i32 {
    cx.world.skill_calc(game, owner, k, calc, level).max(lo)
}

/// `Param_a + (L − 1) × Param_b` of skill `k`; 0 without a record or
/// when L ≤ 0 (`0x004CC7C0`, `0x004E6CA0`, `0x004EFCB0`).
pub(super) fn skill_lin<W: MissileWorld + ?Sized>(
    cx: &Ctx<'_, W>,
    k: i32,
    level: i32,
    a: u8,
    b: u8,
) -> i32 {
    if !cx.world.skill_exists(k) || level <= 0 {
        return 0;
    }
    let pa = cx.world.skill_field(k, SkillField::Param(a));
    let pb = cx.world.skill_field(k, SkillField::Param(b));
    pa.wrapping_add(level.wrapping_sub(1).wrapping_mul(pb))
}

/// The full damage roll `0x005A89A0(missile, unit, record)` into a zeroed
/// 0x70-byte record (`missiles.md` §R6.2, missile seed).
pub fn full_record<W: MissileWorld + ?Sized>(
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    unit: Option<UnitId>,
) -> DamageRecord {
    let d = fill_damage(cx, m, unit);
    let mut rec = DamageRecord {
        physical: d.phys,
        fire: d.fire,
        magic: d.magic,
        lightning: d.light,
        cold: d.cold,
        poison: d.poison,
        burn: d.burn,
        cold_len: d.cold_length,
        poison_len: d.poison_length,
        life_leech: d.life_drain,
        mana_leech: d.mana_drain,
        stamina_leech: d.stamina_drain,
        burn_len: d.burn_length,
        stun_len: d.stun_length,
        ..DamageRecord::default()
    };
    // The crit flag is the record's critical result bit (`combat/hit.md`
    // result 0x2000, as `elem_roll` sets it, `missiles.md` §R9.6).
    // TODO(spec: missiles.md §R6.2): where the 103 / 104 / 106 bypass
    // flags land in the 0x70-byte record is not stated; not carried.
    if d.crit {
        rec.result |= crate::combat::result::CRITICAL;
    }
    rec
}

/// Hit flags |= `HitFlags`, result flags |= `ResultFlags` (`bodies.md`
/// §8 step 4).
pub(super) fn row_flags(rec: &mut DamageRecord, row: &MissileRow) {
    rec.hit_flags |= row.hitflags;
    rec.result |= row.resultflags;
}

/// A zeroed parameter record with the missile's skill and level.
pub(super) fn sub_params<W: MissileWorld + ?Sized>(
    cx: &Ctx<'_, W>,
    m: UnitId,
    owner: Option<UnitId>,
    class: i32,
) -> MissileParams {
    let (skill, level) = skill_level(cx, m);
    MissileParams {
        owner,
        class,
        skill,
        level,
        ..MissileParams::default()
    }
}

/// `missile_at(game, unit, skill, L, m, tx, ty)` = `0x0056EDE0`
/// (`skills/bodies.md` §6.13).
#[allow(clippy::too_many_arguments)]
pub fn missile_at<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    unit: Option<UnitId>,
    skill: i32,
    level: i32,
    class: i32,
    tx: i32,
    ty: i32,
) -> Option<UnitId> {
    let unit = unit?;
    let (tx, ty) = if (tx, ty) == (0, 0) {
        cx.world.target_position(game, unit)?
    } else {
        (tx, ty)
    };
    if (tx, ty) == (0, 0) {
        return None;
    }
    let (ux, uy) = cx.world.position(unit);
    let (dx, dy) = (
        tx.wrapping_sub(ux).unsigned_abs(),
        ty.wrapping_sub(uy).unsigned_abs(),
    );
    if dx.max(dy).wrapping_add(dx.min(dy) / 2) > 100 {
        return None;
    }
    let p = MissileParams {
        flags: pf::POSITION,
        owner: Some(unit),
        class,
        x: tx,
        y: ty,
        skill,
        level,
        ..MissileParams::default()
    };
    create_missile(game, cx, &p)
}

/// The rest of the function is skipped: the original stops (fatal).
pub(super) fn fatal<W: MissileWorld + ?Sized>(cx: &mut Ctx<'_, W>, addr: u32, m: UnitId) {
    cx.store
        .unhandled
        .push(Unhandled::Fatal { addr, missile: m });
}

// ---------------------------------------------------------------- §1–§5

/// The Cairn Stones portal `0x005A9930(game, missile, level)` (§1 step 3).
pub fn open_portal<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    level: i32,
) {
    set_d28(cx, m, 1);
    let owner = cx.owner(game, m);
    let room = room_of(game, m);
    let at = pos(cx, m);
    cx.world.create_portal(game, owner, room, at, level, 60);
    if let Some(r) = room {
        cx.world.refresh_room(game, r);
    }
}

/// §1 Server-do 17 Cairn Stones `0x005AF240`.
pub fn srv_do_17<W: MissileWorld + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, m: UnitId) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 2;
    };
    let f = left(cx, m);
    let (p1, p2, p3, p4, p5) = (
        par(row.param1),
        par(row.param2),
        par(row.param3),
        par(row.param4),
        par(row.param5),
    );
    let sub = i16v(row.submissile1);
    let range = i32::from(row.range as i16);
    if sub > 0 && f < range.wrapping_sub(p1) && f > p1 {
        create_with_collision_check(game, cx, m, p3, p2, sub, 5);
    }
    if p4 >= 0 && data(cx, m).0 == 0 && f <= p5.wrapping_add(p1) {
        open_portal(game, cx, m, p4);
    }
    default_flight(game, cx, m)
}

/// §2 Server-do 28 Volcano `0x005AFB80`.
pub fn srv_do_28<W: MissileWorld + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, m: UnitId) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 2;
    };
    let sub = i16v(row.submissile1);
    if sub < 0 {
        return 2;
    }
    let (k, level) = skill_level(cx, m);
    if !cx.world.skill_exists(k) {
        return 2;
    }
    if let Some(o) = cx.owner(game, m) {
        let mut r = par(row.param2);
        if r < 1 {
            r = calc_max(game, cx, Some(o), k, SkillCalc::AuraRange, level, 1);
        }
        let mut i = par(row.param1);
        if i < 1 {
            i = calc_max(game, cx, Some(o), k, SkillCalc::Calc4, level, 1);
        }
        let e = elapsed(cx, m);
        if par(row.param3) < e && e < par(row.param4) && e % i == 0 {
            let w = data(cx, m).0;
            let seed = cx.world.seed(m);
            *seed = Seed::init_low(w as u32);
            let n = r.wrapping_mul(2).wrapping_add(1);
            let dx = (seed.roll(n) as i32).wrapping_sub(r);
            let dy = (seed.roll(n) as i32).wrapping_sub(r);
            let lo = seed.lo as i32;
            set_d28(cx, m, lo);
            let (x, y) = pos(cx, m);
            let p = MissileParams {
                flags: pf::TARGET_ABSOLUTE | 0x100 | pf::FRAMES_FROM_DISTANCE,
                origin: Some(m),
                target_x: x.wrapping_add(dx),
                target_y: y.wrapping_add(dy),
                gfx: par(row.param5),
                ..sub_params(cx, m, Some(o), sub)
            };
            create_missile(game, cx, &p);
        }
    }
    default_flight(game, cx, m)
}

/// §3 Server-do 34 Baal taunt control `0x005B04A0`.
pub fn srv_do_34<W: MissileWorld + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, m: UnitId) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 2;
    };
    let e = elapsed(cx, m);
    let subs = [row.submissile1, row.submissile2, row.submissile3];
    let intervals = [par(row.param2), par(row.param3), par(row.param4)];
    let n = (0..3)
        .take_while(|&j| i16v(subs[j]) >= 0 && intervals[j] >= 1)
        .count();
    if n == 0 {
        return 2;
    }
    if e < par(row.param1) {
        return srv_do_3(game, cx, m);
    }
    let x = pos(cx, m).0;
    let seed = cx.world.seed(m);
    *seed = Seed::init_low(x as u32);
    let j = seed.roll(n as i32) as usize;
    if e % intervals[j] == 0 {
        let owner = cx.owner(game, m);
        let p = MissileParams {
            origin: Some(m),
            ..sub_params(cx, m, owner, i16v(subs[j]))
        };
        if let Some(c) = create_missile(game, cx, &p) {
            hit_handler(game, cx, c, None, true);
        }
    }
    srv_do_3(game, cx, m)
}

/// §4 step 2: the chaos ice turn of (a, b) by bit 0 of `lo'`.
pub fn chaos_turn(a: i32, b: i32, bit: bool) -> (i32, i32) {
    let (a2, b2) = if bit {
        (
            a.wrapping_mul(4).wrapping_sub(b) / 4,
            b.wrapping_mul(4).wrapping_add(a) / 4,
        )
    } else {
        (
            a.wrapping_mul(4).wrapping_add(b) / 4,
            b.wrapping_mul(4).wrapping_sub(a) / 4,
        )
    };
    (if a2 == 0 { 1 } else { a2 }, if b2 == 0 { 1 } else { b2 })
}

/// §4 Server-do 35 Royal Strike chaos ice `0x005B0640`.
pub fn srv_do_35<W: MissileWorld + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, m: UnitId) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 2;
    };
    let n = par(row.param1).max(1);
    if elapsed(cx, m) % n != 0 {
        return default_flight(game, cx, m);
    }
    let (w, dir) = data(cx, m);
    let seed = cx.world.seed(m);
    *seed = Seed::init_low(w as u32);
    let bit = seed.step() & 1 != 0;
    let lo = seed.lo as i32;
    let (a, b) = chaos_turn(i32::from(dir as i16), dir >> 16, bit);
    let (x, y) = pos(cx, m);
    cx.world
        .set_target_point(m, x.wrapping_add(a), y.wrapping_add(b));
    cx.world.build(game, m);
    set_d28(cx, m, lo);
    set_d2c(cx, m, (b << 16).wrapping_add(a & 0xFFFF));
    default_flight(game, cx, m)
}

/// §5 Server-hit 58 Baal taunt lightning control `0x005ACDF0`.
pub fn srv_hit_58<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 1;
    };
    let h = i16v(row.hitsubmissile1);
    if h < 0 {
        return 1;
    }
    let Some(o) = cx.owner(game, m) else {
        return 1;
    };
    let (x, y) = pos(cx, m);
    let seed = cx.world.seed(m);
    *seed = Seed::init_low(x as u32);
    let r = par(row.shitpar1).max(1);
    let n = r.wrapping_mul(2).wrapping_add(1);
    let tx = x.wrapping_sub(r).wrapping_add(seed.roll(n) as i32);
    let ty = y.wrapping_sub(r).wrapping_add(seed.roll(n) as i32);
    let p = MissileParams {
        flags: pf::TARGET_ABSOLUTE,
        origin: Some(m),
        target_x: tx,
        target_y: ty,
        ..sub_params(cx, m, Some(o), h)
    };
    create_missile(game, cx, &p);
    1
}

// ---------------------------------------------------------------- §6–§11

/// Plague ring RX (`0x006E2510`).
pub const RING_X: [i32; 16] = [0, 1, 2, 2, 2, 2, 2, 1, 0, -1, -2, -2, -2, -2, -2, -1];
/// Plague ring RY (`0x006E24D0`).
pub const RING_Y: [i32; 16] = [2, 2, 2, 1, 0, -1, -2, -2, -2, -2, -2, -1, 0, 1, 2, 2];

/// `ring(game, owner, origin, class, skill, level, a, b, loops)` =
/// `0x005A9370` (§6).
#[allow(clippy::too_many_arguments)]
pub fn ring<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    owner: Option<UnitId>,
    origin: Option<UnitId>,
    class: i32,
    skill: i32,
    level: i32,
    a: i32,
    b: i32,
    loops: i32,
) -> i32 {
    let Some(crow) = cx.row(class).cloned() else {
        return 0;
    };
    let (Some(_), Some(origin)) = (owner, origin) else {
        return 0;
    };
    let (x, y) = cx.world.position(origin);
    let mut p = MissileParams {
        flags: pf::POSITION | pf::TARGET_RELATIVE | pf::VELOCITY | 0x10,
        owner,
        origin: Some(origin),
        class,
        x,
        y,
        skill,
        level,
        ..MissileParams::default()
    };
    // §6 step 2: flags 0x17 (1, 2, 4 and 0x10 as read).
    // TODO(spec: bodies.md §6 step 2): 0x17 holds 0x10 (velocity already
    // fixed point), which §R2.3 reads as "no << 8"; step 3 says the
    // creation shifts it << 8. Flag 0x10 is kept as the number states.
    if loops > 0 {
        p.flags |= pf::LOOPS;
        p.loops = loops;
    }
    p.velocity = par(crow.param1).wrapping_shl(7);
    let s = b.max(1);
    let mut i = 0;
    while i < 16 {
        p.target_x = RING_X[i as usize];
        p.target_y = RING_Y[i as usize];
        create_missile(game, cx, &p);
        i += s;
    }
    if a != 0 {
        if a < 0 {
            // Edge case 4: the original loops forever.
            fatal(cx, 0x005A9370, m);
            return 1;
        }
        p.velocity = par(crow.param2).wrapping_shl(7);
        let mut i = 0;
        while i < 15 {
            p.target_x = RING_X[i as usize + 1];
            p.target_y = RING_Y[i as usize + 1];
            create_missile(game, cx, &p);
            i += a;
        }
    }
    1
}

/// §6 Server-hit 2 Plague Javelin, gas potions `0x005A9D80`.
pub fn srv_hit_2<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    unit: Option<UnitId>,
) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 3;
    };
    let h = i16v(row.hitsubmissile1);
    if h < 0 {
        return 3;
    }
    let origin = unit.unwrap_or(m);
    let owner = cx.owner(game, m);
    let (k, level) = skill_level(cx, m);
    ring(
        game,
        cx,
        m,
        owner,
        Some(origin),
        h,
        k,
        level,
        par(row.shitpar1),
        par(row.shitpar2),
        par(row.shitpar3),
    );
    3
}

/// §7 Server-do 6 Fire Wall maker, Molten Boulder `0x005AE680`.
pub fn srv_do_6<W: MissileWorld + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, m: UnitId) -> i32 {
    let Some(o) = cx.owner(game, m) else {
        return 2;
    };
    let Some(row) = cx.row_of(m).cloned() else {
        return 2;
    };
    let sub = i16v(row.submissile1);
    if sub < 0 || !cx.world.has_path(m) {
        return 2;
    }
    if cx.world.path_new_step(m) {
        let (x, y) = pos(cx, m);
        let p = MissileParams {
            flags: pf::POSITION | pf::TARGET_ABSOLUTE,
            x,
            y,
            target_x: x,
            target_y: y,
            ..sub_params(cx, m, Some(o), sub)
        };
        create_missile(game, cx, &p);
    }
    default_flight(game, cx, m)
}

/// §8 Server-hit 3 potions, bomb on ground `0x005A9F90`.
pub fn srv_hit_3<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    unit: Option<UnitId>,
) -> i32 {
    if unit.is_some() {
        return 0;
    }
    srv_hit_44(game, cx, m, None)
}

/// §8 Server-hit 44 Exploding / Ice Javelin `0x005A9E10`.
pub fn srv_hit_44<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    unit: Option<UnitId>,
) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 1;
    };
    let owner = cx.owner(game, m);
    let mut r = par(row.shitpar1);
    if r == 0 {
        let (k, level) = skill_level(cx, m);
        if !cx.world.skill_exists(k) {
            return 1;
        }
        r = calc_max(game, cx, owner, k, SkillCalc::AuraRange, level, 1);
    }
    let mut rec = full_record(cx, m, unit);
    row_flags(&mut rec, &row);
    let at = pos(cx, m);
    if area_damage(game, cx, owner, at, r, &rec, 0) != 0 {
        1
    } else {
        3
    }
}

/// Meteor scatter MX (`0x006E2550`).
pub const SCATTER_X: [i32; 18] = [2, -2, 0, 0, -3, 0, 3, -1, 1, -1, 2, -4, -3, -1, 0, 1, 3, 4];
/// Meteor scatter MY (`0x006E2598`).
pub const SCATTER_Y: [i32; 18] = [
    -2, -2, 2, 5, 3, 3, 3, 2, 1, -1, -1, -2, -2, -3, -4, -3, -3, -2,
];

/// `scatter(game, missile, class, R, s)` = `0x005AAA90` (§9).
pub fn scatter<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    class: i32,
    range: i32,
    s: i32,
) {
    if class < 0 {
        return;
    }
    let (x, y) = pos(cx, m);
    let owner = cx.owner(game, m);
    let mut p = MissileParams {
        flags: pf::POSITION,
        ..sub_params(cx, m, owner, class)
    };
    if range > 0 {
        p.flags |= pf::RANGE;
        p.range = range;
    }
    let s = s.max(1);
    let mut i = 0;
    while i < 18 {
        p.x = x.wrapping_add(SCATTER_X[i as usize]);
        p.y = y.wrapping_add(SCATTER_Y[i as usize]);
        create_missile(game, cx, &p);
        i += s;
    }
}

/// §9 Server-hit 14 Meteor center, catapult meteor, royal strike meteor
/// `0x005AABB0`.
pub fn srv_hit_14<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    unit: Option<UnitId>,
) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 1;
    };
    let (k, level) = skill_level(cx, m);
    if !cx.world.skill_exists(k) {
        return 1;
    }
    let Some(o) = cx.owner(game, m) else {
        return 1;
    };
    let at = pos(cx, m);
    let mut r = par(row.shitpar1);
    if r <= 0 {
        r = calc_max(game, cx, Some(o), k, SkillCalc::AuraRange, level, 1);
    }
    let mut rec = full_record(cx, m, unit);
    row_flags(&mut rec, &row);
    let c = if area_damage(game, cx, Some(o), at, r, &rec, 0) != 0 {
        1
    } else {
        3
    };
    let h = i16v(row.hitsubmissile1);
    if h >= 0 {
        let range = skill_lin(cx, k, level, 3, 4);
        scatter(game, cx, m, h, range, par(row.shitpar2).max(1));
    }
    c
}

/// §10 Server-hit 36 missile in air `0x005ABF70`.
pub fn srv_hit_36<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    unit: Option<UnitId>,
) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 1;
    };
    let h = i16v(row.hitsubmissile1);
    if h < 0 {
        return 1;
    }
    if unit.is_some() {
        return 0;
    }
    let (x, y) = pos(cx, m);
    let owner = cx.owner(game, m);
    let (k, level) = skill_level(cx, m);
    if let Some(n) = missile_at(game, cx, owner, k, level, h, x, y) {
        let w = data(cx, m).0;
        set_d28(cx, n, w);
    }
    1
}

/// The missile's path target unit (`0x00553540`: refreshed; none when it
/// is the missile itself).
fn path_target_unit<W: MissileWorld + ?Sized>(
    game: &Game,
    cx: &mut Ctx<'_, W>,
    u: UnitId,
) -> Option<UnitId> {
    cx.world.path_target(game, u).filter(|&t| t != u)
}

/// `aim(missile, T)` = `0x005AA460` (§11).
pub fn aim<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    t: Option<UnitId>,
) {
    let Some(row) = cx.row_of(m).cloned() else {
        return;
    };
    let t = t.filter(|&t| !cx.world.is_dead(t));
    let (_, level) = skill_level(cx, m);
    let n = i32::from(row.range as i16)
        .wrapping_add(i32::from(row.levrange as i16).wrapping_mul(level.wrapping_sub(1)));
    if let Some(d) = cx.store.get_mut(m) {
        d.total = clamp_frame(n);
        d.current = clamp_frame(n);
    }
    let d = match t {
        Some(t) => {
            cx.world.set_target_unit(m, t);
            set_d28(cx, m, 5);
            unit_distance(&*cx.world, m, t)
        }
        None => {
            let w = data(cx, m).1;
            let (x, y) = pos(cx, m);
            let px = x.wrapping_add(i32::from(w as i16));
            let py = y.wrapping_add(w >> 16);
            cx.world.set_target_point(m, px, py);
            set_d28(cx, m, 6);
            cx.world.target_distance(m)
        }
    };
    if d < 25 {
        cx.world.build(game, m);
    }
}

/// `retarget(missile, game)` = `0x005AA5B0` (§11).
pub fn retarget<W: MissileWorld + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, m: UnitId) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 1;
    };
    if data(cx, m).0 & 4 != 0 {
        return 1;
    }
    let Some(o) = cx.owner(game, m) else {
        return 1;
    };
    let at = pos(cx, m);
    let t = next_unit(game, cx, o, at, par(row.param2), 3, u32::MAX);
    aim(game, cx, m, t);
    0
}

/// §11 Server-hit 10 Guided Arrow, Bone Spirit `0x005AA650`.
pub fn srv_hit_10<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    unit: Option<UnitId>,
) -> i32 {
    if room_of(game, m).is_some_and(|r| cx.world.in_town(game, r)) {
        return 1;
    }
    let s = data(cx, m).0;
    if unit.is_none() && s & 4 != 0 {
        return 1;
    }
    if unit.is_none() && cx.world.collision_word(game, m) & 4 != 0 {
        return 1;
    }
    if s & 1 != 0 {
        return match unit {
            Some(u) if Some(u) != path_target_unit(game, cx, m) => 4,
            _ => 3,
        };
    }
    if s & 2 == 0 {
        return 3;
    }
    if left(cx, m) > 0 {
        return if s & 4 != 0 { 3 } else { 4 };
    }
    if retarget(game, cx, m) != 0 {
        1
    } else {
        4
    }
}

// ---------------------------------------------------------------- §12–§17

/// §12 / §30: server-hit 16 (`len` from `calc4`, no unit → 0, result 0)
/// and 19 (`auralencalc`, no unit → 1, result 3).
fn goo<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    unit: Option<UnitId>,
    hit_19: bool,
) -> i32 {
    let (k, level) = skill_level(cx, m);
    if !cx.world.skill_exists(k) {
        return 1;
    }
    let Some(o) = cx.owner(game, m) else {
        return 1;
    };
    let Some(u) = unit else {
        return if hit_19 { 1 } else { 0 };
    };
    let s = cx.world.skill_field(k, SkillField::AuraTargetState);
    if s < 0 || s >= cx.world.states_count() {
        return 1;
    }
    let calc = if hit_19 {
        SkillCalc::AuraLen
    } else {
        SkillCalc::Calc4
    };
    let len = calc_max(game, cx, Some(o), k, calc, level, 5);
    let expire = game.frame.wrapping_add(len);
    // Step 6: a fresh list stays out of this run (edge case 8).
    let list = cx.world.state_list_expiry(u, s).is_some();
    if !list && !cx.world.new_state_list(game, u, s, expire, o) {
        return 1;
    }
    if list {
        cx.world.aura_fill(game, u, s, k, level);
    }
    cx.world.mark_state_changed(u, s);
    if list {
        cx.world.set_state_list_expiry(u, s, expire);
    }
    // Timer 12 on the unit (`0x005417D0`); a unit without a timer class
    // has none.
    let _ = game.schedule_event(u, 12, expire, None, 0, 0);
    if hit_19 {
        3
    } else {
        0
    }
}

/// §12 Server-hit 16 Spider goo, vines trail, vines wither `0x005AAE10`.
pub fn srv_hit_16<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    unit: Option<UnitId>,
) -> i32 {
    goo(game, cx, m, unit, false)
}

/// §30 Server-hit 19 finger mage spider `0x005AB110`.
pub fn srv_hit_19<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    unit: Option<UnitId>,
) -> i32 {
    goo(game, cx, m, unit, true)
}

/// §13 Server-hit 18 Shout, Battle Command, Battle Orders `0x005AB0B0`.
pub fn srv_hit_18<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    unit: Option<UnitId>,
) -> i32 {
    let Some(o) = cx.owner(game, m) else {
        return 1;
    };
    if let Some(u) = unit {
        if cx.world.ally_test(game, o, u) {
            let (k, level) = skill_level(cx, m);
            cx.world.shout_state(game, u, o, k, level);
        }
    }
    0
}

/// §14 Server-hit 26 Grim Ward start `0x005AB8D0`.
pub fn srv_hit_26<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 1;
    };
    let h = i16v(row.hitsubmissile1);
    if h < 0 {
        return 1;
    }
    let Some(o) = cx.owner(game, m) else {
        return 1;
    };
    let (k, level) = skill_level(cx, m);
    if !cx.world.skill_exists(k) {
        return 1;
    }
    let mut r = par(row.shitpar1);
    if r <= 0 {
        r = calc_max(game, cx, Some(o), k, SkillCalc::Calc1, level, 5);
    }
    let (x, y) = pos(cx, m);
    let p = MissileParams {
        flags: pf::POSITION | pf::RANGE,
        range: r,
        x,
        y,
        ..sub_params(cx, m, Some(o), h)
    };
    create_missile(game, cx, &p);
    1
}

/// §15 Server-do 14 Grim Ward `0x005AEF70`.
pub fn srv_do_14<W: MissileWorld + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, m: UnitId) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 2;
    };
    let n = par(row.param1).max(1);
    if elapsed(cx, m) % n != 0 {
        return default_flight(game, cx, m);
    }
    let (k, level) = skill_level(cx, m);
    if k <= 0 || level <= 0 {
        return 2;
    }
    cx.world.skill_srv_do(game, m, par(row.param2), k, level);
    default_flight(game, cx, m)
}

/// Blade Fury BX (`0x006E2A58`).
pub const FURY_X: [i32; 8] = [16, 16, 0, -16, -16, -16, 0, 16];
/// Blade Fury BY (`0x006E2A38`).
pub const FURY_Y: [i32; 8] = [0, 16, 16, 16, 0, -16, -16, -16];

/// §16 Server-hit 52 Blade Fury `0x005AC940`.
pub fn srv_hit_52<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 1;
    };
    if i16v(row.hitsubmissile1) < 0 {
        return 1;
    }
    let Some(o) = cx.owner(game, m) else {
        return 1;
    };
    // Edge case 9: the class field is never written (class 0).
    let mut p = MissileParams {
        flags: pf::TARGET_RELATIVE,
        origin: Some(m),
        ..sub_params(cx, m, Some(o), 0)
    };
    let s = par(row.shitpar1).max(1);
    let mut i = 0;
    while i < 8 {
        let (bx, by) = (FURY_X[i as usize], FURY_Y[i as usize]);
        p.target_x = bx;
        p.target_y = by;
        if let Some(n) = create_missile(game, cx, &p) {
            set_d28(cx, n, bx);
            set_d2c(cx, n, by);
        }
        i += s;
    }
    1
}

/// §17 Server-hit 7 Holy Bolt, Fist of the Heavens bolt `0x005A9FB0`.
pub fn srv_hit_7<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    unit: Option<UnitId>,
) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 1;
    };
    let Some(u) = unit else {
        return 1;
    };
    let (k, level) = skill_level(cx, m);
    let owner = cx.owner(game, m);
    if par(row.shitpar1) != 0 {
        if let Some(o) = owner {
            if cx.world.is_pet(game, o, u, k) || cx.world.is_ally(game, o, u, k) {
                if !cx.world.skill_exists(k) {
                    return 1;
                }
                let lo = cx
                    .world
                    .skill_calc(game, Some(o), k, SkillCalc::Calc1, level)
                    .wrapping_shl(8);
                let hi = cx
                    .world
                    .skill_calc(game, Some(o), k, SkillCalc::Calc2, level)
                    .wrapping_shl(8);
                let seed = cx.world.seed(m);
                let mut v = lo.wrapping_add(seed.roll(hi.wrapping_sub(lo)) as i32);
                if v <= 0 {
                    let max = cx.world.stat(m, stat::MAXDAMAGE);
                    let min = cx.world.stat(m, stat::MINDAMAGE);
                    v = damage_roll(cx.world.seed(m), min, max, 0);
                }
                heal(game, cx, u, v);
                let po = i32::from(row.progoverlay as i16);
                if po > 0 {
                    cx.world.overlay(game, u, po);
                }
                return 1;
            }
        }
    }
    let w = &*cx.world;
    match type_of(game, u) {
        Some(UnitType::Player) => {
            if par(row.shitpar2) == 0 {
                3
            } else {
                4
            }
        }
        Some(UnitType::Monster) => match par(row.shitpar2) {
            0 => 3,
            2 if w.is_demon(u) => 3,
            2 => 4,
            _ if w.is_undead(u) => 3,
            _ => 4,
        },
        _ => 4,
    }
}

/// Life (stat 6) := min(life + v, max life).
pub(super) fn heal<W: MissileWorld + ?Sized>(
    _game: &mut Game,
    cx: &mut Ctx<'_, W>,
    u: UnitId,
    v: i32,
) {
    let life = cx.world.stat(u, 6);
    let max = cx.world.max_life(u);
    cx.world.set_stat(u, 6, life.wrapping_add(v).min(max));
}

// ---------------------------------------------------------------- §18–§23

/// §18 Server-do 22 lightning trailing javelin `0x005AF620`.
pub fn srv_do_22<W: MissileWorld + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, m: UnitId) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 2;
    };
    let sub = i16v(row.submissile1);
    if sub == 0 || !cx.world.has_path(m) {
        return 2;
    }
    let (x, y) = pos(cx, m);
    if elapsed(cx, m) < 2 {
        let (tx, ty) = cx.world.path_target_point(m);
        let (dx, dy) = (tx.wrapping_sub(x), ty.wrapping_sub(y));
        set_d28(cx, m, dy.wrapping_neg());
        set_d2c(cx, m, dx);
    }
    if cx.world.path_new_step(m) {
        let owner = cx.owner(game, m);
        let (a, b) = data(cx, m);
        let mut p = MissileParams {
            flags: pf::POSITION | pf::TARGET_RELATIVE | pf::LOOPS,
            x,
            y,
            loops: par(row.param1),
            target_x: a,
            target_y: b,
            ..sub_params(cx, m, owner, sub)
        };
        create_missile(game, cx, &p);
        p.target_x = a.wrapping_neg();
        p.target_y = b.wrapping_neg();
        create_missile(game, cx, &p);
    }
    default_flight(game, cx, m)
}

/// The `zigzag` init callback (`0x005AC040`) as the parameter record's
/// callback id.
pub const ZIGZAG_CALLBACK: u32 = 0x005AC040;

/// `zigzag(missile)` = `0x005AC040` (§19): re-seed from the path target
/// x, path type 10, distance min(total frames, 255), rebuild.
pub fn zigzag<W: MissileWorld + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, m: UnitId) {
    let Some(d) = cx.store.get(m) else {
        return;
    };
    let t = i32::from(d.total).min(255);
    let tx = cx.world.path_target_point(m).0;
    *cx.world.seed(m) = Seed::init_low(tx as u32);
    cx.world.set_path_type(m, 10);
    cx.world.set_path_distance(m, t);
    cx.world.build(game, m);
}

/// Nova SX (`0x006E1510`), SY (`0x006E14E4`), P (`0x006E14F4`), Q
/// (`0x006E14C8`), EX (`0x006E14A8`), EY (`0x006E1488`).
const NOVA_SX: [i32; 4] = [-1, 1, 1, -1];
const NOVA_SY: [i32; 4] = [-1, -1, 1, 1];
const NOVA_P: [i32; 7] = [18, 20, 17, 20, 15, 19, 18];
const NOVA_Q: [i32; 7] = [8, 2, 11, 4, 13, 6, 9];
const NOVA_EX: [i32; 7] = [20, -20, 0, 0, 14, -14, -14];
const NOVA_EY: [i32; 7] = [0, 0, 20, -20, 14, 14, -14];

/// The target offsets `nova(game, n, record)` (`0x0056D4E0`, §19)
/// creates at, in order; `None` when n ≥ 41 (fatal).
pub fn nova_offsets(n: i32) -> Option<Vec<(i32, i32)>> {
    if n.wrapping_sub(1) / 5 > 7 {
        return None;
    }
    let mut v = vec![(14, -14)];
    let mut c = n.wrapping_sub(1);
    for j in 0..7 {
        if c == 0 {
            break;
        }
        for i in 0..4 {
            if c <= 0 {
                break;
            }
            v.push((NOVA_SX[i] * NOVA_P[j], NOVA_SY[i] * NOVA_Q[j]));
            v.push((NOVA_SX[i] * NOVA_Q[j], NOVA_SY[i] * NOVA_P[j]));
            c -= 2;
        }
        v.push((NOVA_EX[j], NOVA_EY[j]));
        c -= 1;
    }
    Some(v)
}

/// `nova(game, n, record)` = `0x0056D4E0` (§19).
fn nova<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    n: i32,
    p: &MissileParams,
) {
    let Some(offsets) = nova_offsets(n) else {
        fatal(cx, 0x0056D4E0, m);
        return;
    };
    let mut p = *p;
    for (dx, dy) in offsets {
        p.target_x = dx;
        p.target_y = dy;
        create_missile(game, cx, &p);
    }
}

/// §19 Server-hit 45 lightning trailing javelin `0x005AC480`.
pub fn srv_hit_45<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 1;
    };
    let h = i16v(row.hitsubmissile1);
    let n = par(row.shitpar1);
    if h < 0 || n <= 0 {
        return 1;
    }
    let (x, y) = pos(cx, m);
    let owner = cx.owner(game, m);
    let mut p = MissileParams {
        flags: pf::POSITION | pf::TARGET_RELATIVE,
        x,
        y,
        ..sub_params(cx, m, owner, h)
    };
    if par(row.shitpar2) != 0 {
        p.init = Some((ZIGZAG_CALLBACK, 0));
    }
    nova(game, cx, m, n, &p);
    1
}

/// §19 Server-hit 38 catapult charged ball `0x005AC0A0`.
pub fn srv_hit_38<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 1;
    };
    let h = i16v(row.hitsubmissile1);
    if h < 0 {
        return 1;
    }
    let Some(o) = cx.owner(game, m) else {
        return 1;
    };
    let (k, level) = skill_level(cx, m);
    let (x, y) = pos(cx, m);
    let p = MissileParams {
        flags: pf::POSITION | pf::TARGET_RELATIVE,
        x,
        y,
        init: Some((ZIGZAG_CALLBACK, 0)),
        ..sub_params(cx, m, Some(o), h)
    };
    let mut n =
        par(row.shitpar1).wrapping_add(level.wrapping_sub(1).wrapping_mul(par(row.shitpar2)));
    if n <= 0 {
        if !cx.world.skill_exists(k) {
            return 1;
        }
        n = calc_max(game, cx, Some(o), k, SkillCalc::Calc4, level, 1);
    }
    nova(game, cx, m, n, &p);
    1
}

/// §20 Server-do 23 / 24 Succubus fireball, firestorm maker `0x005AF790`.
pub fn srv_do_23<W: MissileWorld + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, m: UnitId) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 2;
    };
    let sub = i16v(row.submissile1);
    if sub == 0 {
        return 2;
    }
    if cx.world.path_new_step(m) {
        let (x, y) = pos(cx, m);
        let owner = cx.owner(game, m);
        let mut p = MissileParams {
            flags: pf::POSITION,
            x,
            y,
            ..sub_params(cx, m, owner, sub)
        };
        let loops = par(row.param1);
        if loops > 0 {
            p.flags |= pf::LOOPS;
            p.loops = loops;
        }
        create_missile(game, cx, &p);
    }
    default_flight(game, cx, m)
}

/// §21 Server-do 26 Vines, Plague Vines `0x005AF980`.
pub fn srv_do_26<W: MissileWorld + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, m: UnitId) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 2;
    };
    let sub = i16v(row.submissile1);
    if sub == 0 {
        return 2;
    }
    if elapsed(cx, m) % par(row.param1).max(1) == 0 {
        let owner = cx.owner(game, m);
        let (k, level) = skill_level(cx, m);
        let (x, y) = pos(cx, m);
        missile_at(game, cx, owner, k, level, sub, x, y);
    }
    default_flight(game, cx, m)
}

/// §22 Server-do 31 Wake of Destruction maker, Baal cold maker
/// `0x005B01F0`.
pub fn srv_do_31<W: MissileWorld + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, m: UnitId) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 2;
    };
    let sub = i16v(row.submissile1);
    if sub < 0 {
        return 2;
    }
    let Some(o) = cx.owner(game, m) else {
        // The hit handler's no-unit path inlined (§R5, a4 = 0).
        let mut c = if row.explosion { 0 } else { 2 };
        if row.collidekill != 0 {
            c |= 1;
        }
        if row.alwaysexplode != 0 {
            cx.world.hit_by_missile_event(game, m, None);
            let idx = super::RowExt::srv_hit(&row);
            if srv_hit_in_range(idx) {
                c = run_srv_hit(game, cx, idx, m, None, c);
                if c & 4 != 0 {
                    return 2;
                }
            }
        }
        if c & 1 != 0 && room_of(game, m).is_some() {
            cx.world.clear_footprint(game, m);
        }
        return 2;
    };
    if cx.world.path_new_step(m) {
        let (a, b) = data(cx, m);
        let mut p = MissileParams {
            flags: pf::TARGET_RELATIVE,
            origin: Some(m),
            target_x: a,
            target_y: b,
            ..sub_params(cx, m, Some(o), sub)
        };
        create_missile(game, cx, &p);
        p.target_x = a.wrapping_neg();
        p.target_y = b.wrapping_neg();
        create_missile(game, cx, &p);
    }
    default_flight(game, cx, m)
}

/// §23 Server-hit 56 Armageddon / Diablogeddon control `0x005ACC50`.
pub fn srv_hit_56<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 1;
    };
    let Some(o) = cx.owner(game, m) else {
        return 1;
    };
    let (k, level) = skill_level(cx, m);
    let (x, y) = pos(cx, m);
    let mut rec = full_record(cx, m, None);
    let mut r = par(row.shitpar1);
    if r <= 0 {
        if !cx.world.skill_exists(k) {
            return 1;
        }
        r = calc_max(game, cx, Some(o), k, SkillCalc::AuraRange, level, 1);
    }
    row_flags(&mut rec, &row);
    area_damage(game, cx, Some(o), (x, y), r, &rec, 0);
    let h = i16v(row.hitsubmissile1);
    if h >= 0 {
        missile_at(game, cx, Some(o), k, level, h, x, y);
    }
    1
}

// ---------------------------------------------------------------- §24–§30

/// §24 Server-hit 8 Blaze `0x005AA180`.
pub fn srv_hit_8<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    unit: Option<UnitId>,
) -> i32 {
    let Some(u) = unit else {
        return 2;
    };
    match cx.owner(game, m) {
        Some(o) if o == u && cx.world.has_state(o, 13) => 0,
        _ => 2,
    }
}

/// `fire_disc(game, missile, r, class, h)` = `0x005A9530` (§25).
pub fn fire_disc<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    r: i32,
    class: i32,
    h: i32,
) {
    if room_of(game, m).is_none() {
        return;
    }
    let h = h.max(0);
    let owner = cx.owner(game, m);
    let mut p = MissileParams {
        flags: pf::POSITION,
        ..sub_params(cx, m, owner, class)
    };
    if h != 0 {
        p.flags |= pf::RANGE;
        p.range = h;
    }
    let (x0, y0) = pos(cx, m);
    let mut a = r.wrapping_neg();
    while a <= r {
        let mut b = r.wrapping_neg();
        while b <= r {
            let (cx_, cy) = (x0.wrapping_add(a), y0.wrapping_add(b));
            let inside = a.wrapping_mul(a).wrapping_add(b.wrapping_mul(b)) <= r.wrapping_mul(r);
            if let (Some(room), true) = (room_of(game, m), inside) {
                let out = (x0.wrapping_add(2 * a), y0.wrapping_add(2 * b));
                if !cx.world.line_hits(game, room, (cx_, cy), out, 4) {
                    let cell = cx.world.find_room(game, room, cx_, cy);
                    if cell.is_some_and(|c| !cx.world.in_town(game, c)) {
                        p.x = cx_;
                        p.y = cy;
                        create_missile(game, cx, &p);
                    }
                }
            }
            b += 1;
        }
        a += 1;
    }
}

/// §25 Server-hit 9 Immolation Arrow `0x005AA250`.
pub fn srv_hit_9<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    unit: Option<UnitId>,
) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 1;
    };
    let (k, level) = skill_level(cx, m);
    if !cx.world.skill_exists(k) {
        return 1;
    }
    let owner = cx.owner(game, m);
    let h = i16v(row.hitsubmissile1);
    if h >= 0 {
        cx.world.set_stat(m, stat::COLDLENGTH, 0);
        cx.world.set_stat(m, 134, 0);
        let mut r = par(row.shitpar1);
        if r <= 0 {
            r = calc_max(game, cx, owner, k, SkillCalc::Calc1, level, 1);
        }
        let class = cx.store.get(m).map_or(0, |d| i32::from(d.class));
        let hv = cx
            .world
            .missile_calc(game, m, owner, row.shitcalc1, class, level);
        fire_disc(game, cx, m, r, h, hv);
    }
    let mut r2 = par(row.shitpar2);
    if r2 <= 0 {
        r2 = calc_max(game, cx, owner, k, SkillCalc::Calc2, level, 1);
    }
    let mut rec = DamageRecord::default();
    elem_roll(cx, m, unit, &mut rec);
    row_flags(&mut rec, &row);
    let owner = cx.owner(game, m);
    let at = pos(cx, m);
    area_damage(game, cx, owner, at, r2, &rec, 0);
    3
}

/// §26 Server-do 9 bat lightning bolt `0x005AE940`.
pub fn srv_do_9<W: MissileWorld + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, m: UnitId) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        // Edge case 13: `SubMissile1` read through a null record.
        fatal(cx, 0x005AE940, m);
        return 1;
    };
    if cx.world.path_new_step(m) {
        let (x, y) = pos(cx, m);
        let owner = cx.owner(game, m);
        let p = MissileParams {
            flags: pf::POSITION,
            x,
            y,
            ..sub_params(cx, m, owner, i16v(row.submissile1))
        };
        create_missile(game, cx, &p);
    }
    default_flight(game, cx, m)
}

/// §27 Server-hit 15 spider goo lay `0x005AAD40`.
pub fn srv_hit_15<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 1;
    };
    let h = i16v(row.hitsubmissile1);
    if h < 0 {
        return 1;
    }
    let Some(o) = cx.owner(game, m) else {
        return 1;
    };
    let (x, y) = pos(cx, m);
    let p = MissileParams {
        flags: pf::POSITION,
        x,
        y,
        ..sub_params(cx, m, Some(o), h)
    };
    create_missile(game, cx, &p);
    0
}

/// §28 Server-hit 17 Howl `0x005AAFB0`.
pub fn srv_hit_17<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    unit: Option<UnitId>,
) -> i32 {
    let (k, level) = skill_level(cx, m);
    if !cx.world.skill_exists(k) {
        return 1;
    }
    let Some(o) = cx.owner(game, m) else {
        return 1;
    };
    let Some(u) = unit.filter(|&u| type_of(game, u) == Some(UnitType::Monster)) else {
        return 0;
    };
    let s = cx.world.skill_field(k, SkillField::AuraTargetState);
    if s < 0 || s >= cx.world.states_count() {
        return 1;
    }
    if cx.world.has_state(u, s as u16) {
        return 0;
    }
    let mine = cx
        .world
        .stat(o, 12)
        .wrapping_add(level)
        .wrapping_add(cx.world.skill_field(k, SkillField::Param(2)));
    if mine <= cx.world.stat(u, 12) {
        return 0;
    }
    let a = skill_lin(cx, k, level, 3, 4);
    let b = skill_lin(cx, k, level, 5, 6);
    cx.world.terror(game, o, u, k, a, b);
    0
}

/// §29 Server-do 11 finger mage spider `0x005AEB60`.
pub fn srv_do_11<W: MissileWorld + ?Sized>(game: &mut Game, cx: &mut Ctx<'_, W>, m: UnitId) -> i32 {
    let Some(row) = cx.row_of(m).cloned() else {
        return 2;
    };
    let mut t = path_target_unit(game, cx, m);
    let owner = cx.owner(game, m);
    if t.is_none() {
        if let Some(o) = owner {
            t = path_target_unit(game, cx, o);
        }
    }
    let Some(t) = t else {
        return default_flight(game, cx, m);
    };
    let mut n = par(row.param1);
    if n <= 0 {
        n = 5;
    }
    if left(cx, m) % n != 0 {
        return default_flight(game, cx, m);
    }
    if unit_distance(&*cx.world, m, t) > par(row.param2) {
        return default_flight(game, cx, m);
    }
    let s = par(row.param3).max(1);
    let (x, y) = pos(cx, m);
    let (tx, ty) = cx.world.position(t);
    let dx = tx.wrapping_sub(x).signum() * s;
    let dy = ty.wrapping_sub(y).signum() * s;
    cx.world
        .set_target_point(m, x.wrapping_add(dx), y.wrapping_add(dy));
    cx.world.build(game, m);
    default_flight(game, cx, m)
}
