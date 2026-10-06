// Spec: specs/skills/use.md §5.2, §7; specs/sim/stat-lists.md §10.2, §10.3; specs/sim/units.md §5, §6.1
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

use crate::skills::use_::{
    active_state_event, attack_frame_event, item_aura_event, periodic_event,
};
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
