// Spec: specs/sim/pathing.md §3, §8.1, §9.1, §9.3, §9.4, §9.10, §11.1; specs/missiles/missiles.md §R2.3, §R4; specs/monsters/ai.md §1.4, §7.1; specs/sim/units.md §4.6 (motion on the path provider)
//! Motion on the path provider (`enable_paths`): a missile built and
//! flown by the real creation code (blocker WP1 of `docs/HANDOFF.md`
//! step 7p), and monsters walking through the real monster mode set,
//! the every-tick event 0 and the mode end.

use std::sync::Arc;

use crate::drlg::TileRect;
use crate::missiles::{create_missile, param_flags, MissileParams};
use crate::monsters::ai::seams::{AiModes, AiUnits};
use crate::monsters::ai::ModeTarget;
use crate::path::record::{flags, path_types};
use crate::tick::events::event;
use crate::units::{UnitId, UnitType};
use crate::wiring::action::tests::{Fx, LEVEL};

/// `velocitypercent` (`pathing.md` §8.1).
const STAT_VELOCITY: u16 = 67;
/// Monster modes (`units.md` §4.6).
const WALK: u8 = 2;
const ATTACK1: u8 = 4;
const BLOCK: u8 = 6;

fn fx_paths(on: bool) -> Fx {
    let mut fx = Fx::with_rooms(&[
        (LEVEL, TileRect::new(0, 0, 8, 8)),
        (LEVEL, TileRect::new(8, 0, 8, 8)),
    ]);
    let h = fx.sim.hooks();
    if on {
        h.enable_paths().expect("embedded tables");
    }
    // monstats `Velocity` 6: velocity 0x600 at 100 % (`pathing.md` §8.1
    // rule 2), the player's vector M1 speed.
    // monstats2 `SizeX` 2: pattern 1, a footprint (`path-placement.md`
    // §3), as in `coarse_free_box_avoids_a_monster_footprint`.
    let t = Arc::make_mut(&mut h.tables);
    t.combat.monstats[0].velocity = 6;
    let ex = usize::from(t.combat.monstats[0].monstatsex);
    t.combat.monstats2[ex].sizex = 2;
    fx
}

fn fx() -> Fx {
    fx_paths(true)
}

fn monster(fx: &mut Fx, x: i32, y: i32) -> UnitId {
    let a = fx.a;
    let m = fx.spawn(UnitType::Monster, 0, a, x, y);
    fx.stats(m, &[(STAT_VELOCITY, 100)]);
    m
}

/// An AI mode change (`ai.md` §7.1, `0x005A7E60` + `0x005A7C20`).
fn change(fx: &mut Fx, m: UnitId, mode: u8, t: ModeTarget) -> bool {
    fx.sim
        .with(&mut fx.game, |g, v| v.change_mode(g, m, mode, t))
}

fn mode(fx: &Fx, u: UnitId) -> u32 {
    fx.sim.sys.units.get(u).unwrap().mode
}

/// Ticks while the unit stays in `moving`, recording (precise x,
/// precise y, mode) after each tick.
fn ticks(fx: &mut Fx, u: UnitId, moving: u32, max: usize) -> Vec<(u32, u32, u32)> {
    let mut out = Vec::new();
    for _ in 0..max {
        fx.tick();
        let d = dynamic(fx, u);
        let m = mode(fx, u);
        out.push((d.precise_x, d.precise_y, m));
        if m != moving {
            break;
        }
    }
    out
}

fn fire(fx: &mut Fx, owner: UnitId, tx: i32, ty: i32) -> UnitId {
    let p = MissileParams {
        owner: Some(owner),
        origin: Some(owner),
        class: 0,
        flags: param_flags::TARGET_ABSOLUTE,
        target_x: tx,
        target_y: ty,
        ..MissileParams::default()
    };
    fx.sim
        .missiles(&mut fx.game, |g, cx| create_missile(g, cx, &p))
        .unwrap()
        .expect("created")
}

fn dynamic(fx: &mut Fx, u: UnitId) -> crate::path::DynamicPath {
    fx.sim
        .hooks()
        .paths
        .as_ref()
        .unwrap()
        .dynamic(u)
        .unwrap()
        .clone()
}

