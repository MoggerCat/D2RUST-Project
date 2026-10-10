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

/// The log lines of `m` starting with `p`.
fn logged(fx: &Fx, p: &str, m: UnitId) -> Vec<String> {
    let p = format!("{p} {}", m.0);
    fx.sim
        .sys
        .hooks
        .x
        .log
        .iter()
        .filter(|l| l.starts_with(&p))
        .cloned()
        .collect()
}

/// Type-0 events of `m` with frame codes `codes`, one per frame from 1.
fn frame_events(fx: &mut Fx, m: UnitId, codes: &[u32]) {
    {
        let r = fx.sim.sys.units.get_mut(m).unwrap();
        // The frame advance `0x00623E00` reads the code from the
        // animation record: one frame per event, the codes at frames 1…
        let mut events = [0u8; crate::units::record::ANIM_EVENTS];
        for (i, &c) in codes.iter().enumerate() {
            events[1 + i] = c as u8;
        }
        r.anim.record = Some(crate::units::record::AnimRecord {
            frames: 1 << 8,
            byte_0f: 0,
            events,
        });
        r.anim.speed = 256;
        r.anim.frame = 0;
        r.anim.frame_count = 1 << 16;
    }
    for (i, &c) in codes.iter().enumerate() {
        fx.game
            .schedule_event(m, u32::from(event::MODE_CHANGE), 1 + i as i32, None, c, 0)
            .unwrap();
    }
    for _ in codes {
        fx.frame();
    }
}

// Covers: specs/skills/use.md §5.2
#[test]
fn attack_event0_without_a_used_skill_strikes_once_per_frame_code_event() {
    // `use.md` §5.2 "Monsters" `0x005A7670`: no used skill and a mode that
    // does not move (A2: class 0's monstats2 mv bits are A1 only) → the
    // strike (mode missile, else melee on the path target) on every event
    // 0, whatever its frame code. +0x4E is the frame advance's.
    let mut fx = Fx::new();
    let m = monster(&mut fx);
    fx.sim.sys.units.get_mut(m).unwrap().mode = u32::from(mode::ATTACK2);
    frame_events(&mut fx, m, &[1, 2, 4]);
    let want = vec![format!("attack strike {} false", m.0); 3];
    assert_eq!(logged(&fx, "attack strike", m), want);
    assert!(logged(&fx, "attack skill", m).is_empty());
    assert!(logged(&fx, "sequence frame", m).is_empty());
    // A non-moving mode strikes with no frame advance: +0x4E is untouched.
    assert_eq!(fx.sim.sys.units.get(m).unwrap().anim.action_frame, 0);
}

// Covers: specs/skills/use.md §5.2
#[test]
fn attack_event0_of_a_moving_mode_strikes_only_at_its_trigger_frame() {
    // A1 moves for class 0 (mv bit 4): step, refresh, then the strike
    // (moving flag set) only when trigger(U) holds, i.e. +0x4E = 1.
    let mut fx = Fx::new();
    let m = monster(&mut fx);
    {
        let r = fx.sim.sys.units.get_mut(m).unwrap();
        r.mode = u32::from(mode::ATTACK1);
        r.anim.frame_count = 1 << 16;
    }
    let x0 = fx.sim.sys.hooks.x.position(m).0;
    frame_events(&mut fx, m, &[2, 1, 3]);
    assert_eq!(
        logged(&fx, "attack strike", m),
        [format!("attack strike {} true", m.0)]
    );
    // One path step per event (the fake step moves one subtile).
    assert_eq!(fx.sim.sys.hooks.x.position(m).0, x0 + 3);
}

// Covers: specs/skills/use.md §5.2
#[test]
fn attack_event0_with_a_used_skill_runs_its_branch_on_every_event() {
    // With a used skill the do runs on each event 0 regardless of +0x4E
    // (no frame-code test), and the no-skill strike never runs.
    let mut fx = Fx::new();
    let m = monster(&mut fx);
    fx.sim.sys.units.get_mut(m).unwrap().mode = u32::from(mode::ATTACK1);
    fx.sim.sys.hooks.x.used.insert(
        m,
        crate::skills::SkillEntry {
            skill: 0,
            base: 1,
            owner_guid: -1,
            ..crate::skills::SkillEntry::default()
        },
    );
    frame_events(&mut fx, m, &[3, 0, 1]);
    assert_eq!(logged(&fx, "attack skill", m).len(), 3);
    assert!(logged(&fx, "attack strike", m).is_empty());
}

/// A monster world holding only monster data (the AI-state tests).
struct DataWorld(std::collections::BTreeMap<UnitId, crate::monsters::init::MonsterData>);

