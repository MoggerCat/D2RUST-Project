// Spec: specs/drlg/levels.md, specs/drlg/rooms.md, specs/monsters/population.md, specs/monsters/init.md, specs/sim/tick.md §4 (wiring of the world-generation seams)
//! The world-generation seams wired to their providers:
//!
//! | Seam pair | Adapter |
//! |---|---|
//! | DRLG `LevelTypes` → maze / preset / outdoor | [`levels::WorldTypes`] |
//! | maze → preset maps and rooms | [`maze_presets::MazeToPreset`] |
//! | outdoor → preset maps and rooms, act placer → preset directions | [`outdoor_presets::OutdoorToPreset`] |
//! | population `PopWorld` → DRLG rooms, collision, preset units; units | [`population`] |
//! | population `MonsterInit` → monster init, unit allocation | [`population_init`] |
//! | init `InitHost` → units, stats, AI, population regions | [`init_units`] |
//! | tick room pass (`tick.md` §4) → population | [`dispatch::WorldSim`] |
//! | action hooks → monster init, umods, unit free, monster data (world state lent) | [`monster_world`] |
//! | timer event 7 → init umods; unit free → world state | [`events`] (through the lent world state) |
//!
//! Ownership: [`dispatch::WorldSim`] holds the action systems
//! ([`ActionSim`]: units, stats, AI, missiles, the act DRLGs) and the
//! [`WorldState`] (population state, monster data, minion lists, the
//! tables). The act DRLGs hold the level types through a
//! [`levels::SharedTypes`] handle; [`WorldState::types`] is a clone of it.
//! Each population or init call builds a short-lived [`WorldHost`] over
//! the game, the action view and the world state. Around its timer
//! events and forwarded tick hooks, [`WorldSim`] lends the world state
//! to the action hooks ([`WorldSim::lend`]); [`WorldSim::world`] holds an
//! empty placeholder for the call.
//!
//! Calls with no provider yet go to [`WorldPending`] (defaults: nothing).
//! Nothing here decides game behaviour: every rule stays in its module.

pub mod creation;
pub mod dispatch;
pub mod events;
pub mod init_units;
pub mod levels;
pub mod maze_presets;
pub mod monster_world;
pub mod outdoor_presets;
pub mod population;
pub mod population_init;
pub mod umod_host;

#[cfg(test)]
#[path = "tests/routing.rs"]
mod routing_tests;
#[cfg(any(test, feature = "bench-fixtures"))]
#[cfg_attr(not(test), allow(unused, dead_code))]
pub(crate) mod tests;

use std::collections::BTreeMap;
use std::sync::Arc;

use d2_data::fixup::maps::EquivMatrix;
use d2_data::tables::{
    Difficultylevels, Levels, Monequip, Monlvl, Monprop, Monstats, Monstats2, Monumod, Superuniques,
};

use crate::drlg::maze::MazeError;
use crate::drlg::outdoor::OutdoorError;
use crate::drlg::preset::PresetError;
use crate::game::Game;
use crate::monsters::init::{self, InitTables, MonstatsExtra, MonsterStore, NamedIds};
use crate::monsters::population::{self as pop, CoordRect, OwnerKey, PopState, PopTables, Regions};
use crate::rng::Seed;
use crate::stats::ListId;
use crate::units::{RoomId, UnitId};

use super::action::{Pending, View, WiringError};

pub use creation::{CreatedControls, CreationError, CreationTables};
pub use dispatch::WorldSim;
pub use levels::{SharedTypes, WorldTypes};