// Covers: specs/sim/pathing.md §3 r1, §11 text, §11.1 r3, §9.4 r2; specs/missiles/missiles.md §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r16
#[test]
fn missile_path_is_built_and_flown_on_the_provider() {
    // WP1: the missile's path has type 4 and flag 0x40000 from its
    // allocation, so the build goes to the straight missile path (§11.1):
    // one point (the target), flag 0x20. The arrow's Vel 1 gives 0x100,
    // 75 % → 0xC0 (§R2.3 steps 5, 7); the every-tick event moves it
    // (0x400 · 0xC0) >> 6 = 0xC00 a frame along x (§9.4 rule 2.1, the
    // direction vector (4096, 0)).
    let mut fx = fx();
    let a = fx.a;
    let owner = fx.spawn(UnitType::Monster, 0, a, 41, 30);
    let m = fire(&mut fx, owner, 79, 30);
    let d = dynamic(&mut fx, m);
    assert_eq!(d.path_type, path_types::MISSILE);
    assert_ne!(d.flags & flags::MISSILE, 0);
    assert_ne!(d.flags & flags::ACTIVE, 0);
    assert_eq!((d.point_count, d.point(0).x, d.point(0).y), (1, 79, 30));
    assert_eq!(d.velocity, 0xC0);
    for k in 1..=30u32 {
        fx.frame();
        let d = dynamic(&mut fx, m);
        assert_eq!(
            (d.precise_x, d.precise_y),
            (0x298000 + k * 0xC00, 0x1E8000),
            "frame {k}"
        );
    }
    assert_eq!(fx.sim.hooks().path_position(m), (42, 30));
    fx.assert_clean();
}

// Covers: specs/missiles/missiles.md §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r8
#[test]
fn missile_out_of_range_gets_no_point() {
    // M08 for the build: a target 100 sub-tiles away fails the create
    // (§R2.3 step 8) before any path; 99 away is built with one point.
    let mut fx = fx();
    let a = fx.a;
    let owner = fx.spawn(UnitType::Monster, 0, a, 1, 30);
    let m = fire(&mut fx, owner, 100, 30);
    assert_eq!(dynamic(&mut fx, m).point_count, 1);
    let p = MissileParams {
        owner: Some(owner),
        origin: Some(owner),
        class: 0,
        flags: param_flags::TARGET_ABSOLUTE,
        target_x: 101,
        target_y: 30,
        ..MissileParams::default()
    };
    let r = fx
        .sim
        .missiles(&mut fx.game, |g, cx| create_missile(g, cx, &p))
        .unwrap();
    assert!(r.is_none());
}

// Covers: specs/monsters/ai.md §7.1; specs/sim/pathing.md §8.1 r2, §9.1, §9.10; specs/sim/units.md §4.6
#[test]
fn monster_walks_to_a_point_sub_tile_by_sub_tile() {
    // Vector M1 for a monster: monstats Velocity 6 at 100 % → 0x600;
    // the mode set targets the point, gives the re-path budget 20 and
    // path type 13 (request byte 101), computes one point; event 0 every
    // tick moves +0x6000 a tick, the 14th lands on (31, 10) and the mode
    // end (`ai.md` §1.4) sets the anim mode neutral.
    let mut fx = fx();
    let m = monster(&mut fx, 26, 10);
    assert!(change(&mut fx, m, WALK, ModeTarget::Point(31, 10)));
    assert_eq!(mode(&fx, m), 2);
    let d = dynamic(&mut fx, m);
    assert_eq!(d.path_type, path_types::TOWARD_FINISH);
    assert_eq!((d.target_x, d.target_y, d.target_unit), (31, 10, None));
    assert_eq!(d.repath_budget, 20);
    assert_eq!(d.velocity, 0x600);
    assert!(d.point_count >= 1);
    assert_eq!(
        (
            d.point(d.point_count as usize - 1).x,
            d.point(d.point_count as usize - 1).y
        ),
        (31, 10)
    );
    assert!(fx.timers(m).contains(&(event::MODE_CHANGE, -1)));
    let t = ticks(&mut fx, m, 2, 20);
    assert_eq!(t.len(), 14);
    for (k, &(x, y, md)) in t.iter().take(13).enumerate() {
        assert_eq!(x, 0x1AE000 + k as u32 * 0x6000, "tick {}", k + 1);
        assert_eq!((y, md), (0xA8000, 2), "tick {}", k + 1);
    }
    assert_eq!(t[13], (0x1F8000, 0xA8000, 1));
    assert_eq!(fx.sim.hooks().path_position(m), (31, 10));
    // The footprint moved with the monster (mask 0x100).
    let a = fx.a;
    let cell = |fx: &mut Fx, x, y| {
        crate::path::collision::point_value(&fx.sim.hooks().drlg, Some(a), x, y, 0xFFFF)
    };
    assert_eq!(cell(&mut fx, 26, 10) & 0x100, 0);
    assert_ne!(cell(&mut fx, 31, 10) & 0x100, 0);
    fx.assert_clean();
}