impl<X> crate::wiring::action::monsters::MonsterWorld<X> for DataWorld {
    fn type_init(
        &mut self,
        _: &mut crate::units::hooks::Sim<'_>,
        _: &mut ActionHooks<X>,
        _: UnitId,
    ) {
    }
    fn umods(
        &mut self,
        _: &mut crate::units::hooks::Sim<'_>,
        _: &mut ActionHooks<X>,
        _: UnitId,
        _: Option<UnitId>,
        _: u8,
    ) {
    }
    fn assign_umod(
        &mut self,
        _: &mut crate::units::hooks::Sim<'_>,
        _: &mut ActionHooks<X>,
        _: UnitId,
        _: u8,
    ) {
    }
    fn forget(&mut self, _: UnitId) {}
    fn monster(&self, unit: UnitId) -> Option<&crate::monsters::init::MonsterData> {
        self.0.get(&unit)
    }
    fn monster_mut(&mut self, unit: UnitId) -> Option<&mut crate::monsters::init::MonsterData> {
        self.0.get_mut(&unit)
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

fn ai_state_of(fx: &mut Fx, m: UnitId) -> u32 {
    fx.sim.hooks().monster_data(m).unwrap().ai_state
}

fn leave(fx: &mut Fx, m: UnitId, from: u8, state: u32) {
    fx.sim.hooks().set_monster_ai_state(m, state);
    fx.sim.sys.units.get_mut(m).unwrap().mode = u32::from(from);
    fx.game.frame += 1;
    let ok = fx.sim.with(&mut fx.game, |g, v| {
        v.change_mode(g, m, mode::NEUTRAL, ModeTarget::Unit(m))
    });
    assert!(ok);
}

#[test]
fn ai_state_is_stored_and_follows_the_mode_set() {
    // `ai.md` §3 "AI state": the monster data's `dwAiState` (0 at
    // creation); `0x005734C0` stores a value (a soft hit: 19) and the
    // monster mode set `0x005A7C20` applies `0x005A68E0` to the mode it
    // leaves: not for mode 1; state ≥ 16 → state − 16; 13 leaving mode 3
    // stays; else the state becomes that mode.
    let mut fx = Fx::new();
    let m = monster(&mut fx);
    let world = DataWorld([(m, Default::default())].into_iter().collect());
    fx.sim.sys.hooks.monster_world = Some(Box::new(world));
    assert_eq!(ai_state_of(&mut fx, m), 0);
    fx.sim.hooks().set_monster_ai_state(m, 19);
    assert_eq!(ai_state_of(&mut fx, m), 19);
    // 19 leaving A2 (mode 5) → 3.
    leave(&mut fx, m, mode::ATTACK2, 19);
    assert_eq!(ai_state_of(&mut fx, m), 3);
    // 0 leaving A2 → 5.
    leave(&mut fx, m, mode::ATTACK2, 0);
    assert_eq!(ai_state_of(&mut fx, m), 5);
    // Leaving mode 1: unchanged.
    leave(&mut fx, m, mode::NEUTRAL, 19);
    assert_eq!(ai_state_of(&mut fx, m), 19);
    // 13 leaving mode 3 stays 13; leaving mode 4 it becomes 4.
    leave(&mut fx, m, mode::GETHIT, 13);
    assert_eq!(ai_state_of(&mut fx, m), 13);
    leave(&mut fx, m, mode::ATTACK1, 13);
    assert_eq!(ai_state_of(&mut fx, m), 4);
    fx.assert_clean();
}

/// `0x005B1990` (`ai.md` §5.2 target-node table): a good NPC's
/// registration (NpcBarb, slot 8) puts it at the head of list 8 and sets
/// unit +0xD0; the main search's lists then hold it after the host's own
/// nodes, newest first, and a removed unit leaves its list.
// Covers: specs/monsters/ai.md §5.2 r5
#[test]
fn registered_good_npcs_join_target_list_8_newest_first() {
    use crate::monsters::ai::{AiSummons, AiTargets};
    let mut fx = Fx::new();
    let a = fx.spawn(UnitType::Monster, 0, fx.a, 10, 10);
    let b = fx.spawn(UnitType::Monster, 0, fx.a, 12, 10);
    let lists = |fx: &mut Fx| {
        let s = &mut fx.sim.sys;
        let v = View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks);
        v.target_nodes(&fx.game)
    };
    for u in [a, b] {
        let s = &mut fx.sim.sys;
        let mut v = View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks);
        assert_eq!(v.target_slot(u), 11);
        v.register_target_node(&mut fx.game, u, 8);
        assert_eq!(v.target_slot(u), 8);
    }
    assert_eq!(lists(&mut fx)[8], vec![b, a]);
    assert!(lists(&mut fx)[9].is_empty());
    fx.game.remove_unit(b).unwrap();
    assert_eq!(lists(&mut fx)[8], vec![a]);
}

