// Spec: specs/missiles/missiles.md §R9 (catalogues `srvdo.tsv`, `srvhit.tsv`)
//! The server-do and server-hit function tables (§R9.1) and their
//! dispatch. Bodies the spec gives in full are implemented; every other
//! non-null entry is a stub that logs [`Unhandled`] and does nothing.
//! The address tables below are copied from `srvdo.tsv` / `srvhit.tsv` and
//! checked against them by `tests::catalogues_match_tsv` (METHODS M05).
//!
//! Implemented: server-do 1 (default flight, §R4); the seeded sub-missile
//! helper `0x005A9820` (§R9.3) that server-do 8, 10, 17 and 25 call.
//! Stubs: every other server-do (2, 3, 5–37) and every server-hit
//! (1–29, 31–33, 35–40, 43–45, 47–48, 50–59): their bodies are
//! `summarized` or `D2MOO-only` in the catalogues.

use crate::game::Game;
use crate::units::UnitId;

use super::create::{create_missile, MissileParams};
use super::flight::default_flight;
use super::{param_flags, Ctx, MissileWorld, Unhandled, SRV_DO_COUNT, SRV_HIT_COUNT};

/// `srvdo.tsv` `index`, `addr_114d`: table `0x0073C768` (53 entries).
pub const SRV_DO: [Option<u32>; SRV_DO_COUNT as usize] = [
    None,
    Some(0x005B0BC0),
    Some(0x005AE400),
    Some(0x005AE480),
    None,
    Some(0x005AE520),
    Some(0x005AE680),
    Some(0x005AE780),
    Some(0x005AE8A0),
    Some(0x005AE940),
    Some(0x005AEA60),
    Some(0x005AEB60),
    Some(0x005AECA0),
    Some(0x005AEDA0),
    Some(0x005AEF70),
    Some(0x005AF030),
    Some(0x005AF170),
    Some(0x005AF240),
    Some(0x005AF300),
    Some(0x005B0940),
    Some(0x005AF540),
    Some(0x005AF590),
    Some(0x005AF620),
    Some(0x005AF790),
    Some(0x005AF790),
    Some(0x005AF880),
    Some(0x005AF980),
    Some(0x005AFA30),
    Some(0x005AFB80),
    Some(0x005AFD70),
    Some(0x005B0010),
    Some(0x005B01F0),
    Some(0x005B03E0),
    Some(0x005AFEC0),
    Some(0x005B04A0),
    Some(0x005B0640),
    Some(0x005B0A40),
    Some(0x005B0AA0),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
];

/// `srvhit.tsv` `index`, `addr_114d`: table `0x0073C840` (71 entries).
pub const SRV_HIT: [Option<u32>; SRV_HIT_COUNT as usize] = [
    None,
    Some(0x005A9A70),
    Some(0x005A9D80),
    Some(0x005A9F90),
    Some(0x005B07A0),
    Some(0x005ABC40),
    Some(0x005AA1C0),
    Some(0x005A9FB0),
    Some(0x005AA180),
    Some(0x005AA250),
    Some(0x005AA650),
    Some(0x005B0870),
    Some(0x005AA730),
    Some(0x005AA8B0),
    Some(0x005AABB0),
    Some(0x005AAD40),
    Some(0x005AAE10),
    Some(0x005AAFB0),
    Some(0x005AB0B0),
    Some(0x005AB110),
    Some(0x005AB370),
    Some(0x005AB500),
    Some(0x005ADD20),
    Some(0x005ACFC0),
    Some(0x005A9BF0),
    Some(0x005AB820),
    Some(0x005AB8D0),
    Some(0x005ABA00),
    Some(0x005ABA10),
    Some(0x005ABB00),
    None,
    Some(0x005ABD70),
    Some(0x005ABE50),
    Some(0x005ABEB0),
    None,
    Some(0x005ABEE0),
    Some(0x005ABF70),
    Some(0x005AC020),
    Some(0x005AC0A0),
    Some(0x005AC1D0),
    Some(0x005AC250),
    None,
    None,
    Some(0x005AC350),
    Some(0x005A9E10),
    Some(0x005AC480),
    None,
    Some(0x005AC550),
    Some(0x005AC6D0),
    None,
    Some(0x005AC800),
    Some(0x005AC870),
    Some(0x005AC940),
    Some(0x005ACA50),
    Some(0x005ACAF0),
    Some(0x005ACB60),
    Some(0x005ACC50),
    Some(0x005AD970),
    Some(0x005ACDF0),
    Some(0x005ACF20),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
];

/// The catalogue texts, for the consistency test.
pub const SRVDO_TSV: &str = include_str!("../../../../specs/missiles/srvdo.tsv");
pub const SRVHIT_TSV: &str = include_str!("../../../../specs/missiles/srvhit.tsv");