// Covers: specs/sim/units.md §4.6
#[test]
fn without_the_provider_the_monster_does_not_move() {
    // M08: the same mode change with the provider off sets the mode and
    // the every-tick event, but nothing steps the monster.
    let mut fx = fx_paths(false);
    let m = monster(&mut fx, 26, 10);
    let before = fx.sim.hooks().path_position(m);
    assert!(change(&mut fx, m, WALK, ModeTarget::Point(31, 10)));
    for _ in 0..20 {
        fx.tick();
    }
    // The walk start's mode set is the provider's (module doc).
    assert_eq!(mode(&fx, m), 1);
    assert_eq!(fx.sim.hooks().path_position(m), before);
}

// Covers: specs/monsters/ai.md §7.1
#[test]
fn a_mode_that_does_not_move_targets_the_unit_and_computes_nothing() {
    // Request byte 100: path type 0, no point; the request's unit is the
    // path target (`0x00648B90`), so the AI's path target reads it.
    let mut fx = fx();
    let m = monster(&mut fx, 26, 10);
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 28, 10);
    change(&mut fx, m, BLOCK, ModeTarget::Unit(p));
    let d = dynamic(&mut fx, m);
    assert_eq!((d.path_type, d.point_count), (0, 0));
    assert_eq!(d.target_unit.map(|t| t.unit), Some(p));
    assert_eq!(d.repath_budget, 20);
    let target = fx.sim.with(&mut fx.game, |_, v| AiUnits::path_target(v, m));
    assert_eq!(target, Some(p));
    fx.assert_clean();
}

// Covers: specs/monsters/ai.md §7.1; specs/sim/pathing.md §3 r5
#[test]
fn a_failed_type_13_compute_retries_with_type_15() {
    // A target in no room: the type-13 compute gives 0 points (§3 step
    // 5) and the set-up retries with type 15, which fails the same way
    // (M08: an open target keeps 13, test above).
    let mut fx = fx();
    let m = monster(&mut fx, 26, 10);
    change(&mut fx, m, WALK, ModeTarget::Point(26, 60));
    let d = dynamic(&mut fx, m);
    assert_eq!((d.path_type, d.point_count), (path_types::WALL_FOLLOW, 0));
    fx.assert_clean();
}

// Covers: specs/monsters/ai.md §2.2 r2
#[test]
fn path_blocked_reads_path_flag_0x800() {
    let mut fx = fx();
    let m = monster(&mut fx, 26, 10);
    let blocked = |fx: &mut Fx| {
        fx.sim
            .with(&mut fx.game, |_, v| AiModes::path_blocked(v, m))
    };
    assert!(!blocked(&mut fx));
    fx.sim
        .hooks()
        .paths
        .as_mut()
        .unwrap()
        .dynamic_mut(m)
        .unwrap()
        .flags |= 0x800;
    assert!(blocked(&mut fx));
}

// Covers: specs/monsters/ai.md §1.4
#[test]
fn an_attack_end_requests_neutral() {
    // Event 1 of A1 is the mode end `0x005A8030`: not an inline-think
    // mode, so a mode change to neutral (which schedules the think at
    // f + aidel, `units.md` §4.6).
    let mut fx = fx();
    let m = monster(&mut fx, 26, 10);
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 28, 10);
    change(&mut fx, m, ATTACK1, ModeTarget::Unit(p));
    // The attack start body is not this module's (it leaves the mode):
    // the mode field is set here.
    fx.sim.sys.units.get_mut(m).unwrap().mode = 4;
    fx.sim.with(&mut fx.game, |g, v| {
        let mut sim = crate::units::hooks::Sim {
            game: g,
            units: &mut *v.units,
            stats: &mut *v.stats,
            data: v.data,
        };
        crate::units::modes::monster_event(&mut sim, &mut *v.h, m, true).unwrap();
    });
    assert_eq!(mode(&fx, m), 1);
    let f = fx.game.frame;
    assert!(fx.timers(m).contains(&(event::AI_THINK, f + 15)));
    fx.assert_clean();
}

/// Installs the AI of class 0 (Idle) on `m` (`ai.md` §3.3).
fn install_ai(fx: &mut Fx, m: UnitId) {
    fx.sim
        .ai(&mut fx.game, |g, cx| {
            cx.store.entry(m).control = Some(crate::monsters::ai::AiControl::default());
            crate::monsters::ai::install(g, cx, m, 0);
        })
        .unwrap();
}