/// What went wrong in a world-generation adapter, in order.
#[derive(Debug, PartialEq, Eq)]
pub enum WorldgenError {
    Preset(PresetError),
    Maze(MazeError),
    Outdoor(OutdoorError),
    /// A maze level allocated while the maze generator runs (level id).
    MazeBusy(u32),
    /// The population state was needed while population held it.
    PopStateLent(&'static str),
    /// The AI store was lent out (a monster created inside a think).
    AiLent,
    /// A room without an active DRLG room where population needed one.
    NoActiveRoom(RoomId),
    /// A population modifier call init has no entry for.
    Modifier(u8),
    Wiring(WiringError),
}

/// Seams of population and init whose provider is not written or not
/// implemented yet (objects, quests, owner data, alignment; the free
/// point without the path provider). The DRLG data population reads are
/// answered by the act DRLG (`drlg/levels.md` §11.6, [`population`]). Every default is the narrowest reading:
/// nothing happens, or the value that makes the caller do nothing. A host
/// (or a test) overrides what it can provide.
#[allow(unused_variables)]
pub trait WorldPending: Pending {
    // ---- path provider ---------------------------------------------------

    /// `0x0064E840`: the nearest free point.
    fn nearest_free_point(&self, room: RoomId, x: i32, y: i32) -> Option<(RoomId, i32, i32)> {
        None
    }

    // ---- quests ----------------------------------------------------------

    /// `0x005444B0`.
    fn quest_flag(&self, flag: u8) -> bool {
        false
    }
    /// `0x005B5210(game) ≠ 0`.
    fn chaos_blocks_population(&self) -> bool {
        false
    }
    /// `0x00544E80`: the quest hook of a new boss.
    fn boss_quest_hook(&mut self, boss: UnitId) {}

    // ---- monster creation pieces without a provider ----------------------

    /// `0x00552D60`: the monster's coordinate record.
    fn set_coord_record(
        &mut self,
        unit: UnitId,
        rect: Option<CoordRect>,
        room: RoomId,
        x: i32,
        y: i32,
    ) {
    }
    /// `0x005543B0`: the alignment value (units spec).
    fn set_alignment(&mut self, unit: UnitId, align: u8) {}
    /// `0x00573570(unit, flag, 1)` for flags other than 2.
    fn set_monster_flag(&mut self, unit: UnitId, flag: u32) {}
    /// `0x0058F030` (`monsters/ai.md`).
    fn set_owner_data(&mut self, unit: UnitId, owner: OwnerKey, a: i32, b: i32, c: i32) {}
    fn unique_minion_owner_data(&mut self, boss: UnitId, minion: UnitId) {}
    fn superunique_owner_data(&mut self, boss: UnitId) {}
    /// `0x005B24E0` (`population.md` open question 4).
    fn group_spawn(&mut self, boss: UnitId, class: i32, a: i32, b: i32, c: i32, flags: u16) {}
    /// `0x005B1990`.
    fn change_alignment(&mut self, unit: UnitId, a: i32, b: i32) {}

    // ---- umod callbacks (`monsters/umod-callbacks.md`) -------------------
    //
    // The calls of the callback bodies whose provider is a system the
    // world host does not reach (skill use, AI params, quests, items,
    // pets). [`init::InitHost`] on [`WorldHost`] answers the rest itself
    // (`umod_host.rs`).