/// Server-do indices with a body here.
pub const SRV_DO_IMPLEMENTED: [i16; 8] = [1, 2, 3, 5, 7, 8, 10, 25];
/// Server-hit indices with a body here.
pub const SRV_HIT_IMPLEMENTED: [i16; 4] = [1, 4, 12, 13];

/// Whether a server-hit index is called (1…70, §R5 step 6.3).
pub fn srv_hit_in_range(index: i16) -> bool {
    (1..SRV_HIT_COUNT).contains(&index)
}

/// Runs server-do `index` (§R3 step 7; the caller checked 1…52).
pub fn run_srv_do<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    index: i16,
    m: UnitId,
) -> i32 {
    use super::bodies::*;
    match index {
        1 => default_flight(game, cx, m),
        2 => srv_do_2(game, cx, m),
        3 => srv_do_3(game, cx, m),
        5 => srv_do_5(game, cx, m),
        7 => srv_do_7(game, cx, m),
        8 => srv_do_8(game, cx, m),
        10 => srv_do_10(game, cx, m),
        25 => srv_do_25(game, cx, m),
        _ => {
            let entry = SRV_DO.get(index as usize).copied().flatten();
            cx.store.unhandled.push(match entry {
                Some(_) => Unhandled::SrvDo { index, missile: m },
                None => Unhandled::NullSrvDo { index, missile: m },
            });
            // TODO(missiles.md open question 8): stub; the body is not
            // spec'd. Keeps the missile, does nothing else.
            1
        }
    }
}

/// Runs server-hit `index` with the handler's current result `c`
/// (§R5 step 5/6.3; the caller checked 1…70): the §R9.6 bodies
/// ([`super::bodies`]); a stub returns `c` unchanged, as if the row had
/// no function.
pub fn run_srv_hit<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    index: i16,
    m: UnitId,
    unit: Option<UnitId>,
    c: i32,
) -> i32 {
    use super::bodies::*;
    match index {
        1 => return srv_hit_1(game, cx, m, unit),
        4 => return srv_hit_4(game, cx, m, unit),
        12 => return srv_hit_12(game, cx, m, unit),
        13 => return srv_hit_13(game, cx, m, unit),
        _ => {}
    }
    let entry = SRV_HIT.get(index as usize).copied().flatten();
    cx.store.unhandled.push(match entry {
        Some(_) => Unhandled::SrvHit { index, missile: m },
        None => Unhandled::NullSrvHit { index, missile: m },
    });
    // TODO(missiles.md open question 8): stub; the body is not spec'd.
    c
}

/// `0x005A9820` (§R9.3, D2MOO `MISSMODE_CreateMissileWithCollisionCheck`):
/// every `interval` elapsed frames, re-seed the missile with
/// `init_low(x + elapsed)`, draw an offset within `range` and create
/// `class` there unless the collision there matches `mask`.
pub fn create_with_collision_check<W: MissileWorld + ?Sized>(
    game: &mut Game,
    cx: &mut Ctx<'_, W>,
    m: UnitId,
    range: i32,
    interval: i32,
    class: i32,
    mask: u16,
) -> Option<UnitId> {
    let d = cx.store.get(m)?;
    let elapsed = d.elapsed();
    let (skill, level) = (i32::from(d.skill), i32::from(d.level));
    // Step 1 (signed remainder).
    if interval == 0 || elapsed % interval != 0 {
        return None;
    }
    // Step 2.
    let owner = cx.owner(game, m)?;
    let room = game.lists.unit(m)?.room()?;
    // Steps 3–4.
    let (x, y) = cx.world.position(m);
    let (dx, dy) = seeded_offset(cx.world.seed(m), x, elapsed, range);
    let (sx, sy) = (x.wrapping_add(dx), y.wrapping_add(dy));
    // Step 5.
    if cx.world.collision_at(game, room, sx, sy, mask) != 0 {
        return None;
    }
    let p = MissileParams {
        flags: param_flags::POSITION | param_flags::TARGET_RELATIVE,
        owner: Some(owner),
        class,
        x: sx,
        y: sy,
        skill,
        level,
        ..MissileParams::default()
    };
    create_missile(game, cx, &p)
}

/// §R9.3 steps 3–4: re-seed `{x + elapsed, 666}`, then dx, dy =
/// `roll(2(r − 1)) − (r − 1)` each, in that order.
pub fn seeded_offset(seed: &mut crate::rng::Seed, x: i32, elapsed: i32, range: i32) -> (i32, i32) {
    *seed = crate::rng::Seed::init_low(x.wrapping_add(elapsed) as u32);
    let r1 = range.wrapping_sub(1);
    let dx = (seed.roll(r1.wrapping_mul(2)) as i32).wrapping_sub(r1);
    let dy = (seed.roll(r1.wrapping_mul(2)) as i32).wrapping_sub(r1);
    (dx, dy)
}
