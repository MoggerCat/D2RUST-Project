// Spec: specs/world/npc.md §2 rule 3 (approach, talk on arrival); specs/sim/pathing.md §1.2
//! The NPC approach of the wired host (C→S 0x13 at distance 7–8): the
//! run request to the NPC (`0x00580A70`, no skill, mode 3), then the
//! queued interaction, which runs the 0x13 handling again when the
//! player's run stops ("talk on arrival", `npc.md` §2 rule 3.4). The
//! client sends no second 0x13.
//!
//! d2rs-own, unverified: the arrival is read from the player's mode
//! leaving walk / run / town walk at the host's after-tick pass, not from
//! the step result of `0x00580C20`; the queue is dropped by the player's
//! next walk request (`clear_queued_action`, `pathing.md` §1.2 step 4).

use d2_sim::game::Game;
use d2_sim::units::UnitId;
use d2_sim::world::npc::NpcWorld;

use super::wired::{TradeRest, WiredWorld};
use super::{ActionEvents, NpcRun, WorldHost};

/// Unit modes of a moving player (walk, run, town walk).
const MOVING: [u8; 3] = [2, 3, 6];

impl<R: TradeRest, S> WiredWorld<R, S> {
    /// Drops the queued interaction of `player` (a new walk request).
    pub(super) fn drop_queued(&mut self, player: UnitId) {
        self.state.queued.retain(|q| q.0 != player);
    }

    /// The queued interactions whose run has ended run the 0x13 handling
    /// again; then the approach requests of the last calls start their
    /// run and queue the interaction.
    pub(super) fn approaches<D: ActionEvents>(&mut self, game: &mut Game, events: &mut D)
    where
        Self: WorldHost<D>,
    {
        let waiting = std::mem::take(&mut self.state.queued);
        for (player, guid) in waiting {
            let moving = self.desk(game, events, |desk, _, _| {
                MOVING.contains(&NpcWorld::mode(desk, player))
            });
            if moving {
                self.state.queued.push((player, guid));
                continue;
            }
            let mut msg = vec![0x13, 1, 0, 0, 0];
            msg.extend_from_slice(&guid.to_le_bytes());
            let call = NpcRun {
                player,
                msg: &msg,
            };
            WorldHost::<D>::npc(self, game, events, call);
            // The arrival does not queue another approach.
            self.state.approaches.clear();
        }
        let asked = std::mem::take(&mut self.state.approaches);
        for (player, npc) in asked {
            let guid = self.desk(game, events, |desk, _, _| NpcWorld::guid(desk, npc));
            let started = events.action().with(game, |g, v| {
                v.h.paths.is_some()
                    && d2_sim::wiring::path::PathCtx::of(v, g)
                        .approach_unit(player, npc)
                        .is_some()
            });
            if started {
                self.drop_queued(player);
                self.state.queued.push((player, guid));
            }
        }
    }
}