    /// Target `skills/bodies.md` §2.1 (§17, §19).
    fn umod_target(&self, unit: UnitId) -> Option<UnitId> {
        None
    }
    /// Target position `0x0056D2C0` (`skills/bodies.md` §2.4; §5, the
    /// missile fallback of §2.4).
    fn umod_target_position(&self, unit: UnitId) -> Option<(i32, i32)> {
        None
    }
    /// `apply_state` (`skills/bodies.md` §2.7; §5).
    fn umod_apply_state(&mut self, req: init::callbacks::StateApply) -> Option<ListId> {
        None
    }
    /// Owner data `0x0058F030(game, unit, −1, 1, 0, 0)` (`ai.md`; §9).
    fn clear_owner_data(&mut self, unit: UnitId) {}
    /// Pet remove `0x005750E0(game, owner, GUID, 1)` (`sim/pets.md` §6;
    /// §14).
    fn remove_pet(&mut self, owner: UnitId, pet: UnitId) {}
    /// A quest death call (§15; `umod-callbacks.md` OQ4).
    fn quest_death(&mut self, unit: UnitId, call: u32) {}
    /// Umod 24's belt steal (§17 steps 3–4; unreachable in 1.14d).
    fn steal_belt_item(&mut self, unit: UnitId, target: UnitId) {}
    /// `0x005B2490(game, unit, class, mode, spread, flags)` (§25).
    fn spawn_near(&mut self, unit: UnitId, class: u32, mode: u32, spread: i32, flags: u32) {}
    /// AI param 0 (`0x0058EC50(unit, 1)`; §23.2).
    fn ai_param0(&mut self, unit: UnitId) -> i32 {
        0
    }
    /// `0x0058EC00`: AI param 0 := v (§23.2).
    fn set_ai_param0(&mut self, unit: UnitId, v: i32) {}
    /// The raise test `0x00645510(unit, 0)` (`skills/bodies.md` §3.6;
    /// §23.2).
    fn can_raise(&mut self, unit: UnitId) -> bool {
        false
    }
    /// `0x005DEAD0(game, unit, mode, skill, 0, 0, 0)` (`ai.md`; §23.2).
    fn ai_use_skill(&mut self, unit: UnitId, mode: u32, skill: u16) {}
    /// The level of the unit's skill entry (`0x006439F0`,
    /// `0x006442A0(unit, entry, 1)`; §27).
    fn skill_level(&mut self, unit: UnitId, skill: u16) -> Option<i32> {
        None
    }

    // ---- objects and units restore ---------------------------------------

    fn create_object(&mut self, room: RoomId, class: i32, x: i32, y: i32) {}
    fn barricade_object(&mut self, unit: UnitId, class: i32) {}
    /// `0x0058F000`, `0x00666120` after a preset monster's creation.
    fn preset_created(&mut self, unit: UnitId, preset: &pop::PresetUnit) {}
    /// `0x00542B40` (`sim/units.md`).
    fn restore_inactive_units(&mut self, room: RoomId) {}
    /// `0x00552610` (objects spec).
    fn populate_objects(&mut self, room: RoomId) {}
}

/// The tables population and init read (typed `d2_data` records, owned).
#[derive(Clone, Debug, Default)]
pub struct WorldTables {
    pub pop: PopTables,
    pub monstats: Vec<Monstats>,
    pub monstats2: Vec<Monstats2>,
    pub monlvl: Vec<Monlvl>,
    pub levels: Vec<Levels>,
    pub monprop: Vec<Monprop>,
    pub monequip: Vec<Monequip>,
    pub monumod: Vec<Monumod>,
    pub superuniques: Vec<Superuniques>,
    pub difficultylevels: Vec<Difficultylevels>,
    pub monstats_extra: Vec<MonstatsExtra>,
    pub components: Vec<[u8; 16]>,
    pub ids: NamedIds,
    /// The montype equivalence matrix (`runtime-maps.md` §2): init's
    /// "MonType is that type or nested in it" (`init.md` §17.3).
    pub montype_equiv: EquivMatrix,
}

impl WorldTables {
    /// Init's view of the tables.
    pub fn init(&self) -> init::Ctx<'_> {
        init::Ctx {
            tables: InitTables {
                monstats: &self.monstats,
                monstats2: &self.monstats2,
                monlvl: &self.monlvl,
                levels: &self.levels,
                monprop: &self.monprop,
                monequip: &self.monequip,
                monumod: &self.monumod,
                superuniques: &self.superuniques,
                difficultylevels: &self.difficultylevels,
                monstats_extra: &self.monstats_extra,
                components: &self.components,
                ids: self.ids,
            },
        }
    }
}

