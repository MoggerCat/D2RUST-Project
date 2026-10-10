// Spec: specs/world/objects.md §7.3 rule 4 (walk to the object, operate on arrival); specs/sim/pathing.md §1.2
//! The walk to an object of the wired host (C→S 0x13, object not in
//! interact range or the line blocked): the run request to the object
//! (`0x00548A50`, player mode 3 toward (type 2, GUID)), then the 0x13
//! object case again when the run ends. Same shape as the NPC and item
//! approaches ([`super::npc_approach`], [`super::item_approach`]).
//!
//! PROVISIONAL (REC-1930, d2rs-own, unverified): the arrival is read from
//! the player's mode leaving walk / run / town walk at the end of tick
//! step 4 (1.14d: the step result 2 of `0x00580C20`, `objects.md` §7.3
//! rule 4.4); a new walk request drops the queued operate. The retry runs
//! inside the stop frame of `0x00580C20` in 1.14d; a run it starts there
//! is ended at once by the ENDANIM neutral start of the step result 2
//! (`units.md` §4.5) and +0x150 is cleared: recorded `gen-obj-78` /
//! `gen-obj-39` (the player stops at the blocked step, the retry's run is
//! computed, the mode is neutral that frame and stays).

use d2_sim::game::Game;
use d2_sim::units::UnitId;
use d2_sim::wiring::action::ObjectCase;
use d2_sim::world::npc::NpcWorld;

use super::wired::{TradeRest, WiredWorld};
use super::{ActionEvents, WaypointOperate, WorldHost};

/// Unit modes of a moving player (walk, run, town walk).
const MOVING: [u8; 3] = [2, 3, 6];

impl<R: TradeRest, S> WiredWorld<R, S> {
    /// Starts the run of `player` to `object` and queues the operate.
    /// `retry`: called from the stop-frame retry (no new queue; the run is
    /// ended by the neutral start that follows the step result 2).
    pub(super) fn start_object_walk<D: ActionEvents>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        (player, object): (UnitId, UnitId),
        retry: bool,
    ) {
        let Some(guid) = game.lists.unit(object).map(|e| e.guid) else {
            return;
        };
        let started = events.action().with(game, |g, v| {
            v.h.paths.is_some()
                && d2_sim::wiring::path::PathCtx::of(v, g)
                    .approach_unit(player, object)
                    .is_some()
        });
        if started && retry {
            events.action().with(game, |g, v| {
                d2_sim::wiring::path::PathCtx::of(v, g).neutral_start(player);
            });
        } else if started {
            self.drop_queued(player);
            self.item_queued.retain(|q| q.0 != player);
            self.object_queued.retain(|q| q.0 != player);
            self.object_queued.push((player, guid));
        }
    }

    /// The queued operates whose run has ended run the 0x13 object case
    /// again (end of tick step 4).
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
            match WorldHost::<D>::objects(self, game, events, player, guid) {
                // Still out of range: a new run (+0x150 cleared after it),
                // ended by the ENDANIM neutral start (module doc).
                Some(ObjectCase::Walk(object)) => {
                    self.start_object_walk(game, events, (player, object), true);
                }
                Some(ObjectCase::Waypoint(_)) => {
                    let _ = WorldHost::<D>::waypoints(
                        self,
                        game,
                        events,
                        WaypointOperate { player, guid },
                    );
                }
                Some(ObjectCase::Code(_)) | None => {}
            }
        }
    }
}
