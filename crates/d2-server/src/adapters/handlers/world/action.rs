// Spec: specs/world/waypoints.md §6, §7.1
//! [`ActionWorld`]: the world systems whose seams have a provider in
//! `d2_sim::wiring::action` — the waypoints (`WaypointView` on
//! [`ActionSim`]). The NPC, vendor and quest seams (`NpcWorld`,
//! `VendorWorld`, the quests' `QuestRest`) have no provider in `d2-sim`
//! yet, so their ids stay stubs on this host.

use d2_sim::game::Game;
use d2_sim::units::UnitId;
use d2_sim::wiring::action::{ActionSim, Pending};
use d2_sim::world::waypoints::{ArrivalList, WaypointData};

use super::{WaypointCall, WorldFault, WorldHost};

/// Messages queued by the action wiring's `Pending::send` (the transport
/// seam), handed back in send order.
pub trait Outbox {
    fn take_sent(&mut self) -> Vec<(UnitId, Vec<u8>)>;
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

impl<X: Pending + Outbox> WorldHost<ActionSim<X>> for ActionWorld {
    fn waypoints<C: WaypointCall>(
        &mut self,
        game: &mut Game,
        events: &mut ActionSim<X>,
        call: C,
    ) -> Option<C::Out> {
        let data = self.waypoints.as_ref()?;
        let arrivals = &mut self.arrivals;
        Some(events.waypoints(game, |w| call.call(data, arrivals, w)))
    }

    fn take_sent(&mut self, events: &mut ActionSim<X>) -> Vec<(UnitId, Vec<u8>)> {
        events.hooks().x.take_sent()
    }

    fn fault(&mut self, fault: WorldFault) {
        self.faults.push(fault);
    }
}
