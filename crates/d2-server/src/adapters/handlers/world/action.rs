// Spec: specs/world/waypoints.md §6, §7.1; specs/sim/rng.md §5.3
//! [`ActionWorld`]: the systems whose seams have a provider in
//! `d2_sim::wiring::action` alone — the waypoints (`WaypointView` on
//! [`ActionSim`], reached through [`ActionEvents`]) and the skill
//! handlers (a [`SkillHost`] slot, `handlers::skills::wired`). The NPC,
//! vendor, quest and cube ids stay stubs on this host;
//! [`super::WiredWorld`] adds them on the interaction and economy
//! wiring.
//!
//! Game creation ([`ActionEvents::create_game`]): the game seed and the
//! creation fields have their one home on the action wiring
//! (`ActionHooks::game_seed`, `ActionHooks::ai_info`,
//! `UnitData::expansion`); the copies other `d2-sim` readers hold are
//! written from the same values there.

use d2_sim::game::Game;
use d2_sim::tick::EventDispatch;
use d2_sim::units::UnitId;
use d2_sim::wiring::action::{ActionSim, Pending};
use d2_sim::wiring::economy::GameFields;
use d2_sim::wiring::worldgen::{WorldPending, WorldSim};
use d2_sim::world::waypoints::{ArrivalList, WaypointData};

use super::super::skills::{Call as SkillCall, Handled as SkillHandled, NoSkills, SkillHost};
use super::super::walk::{WalkCall, WalkResult};
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

    /// Game creation's fields (`docs/HANDOFF.md` §7 I7, W16) from
    /// `fields`: the game seed (`rng.md` §5.3) and the creation fields
    /// (difficulty +0x6D, expansion +0x70, game type +0x6A, ladder +0x74;
    /// the item format +0x78 follows from the expansion, `generation.md`
    /// §1.2) written to their home, `ActionHooks::game_seed`,
    /// `ActionHooks::ai_info` and `UnitData::expansion`, and to the
    /// difficulty copy the unit code reads (`UnitData::difficulty`). The
    /// economy's `GameFields` are built from the home for each call
    /// (`WiredWorld::with_economy`); `fields.uniques` is not used (a new
    /// game's unique bits are zero, `cube.md` Inputs).
    fn create_game(&mut self, fields: &GameFields) {
        let a = self.action();
        a.sys.hooks.game_seed = fields.seed;
        a.sys.hooks.ai_info = fields.ai_info();
        a.sys.data.difficulty = fields.difficulty;
        a.sys.data.expansion = fields.expansion;
    }
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

    /// Also the world-generation copies (`WorldState::pop_info`,
    /// `init_info`; their player counts are kept).
    fn create_game(&mut self, fields: &GameFields) {
        let a = &mut self.action;
        a.sys.hooks.game_seed = fields.seed;
        a.sys.hooks.ai_info = fields.ai_info();
        a.sys.data.difficulty = fields.difficulty;
        a.sys.data.expansion = fields.expansion;
        let w = &mut self.world;
        w.init_info.difficulty = fields.difficulty;
        w.init_info.expansion = fields.expansion;
        w.init_info.game_type = fields.game_type;
        w.init_info.ladder = fields.ladder;
        w.pop_info.difficulty = fields.difficulty;
        w.pop_info.expansion = fields.expansion;
    }
}

/// The systems of a game wired on [`ActionSim`]: the waypoints and the
/// skill handlers' slot `S` ([`NoSkills`]: skill ids stay stubs).
#[derive(Default)]
pub struct ActionWorld<S = NoSkills> {
    /// Waypoint tables (`WaypointData::new(levels, objects)`); `None`:
    /// 0x49 stays a stub.
    pub waypoints: Option<WaypointData>,
    /// The object control's arrival list (`waypoints.md` §7.1).
    pub arrivals: ArrivalList,
    /// Fatal paths met by the handlers, in order.
    pub faults: Vec<WorldFault>,
    /// The skill handlers (`handlers::skills::wired::WiredSkills`).
    pub skills: S,
}

impl<D: ActionEvents, S: SkillHost<D>> WorldHost<D> for ActionWorld<S>
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

    fn skill(&mut self, call: SkillCall<'_, D>) -> Option<SkillHandled> {
        self.skills.handle(call)
    }

    /// `handlers::walk::run` (the path provider of the action wiring).
    fn walk(&mut self, game: &mut Game, events: &mut D, call: WalkCall) -> Option<WalkResult> {
        super::super::walk::run(game, events, call)
    }

    fn take_sent(&mut self, events: &mut D) -> Vec<(UnitId, Vec<u8>)> {
        events.action().hooks().x.take_sent()
    }

    fn fault(&mut self, fault: WorldFault) {
        self.faults.push(fault);
    }
}
