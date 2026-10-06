// Spec: specs/monsters/init.md §22; specs/sim/tick.md §5.5, §5.6; specs/sim/units.md §3.2
//! Monster timer event 7 through [`WorldSim`]'s dispatcher on a monster
//! created by monster init (population's preset spawn): the unit
//! dispatch runs the umod dispatcher in mode 2 on the real monster data
//! and the real timer queue; the frozen-monster drop still applies.
//! Unit removal through [`WorldSim::remove_unit`] frees the world state's
//! part of the unit.

use super::population::{isle, monsters};
use super::*;
use crate::monsters::init::Unhandled;
use crate::monsters::population::preset;
use crate::stats::states::state;
use crate::tick::events::event;

/// A preset monster created through monster init.
fn monster(fx: &mut Fx) -> UnitId {
    let (a, _) = isle(fx);
    fx.sim.create_regions();
    fx.sim
        .population(&mut fx.game, |cx| preset::place_presets(cx, a));
    monsters(fx)[0]
}

fn umod_timers(fx: &Fx, u: UnitId) -> Vec<i32> {
    let t = &fx.game.timers;
    let mut v: Vec<_> = t
        .unit_timers(u)
        .into_iter()
        .filter(|&id| t.event(id).is_some_and(|e| e.0 == event::MON_UMOD))
        .filter_map(|id| t.expire(id))
        .collect();
    v.sort();
    v
}

fn run_to(fx: &mut Fx, frame: i32) {
    while fx.game.frame < frame {
        fx.game.frame += 1;
        crate::tick::run_timer_events(&mut fx.game, &mut fx.sim);
    }
}

// Covers: specs/monsters/init.md §22
#[test]
fn event_7_runs_the_umod_dispatcher_on_the_real_monster_data() {
    let mut fx = Fx::new(isle_ds1s());
    let u = monster(&mut fx);
    let f0 = fx.game.frame;
    // Umod 41 (`always_run_ai`): its event-7 handler runs the AI tick
    // (a seam) and schedules event 7 again at frame + 75.
    fx.sim.world.monsters.get_mut(u).unwrap().umods[0] = 41;
    fx.game
        .schedule_event(u, u32::from(event::MON_UMOD), f0 + 1, None, 0, 0)
        .unwrap();
    run_to(&mut fx, f0 + 1);
    assert_eq!(umod_timers(&fx, u), [f0 + 76]);
    run_to(&mut fx, f0 + 76);
    assert_eq!(umod_timers(&fx, u), [f0 + 151]);
    assert!(fx.sim.world.monsters.unhandled.is_empty());
    fx.assert_clean();
}

// Covers: specs/monsters/init.md §22
#[test]
fn event_7_reaches_a_callback_without_a_body_as_unhandled() {
    let mut fx = Fx::new(isle_ds1s());
    let u = monster(&mut fx);
    let f0 = fx.game.frame;
    // Umod 21 (`killself`): its mode-2 callback `0x005A3AA0` has no body.
    fx.sim.world.monsters.get_mut(u).unwrap().umods[0] = 21;
    fx.game
        .schedule_event(u, u32::from(event::MON_UMOD), f0 + 1, None, 0, 0)
        .unwrap();
    run_to(&mut fx, f0 + 1);
    assert_eq!(
        fx.sim.world.monsters.unhandled,
        [Unhandled::Callback {
            addr: 0x005A_3AA0,
            unit: u,
            umod: 21,
            mode: 2,
        }]
    );
    fx.assert_clean();
}

// Covers: specs/sim/tick.md §5.6
#[test]
fn event_7_of_a_frozen_monster_is_dropped_by_the_dispatcher() {
    let mut fx = Fx::new(isle_ds1s());
    let u = monster(&mut fx);
    let f0 = fx.game.frame;
    fx.sim.world.monsters.get_mut(u).unwrap().umods[0] = 41;
    fx.sim.action.with(&mut fx.game, |_, v| {
        v.set_state(u, state::FREEZE as u16, true);
    });
    fx.game
        .schedule_event(u, u32::from(event::MON_UMOD), f0 + 1, None, 0, 0)
        .unwrap();
    run_to(&mut fx, f0 + 1);
    // Not run: nothing rescheduled it.
    assert!(umod_timers(&fx, u).is_empty());
    fx.assert_clean();
}

// Covers: specs/sim/units.md §3.2
#[test]
fn removal_frees_the_world_state_of_the_unit() {
    let mut fx = Fx::new(isle_ds1s());
    let u = monster(&mut fx);
    let m = UnitId(u.0 + 1000);
    fx.sim.world.minions.insert(u, vec![m]);
    fx.sim.world.owners.insert(u, m);
    fx.sim.world.superunique_tail.insert(u, (0, false));
    assert!(fx.sim.world.monsters.get(u).is_some());
    assert!(fx.sim.action.hooks().ai_store().get(u).is_some());
    fx.sim.remove_unit(&mut fx.game, u);
    fx.assert_clean();
    assert!(fx.game.lists.unit(u).is_none());
    assert!(fx.sim.world.monsters.get(u).is_none());
    assert!(!fx.sim.world.minions.contains_key(&u));
    assert!(!fx.sim.world.owners.contains_key(&u));
    assert!(!fx.sim.world.superunique_tail.contains_key(&u));
    // The action state left too (AI control).
    assert!(fx.sim.action.hooks().ai_store().get(u).is_none());
}
