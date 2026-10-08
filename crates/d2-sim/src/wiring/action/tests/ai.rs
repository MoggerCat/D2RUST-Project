// Spec: specs/monsters/ai.md §1.1, §2, §3.3; specs/monsters/ai-bodies.md §9 (Idle, GoodNpcRanged); specs/sim/tick.md §5.2 rule 4, §5.6; specs/sim/units.md §4.6
//! Monster AI ↔ units (modes, timer events): the think run by the unit
//! dispatch, the freeze drop, mode changes through the real monster mode
//! set, the state-54 rule before a think is scheduled.

use super::*;
use crate::monsters::ai::{install, mode, AiControl, AiModes, ModeTarget};
use crate::stats::states::state;
use crate::tick::events::event;

/// A monster of class 0 (AI 1, Idle) in room A with its AI installed.
fn monster(fx: &mut Fx) -> UnitId {
    let m = fx.spawn(UnitType::Monster, 0, fx.a, 10, 10);
    fx.sim
        .ai(&mut fx.game, |g, cx| {
            cx.store.entry(m).control = Some(AiControl::default());
            install(g, cx, m, 0);
        })
        .unwrap();
    m
}

fn thinks(fx: &Fx, m: UnitId) -> Vec<i32> {
    fx.timers(m)
        .into_iter()
        .filter(|t| t.0 == event::AI_THINK)
        .map(|t| t.1)
        .collect()
}

#[test]
fn think_runs_through_the_unit_dispatch_and_reschedules() {
    // `ai-bodies.md` §9 Idle: the next think 200 frames later (vector "idle AI
    // thinks every 200"), scheduled through the real timer queue.
    let mut fx = Fx::new();
    let m = monster(&mut fx);
    fx.game
        .schedule_event(m, u32::from(event::AI_THINK), 1, None, 0, 0)
        .unwrap();
    fx.frame();
    assert_eq!(thinks(&fx, m), [201]);
    // Nothing runs until then; at 201 it thinks again.
    for _ in 0..199 {
        fx.frame();
    }
    assert_eq!(thinks(&fx, m), [201]);
    fx.frame();
    assert_eq!(thinks(&fx, m), [401]);
    assert!(fx.sim.hooks().ai_store().unhandled.is_empty());
    fx.assert_clean();
}

#[test]
fn frozen_monster_drops_think_and_reset() {
    // `tick.md` §5.6: types 2 and 10 are dropped while the monster has
    // state 1 and is alive; a dropped think is not rescheduled
    // (`ai.md` §1.1).
    let mut fx = Fx::new();
    let m = monster(&mut fx);
    fx.sim.with(&mut fx.game, |_, v| {
        v.set_state(m, state::FREEZE as u16, true)
    });
    for ev in [event::AI_THINK, event::AI_RESET] {
        fx.game
            .schedule_event(m, u32::from(ev), 1, None, 0, 0)
            .unwrap();
    }
    fx.frame();
    assert!(thinks(&fx, m).is_empty());
    assert!(fx.timers(m).is_empty());
    // Unfrozen: the think runs.
    fx.sim.with(&mut fx.game, |_, v| {
        v.set_state(m, state::FREEZE as u16, false)
    });
    fx.game
        .schedule_event(m, u32::from(event::AI_THINK), 2, None, 0, 0)
        .unwrap();
    fx.frame();
    assert_eq!(thinks(&fx, m), [202]);
    fx.assert_clean();
}

#[test]
fn ai_mode_change_runs_the_monster_mode_set() {
    // `units.md` §4.6: neutral start `0x005A73E0` = mode 1 and a think at
    // f + aidel (15) unless one is pending later.
    let mut fx = Fx::new();
    let m = monster(&mut fx);
    fx.sim.sys.units.get_mut(m).unwrap().mode = u32::from(mode::WALK);
    fx.game.frame = 30;
    let ok = fx.sim.with(&mut fx.game, |g, v| {
        v.change_mode(g, m, mode::NEUTRAL, ModeTarget::Unit(m))
    });
    assert!(ok);
    assert_eq!(
        fx.sim.sys.units.get(m).unwrap().mode,
        u32::from(mode::NEUTRAL)
    );
    assert_eq!(thinks(&fx, m), [45]);
    // State 54: the mode set is a fatal assertion in 1.14d → refused.
    fx.sim.with(&mut fx.game, |_, v| {
        v.set_state(m, state::UNINTERRUPTABLE as u16, true)
    });
    let ok = fx.sim.with(&mut fx.game, |g, v| {
        v.change_mode(g, m, mode::WALK, ModeTarget::Point(12, 10))
    });
    assert!(!ok);
    assert_eq!(fx.sim.hooks().errors.len(), 1);
}

