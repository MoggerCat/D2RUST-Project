// Spec: specs/monsters/ai-bodies-6.md §14 (AssassinSentry); specs/skills/bodies.md §8.3
//! d2rs-own, unverified: the preview's think for the laid traps (the
//! Assassin's sentries). The real think (`0x005EA3D0`) needs the AI
//! owner link and the unit's skill entries, which the live host does not
//! give summoned units yet (`docs/handoff/q-summons.md` "Left" 1), so
//! each frame this driver gives every trap of
//! [`d2_sim::wiring::action::ActionHooks::sentries`] the steps of §14:
//!
//! 1. the owner gone, dead or in a town room: the trap dies (mode 0);
//! 2. no shots left: the trap dies (the think after the last shot);
//! 3. a hostile monster within [`RANGE`] and the shot gap spent: one
//!    shot is taken (shots − 1) and the trap uses its skill on it.
//!
//! The skill is the class's monstats `Skill1` (else the laying skill) at
//! the laying skill's level, in the class's `Sk1mode` (else attack 1).
//! No RNG draw (the original's `roll(100) ≥ aip1` is always false for the
//! lightning sentry's `aip1` 100). PROVISIONAL: REC-241 (docs/HANDOFF.md
//! §7).

use std::collections::BTreeSet;

use d2_sim::game::Game;
use d2_sim::monsters::ai::{mode, AiModes, AiUnits, ModeTarget};
use d2_sim::units::UnitId;
use d2_sim::wiring::action::{Pending, View};

use super::hireling_drive::nearest_hostile;
use super::wired::{TradeRest, WiredWorld};
use super::ActionEvents;

/// Squared sub-tile range of a shot (`aip4` 25 of the lightning sentry).
const RANGE: i64 = 25 * 25;
/// Frames between two shots (`aip3` 15: the idle after a missed look).
const GAP: i32 = 15;

impl<R: TradeRest, S> WiredWorld<R, S> {
    /// One frame of the traps' think (module docs); `allies` are the
    /// followers, never targets.
    pub(super) fn drive_sentries<D: ActionEvents>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        allies: &[(UnitId, u32)],
    ) {
        events.action().with(game, |g, v| {
            if v.h.sentries.is_empty() {
                return;
            }
            let mut friends: BTreeSet<UnitId> = allies.iter().map(|&(m, _)| m).collect();
            friends.extend(v.h.sentries.keys().copied());
            let traps: Vec<UnitId> = v.h.sentries.keys().copied().collect();
            for t in traps {
                think(v, g, t, &friends);
            }
        });
    }
}

fn die<X: Pending>(v: &mut View<'_, X>, g: &mut Game, trap: UnitId) {
    AiModes::change_mode(v, g, trap, mode::DEATH, ModeTarget::Point(0, 0));
    v.h.sentries.remove(&trap);
}

fn think<X: Pending>(v: &mut View<'_, X>, g: &mut Game, trap: UnitId, friends: &BTreeSet<UnitId>) {
    let Some(s) = v.h.sentries.get(&trap).copied() else {
        return;
    };
    if AiUnits::is_dead(v, trap) {
        v.h.sentries.remove(&trap);
        return;
    }
    // 1. The owner must stand outside the town.
    let owner_ok = g.lists.unit(s.owner).is_some()
        && !AiUnits::is_dead(v, s.owner)
        && g.lists
            .unit(s.owner)
            .and_then(|e| e.room())
            .is_some_and(|r| !v.h.drlg.in_town(g, r));
    // 2. No shots left.
    if !owner_ok || s.shots <= 0 {
        die(v, g, trap);
        return;
    }
    if g.frame < s.next {
        return;
    }
    // 3. A hostile in range.
    let Some((_, target)) = nearest_hostile(v, g, trap, friends, RANGE) else {
        return;
    };
    let class = usize::try_from(AiUnits::class(v, trap)).unwrap_or(usize::MAX);
    let skill1 =
        v.h.tables
            .combat
            .monstats
            .get(class)
            .map_or(0xFFFF, |m| m.skill1);
    let skill = if skill1 == 0xFFFF || skill1 == 0 {
        s.skill
    } else {
        i32::from(skill1)
    };
    let m =
        v.h.tables
            .skill_modes
            .get(class)
            .map_or(0, |modes| modes[0]);
    let m = if (1..16).contains(&m) {
        m
    } else {
        mode::ATTACK1
    };
    if let Some(e) = v.h.sentries.get_mut(&trap) {
        e.shots -= 1;
        e.next = g.frame + GAP;
    }
    AiModes::set_current_skill(v, trap, skill);
    AiModes::set_skill_flag(v, trap);
    AiModes::set_path_steps(v, trap, 1);
    AiModes::change_mode(v, g, trap, m, ModeTarget::Unit(target));
}
