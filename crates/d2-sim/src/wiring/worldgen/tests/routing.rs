// Spec: specs/monsters/init.md §5, §22; specs/sim/units.md §3.1, §3.2, §4.6; specs/combat/damage.md §5.2 step 9; specs/missiles/missiles.md rule 28; specs/monsters/ai.md §2.4
//! The action hooks' monster routes on the wired sim ([`WorldSim`] with
//! its world state lent to the action hooks): monster init on an
//! allocation made by action code, the world state's part of a removal
//! made by action code, the umod dispatcher from the monster mode change
//! (modes 0, 1), combat (mode 3) and missile creation (mode 5), and the
//! monster-data queries. Each test runs the same call once more on the
//! action systems alone (no world lent): the route's effect disappears,
//! so each check can fail (M08).

use super::tests::{isle_ds1s, Fx, ISLE};
use crate::combat::CombatWorld;
use crate::drlg::TileRect;
use crate::missiles::MissileHooks;
use crate::monsters::ai::AiUnits;
use crate::monsters::init::{type_flag, Unhandled};
use crate::monsters::population::{Alloc, MonsterInit};
use crate::rng::Seed;
use crate::stats::stat;
use crate::units::lifecycle::AllocRequest;
use crate::units::{RoomId, UnitId, UnitType};

use super::super::action::monsters::umod_mode;

/// [`ISLE`] generated, its room at (8000, 8000) streamed, the regions
/// created; the active room.
fn room(fx: &mut Fx) -> RoomId {
    let (_, rooms) = fx.generate(ISLE).unwrap();
    let r = *rooms
        .iter()
        .find(|&&r| fx.drlg().room(r).rect == TileRect::new(8000, 8000, 8, 8))
        .unwrap();
    let active = fx.stream(&[r]).unwrap()[0];
    fx.sim.create_regions();
    active
}

/// A monster of class 0 in mode 1 (neutral) in `room`.
fn request(room: RoomId) -> AllocRequest {
    AllocRequest {
        ty: UnitType::Monster,
        class: 0,
        room: Some(room),
        add: true,
        fixed_guid: None,
        mode: 1,
        allied: false,
    }
}

/// A monster allocated by action code ([`crate::wiring::action::View::allocate`])
/// with the world state lent.
fn lent_monster(fx: &mut Fx, room: RoomId) -> UnitId {
    let req = request(room);
    fx.sim
        .with(&mut fx.game, |g, v| v.allocate(g, &req, 40010, 40010))
        .expect("monster")
}

/// The umod callbacks recorded without a body since `from`.
fn unhandled(fx: &Fx, from: usize) -> Vec<(u32, UnitId, u8, u8)> {
    fx.sim.world.monsters.unhandled[from..]
        .iter()
        .map(|u| match *u {
            Unhandled::Callback {
                addr,
                unit,
                umod,
                mode,
            } => (addr, unit, umod, mode),
        })
        .collect()
}

// Covers: specs/monsters/init.md §5 r3, §5 r6
#[test]
fn action_allocation_runs_the_monster_type_init() {
    let mut fx = Fx::new(isle_ds1s());
    let a = room(&mut fx);
    let u = lent_monster(&mut fx, a);
    fx.assert_clean();
    // `init.md` §5 through the allocator's per-kind init: monster data
    // with the class (step 3) and the room's level id (step 6), the AI
    // control and its first setup (steps 2, 5), unit flags 0x0A (step 1).
    let md = fx.sim.world.monsters.get(u).expect("monster data");
    assert_eq!((md.class, md.level_id), (0, ISLE as i32));
    let hooks = &mut fx.sim.action.sys.hooks;
    assert_ne!(hooks.ai_store().control(u).expect("AI control").function, 0);
    let rec = fx.sim.action.sys.units.get(u).unwrap();
    assert_eq!(rec.flags & 0x0A, 0x0A);
    // The same allocation without the world lent: no type init.
    let req = request(a);
    let bare = fx
        .sim
        .action
        .with(&mut fx.game, |g, v| v.allocate(g, &req, 40012, 40012))
        .expect("monster");
    assert!(fx.sim.world.monsters.get(bare).is_none());
    assert!(fx.sim.action.sys.hooks.ai_store().control(bare).is_none());
    let rec = fx.sim.action.sys.units.get(bare).unwrap();
    assert_eq!(rec.flags & 0x0A, 0);
}

