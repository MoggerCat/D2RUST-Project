// Spec: specs/world/quests.md §4.3 (dispatch), §4.4 (kill parse), §4.5 (level change), §4.6 (add link)
//! The quest events of the play host (task `q-a1-tower`): monster init's
//! chain links and monster deaths, which the action wiring's seams queue
//! ([`Pending::take_quest_events`]), and the players' level changes,
//! which are read off the quest world once per tick. They run on the
//! quest control after the tick (`after_tick`), as `0x005436B0`,
//! `0x00543A30` and `0x00543B90` do.
//!
//! PROVISIONAL (REC-129): the original calls these from inside monster
//! init, the kill and the warp; here they run once per tick after the
//! tick's steps, in the order links, level changes, kills. `// d2rs-own,
//! unverified`.

use std::collections::BTreeMap;

use d2_sim::game::Game;
use d2_sim::units::UnitId;
use d2_sim::wiring::action::{Pending, QuestEvent};
use d2_sim::world::quests::QuestWorld;

use super::{quest_call, ActionEvents, TradeRest, WiredWorld};

/// The level each player was last seen in.
#[derive(Debug, Default)]
pub struct QuestLevels(BTreeMap<UnitId, u32>);

impl<R: TradeRest, S> WiredWorld<R, S> {
    /// Runs the queued quest events and the level changes since the last
    /// tick on the quest control.
    pub(super) fn run_quest_events<D: ActionEvents>(&mut self, game: &mut Game, events: &mut D) {
        let queued = events.action().sys.hooks.x.take_quest_events();
        let frame = game.frame;
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
                    if let QuestEvent::Kill { victim, killer } = *e {
                        q.monster_killed(w, victim, killer);
                    }
                }
                // Tick step 8 `0x00543E10`: the quest updater (timers such
                // as A1Q2's 15, `quests-act1.md` §10.5 r5) runs on every
                // 20th frame (`quests.md` §5; REC-134).
                if frame % 20 == 0 {
                    q.update(w);
                }
            });
            seen
        });
        self.quest_levels.0 = seen;
    }
}
