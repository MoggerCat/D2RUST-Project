// Spec: specs/world/quests.md §4.3 (dispatch), §4.4 (kill parse), §4.5 (level change), §4.6 (add link)
//! The quest events of the play host (tasks `q-a1-tower`, `q-a2-quests`): monster init's
//! chain links and monster deaths, which the action wiring's seams queue
//! ([`Pending::take_quest_events`]), and the players' level changes,
//! which are read off the quest world once per tick. They run on the
//! quest control after the tick (`after_tick`), as `0x005436B0`,
//! `0x00543A30` and `0x00543B90` do.
//!
//! PROVISIONAL (REC-129; Act II hooks REC-141): the original calls these from inside monster
//! init, the kill and the warp; here they run once per tick after the
//! tick's steps, in the order links, level changes, kills. `// d2rs-own,
//! unverified`.

use std::collections::BTreeMap;

use d2_sim::game::Game;
use d2_sim::units::UnitId;
use d2_sim::wiring::action::{Pending, QuestEvent};
use d2_sim::world::quests::{act2, QuestWorld};

use super::{quest_call, ActionEvents, TradeRest, WiredWorld};

/// The level each player was last seen in.
#[derive(Debug, Default)]
pub struct QuestLevels(BTreeMap<UnitId, u32>);

impl<R: TradeRest, S> WiredWorld<R, S> {
    /// Runs the queued quest events and the level changes since the last
    /// tick on the quest control.
    pub(super) fn run_quest_events<D: ActionEvents>(&mut self, game: &mut Game, events: &mut D) {
        let mut queued = events.action().sys.hooks.x.take_quest_events();
        // The cube's `hst ` hook (`0x0059E5C0`), recorded by its pending.
        if let Some(cube) = self.cube.as_mut() {
            for (player, code) in cube.pending.take_quest_items() {
                if code == *b"hst " {
                    queued.push(QuestEvent::StaffAssembled { player });
                }
            }
        }
        let mut seen = std::mem::take(&mut self.quest_levels.0);
        seen = self.desk(game, events, |desk, ctl, inv| {
            let ((), _) = quest_call(desk, ctl, inv, |q, w| {
                for e in &queued {
                    if let QuestEvent::Link { unit, chain } = *e {
                        q.add_link(w, unit, chain, None);
                    }
                }
                let mut now = BTreeMap::new();
                for p in w.players() {
                    let Some(level) = w.unit_level(p) else {
                        continue;
                    };
                    if let Some(&old) = seen.get(&p) {
                        if old != level {
                            q.changed_level(w, p, old, level);
                        }
                    }
                    now.insert(p, level);
                }
                seen = now;
                for e in &queued {
                    match *e {
                        QuestEvent::RadamentActivated { unit } => act2::q1::radament_ai(q, w, unit),
                        QuestEvent::SummonerActivated => act2::q5::summoner_seen(q, w),
                        QuestEvent::StaffAssembled { player } => {
                            act2::q2::staff_assembled(q, w, player)
                        }
                        _ => {}
                    }
                }
                for e in &queued {
                    if let QuestEvent::Kill { victim, killer } = *e {
                        q.monster_killed(w, victim, killer);
                    }
                }
            });
            seen
        });
        self.quest_levels.0 = seen;
    }
}
