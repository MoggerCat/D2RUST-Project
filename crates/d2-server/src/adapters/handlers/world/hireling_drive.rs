// Spec: specs/world/hirelings-ai.md; specs/monsters/ai-bodies-6.md §7 (Hireable think); specs/world/hirelings.md §6 r1
//! d2rs-own, unverified: the preview's stand-in for the Hireable AI think
//! (`0x005E52D0`). The real think needs the AI target providers
//! (`Pending::good_target_search` and the owner link), which the live
//! host does not have yet (`docs/handoff/stitch-combat.md` §1 row 12), so
//! each frame this driver gives every living hireling one decision:
//!
//! 1. a hostile monster within [`SIGHT`] of the hireling: walk to it, and
//!    when within [`MELEE`] attack it (mode 4) and take its life by the
//!    hireling's mean damage every [`SWING`] frames;
//! 2. else farther than [`LEASH`] from the owner: walk to the owner
//!    (run when farther than [`RUN_FROM`]);
//! 3. else idle.
//!
//! Everything is integer and drawn from no RNG (determinism). PROVISIONAL:
//! REC-100 (docs/HANDOFF.md §7): no experience share, no drops, no
//! get-hit, no skill pick (`hireling.txt` chances), no player-ward AI.

use std::collections::BTreeSet;

use d2_sim::game::Game;
use d2_sim::monsters::ai::{mode, AiModes, AiUnits, ModeTarget};
use d2_sim::stats::stat;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::combat::HIRELING_CLASSES;
use d2_sim::wiring::action::{Pending, View};

use super::wired::{TradeRest, WiredWorld};
use super::ActionEvents;

/// Squared sub-tile distances (path positions).
const SIGHT: i64 = 20 * 20;
const MELEE: i64 = 3 * 3;
const LEASH: i64 = 6 * 6;
const RUN_FROM: i64 = 14 * 14;
/// Stats 21 / 22 (`itemstatcost`): minimum / maximum damage.
const MINDAMAGE: u16 = 21;
const MAXDAMAGE: u16 = 22;
/// Frames between swings, and between walk re-issues.
const SWING: i32 = 20;
const REPATH: i32 = 10;

fn d2(a: (i32, i32), b: (i32, i32)) -> i64 {
    let (dx, dy) = (i64::from(a.0 - b.0), i64::from(a.1 - b.1));
    dx * dx + dy * dy
}

impl<R: TradeRest, S> WiredWorld<R, S> {
    /// One frame of the hirelings' stand-in think (module docs). Without
    /// hireling tables there is no hireling.
    pub fn drive_hirelings<D: ActionEvents>(&mut self, game: &mut Game, events: &mut D) {
        events.action().with(game, |g, v| v.pet_sweep(g));
        let mut mercs: Vec<(UnitId, u32)> = if self.state.hireling_tables.is_none() {
            Vec::new()
        } else {
            self.state
                .hirelings
                .lists
                .iter()
                .flat_map(|(&p, l)| l.nodes.iter().filter(|n| !n.dead).map(move |n| (p, n.guid)))
                .collect::<Vec<_>>()
                .into_iter()
                .filter_map(|(p, guid)| {
                    game.lists
                        .find_unit(UnitType::Monster, guid)
                        .map(|m| (m, p.0))
                })
                .collect()
        };
        // The summoned pets (`ActionHooks::pet_lists`, `q-summons`) follow
        // and fight by the same think.
        let pets: Vec<(UnitId, u32)> = events.action().with(game, |g, v| {
            v.h.pet_lists
                .iter()
                .flat_map(|(&p, l)| {
                    l.entries
                        .iter()
                        .flat_map(move |e| e.nodes.iter().map(move |n| (p, n.guid)))
                })
                .filter_map(|(p, guid)| {
                    g.lists
                        .find_unit(UnitType::Monster, guid as u32)
                        .map(|m| (m, p.0))
                })
                .collect()
        });
        // The laid traps do not follow: their own think shoots.
        let traps: BTreeSet<UnitId> = events
            .action()
            .with(game, |_, v| v.h.sentries.keys().copied().collect());
        mercs.extend(pets.into_iter().filter(|(m, _)| !traps.contains(m)));
        let frame = game.frame;
        self.drive_sentries(game, events, &mercs);
        if mercs.is_empty() {
            return;
        }
        let friends: BTreeSet<UnitId> = mercs.iter().map(|&(m, _)| m).collect();
        events.action().with(game, |g, v| {
            for (merc, owner) in mercs {
                think(v, g, merc, UnitId(owner), frame, &friends);
            }
        });
    }
}

