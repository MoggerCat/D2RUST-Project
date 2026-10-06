// Spec: specs/sim/tick.md §3, §5.5, §5.6; specs/world/objects.md §2, §14; specs/sim/units.md §5; specs/drlg/rooms.md §4.1, §7, §8; specs/drlg/levels.md §9
//! [`ActionSim`]: the one dispatcher the tick runs. Timer events go to
//! the unit dispatch (`units.md` §5: the per-kind handler tables and the
//! monster freeze drop of `tick.md` §5.6), whose hooks run the missile
//! class handler, the AI think and the AI reset ([`super::units`]). The
//! tick hooks of steps 9 and 10 and the client room change run the DRLG
//! ([`super::rooms`]); every other tick hook keeps its default.

use std::sync::Arc;

use crate::game::Game;
use crate::missiles;
use crate::monsters::ai;
use crate::stats::StatData;
use crate::tick::timer::TimerRun;
use crate::tick::{EventDispatch, TickHooks};
use crate::units::dispatch::UnitSystem;
use crate::units::hooks::UnitData;
use crate::units::{ClientId, RoomId, UnitId, UnitType};
use crate::world::objects::{ObjectControl, ObjectTables};

use super::combat::CombatView;
use super::objects::{ObjectCase, ObjectState, ObjectView};
use super::waypoints::WaypointView;
use super::{ActionHooks, ActionTables, Pending, View, WiringError};

/// The action systems of one game, as one [`EventDispatch`] and
/// [`TickHooks`] for [`crate::tick::tick`].
pub struct ActionSim<X> {
    pub sys: UnitSystem<ActionHooks<X>>,
}

impl<X: Pending> ActionSim<X> {
    pub fn new(stat_data: Arc<StatData>, data: UnitData, hooks: ActionHooks<X>) -> Self {
        Self {
            sys: UnitSystem::new(stat_data, data, hooks),
        }
    }

    /// The shared state.
    pub fn hooks(&mut self) -> &mut ActionHooks<X> {
        &mut self.sys.hooks
    }

    /// Runs `f` on a [`View`] of the unit side (allocation, removal,
    /// stats, states) with the game.
    pub fn with<R>(
        &mut self,
        game: &mut Game,
        f: impl FnOnce(&mut Game, &mut View<'_, X>) -> R,
    ) -> R {
        let s = &mut self.sys;
        let mut v = View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks);
        f(game, &mut v)
    }

    /// Runs `f` with the missile code's context (creation from skills,
    /// tests).
    pub fn missiles<R>(
        &mut self,
        game: &mut Game,
        f: impl FnOnce(&mut Game, &mut missiles::Ctx<'_, View<'_, X>>) -> R,
    ) -> Option<R> {
        let s = &mut self.sys;
        let mut store = s.hooks.missiles.take()?;
        let t = s.hooks.tables.clone();
        let r = {
            let mut v = View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks);
            let mut cx = missiles::Ctx {
                tables: &t.missiles,
                store: &mut store,
                world: &mut v,
            };
            f(game, &mut cx)
        };
        s.hooks.missiles = Some(store);
        Some(r)
    }

    /// Runs `f` with the AI code's context (AI install at monster
    /// creation, `monsters/init.md`; tests).
    pub fn ai<R>(
        &mut self,
        game: &mut Game,
        f: impl FnOnce(&mut Game, &mut ai::Ctx<'_, View<'_, X>>) -> R,
    ) -> Option<R> {
        let s = &mut self.sys;
        let mut store = s.hooks.ai.take()?;
        let t = s.hooks.tables.clone();
        let info = s.hooks.ai_info;
        let r = {
            let mut v = View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks);
            let mut cx = ai::Ctx {
                tables: ai::AiTables {
                    monstats: &t.combat.monstats,
                    monstats2: &t.combat.monstats2,
                    levels: &t.levels,
                    skill_modes: &t.skill_modes,
                },
                info,
                store: &mut store,
                world: &mut v,
            };
            f(game, &mut cx)
        };
        s.hooks.ai = Some(store);
        Some(r)
    }

    /// Runs `f` with combat's view and tables (skill functions, tests).
    pub fn combat<R>(
        &mut self,
        game: &mut Game,
        f: impl FnOnce(&mut CombatView<'_, X>, &ActionTables) -> R,
    ) -> R {
        let s = &mut self.sys;
        let t = s.hooks.tables.clone();
        let mut w = CombatView {
            game,
            v: View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks),
        };
        f(&mut w, &t)
    }

    /// Runs `f` with the waypoint code's view (object init/operate, the
    /// C→S 0x49 handler).
    pub fn waypoints<R>(
        &mut self,
        game: &mut Game,
        f: impl FnOnce(&mut WaypointView<'_, X>) -> R,
    ) -> R {
        let s = &mut self.sys;
        let mut w = WaypointView {
            game,
            v: View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks),
        };
        f(&mut w)
    }

    /// Game creation's object control `0x00546C60` (`objects.md` §2): one
    /// step of the game seed ([`ActionHooks::game_seed`]), the control,
    /// the shrine lists and the level regions from `tables`.
    ///
    /// TODO(rng.md §5.2): game creation derives, in order, the monster
    /// regions (`WorldSim::create_regions`), this control, the NPC
    /// control and the quest control, each from one game-seed step. d2rs
    /// has no single game-creation sequence yet: the host calls this
    /// right after the regions and before the NPC and quest controls.
    pub fn create_objects(&mut self, tables: Arc<ObjectTables>) {
        let h = &mut self.sys.hooks;
        h.objects = Some(ObjectState::new(&mut h.game_seed, tables));
    }

    /// A host holding the quest control takes the object module's quest
    /// routes from a queue ([`ActionSim::take_quest_calls`]) instead of
    /// [`super::Pending::object_route`]; call right after
    /// [`ActionSim::create_objects`]. No object state: nothing.
    pub fn route_quest_objects(&mut self) {
        if let Some(st) = self.sys.hooks.objects.as_mut() {
            st.route_quests();
        }
    }

    /// The queued quest routes ([`super::QuestObjectCall`]), in order.
    pub fn take_quest_calls(&mut self) -> Vec<super::QuestObjectCall> {
        self.sys
            .hooks
            .objects
            .as_mut()
            .map(|s| s.take_quest_calls())
            .unwrap_or_default()
    }

    /// Runs `f` with the object control, the tables and the object
    /// code's view (tests, skills that call the dispatch directly).
    /// `None`: no object state.
    pub fn objects<R>(
        &mut self,
        game: &mut Game,
        f: impl FnOnce(&mut ObjectControl, &ObjectTables, &mut ObjectView<'_, X>) -> R,
    ) -> Option<R> {
        let s = &mut self.sys;
        let mut v = View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks);
        super::objects::with_objects(game, &mut v, f)
    }

    /// The C→S 0x13 object case (`waypoints.md` §5.2,
    /// [`View::object_message`]). `None`: no object state.
    pub fn operate_object_message(
        &mut self,
        game: &mut Game,
        player: UnitId,
        guid: u32,
    ) -> Option<ObjectCase> {
        self.with(game, |g, v| v.object_message(g, player, guid))
    }

    fn log(&mut self, r: Result<(), WiringError>) {
        if let Err(e) = r {
            self.sys.hooks.errors.push(e);
        }
    }
}

