// Spec: specs/skills/use.md §5.2, §7; specs/sim/stat-lists.md §10.2, §10.3; specs/sim/units.md §4.6 r7, r10, r13, §5, §6.1
//! The skill timer events of the unit dispatch on the skill use
//! pipeline: event 5 (active state), 8 (periodic skills and auras) and 9
//! (item auras) reach [`crate::skills::use_`] through [`UseView`].
//!
//! The unit dispatch (`units.md` §5, `stat-lists.md` §10.2, §10.3) does
//! the checks it owns (skill id range, `srvactivefunc` < 191, item-aura
//! level from stat 151, the type-9 cancel) and calls the action hooks,
//! which hand the event to [`Pending::skill_event`]. A seam value that
//! also implements [`UseRest`] routes it here:
//!
//! ```text
//! fn skill_event(h: &mut ActionHooks<Self>, sim: &mut Sim<'_>, ev: SkillEvent) {
//!     d2_sim::wiring::interaction::skill_events::route(h, sim, ev)
//! }
//! ```
//!
//! Player event 0 in an attack, cast or skill mode (the action frame
//! `0x00580460`, `units.md` §4.5) reaches
//! [`crate::skills::use_::attack_frame_event`] the same way, through
//! [`Pending::action_frame`] and [`action_frame`].
//!
//! Event 14 (callback `0x00554570`) is not routed: `use.md` §6 states it
//! is never scheduled in 1.14d and its body is not specified.

use crate::skills::levels::skill_level;
use crate::skills::use_::{
    active_state_event, attack_frame_event, do_skill, item_aura_event, periodic_event, start,
    UseWorld, FLAG_MISSILE_FIRED, SKILL_ARRIVED, SKILL_MOVING,
};
use crate::skills::SkillUnits;
use crate::units::hooks::Sim;
use crate::units::UnitId;
use crate::wiring::action::combat::CombatView;
use crate::wiring::action::{ActionHooks, Pending, SkillEvent, View};

use super::{UseRest, UseView};

/// Runs one skill timer event on the skill use pipeline.
pub fn route<X: Pending + UseRest>(h: &mut ActionHooks<X>, sim: &mut Sim<'_>, ev: SkillEvent) {
    let t = h.tables.clone();
    let mut w = UseView {
        cv: CombatView {
            game: sim.game,
            v: View::of(sim.units, sim.stats, sim.data, h),
        },
    };
    match ev {
        // `stat-lists.md` §10.2: f(game, unit, skill, a2).
        // TODO(stat-lists.md §10.2): a2 is read as the do function's
        // level argument (not stated).
        SkillEvent::ActiveState {
            unit,
            f,
            skill,
            arg2,
        } => {
            active_state_event(&mut w, unit, f, skill as i32, arg2 as i32);
        }
        // `use.md` §7: arg −1 (the aura form) or a skill id; the level of
        // the second form comes from stat 351, not from a2.
        SkillEvent::Periodic { unit, arg1, .. } => {
            periodic_event(&mut w, &t.skills, unit, arg1 as i32);
        }
        // `stat-lists.md` §10.3: `0x0056F7F0`(game, unit, skill, l, 1, 1, 0).
        //
        // TODO(stat-lists.md §10.3): the second call `0x0056CE70`(game,
        // unit, a1, skill, l, 0) is not specified (no body in `use.md`);
        // it is not run, so nothing reschedules the type-9 event.
        SkillEvent::ItemAura {
            unit, skill, level, ..
        } => {
            item_aura_event(&mut w, &t.skills, unit, skill as i32, level);
        }
    }
}

/// The player action frame `0x00580460` (`use.md` §5.2) on the skill use
/// pipeline: the used skill's do function on an action event (a1 1 or
/// 2); returns 1, or 2 when the unit died.
pub fn action_frame<X: Pending + UseRest>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    unit: UnitId,
    a1: u32,
    a2: u32,
) -> u32 {
    let t = h.tables.clone();
    let mut w = UseView {
        cv: CombatView {
            game: sim.game,
            v: View::of(sim.units, sim.stats, sim.data, h),
        },
    };
    attack_frame_event(&mut w, &t.skills, unit, a1 as i32, a2 as i32) as u32
}

/// The skill start `0x0056FAF0` (`use.md` §5.3) of a monster's attack /
/// skill and sequence starts (`units.md` §4.6 rules 7, 10) on the skill
/// use pipeline; its result.
pub fn monster_skill_start<X: Pending + UseRest>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    unit: UnitId,
) -> i32 {
    let t = h.tables.clone();
    let mut w = UseView {
        cv: CombatView {
            game: sim.game,
            v: View::of(sim.units, sim.stats, sim.data, h),
        },
    };
    start(&mut w, &t.skills, unit)
}

/// The skill part of the monster sequence event 0 `0x005A8670`
/// (`units.md` §4.6 rule 13) on the skill use pipeline: E := the used
/// skill, f := its E flags (`0x006446A0`; 0 without a used skill), "do
/// left" := 1. f bit 0 (a moving skill): the target check and step
/// (`0x00553490`, `0x00554CA0`; the path provider's step when it is on);
/// result 2 → E flags := f | 2 (`0x00644660`), the do `0x0056FC50`, do
/// left := 0. Then by the frame code (unit byte +0x4E): 4 → the do; else
/// do left, unit flag 0x40 clear and code 1 or 2 → the do. The animation
/// refresh that follows is the caller's.
pub fn monster_sequence_frame<X: Pending + UseRest>(
    h: &mut ActionHooks<X>,
    sim: &mut Sim<'_>,
    unit: UnitId,
) {
    let t = h.tables.clone();
    let paths = h.paths.is_some();
    let mut w = UseView {
        cv: CombatView {
            game: sim.game,
            v: View::of(sim.units, sim.stats, sim.data, h),
        },
    };
    let do_it = |w: &mut UseView<'_, X>| {
        if let Some(e) = w.used_skill(unit) {
            let l = skill_level(w, &t.skills, Some(unit), Some(&e), true);
            do_skill(w, &t.skills, unit, e.skill, l);
        }
    };
    let f = if w.used_skill(unit).is_some() {
        w.used_skill_flags(unit)
    } else {
        0
    };
    let mut do_left = true;
    if f & SKILL_MOVING != 0 {
        let stopped = if paths {
            let game = &mut *w.cv.game;
            crate::wiring::path::PathCtx::of(&mut w.cv.v, game).step(unit)
                == Some(crate::path::walk::Step::Stopped)
        } else {
            w.step_path(unit) == 2
        };
        if stopped {
            w.set_used_skill_flags(unit, f | SKILL_ARRIVED);
            do_it(&mut w);
            do_left = false;
        }
    }
    let (code, flags) =
        w.cv.v
            .units
            .get(unit)
            .map_or((0, 0), |r| (r.anim.action_frame, r.flags));
    if code == 4 || (do_left && flags & FLAG_MISSILE_FIRED == 0 && matches!(code, 1 | 2)) {
        do_it(&mut w);
    }
}