#[test]
fn ai_state_toggle_queues_the_unit_update() {
    // `ai-bodies.md` §9.26 / `stat-lists.md` §9.2: the AI's `0x00639DB0`
    // toggles the state and then always inserts the unit into its room's
    // update queue, also when the bit did not change; a state outside the
    // states table does nothing.
    use crate::monsters::ai::AiUnits;
    let mut fx = Fx::new();
    let m = monster(&mut fx);
    let _ = fx.game.lists.clear_update_queue(fx.a);
    fx.sim.with(&mut fx.game, |g, v| {
        AiUnits::set_state(v, g, m, u16::MAX, true)
    });
    assert!(fx.game.lists.update_queue(fx.a).is_empty());
    let freeze = state::FREEZE as u16;
    fx.sim.with(&mut fx.game, |g, v| {
        AiUnits::set_state(v, g, m, freeze, true)
    });
    assert!(fx.sim.sys.stats.has_state(m, state::FREEZE));
    assert_eq!(fx.game.lists.update_queue(fx.a), [m]);
    let _ = fx.game.lists.clear_update_queue(fx.a);
    // Unchanged bit: queued again.
    fx.sim.with(&mut fx.game, |g, v| {
        AiUnits::set_state(v, g, m, freeze, true)
    });
    assert_eq!(fx.game.lists.update_queue(fx.a), [m]);
    fx.assert_clean();
}

#[test]
fn think_scheduled_with_state_54_clears_it_first() {
    // `tick.md` §5.2 rule 4 / `ai.md` §1.1: `0x005544B0(unit, 0)` clears
    // state 54 and cancels the monster's type-2 events before the new
    // think is scheduled (here by the neutral start of `units.md` §4.6).
    let mut fx = Fx::new();
    let m = monster(&mut fx);
    fx.game
        .schedule_event(m, u32::from(event::AI_THINK), 5, None, 0, 0)
        .unwrap();
    fx.sim.with(&mut fx.game, |_, v| {
        v.set_state(m, state::UNINTERRUPTABLE as u16, true)
    });
    fx.game.frame = 10;
    let r = fx.sim.sys.with(&mut fx.game, |sim, hooks| {
        crate::units::modes::monster_neutral(sim, hooks, m)
    });
    assert_eq!(r, Ok(()));
    assert!(!fx.sim.sys.stats.has_state(m, state::UNINTERRUPTABLE));
    assert_eq!(thinks(&fx, m), [25]);
    fx.assert_clean();
}

// Covers: specs/monsters/ai-bodies.md §9.31 r3
#[test]
fn good_npc_ranged_takes_ai_turns() {
    // `ai-bodies.md` §9.31 on the wired host: a class-0 monster with AI 60
    // (GoodNpcRanged). No secondary target (the pending default), so each
    // think ends in step 3: `lo' % 100` < 20 → wander 5, else idle 10.
    let mut fx = Fx::new();
    std::sync::Arc::get_mut(&mut fx.sim.sys.hooks.tables)
        .expect("tables not shared yet")
        .combat
        .monstats[0]
        .ai = 60;
    let m = monster(&mut fx);
    assert_eq!(
        fx.sim.hooks().ai_store().control(m).map(|c| c.function),
        Some(0x005E_7AC0)
    );
    fx.seed(m, seed_giving(50));
    fx.game
        .schedule_event(m, u32::from(event::AI_THINK), 1, None, 0, 0)
        .unwrap();
    fx.frame();
    // 50 ≥ 20 → idle 10 (neutral already: no mode change).
    assert_eq!(thinks(&fx, m), [11]);
    assert_eq!(
        fx.sim.sys.units.get(m).unwrap().mode,
        u32::from(mode::NEUTRAL)
    );
    // Another idle-10 turn at 11.
    fx.seed(m, seed_giving(99));
    for _ in 0..10 {
        fx.frame();
    }
    assert_eq!(thinks(&fx, m), [21]);
    // The turn at 21 draws 5 < 20 → wander 5: exactly the wander draws
    // around its own position (`ai.md` §7.2), and a walk request the real
    // monster mode set runs. Without the path provider the monster has no
    // path, so its point count is 0: the walk start fails into the
    // neutral start (`units.md` §4.6 rule 5), which schedules the next
    // think at 21 + aidel (0 → 15).
    fx.seed(m, seed_giving(5));
    let pos = fx.sim.hooks().x.position(m);
    let mut want = seed_giving(5);
    want.step();
    crate::monsters::ai::wander_point(&mut want, pos, 5);
    for _ in 0..10 {
        fx.frame();
    }
    assert_eq!(fx.sim.sys.units.get(m).unwrap().seed, want);
    assert_eq!(thinks(&fx, m), [36]);
    assert_eq!(
        fx.sim.sys.units.get(m).unwrap().mode,
        u32::from(mode::NEUTRAL)
    );
    assert!(fx.sim.hooks().ai_store().unhandled.is_empty());
    fx.assert_clean();
}

// Covers: specs/skills/use.md §5.2
#[test]
fn attack_event0_runs_the_skill_frame_and_keeps_the_frame_code() {
    // PROVISIONAL (REC-111): the attack-family event 0 `0x005A7670` runs the
    // skill part of the sequence frame with unit +0x4E := the timer's code.
    let mut fx = Fx::new();
    let m = monster(&mut fx);
    fx.sim.sys.units.get_mut(m).unwrap().mode = u32::from(mode::ATTACK1);
    fx.game
        .schedule_event(m, u32::from(event::MODE_CHANGE), 1, None, 1, 0)
        .unwrap();
    fx.frame();
    assert!(
        fx.sim
            .hooks()
            .x
            .log
            .contains(&format!("sequence frame {}", m.0)),
        "{:?}",
        fx.sim.hooks().x.log
    );
    assert_eq!(fx.sim.sys.units.get(m).unwrap().anim.action_frame, 1);
}