/// One frame of the follower think (module docs) for `merc` of `owner`;
/// `friends` are never targets (the hirelings and pets of the game).
pub(super) fn think<X: Pending>(
    v: &mut View<'_, X>,
    g: &mut Game,
    merc: UnitId,
    owner: UnitId,
    frame: i32,
    friends: &BTreeSet<UnitId>,
) {
    if AiUnits::is_dead(v, merc) || AiUnits::is_dead(v, owner) {
        return;
    }
    let me = AiUnits::position(v, merc);
    let boss = AiUnits::position(v, owner);
    let slot = frame.wrapping_add(merc.0 as i32);
    let target = nearest_hostile(v, g, merc, friends, SIGHT);
    if let Some((dist, t)) = target {
        let tp = AiUnits::position(v, t);
        if dist <= MELEE {
            let cur = AiUnits::anim_mode(v, merc);
            if cur != mode::ATTACK1 {
                AiModes::change_mode(v, g, merc, mode::ATTACK1, ModeTarget::Unit(t));
            }
            if slot.rem_euclid(SWING) == 0 {
                let lo = v.stats.unit_total(merc, MINDAMAGE, 0);
                let hi = v.stats.unit_total(merc, MAXDAMAGE, 0);
                let dmg = ((lo + hi) / 2).max(1) << 8;
                if AiUnits::life_percent(v, t) > 0 {
                    AiUnits::add_life(v, t, -dmg);
                }
                if v.stats.unit_total(t, stat::HITPOINTS, 0) <= 0 {
                    AiModes::change_mode(v, g, t, mode::DEATH, ModeTarget::Unit(merc));
                }
            }
        } else if slot.rem_euclid(REPATH) == 0 {
            AiModes::change_mode(v, g, merc, mode::WALK, ModeTarget::Point(tp.0, tp.1));
        }
    } else if d2(me, boss) > LEASH {
        if slot.rem_euclid(REPATH) == 0 {
            let m = if d2(me, boss) > RUN_FROM {
                mode::RUN
            } else {
                mode::WALK
            };
            AiModes::change_mode(v, g, merc, m, ModeTarget::Point(boss.0, boss.1));
        }
    } else if matches!(
        AiUnits::anim_mode(v, merc),
        mode::WALK | mode::RUN | mode::ATTACK1
    ) && slot.rem_euclid(REPATH) == 0
    {
        AiModes::change_mode(v, g, merc, mode::NEUTRAL, ModeTarget::Point(me.0, me.1));
    }
}

/// The nearest living hostile monster within squared distance `sight` of
/// `from` (never a friend, a hireling, a seller, or in a town room), as
/// (squared distance, unit). Shared by the followers' think and the
/// traps' ([`super::sentry_drive`]).
pub(super) fn nearest_hostile<X: Pending>(
    v: &mut View<'_, X>,
    g: &Game,
    from: UnitId,
    friends: &BTreeSet<UnitId>,
    sight: i64,
) -> Option<(i64, UnitId)> {
    let me = AiUnits::position(v, from);
    g.lists
        .units_of_type(UnitType::Monster)
        .into_iter()
        .filter(|&m| m != from && !AiUnits::is_dead(v, m))
        .filter(|&m| {
            let class = AiUnits::class(v, m);
            !friends.contains(&m)
                && !HIRELING_CLASSES.iter().any(|&c| c as i32 == class)
                && !d2_sim::world::npc::SELLERS
                    .iter()
                    .any(|&c| i32::from(c) == class)
                && g.lists
                    .unit(m)
                    .and_then(|e| e.room())
                    .is_some_and(|r| !v.h.drlg.in_town(g, r))
        })
        .map(|m| (d2(me, AiUnits::position(v, m)), m))
        .filter(|&(d, _)| d <= sight)
        .min()
}