impl<X: Pending> EventDispatch for ActionSim<X> {
    fn run_event(&mut self, game: &mut Game, run: &TimerRun) {
        self.sys.run_event(game, run);
    }
}

impl<X: Pending> TickHooks for ActionSim<X> {
    /// Step 9 `0x0061A790` (`rooms.md` §7.2).
    fn room_inactivity(&mut self, game: &mut Game, room: RoomId) -> u32 {
        match self.sys.hooks.drlg.room_inactivity(game, room) {
            Ok(n) => n,
            Err(e) => {
                self.sys.hooks.errors.push(e);
                0
            }
        }
    }

    /// Step 9 `0x0061A3F0` (`rooms.md` §8.1).
    fn act_allows_room_removal(&mut self, _: &mut Game, act: u8, room: RoomId) -> bool {
        match self.sys.hooks.drlg.allows_removal(act, room) {
            Ok(b) => b,
            Err(e) => {
                self.sys.hooks.errors.push(e);
                false
            }
        }
    }

    /// Step 9, the rest of `0x0061A910` (`rooms.md` §8.2).
    fn room_deactivated(&mut self, game: &mut Game, act: u8, room: RoomId) {
        let r = self
            .sys
            .hooks
            .drlg
            .remove_active_room(&mut game.lists, act, room);
        self.log(r);
    }

    /// Step 10 `0x0061AA20` (`levels.md` §9.2).
    fn free_inactive_rooms(&mut self, _: &mut Game, act: u8) {
        let r = self.sys.hooks.drlg.free_inactive_rooms(act);
        self.log(r);
    }

    /// Per-client update (`tick.md` §6.5, `0x0053A5D0`): with the path
    /// provider on, a player's movement messages (`pathing.md` §10 rules
    /// 2–3, [`crate::wiring::path::walk::update_messages`]). Other units'
    /// update messages (the unit-update spec) are not written, except
    /// objects: the object update pass `0x00581AD0` (`objects.md` §14,
    /// [`View::object_update`]) to the client's player.
    fn send_unit_update(&mut self, game: &mut Game, client: ClientId, unit: UnitId) {
        let s = &mut self.sys;
        let mut v = View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks);
        if game
            .lists
            .unit(unit)
            .is_some_and(|e| e.ty == UnitType::Object)
        {
            if let Some(receiver) = game.lists.client(client).and_then(|c| c.player) {
                v.object_update(game, receiver, unit);
            }
            return;
        }
        if v.h.paths.is_none() {
            return;
        }
        crate::wiring::path::walk::update_messages(&mut v, game, client, unit);
    }

    /// Per-client update (`tick.md` §6.5): the player's room differs from
    /// the client's. The room switch `0x00537B50` (`rooms.md` §4.1,
    /// `levels.md` §9.1) runs on the DRLG and the client's room becomes
    /// the player's.
    ///
    /// TODO(tick.md §6.5): the level-change calls `0x00543B90`,
    /// `0x00537340` are not specified; a change between acts (act change,
    /// `waypoints.md` open question 1) is not handled: the client keeps
    /// its room.
    fn client_level_change(&mut self, game: &mut Game, client: ClientId) {
        let Some(e) = game.lists.client(client) else {
            return;
        };
        let old = e.room;
        let new = e
            .player
            .and_then(|p| game.lists.unit(p))
            .and_then(|u| u.room());
        let act_of = |r: Option<RoomId>| r.and_then(|r| game.lists.room(r)).map(|r| r.act);
        let (old_act, new_act) = (act_of(old), act_of(new));
        let act = match (old_act, new_act) {
            (Some(a), Some(b)) if a != b => return,
            (Some(a), _) | (None, Some(a)) => a,
            (None, None) => return,
        };
        let r = self
            .sys
            .hooks
            .drlg
            .client_changes_room(&mut game.lists, act, client, old, new);
        self.log(r);
        if let Some(e) = game.lists.client_mut(client) {
            e.room = new;
        }
    }
}