#[test]
fn population_allocation_keeps_one_type_init() {
    // Population holds the world state itself ([`WorldSim::host`]): the
    // allocator's hook finds no world and population's own type init runs
    // once. Two games from the same state, one monster each, population's
    // creation in one and action code's in the other: the same unit seed
    // and stats after creation (one type init each; a second one would
    // draw again on the unit seed).
    let mut fx = Fx::new(isle_ds1s());
    let a = room(&mut fx);
    let pop = fx
        .sim
        .host(&mut fx.game, |h| {
            let alloc = Alloc {
                class: 0,
                room: a,
                x: 40010,
                y: 40010,
                mode: 1,
                guid: None,
            };
            let mut st = std::mem::take(&mut h.w.pop);
            let u = h.allocate_monster(alloc, &mut st);
            h.w.pop = st;
            u
        })
        .expect("monster");
    fx.assert_clean();
    let mut other = Fx::new(isle_ds1s());
    let b = room(&mut other);
    assert_eq!(a, b);
    let act = lent_monster(&mut other, b);
    other.assert_clean();
    let unit = |fx: &Fx, u| {
        let r = fx.sim.action.sys.units.get(u).unwrap();
        let life = fx.sim.action.sys.stats.unit_base(u, stat::HITPOINTS, 0);
        (r.init_seed, r.seed, life)
    };
    assert_eq!(unit(&fx, pop), unit(&other, act));
    let (init_seed, seed, _) = unit(&other, act);
    assert_ne!(seed, Seed::init_low(init_seed));
}

#[test]
fn action_removal_frees_the_world_state() {
    let mut fx = Fx::new(isle_ds1s());
    let a = room(&mut fx);
    let u = lent_monster(&mut fx, a);
    let m = lent_monster(&mut fx, a);
    fx.sim.world.minions.insert(u, vec![m]);
    fx.sim.world.owners.insert(m, u);
    // Removal by action code (`View::remove`, as a missile's
    // collide-kill) with the world lent.
    fx.sim.with(&mut fx.game, |g, v| v.remove(g, u));
    assert!(fx.sim.world.monsters.get(u).is_none());
    assert!(!fx.sim.world.minions.contains_key(&u));
    assert!(fx.sim.world.monsters.get(m).is_some());
    // The same removal without the world lent leaves stale state.
    fx.sim.action.with(&mut fx.game, |g, v| v.remove(g, m));
    assert!(fx.sim.world.monsters.get(m).is_some());
    assert!(fx.sim.world.owners.contains_key(&m));
    fx.assert_clean();
}

#[test]
fn monster_mode_change_runs_umod_modes_0_then_1() {
    let mut fx = Fx::new(isle_ds1s());
    let a = room(&mut fx);
    let u = lent_monster(&mut fx, a);
    // Umod 14 (spcdamage) has a mode-0 callback, umod 15 (partydead) a
    // mode-1 callback; neither body is specified (`init.md` OQ8), so
    // the dispatcher records them.
    fx.sim.world.monsters.get_mut(u).unwrap().umods[..2].copy_from_slice(&[14, 15]);
    let n = fx.sim.world.monsters.unhandled.len();
    let ok = fx
        .sim
        .with(&mut fx.game, |g, v| v.monster_set_mode(g, u, 1));
    assert!(ok);
    fx.assert_clean();
    assert_eq!(
        unhandled(&fx, n),
        [
            (0x005A_3B50, u, 14, umod_mode::MODE_CHANGE),
            (0x005A_2D10, u, 15, umod_mode::MODE_SET),
        ]
    );
    // Without the world lent the mode still changes; no callback runs.
    let n = fx.sim.world.monsters.unhandled.len();
    let ok = fx
        .sim
        .action
        .with(&mut fx.game, |g, v| v.monster_set_mode(g, u, 1));
    assert!(ok);
    assert_eq!(unhandled(&fx, n), []);
}