#[test]
fn natural_skill_level_is_read_by_the_ai() {
    // `init.md` §6 step 14 gives the monster its entry; the AI's
    // `0x006442A0` read is that base level, 1-fallback is the body's.
    let mut fx = Fx::new();
    let m = monster(&mut fx);
    fx.sim.with(&mut fx.game, |_, v| {
        assert_eq!(
            crate::monsters::ai::AiActs::skill_level(&*v, m, 352, false),
            None
        );
        v.h.natural_skills.entry(m).or_default().insert(352, 4);
        assert_eq!(
            crate::monsters::ai::AiActs::skill_level(&*v, m, 352, false),
            Some(4)
        );
        assert_eq!(
            crate::monsters::ai::AiActs::skill_level(&*v, m, 300, false),
            None
        );
        // A summon's entry (`monster_skills`) wins.
        v.h.monster_skills.entry(m).or_default().insert(352, 9);
        assert_eq!(
            crate::monsters::ai::AiActs::skill_level(&*v, m, 352, false),
            Some(9)
        );
    });
    fx.assert_clean();
}

#[test]
fn used_skill_takes_the_monster_entry_level() {
    // A monster without a skill list casts with its init entry's base
    // level (doomknight2 DoomKnightMissile at `Sk1lvl` 3: the missile's
    // level in `milestone-hellforge`), a summon's entry winning; a skill
    // with no entry keeps the seam's answer.
    let mut fx = Fx::new();
    let m = monster(&mut fx);
    let entry = |skill| crate::skills::SkillEntry {
        skill,
        base: 1,
        owner_guid: -1,
        ..Default::default()
    };
    fx.sim.with(&mut fx.game, |_, v| {
        v.h.x.used.insert(m, entry(335));
        assert_eq!(v.h.used_skill_of(m).map(|e| e.base), Some(1));
        v.h.natural_skills.entry(m).or_default().insert(335, 3);
        assert_eq!(v.h.used_skill_of(m).map(|e| e.base), Some(3));
        v.h.monster_skills.entry(m).or_default().insert(335, 7);
        assert_eq!(v.h.used_skill_of(m).map(|e| e.base), Some(7));
        v.h.x.used.insert(m, entry(300));
        assert_eq!(v.h.used_skill_of(m).map(|e| e.base), Some(1));
    });
    fx.assert_clean();
}

/// `0x0063E9F0` / `0x0063E940` / `0x0063E990` / `0x0063EDC0` read the
/// monstats flags of the monster's class (`ai.md` §2.4 step 1): a boss
/// such as Griswold gets the boss-sound idle, which defers its first
/// think by 20 frames (rc-drop-content, `items-drops-nor-11`). A unit
/// without monster data keeps the `Pending` answer (false here).
// Covers: specs/monsters/ai.md §2.4
#[test]
fn monstats_flags_answer_boss_demon_undead_prime_evil() {
    let mut fx = Fx::new();
    let m = fx.spawn(UnitType::Monster, 0, fx.a, 10, 10);
    let flags = |fx: &mut Fx| {
        let h = fx.sim.hooks();
        (
            h.is_boss(m),
            h.is_demon(m),
            h.is_undead(m),
            h.is_prime_evil(m),
        )
    };
    let data = crate::monsters::init::MonsterData::default();
    fx.sim.sys.hooks.monster_world = Some(Box::new(super::sound::DataOnly(
        [(m, data)].into_iter().collect(),
    )));
    assert_eq!(flags(&mut fx), (false, false, false, false));
    let mut tables = (*fx.sim.sys.hooks.tables).clone();
    let row = &mut tables.combat;
    row.monstats[0].boss = true;
    row.monstats[0].hundead = true;
    fx.sim.sys.hooks.tables = std::sync::Arc::new(tables);
    assert_eq!(flags(&mut fx), (true, false, true, false));
    let mut tables = (*fx.sim.sys.hooks.tables).clone();
    let row = &mut tables.combat;
    row.monstats[0].demon = true;
    row.monstats[0].primeevil = true;
    fx.sim.sys.hooks.tables = std::sync::Arc::new(tables);
    assert_eq!(flags(&mut fx), (true, true, true, true));
}