// Covers: specs/monsters/ai.md §1.4, §7.2; specs/sim/pathing.md §9.5 r3
#[test]
fn an_ai_walk_to_a_unit_moves_then_rethinks_at_the_path_end() {
    // The AI's walk to a unit (`0x005DEC80`) through the real tactic,
    // mode set, every-tick step and mode end: the monster walks toward
    // the player and, the frame its path ends, the inline think runs
    // (Idle: neutral and the next think 200 frames later).
    let mut fx = fx();
    let m = monster(&mut fx, 20, 30);
    install_ai(&mut fx, m);
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 30, 30);
    let ok = fx
        .sim
        .ai(&mut fx.game, |g, cx| {
            crate::monsters::ai::walk_to(g, cx, m, Some(p), 0)
        })
        .unwrap();
    assert!(ok);
    assert_eq!(mode(&fx, m), 2);
    let t = ticks(&mut fx, m, 2, 60);
    // Seven sub-tiles at 0x6000 a tick: the 19th tick lands on (27, 30)
    // and stops; the think runs in that tick (f + 200, Idle).
    assert_eq!(t.len(), 19);
    for w in t.windows(2).take(17) {
        assert_eq!(w[1].0 - w[0].0, 0x6000);
    }
    assert_eq!(t[18], (0x1B8000, 0x1E8000, 1));
    assert_eq!(fx.sim.hooks().path_position(m), (27, 30));
    let f = fx.game.frame;
    assert!(fx.timers(m).contains(&(event::AI_THINK, f + 200)));
    fx.assert_clean();
}

// Covers: specs/missiles/bodies.md §19 l4 r1, §19 l4 r2, §19 l4 r3; specs/sim/pathing.md §11.2 r1, §11.2 r3, §11.2 r4
#[test]
fn zigzag_init_rebuilds_a_charged_bolt_path_on_the_provider() {
    // The `zigzag` init callback (`missiles.md` §R2.3 step 21) on the
    // wired seams: seed := init_low(target x), type 10, step counts :=
    // min(total 50, 77), rebuild: n = 50 >> 1 = 25 draws, 26 points from
    // the start cell, each two sub-tiles in one of 8 directions.
    let fire_zig = |tx: i32| {
        let mut fx = fx();
        let a = fx.a;
        let owner = fx.spawn(UnitType::Monster, 0, a, 41, 30);
        let p = MissileParams {
            owner: Some(owner),
            origin: Some(owner),
            class: 0,
            flags: param_flags::TARGET_ABSOLUTE,
            target_x: tx,
            target_y: 30,
            init: Some((crate::missiles::bodies_ext::ZIGZAG_CALLBACK, 0)),
            ..MissileParams::default()
        };
        let m = fx
            .sim
            .missiles(&mut fx.game, |g, cx| create_missile(g, cx, &p))
            .unwrap()
            .expect("created");
        fx.assert_clean();
        let d = dynamic(&mut fx, m);
        let seed = fx.sim.sys.units.get(m).unwrap().seed;
        (d, seed)
    };
    let (d, seed) = fire_zig(79);
    assert_eq!(d.path_type, 10);
    assert_eq!((d.dist_budget, d.max_distance), (50, 50));
    assert_eq!(d.point_count, 26);
    assert_eq!((d.point(0).x, d.point(0).y), (41, 30));
    for k in 1..26 {
        let (p, q) = (d.point(k - 1), d.point(k));
        let (dx, dy) = (q.x - p.x, q.y - p.y);
        assert!(
            matches!(dx, -2 | 0 | 2) && matches!(dy, -2 | 0 | 2) && (dx, dy) != (0, 0),
            "point {k}: ({dx}, {dy})"
        );
    }
    let mut want = crate::rng::Seed::init_low(79);
    for _ in 0..25 {
        want.step();
    }
    assert_eq!(seed, want);
    // M08: another target x re-seeds differently.
    let (d2, _) = fire_zig(78);
    assert_ne!(
        (0..26).map(|k| d.point(k)).collect::<Vec<_>>(),
        (0..26).map(|k| d2.point(k)).collect::<Vec<_>>()
    );
}

// Covers: specs/sim/path-placement.md §6 r4; specs/missiles/bodies-2.md §46 r2
#[test]
fn missile_body_teleport_moves_the_path_and_clears_the_points() {
    let mut fx = fx();
    let (a, b) = (fx.a, fx.b);
    let owner = fx.spawn(UnitType::Monster, 0, a, 41, 30);
    let m = fire(&mut fx, owner, 79, 30);
    assert_eq!(dynamic(&mut fx, m).point_count, 1);
    fx.sim.with(&mut fx.game, |g, v| {
        crate::missiles::MissileBodies::path_teleport(v, g, m, Some(b), 50, 32)
    });
    let d = dynamic(&mut fx, m);
    assert_eq!((d.x(), d.y(), d.room, d.point_count), (50, 32, Some(b), 0));
    let target = fx.sim.with(&mut fx.game, |_, v| {
        crate::missiles::MissileBodies::path_target_point(v, m)
    });
    assert_eq!(target, (79, 30));
    fx.assert_clean();
}

