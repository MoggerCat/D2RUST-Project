// Spec: specs/world/objects.md §7.3 (rule 4: walk to the object); specs/world/npc.md §2 rule 3 (the queued interaction); specs/sim/pathing.md §1.2
//! The walk to an object of the wired host (C→S 0x13 type 2 out of
//! interact range or behind the line test): the run request to the
//! object (`0x00548A50`, player mode 3 toward (type 2, GUID)), then the
//! 0x13 object case again when the run ends, as the NPC approach
//! ([`super::npc_approach`]) and the item walk ([`super::item_approach`]):
//! the arrival is read from the player's mode leaving walk / run / town
//! walk at the end of tick step 4; a new walk request drops the queue
//! (`clear_queued_action`, `pathing.md` §1.2 step 4). The arrival's own
//! case does not walk again.

use d2_sim::game::Game;
use d2_sim::units::UnitId;
use d2_sim::wiring::action::ObjectCase;
use d2_sim::world::npc::NpcWorld;

use super::wired::{TradeRest, WiredWorld};
use super::{ActionEvents, WaypointOperate, WorldHost};

/// Unit modes of a moving player (walk, run, town walk).
const MOVING: [u8; 3] = [2, 3, 6];

impl<R: TradeRest, S> WiredWorld<R, S> {
    /// Starts the run of `player` to the object with `guid` and queues
    /// the 0x13 object case for its end.
    pub(super) fn start_object_walk<D: ActionEvents>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        player: UnitId,
        guid: u32,
    ) {
        let Some(object) = game.lists.find_unit(d2_sim::units::UnitType::Object, guid) else {
            return;
        };
        let started = events.action().with(game, |g, v| {
            v.h.paths.is_some()
                && d2_sim::wiring::path::PathCtx::of(v, g)
                    .approach_unit(player, object)
                    .is_some()
        });
        if started {
            self.drop_queued(player);
            self.item_queued.retain(|q| q.0 != player);
            self.object_queued.retain(|q| q.0 != player);
            self.object_queued.push((player, guid));
        }
    }

    /// The queued object cases whose run has ended run again (end of
    /// tick step 4, `WiredWorld::timer_step_work`); an operate 23 runs
    /// the waypoint operate as the handler does.
    pub(super) fn object_arrivals<D: ActionEvents>(&mut self, game: &mut Game, events: &mut D)
    where
        Self: WorldHost<D>,
    {
        let waiting = std::mem::take(&mut self.object_queued);
        for (player, guid) in waiting {
            let moving = self.desk(game, events, |desk, _, _| {
                MOVING.contains(&NpcWorld::mode(desk, player))
            });
            if moving {
                self.object_queued.push((player, guid));
                continue;
            }
            self.object_arriving = true;
            let case = WorldHost::<D>::objects(self, game, events, player, guid);
            self.object_arriving = false;
            if let Some(ObjectCase::Waypoint(_)) = case {
                WorldHost::<D>::waypoints(self, game, events, WaypointOperate { player, guid });
            }
        }
    }
}
