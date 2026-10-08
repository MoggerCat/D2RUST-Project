// Spec: specs/sim/tick.md §3, §4 (room pass), §5; specs/monsters/population.md §1.1
//! [`WorldSim`]: the action systems ([`ActionSim`]) plus the world state,
//! as one [`EventDispatch`] and [`TickHooks`] for [`crate::tick::tick`].
//! Timer events run the action systems' unit dispatch with the world
//! state lent to the action hooks ([`super::monster_world`]: monster
//! event 7, monster init on allocation, the world state's part of a unit
//! free, the umod callbacks, monster-data queries). Every tick hook goes
//! to the action systems, with the world state lent too, except the
//! room-pass hooks of step 3 (`tick.md` §4), which run population
//! (`population.md` §1.1) on a [`WorldHost`] holding the world state:
//! ambient spawns, presets, inactive restore, objects, monster
//! population.

use std::sync::Arc;

use crate::game::Game;
use crate::monsters::init;
use crate::monsters::population::{self as pop, preset, room};
use crate::stats::StatData;
use crate::tick::timer::TimerRun;
use crate::tick::{EventDispatch, TickHooks};
use crate::units::hooks::UnitData;
use crate::units::{ClientId, RoomId, UnitId};

use super::super::action::{ActionHooks, ActionSim};
use super::{View, WorldHost, WorldPending, WorldState, WorldgenError};

/// The action systems and the world-generation state of one game.
pub struct WorldSim<X> {
    pub action: ActionSim<X>,
    pub world: WorldState,
}

impl<X: WorldPending> WorldSim<X> {
    pub fn new(
        stat_data: Arc<StatData>,
        data: UnitData,
        hooks: ActionHooks<X>,
        world: WorldState,
    ) -> Self {
        Self {
            action: ActionSim::new(stat_data, data, hooks),
            world,
        }
    }

