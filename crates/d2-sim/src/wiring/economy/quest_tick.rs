// Spec: specs/sim/tick.md §3 (step 8), §6; specs/world/quests.md §5
//! Tick step 8, the quest updater `0x00543E10` (`quests.md` §5), on the
//! game's [`QuestControl`] with the economy's quest world
//! ([`EconomyQuests`]): [`QuestTick`] wraps the tick hooks of a game
//! (any [`TickHooks`] with a unit side, e.g. the action or world
//! dispatcher) and runs [`QuestControl::update`] as its `update_quests`.
//! The tick decides when (`frame % 20 = 0`, `tick.md` §3); every other
//! hook and every timer event is the wrapped dispatcher's.

use crate::game::Game;
use crate::items::ItemTables;
use crate::stats::StatLists;
use crate::tick::timer::TimerRun;
use crate::tick::{EventDispatch, TickHooks};
use crate::units::dispatch::UnitSystem;
use crate::units::hooks::UnitData;
use crate::units::lifecycle::LifecycleHooks;
use crate::units::record::Units;
use crate::units::{ClientId, RoomId, UnitId};
use crate::wiring::action::{ActionHooks, ActionSim, Pending};
use crate::wiring::worldgen::{WorldPending, WorldSim};
use crate::world::quests::QuestControl;

use super::{Economy, EconomyQuests, GameFields, ItemStore, QuestRest};

/// The unit side an [`Economy`] is built on: unit records, stat lists,
/// unit tables and the lifecycle hooks of a unit system.
pub trait UnitSide {
    type Hooks: LifecycleHooks;
    fn unit_side(&mut self) -> (&mut Units, &mut StatLists, &UnitData, &mut Self::Hooks);
}

impl<H: LifecycleHooks> UnitSide for UnitSystem<H> {
    type Hooks = H;
    fn unit_side(&mut self) -> (&mut Units, &mut StatLists, &UnitData, &mut H) {
        (
            &mut self.units,
            &mut self.stats,
            &self.data,
            &mut self.hooks,
        )
    }
}

impl<X: Pending> UnitSide for ActionSim<X> {
    type Hooks = ActionHooks<X>;
    fn unit_side(&mut self) -> (&mut Units, &mut StatLists, &UnitData, &mut ActionHooks<X>) {
        self.sys.unit_side()
    }
}

impl<X: WorldPending> UnitSide for WorldSim<X> {
    type Hooks = ActionHooks<X>;
    fn unit_side(&mut self) -> (&mut Units, &mut StatLists, &UnitData, &mut ActionHooks<X>) {
        self.action.sys.unit_side()
    }
}

/// A game's tick hooks plus its quests: the economy's parts beside the
/// unit side, the quest control block and the quests' rest.
pub struct QuestTick<'q, S, R> {
    pub sim: &'q mut S,
    pub fields: &'q mut GameFields,
    pub tables: &'q ItemTables,
    pub items: &'q mut ItemStore,
    pub quests: &'q mut QuestControl,
    pub rest: &'q mut R,
}

impl<S: EventDispatch, R> EventDispatch for QuestTick<'_, S, R> {
    fn run_event(&mut self, game: &mut Game, run: &TimerRun) {
        self.sim.run_event(game, run);
    }
}

impl<S: TickHooks + UnitSide, R: QuestRest> TickHooks for QuestTick<'_, S, R> {
    /// Step 8 `0x00543E10` (`quests.md` §5).
    fn update_quests(&mut self, game: &mut Game) {
        let (units, stats, data, hooks) = self.sim.unit_side();
        let mut econ = Economy {
            game,
            units,
            stats,
            data,
            hooks,
            fields: &mut *self.fields,
            tables: self.tables,
            items: &mut *self.items,
        };
        let mut w = EconomyQuests::new(&mut econ, &mut *self.rest);
        self.quests.update(&mut w);
    }

    fn advance_environment(&mut self, game: &mut Game, act: u8) -> bool {
        self.sim.advance_environment(game, act)
    }
    fn environment_changed(&mut self, game: &mut Game, act: u8, client: ClientId) {
        self.sim.environment_changed(game, act, client)
    }
    fn ambient_spawns(&mut self, game: &mut Game, room: RoomId) {
        self.sim.ambient_spawns(game, room)
    }
    fn spawn_presets(&mut self, game: &mut Game, room: RoomId) {
        self.sim.spawn_presets(game, room)
    }
    fn restore_inactive_units(&mut self, game: &mut Game, room: RoomId) {
        self.sim.restore_inactive_units(game, room)
    }
    fn populate_objects(&mut self, game: &mut Game, room: RoomId) {
        self.sim.populate_objects(game, room)
    }
    fn populate_monsters(&mut self, game: &mut Game, room: RoomId) {
        self.sim.populate_monsters(game, room)
    }
    fn client_room_ready(&mut self, game: &mut Game, client: ClientId) -> bool {
        self.sim.client_room_ready(game, client)
    }
    fn send_load_complete(&mut self, game: &mut Game, client: ClientId) {
        self.sim.send_load_complete(game, client)
    }
    fn refresh_inventory(&mut self, game: &mut Game, client: ClientId) {
        self.sim.refresh_inventory(game, client)
    }
    fn join_sequence(&mut self, game: &mut Game, client: ClientId) {
        self.sim.join_sequence(game, client)
    }
    fn send_removed_units(&mut self, game: &mut Game, client: ClientId) {
        self.sim.send_removed_units(game, client)
    }
    fn send_unit_update(&mut self, game: &mut Game, client: ClientId, unit: UnitId) {
        self.sim.send_unit_update(game, client, unit)
    }
    fn client_update_messages(&mut self, game: &mut Game, client: ClientId) {
        self.sim.client_update_messages(game, client)
    }
    fn client_level_change(&mut self, game: &mut Game, client: ClientId) {
        self.sim.client_level_change(game, client)
    }
    fn arena_sync(&mut self, game: &mut Game, client: ClientId) {
        self.sim.arena_sync(game, client)
    }
    fn unit_update(&mut self, game: &mut Game, unit: UnitId) {
        self.sim.unit_update(game, unit)
    }
    fn clear_arena_flag(&mut self, game: &mut Game) {
        self.sim.clear_arena_flag(game)
    }
    fn free_removal_records(&mut self, game: &mut Game, room: RoomId) {
        self.sim.free_removal_records(game, room)
    }
    fn room_inactivity(&mut self, game: &mut Game, room: RoomId) -> u32 {
        self.sim.room_inactivity(game, room)
    }
    fn act_allows_room_removal(&mut self, game: &mut Game, act: u8, room: RoomId) -> bool {
        self.sim.act_allows_room_removal(game, act, room)
    }
    fn compress_unit(&mut self, game: &mut Game, unit: UnitId) {
        self.sim.compress_unit(game, unit)
    }
    fn room_deactivated(&mut self, game: &mut Game, act: u8, room: RoomId) {
        self.sim.room_deactivated(game, act, room)
    }
    fn free_inactive_rooms(&mut self, game: &mut Game, act: u8) {
        self.sim.free_inactive_rooms(game, act)
    }
    fn delete_inactive_items(&mut self, game: &mut Game, act: u8) {
        self.sim.delete_inactive_items(game, act)
    }
    fn expire_inactive_unit_items(&mut self, game: &mut Game, act: u8) {
        self.sim.expire_inactive_unit_items(game, act)
    }
}
