// Spec: specs/skills/use.md §7; specs/sim/stat-lists.md §10.2, §10.3; specs/sim/tick.md §5.5, §5.6
//! The skill timer events through the combined dispatcher: a timer of
//! type 5, 8 or 9 on the real queue runs the unit dispatch
//! (`stat-lists.md` §10.2, §10.3), whose hooks hand it to the skill use
//! pipeline ([`crate::wiring::interaction::skill_events::route`]); the
//! pipeline's do core calls the skill's server-do function (the fake
//! logs it) and the periodic form reschedules itself on the same queue.

use std::sync::Arc;

use crate::skills::fake::skill_rec;
use crate::skills::{SkillEntry, SkillTables};
use crate::stats::stat as sst;
use crate::stats::StatData;
use crate::tick::events::event;
use crate::units::{UnitId, UnitType};

use super::skill_use::{skills, Fx};

/// The aura skill: `aura`, `srvdofunc` 65 (the aura do), no missile,
/// no mana, no formulas.
const AURA: i32 = 1;
/// Its aura state.
const AURA_STATE: u16 = 40;
/// `srvactivefunc` of the aura state: hurricane (`use.md` §7).
const HURRICANE: u16 = 145;

fn aura_skills() -> SkillTables {
    let mut t = skills();
    let mut a = skill_rec();
    a.aura = true;
    a.srvdofunc = 65;
    a.srvmissile = 0xFFFF;
    a.aurastate = AURA_STATE;
    a.perdelay = 0xFFFF_FFFF;
    a.delay = 0xFFFF_FFFF;
    t.skills.push(a);
    t
}

/// The interaction stat data with the aura skill's `aurastate` and the
/// state's `srvactivefunc`.
fn aura_stats() -> Arc<StatData> {
    let mut d = (*super::stat_data()).clone();
    d.aurastate[AURA as usize] = AURA_STATE;
    d.states.set_srvactivefunc(u32::from(AURA_STATE), HURRICANE);
    Arc::new(d)
}

fn fx() -> (Fx, UnitId) {
    let mut fx = Fx::with(aura_stats(), aura_skills());
    let p = fx.spawn(UnitType::Player, 10, 10);
    (fx, p)
}

fn entry(skill: i32) -> SkillEntry {
    SkillEntry {
        skill,
        base: 1,
        owner_guid: -1,
        ..SkillEntry::default()
    }
}

/// The unit's pending timers of `kind`: (expire, arg1, arg2).
fn timers(fx: &Fx, u: UnitId, kind: u8) -> Vec<(i32, u32, u32)> {
    let t = &fx.game.timers;
    let mut v: Vec<_> = t
        .unit_timers(u)
        .into_iter()
        .filter_map(|id| {
            let (k, a1, a2) = t.event(id)?;
            (k == kind).then_some((t.expire(id)?, a1, a2))
        })
        .collect();
    v.sort();
    v
}

fn srvdo_count(fx: &Fx, f: u16) -> usize {
    let want = format!("srvdo {f}");
    fx.sim
        .sys
        .hooks
        .x
        .log
        .iter()
        .filter(|l| **l == want)
        .count()
}

// Covers: specs/skills/use.md §7
#[test]
fn periodic_aura_event_runs_the_do_core_and_reschedules_on_the_queue() {
    let (mut fx, p) = fx();
    fx.sim.sys.hooks.x.right.insert(p, entry(AURA));
    fx.sim.sys.hooks.x.skills.insert(p, vec![entry(AURA)]);
    // The aura form (−1, 0), due at frame 1.
    fx.game
        .schedule_event(p, u32::from(event::PERIODIC_SKILLS), 1, None, u32::MAX, 0)
        .unwrap();
    fx.frame();
    // The do core ran the aura's do function once and rescheduled the
    // aura form at `period` (perdelay ≤ 5 → 5: frames ≡ 1 mod 5).
    assert_eq!(srvdo_count(&fx, 65), 1);
    assert_eq!(timers(&fx, p, event::PERIODIC_SKILLS), [(6, u32::MAX, 0)]);
    for _ in 1..11 {
        fx.frame();
    }
    // Frames 6 and 11: two more runs, the next one at 16.
    assert_eq!(srvdo_count(&fx, 65), 3);
    assert_eq!(timers(&fx, p, event::PERIODIC_SKILLS), [(16, u32::MAX, 0)]);
    fx.assert_clean();
}

// Covers: specs/skills/use.md §7
#[test]
fn periodic_aura_event_without_an_aura_right_skill_is_not_rescheduled() {
    let (mut fx, p) = fx();
    // The right skill is the missile skill (no `aura`): the event is
    // deleted (it has run, nothing reschedules it).
    fx.sim.sys.hooks.x.right.insert(p, entry(0));
    fx.game
        .schedule_event(p, u32::from(event::PERIODIC_SKILLS), 1, None, u32::MAX, 0)
        .unwrap();
    fx.frame();
    assert_eq!(srvdo_count(&fx, 65), 0);
    assert!(timers(&fx, p, event::PERIODIC_SKILLS).is_empty());
    fx.assert_clean();
}

// Covers: specs/sim/stat-lists.md §10.3; specs/skills/use.md §7
#[test]
fn item_aura_event_reads_stat_151_and_runs_the_do_core() {
    let (mut fx, p) = fx();
    // Item aura of skill 1 at level 3 (stat 151, layer = skill).
    fx.sim.with(&mut fx.game, |_, v| {
        v.stats
            .unit_set(&mut *v.h, p, sst::ITEM_AURA, 3, AURA as u16);
    });
    fx.game
        .schedule_event(p, u32::from(event::PERIODIC_STATS), 1, None, 0, AURA as u32)
        .unwrap();
    fx.frame();
    assert_eq!(srvdo_count(&fx, 65), 1);
    // No item aura (stat 151 of the skill ≤ 0): the dispatch cancels the
    // unit's type-9 events and nothing reaches the skills.
    fx.sim.with(&mut fx.game, |_, v| {
        v.stats
            .unit_set(&mut *v.h, p, sst::ITEM_AURA, 0, AURA as u16);
    });
    fx.game
        .schedule_event(p, u32::from(event::PERIODIC_STATS), 2, None, 0, AURA as u32)
        .unwrap();
    fx.game
        .schedule_event(p, u32::from(event::PERIODIC_STATS), 9, None, 0, AURA as u32)
        .unwrap();
    fx.frame();
    assert_eq!(srvdo_count(&fx, 65), 1);
    assert!(timers(&fx, p, event::PERIODIC_STATS).is_empty());
    fx.assert_clean();
}

// Covers: specs/sim/stat-lists.md §10.2; specs/skills/use.md §7
#[test]
fn active_state_event_calls_the_aura_states_server_do_function() {
    let (mut fx, p) = fx();
    // a1 = the skill: its aura state's `srvactivefunc` (145) is the do
    // function the skill pipeline calls.
    fx.game
        .schedule_event(p, u32::from(event::ACTIVE_STATE), 1, None, AURA as u32, 4)
        .unwrap();
    // a1 = 0 is out of range (§10.2): nothing.
    fx.game
        .schedule_event(p, u32::from(event::ACTIVE_STATE), 1, None, 0, 4)
        .unwrap();
    fx.frame();
    assert_eq!(srvdo_count(&fx, HURRICANE), 1);
    assert_eq!(fx.sim.sys.hooks.x.log.len(), 1);
    fx.assert_clean();
}