// Covers: specs/monsters/ai.md §7.5
#[test]
fn every_request_but_get_hit_refills_the_budget_and_retargets() {
    // §7.5 r1: GH (mode 3) changes neither the target nor the budget;
    // r2–r3: any other mode, moving or not, sets the target and 20.
    let mut fx = fx();
    let m = monster(&mut fx, 26, 10);
    let a = fx.a;
    let p = fx.spawn(UnitType::Player, 0, a, 28, 10);
    change(&mut fx, m, BLOCK, ModeTarget::Unit(p));
    let drain = |fx: &mut Fx| {
        let d = fx
            .sim
            .hooks()
            .paths
            .as_mut()
            .unwrap()
            .dynamic_mut(m)
            .unwrap();
        d.repath_budget = 7;
    };
    drain(&mut fx);
    change(&mut fx, m, 3, ModeTarget::Point(40, 40));
    let d = dynamic(&mut fx, m);
    assert_eq!(d.repath_budget, 7);
    assert_eq!(d.target_unit.map(|t| t.unit), Some(p));
    // Neutral (not moving): the point target and the budget again.
    change(&mut fx, m, 1, ModeTarget::Point(27, 11));
    let d = dynamic(&mut fx, m);
    assert_eq!(d.repath_budget, 20);
    assert_eq!((d.target_x, d.target_y, d.target_unit), (27, 11, None));
}

// Covers: specs/sim/pathing.md §13.1 r1
#[test]
fn step_counts_use_the_low_byte_capped_at_77() {
    use crate::wiring::path::missiles::step_count_byte;
    // §13.1 rule 1: −1 → 255 → 77, −256 → 0, −200 → 56; 300 → 44 (low
    // byte); 77 and 50 as given.
    for (n, b) in [
        (-1, 77),
        (-256, 0),
        (-200, 56),
        (300, 44),
        (77, 77),
        (50, 50),
        (78, 77),
    ] {
        assert_eq!(step_count_byte(n), b, "n {n}");
    }
    let mut fx = fx();
    let a = fx.a;
    let owner = fx.spawn(UnitType::Monster, 0, a, 41, 30);
    let m = fire(&mut fx, owner, 79, 30);
    fx.sim.with(&mut fx.game, |_, v| {
        crate::missiles::MissileBodies::set_path_distance(v, m, -200)
    });
    let d = dynamic(&mut fx, m);
    assert_eq!((d.dist_budget, d.max_distance), (56, 56));
}

// Covers: specs/sim/pathing.md §13.2 r1, §13.2 r2, §13.2 r3
#[test]
fn target_position_clears_a_stale_target_and_reads_the_point() {
    use crate::path::TargetUnit;
    let mut fx = fx();
    let a = fx.a;
    let owner = fx.spawn(UnitType::Monster, 0, a, 41, 30);
    let p = fx.spawn(UnitType::Player, 0, a, 50, 33);
    let m = fire(&mut fx, owner, 79, 30);
    let guid = fx.game.lists.unit(p).unwrap().guid;
    let set_target = |fx: &mut Fx, g: u32| {
        fx.sim
            .hooks()
            .paths
            .as_mut()
            .unwrap()
            .dynamic_mut(m)
            .unwrap()
            .target_unit = Some(TargetUnit {
            unit: p,
            ty: UnitType::Player,
            guid: g,
        });
    };
    let pos = |fx: &mut Fx| {
        fx.sim.with(&mut fx.game, |g, v| {
            crate::missiles::MissileBodies::target_position(v, g, m)
        })
    };
    // A live target: its position, the target kept.
    set_target(&mut fx, guid);
    assert_eq!(pos(&mut fx), Some((50, 33)));
    assert!(dynamic(&mut fx, m).target_unit.is_some());
    // A stale one (its GUID no longer resolves to it): cleared, and the
    // stored point (79, 30) is read instead.
    set_target(&mut fx, guid.wrapping_add(1000));
    assert_eq!(pos(&mut fx), Some((79, 30)));
    assert!(dynamic(&mut fx, m).target_unit.is_none());
    // A point with x = 0: result 0.
    fx.sim
        .hooks()
        .paths
        .as_mut()
        .unwrap()
        .dynamic_mut(m)
        .unwrap()
        .target_x = 0;
    assert_eq!(pos(&mut fx), None);
}
