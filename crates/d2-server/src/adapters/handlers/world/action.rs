// Spec: specs/world/waypoints.md §6, §7.1
//! [`ActionWorld`]: the world systems whose seams have a provider in
//! `d2_sim::wiring::action` — the waypoints (`WaypointView` on
//! [`ActionSim`], reached through [`ActionEvents`]). The NPC, vendor and quest seams (`NpcWorld`,
//! `VendorWorld`, the quests' `QuestRest`) have no provider in `d2-sim`
//! yet, so their ids stay stubs on this host.

use d2_sim::game::Game;
use d2_sim::tick::EventDispatch;
use d2_sim::units::UnitId;
use d2_sim::wiring::action::{ActionSim, Pending};
use d2_sim::wiring::worldgen::{WorldPending, WorldSim};
use d2_sim::world::waypoints::{ArrivalList, WaypointData};

use super::{WaypointCall, WorldFault, WorldHost};

/// Messages queued by the action wiring's `Pending::send` (the transport
/// seam), handed back in send order.
pub trait Outbox {
    fn take_sent(&mut self) -> Vec<(UnitId, Vec<u8>)>;
}

/// A game dispatch that holds the action wiring: [`ActionSim`] itself, or
/// the world-generation dispatch [`WorldSim`] around it (its `action`
/// field). The handlers on the action seams (waypoints here, skills in
/// `handlers::skills::wired`) reach the action systems through it.
pub trait ActionEvents: EventDispatch {
    type X: Pending;
    fn action(&mut self) -> &mut ActionSim<Self::X>;
}

impl<X: Pending> ActionEvents for ActionSim<X> {
    type X = X;
    fn action(&mut self) -> &mut ActionSim<X> {
        self
    }
}

impl<X: WorldPending> ActionEvents for WorldSim<X> {
    type X = X;
    fn action(&mut self) -> &mut ActionSim<X> {
        &mut self.action
    }
}

/// The world state of a game wired on [`ActionSim`].
#[derive(Default)]
pub struct ActionWorld {
    /// Waypoint tables (`WaypointData::new(levels, objects)`); `None`:
    /// 0x49 stays a stub.
    pub waypoints: Option<WaypointData>,
    /// The object control's arrival list (`waypoints.md` §7.1).
    pub arrivals: ArrivalList,
    /// Fatal paths met by the handlers, in order.
    pub faults: Vec<WorldFault>,
}

impl<D: ActionEvents> WorldHost<D> for ActionWorld
where
    D::X: Outbox,
{
    fn waypoints<C: WaypointCall>(
        &mut self,
        game: &mut Game,
        events: &mut D,
        call: C,
    ) -> Option<C::Out> {
        let data = self.waypoints.as_ref()?;
        let arrivals = &mut self.arrivals;
        Some(
            events
                .action()
                .waypoints(game, |w| call.call(data, arrivals, w)),
        )
    }

    fn take_sent(&mut self, events: &mut D) -> Vec<(UnitId, Vec<u8>)> {
        events.action().hooks().x.take_sent()
    }

    fn fault(&mut self, fault: WorldFault) {
        self.faults.push(fault);
    }
}