/// The world-generation state of a game beside the action systems.
pub struct WorldState {
    /// The level types the act DRLGs use (shared handle).
    pub types: SharedTypes,
    pub tables: Arc<WorldTables>,
    /// Regions, monster seed, superunique bits (`population.md`).
    pub pop: PopState,
    /// `pop` is lent to a population call (`pop` holds a placeholder).
    pop_lent: bool,
    pub pop_info: pop::GameInfo,
    pub init_info: init::GameInfo,
    /// Monster data of every monster (unit +0x14).
    pub monsters: MonsterStore,
    /// Minion lists (`0x0058F100`), in insertion order.
    pub minions: BTreeMap<UnitId, Vec<UnitId>>,
    /// Owner of a minion (`0x005DD330`).
    pub owners: BTreeMap<UnitId, UnitId>,
    /// Superuniques whose `init.md` §20 steps 4–5 run before their
    /// closing modifier 22: row and the aura flag.
    pub superunique_tail: BTreeMap<UnitId, (u16, bool)>,
    pub errors: Vec<WorldgenError>,
}

impl WorldState {
    pub fn new(types: SharedTypes, tables: Arc<WorldTables>, info: init::GameInfo) -> Self {
        Self {
            types,
            tables,
            pop: PopState::default(),
            pop_lent: false,
            pop_info: pop::GameInfo {
                difficulty: info.difficulty,
                expansion: info.expansion,
            },
            init_info: info,
            monsters: MonsterStore::new(),
            minions: BTreeMap::new(),
            owners: BTreeMap::new(),
            superunique_tail: BTreeMap::new(),
            errors: Vec::new(),
        }
    }

    /// Game creation (`population.md` §2.1): the regions on the game seed,
    /// and `lo'` as the monster seed.
    pub fn create_regions(&mut self, game_seed: &mut Seed) {
        let (regions, lo) = Regions::create(&self.tables.pop, self.pop_info, game_seed);
        self.pop.regions = regions;
        self.pop.mon_seed = lo;
    }
}

/// The host of one population or init call: the game, the action view
/// (units, stats, shared action state) and the world state.
pub struct WorldHost<'a, X> {
    pub game: &'a mut Game,
    pub v: View<'a, X>,
    pub w: &'a mut WorldState,
}

impl<X: WorldPending> WorldHost<'_, X> {
    /// Runs `f` with population's context; the population state is lent
    /// to it for the call.
    pub fn population<R>(&mut self, f: impl FnOnce(&mut pop::Ctx<'_, Self>) -> R) -> R {
        if self.w.pop_lent {
            self.w
                .errors
                .push(WorldgenError::PopStateLent("population"));
        }
        let t = self.w.tables.clone();
        let info = self.w.pop_info;
        let mut st = std::mem::take(&mut self.w.pop);
        self.w.pop_lent = true;
        let r = f(&mut pop::Ctx {
            tables: &t.pop,
            info,
            state: &mut st,
            host: self,
        });
        self.w.pop = st;
        self.w.pop_lent = false;
        r
    }

    /// Runs `f` with init's tables.
    pub fn init<R>(&mut self, f: impl FnOnce(&init::Ctx<'_>, &mut Self) -> R) -> R {
        let t = self.w.tables.clone();
        f(&t.init(), self)
    }

    /// Runs `f` with the population state population passed into a seam
    /// call put back in place (init's region reads during allocation and
    /// boss marking), then lends it out again.
    pub(super) fn with_state<R>(
        &mut self,
        state: &mut PopState,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        std::mem::swap(state, &mut self.w.pop);
        let lent = std::mem::replace(&mut self.w.pop_lent, false);
        let r = f(self);
        std::mem::swap(state, &mut self.w.pop);
        self.w.pop_lent = lent;
        r
    }

    /// The regions, unless population holds them.
    pub(super) fn regions(&mut self, what: &'static str) -> Option<&mut Regions> {
        if self.w.pop_lent {
            self.w.errors.push(WorldgenError::PopStateLent(what));
            return None;
        }
        Some(&mut self.w.pop.regions)
    }

    /// The level id of an active room (`levels.txt` row), 0 without one.
    pub fn room_level_id(&self, room: RoomId) -> i32 {
        self.v
            .h
            .drlg
            .level_id(self.game, room)
            .map_or(0, |l| l as i32)
    }

    /// The room a unit stands in.
    pub fn room_of(&self, unit: UnitId) -> Option<RoomId> {
        self.game.lists.unit(unit).and_then(|e| e.room())
    }
}