    /// Runs `f` on a [`WorldHost`] over the game.
    pub fn host<R>(&mut self, game: &mut Game, f: impl FnOnce(&mut WorldHost<'_, X>) -> R) -> R {
        let s = &mut self.action.sys;
        let mut h = WorldHost {
            game,
            v: View::of(&mut s.units, &mut s.stats, &s.data, &mut s.hooks),
            w: &mut self.world,
        };
        f(&mut h)
    }

    /// Runs `f` with population's context (game creation, tests).
    pub fn population<R>(
        &mut self,
        game: &mut Game,
        f: impl FnOnce(&mut pop::Ctx<'_, WorldHost<'_, X>>) -> R,
    ) -> R {
        self.host(game, |h| h.population(f))
    }

    /// Runs `f` with init's tables and host (event 7, the mode-change and
    /// missile callbacks, tests).
    pub fn init<R>(
        &mut self,
        game: &mut Game,
        f: impl FnOnce(&init::Ctx<'_>, &mut WorldHost<'_, X>) -> R,
    ) -> R {
        self.host(game, |h| h.init(f))
    }

    /// Game creation (`population.md` §2.1): the regions on the game seed
    /// of the action state.
    pub fn create_regions(&mut self) {
        let mut seed = self.action.sys.hooks.game_seed;
        self.world.create_regions(&mut seed);
        self.action.sys.hooks.game_seed = seed;
    }

    /// Game creation's object control (`objects.md` §2,
    /// [`ActionSim::create_objects`]): call right after
    /// [`WorldSim::create_regions`] (`rng.md` §5.2 order).
    pub fn create_objects(&mut self, tables: Arc<crate::world::objects::ObjectTables>) {
        self.action.create_objects(tables);
    }

    /// Every error so far: world adapters, level types, action adapters
    /// and the unit dispatch.
    pub fn errors(&self) -> Vec<String> {
        let mut out: Vec<String> = self.world.errors.iter().map(|e| format!("{e:?}")).collect();
        out.extend(
            self.world
                .types
                .borrow()
                .errors
                .iter()
                .map(|e: &WorldgenError| format!("{e:?}")),
        );
        out.extend(
            self.action
                .sys
                .hooks
                .errors
                .iter()
                .map(|e| format!("{e:?}")),
        );
        out.extend(self.action.sys.errors.iter().map(|e| format!("{e:?}")));
        out
    }
}

impl<X: WorldPending> EventDispatch for WorldSim<X> {
    /// The unit dispatch of the action systems with the world state lent
    /// to the action hooks ([`super::events`]: monster event 7, the world
    /// state's part of a unit free, the monster routes of the action
    /// adapters).
    fn run_event(&mut self, game: &mut Game, run: &TimerRun) {
        self.lend(|a| a.run_event(game, run));
    }
}

/// Tick hooks forwarded to the action systems with the world state lent.
macro_rules! lent {
    ($( $(#[$m:meta])* fn $name:ident(&mut self, game: &mut Game $(, $a:ident: $t:ty)*) $(-> $r:ty)?; )*) => {
        $(
            $(#[$m])*
            fn $name(&mut self, game: &mut Game $(, $a: $t)*) $(-> $r)? {
                self.lend(|s| s.$name(game $(, $a)*))
            }
        )*
    };
}

impl<X: WorldPending> TickHooks for WorldSim<X> {
    /// Step 3 `0x0054F060` (`population.md` §12).
    fn ambient_spawns(&mut self, game: &mut Game, r: RoomId) {
        self.population(game, |cx| room::ambient(cx, r));
    }
    /// Step 3 `0x005559A0` (`population.md` §11.1).
    fn spawn_presets(&mut self, game: &mut Game, r: RoomId) {
        self.population(game, |cx| preset::place_presets(cx, r));
        // PROVISIONAL (REC-95): the warp tile units of the room's presets.
        self.host(game, |h| {
            let WorldHost { game, v, .. } = h;
            v.spawn_warp_tiles(game, r);
        });
    }
    /// Step 3 `0x00542B40` (`units.md` §3.4 rule 4): on the action
    /// wiring's inactive store when it is on, else the host's.
    fn restore_inactive_units(&mut self, game: &mut Game, r: RoomId) {
        if !self.lend(|a| a.restore(game, r)) {
            self.host(game, |h| h.v.h.x.restore_inactive_units(r));
        }
    }
    /// Step 3 `0x00552610`.
    fn populate_objects(&mut self, game: &mut Game, r: RoomId) {
        self.host(game, |h| h.v.h.x.populate_objects(r));
    }
    /// Step 3 `0x0054EC90` (`population.md` §3).
    fn populate_monsters(&mut self, game: &mut Game, r: RoomId) {
        self.population(game, |cx| room::populate_room(cx, r));
    }

    lent! {
        fn advance_environment(&mut self, game: &mut Game, act: u8) -> bool;
        fn environment_changed(&mut self, game: &mut Game, act: u8, client: ClientId);
        fn client_room_ready(&mut self, game: &mut Game, client: ClientId) -> bool;
        fn send_load_complete(&mut self, game: &mut Game, client: ClientId);
        fn refresh_inventory(&mut self, game: &mut Game, client: ClientId);
        fn join_sequence(&mut self, game: &mut Game, client: ClientId);
        fn send_removed_units(&mut self, game: &mut Game, client: ClientId);
        fn send_unit_update(&mut self, game: &mut Game, client: ClientId, unit: UnitId);
        fn client_update_messages(&mut self, game: &mut Game, client: ClientId);
        fn client_level_change(&mut self, game: &mut Game, client: ClientId);
        fn arena_sync(&mut self, game: &mut Game, client: ClientId);
        fn unit_update(&mut self, game: &mut Game, unit: UnitId);
        fn clear_arena_flag(&mut self, game: &mut Game);
        fn free_removal_records(&mut self, game: &mut Game, r: RoomId);
        fn update_quests(&mut self, game: &mut Game);
        fn room_inactivity(&mut self, game: &mut Game, r: RoomId) -> u32;
        fn act_allows_room_removal(&mut self, game: &mut Game, act: u8, r: RoomId) -> bool;
        fn compress_unit(&mut self, game: &mut Game, unit: UnitId);
        fn room_deactivated(&mut self, game: &mut Game, act: u8, r: RoomId);
        fn free_inactive_rooms(&mut self, game: &mut Game, act: u8);
        fn delete_inactive_items(&mut self, game: &mut Game, act: u8);
        fn expire_inactive_unit_items(&mut self, game: &mut Game, act: u8);
    }
}