#[test]
fn monster_hit_hook_runs_umod_mode_3() {
    let mut fx = Fx::new(isle_ds1s());
    let a = room(&mut fx);
    let u = lent_monster(&mut fx, a);
    // Umod 7 (curse): mode-3 callback `0x005A2530` (body unread).
    fx.sim.world.monsters.get_mut(u).unwrap().umods[0] = 7;
    let n = fx.sim.world.monsters.unhandled.len();
    let game = &mut fx.game;
    fx.sim
        .lend(|s| s.combat(game, |cv, _| cv.monster_hit_hook(u)));
    assert_eq!(unhandled(&fx, n), [(0x005A_2530, u, 7, umod_mode::HIT)]);
    let n = fx.sim.world.monsters.unhandled.len();
    fx.sim
        .action
        .combat(&mut fx.game, |cv, _| cv.monster_hit_hook(u));
    assert_eq!(unhandled(&fx, n), []);
    fx.assert_clean();
}

#[test]
fn missile_hook_runs_umod_mode_5_on_the_missile() {
    let mut fx = Fx::new(isle_ds1s());
    let a = room(&mut fx);
    let owner = lent_monster(&mut fx, a);
    // The missile's id (the dispatcher hands it to the callbacks in mode
    // 5); a second unit stands in for it.
    let missile = lent_monster(&mut fx, a);
    // Umod 29 (multishot): mode-5 callback `0x005A3610` (body unread).
    fx.sim.world.monsters.get_mut(owner).unwrap().umods[0] = 29;
    let n = fx.sim.world.monsters.unhandled.len();
    fx.sim
        .with(&mut fx.game, |g, v| v.unique_mod_missile(g, owner, missile));
    assert_eq!(
        unhandled(&fx, n),
        [(0x005A_3610, missile, 29, umod_mode::MISSILE)]
    );
    let n = fx.sim.world.monsters.unhandled.len();
    fx.sim
        .action
        .with(&mut fx.game, |g, v| v.unique_mod_missile(g, owner, missile));
    assert_eq!(unhandled(&fx, n), []);
    fx.assert_clean();
}

#[test]
fn monster_data_queries_read_the_lent_world() {
    let mut fx = Fx::new(isle_ds1s());
    let a = room(&mut fx);
    let u = lent_monster(&mut fx, a);
    fx.sim.world.monsters.get_mut(u).unwrap().type_flags |= type_flag::UNIQUE;
    fx.sim
        .with(&mut fx.game, |_, v| v.set_base(u, stat::LEVEL, 7));
    let game = &mut fx.game;
    let lent = fx.sim.lend(|s| {
        let flags = s.combat(game, |cv, _| {
            (
                cv.monster_flag(u, 8),
                cv.monster_flag(u, 4),
                cv.monster_flag(u, 0x0A),
            )
        });
        let ai = s.with(game, |_, v| {
            (v.is_unique(u), v.is_champion(u), v.monster_level(u))
        });
        (flags, ai)
    });
    assert_eq!(lent, ((true, false, true), (true, false, 7)));
    // Without the world: the pending defaults (false, level 0).
    let game = &mut fx.game;
    let flags = fx.sim.action.combat(game, |cv, _| cv.monster_flag(u, 8));
    let ai = fx
        .sim
        .action
        .with(game, |_, v| (v.is_unique(u), v.monster_level(u)));
    assert_eq!((flags, ai), (false, (false, 0)));
    fx.assert_clean();
}
