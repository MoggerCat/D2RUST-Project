// Spec: specs/sim/tick.md §3, §5.6; specs/sim/units.md §5 (wiring of the action seams)
//! The "action" seams wired to their providers: combat on the unit
//! records and stat lists ([`combat`]); missiles and monster AI on
//! combat, units (modes, timer events) and the DRLG (rooms, collision)
//! ([`missiles`], [`ai`]); DRLG room activation on the act room lists of
//! [`crate::units::UnitLists`] ([`rooms`]); waypoints on the DRLG levels
//! ([`waypoints`]); and one dispatcher the tick runs ([`dispatch`]).
//!
//! Ownership: [`ActionSim`] holds a [`crate::units::dispatch::UnitSystem`] (unit records, stat
//! lists, unit tables) whose hooks are [`ActionHooks`] (DRLG, missile and
//! AI state, tables, combat lists). Timer events go through the unit
//! dispatch of `units.md` §5 (the handler tables of `tick.md` §5.6, with
//! the monster freeze drop); its hooks run the missile class handler
//! (missile events), the AI think (monster type 2) and the type-10 reset.
//! Each seam call builds a short-lived [`View`] (or [`combat::CombatView`])
//! over the parts it needs.
//!
//! Calls with no provider yet go to [`Pending`] (defaults: nothing).
//! Nothing here decides game behaviour: every rule stays in its module;
//! an adapter only maps one seam call to the provider's call.

pub mod ai;
pub mod combat;
pub mod dispatch;
pub mod missiles;
pub mod pending;
pub mod rooms;
pub mod units;
pub mod waypoints;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::sync::Arc;

use d2_data::tables::{Levels, Missiles as MissileRow};

use crate::combat::{CombatEntry, CombatTables};
use crate::drlg::{DrlgData, DrlgError, Dungeon, LevelTypes, TileSource};
use crate::game::Game;
use crate::missiles::MissileStore;
use crate::monsters::ai::{AiStore, GameInfo};
use crate::rng::Seed;
use crate::skills::SkillTables;
use crate::stats::StatLists;
use crate::units::hooks::{Sim, UnitData};
use crate::units::modes::UnitError;
use crate::units::record::Units;
use crate::units::UnitId;
use crate::world::waypoints::WaypointRecords;

pub use dispatch::ActionSim;
pub use pending::{NoPending, Pending, SkillEvent};

/// The tables the action modules read (typed `d2_data` records).
#[derive(Debug, Clone)]
pub struct ActionTables {
    /// `missiles.txt` rows.
    pub missiles: Vec<MissileRow>,
    pub skills: SkillTables,
    /// Also the monstats / monstats2 the AI reads.
    pub combat: CombatTables,
    /// `levels.txt` rows (AI).
    pub levels: Vec<Levels>,
    /// `Sk1mode..Sk3mode` per monstats row ([`crate::monsters::ai::skill_modes`]).
    pub skill_modes: Vec<[u8; 3]>,
}

/// The DRLG side of a game: the acts' DRLGs and their services.
pub struct DrlgWorld {
    pub dungeon: Dungeon,
    pub data: Arc<DrlgData>,
    pub tiles: Box<dyn TileSource>,
    pub types: Box<dyn LevelTypes>,
}

/// What went wrong in an adapter (fatal assertions of 1.14d, API misuse,
/// a provider's error), in order.
#[derive(Debug, PartialEq, Eq)]
pub enum WiringError {
    /// A store was needed while it was lent out (a missile or AI call
    /// re-entered its own module).
    Reentrant(&'static str),
    Drlg(DrlgError),
    Unit(UnitError),
}

/// The [`crate::units::hooks::UnitHooks`] of [`ActionSim`]'s unit system
/// and the state every action adapter shares.
pub struct ActionHooks<X> {
    pub tables: Arc<ActionTables>,
    pub drlg: DrlgWorld,
    /// Lent to the missile code during a missile call (`None` then).
    pub missiles: Option<MissileStore>,
    /// Lent to the AI code during a think (`None` then).
    pub ai: Option<AiStore>,
    /// Game fields the AI reads (game +0x6A, +0x74, +0x6D).
    pub ai_info: GameInfo,
    /// Combat lists (unit +0xAC, `damage.md` §3 step 3), first = newest.
    pub combat_lists: BTreeMap<UnitId, Vec<CombatEntry>>,
    /// The process-wide element hit-class byte `0x0088CAD0`.
    pub hit_class: u8,
    /// The game seed of `rng.md` §5.3 (unit allocation).
    pub game_seed: Seed,
    /// Waypoint records per player (player data +0x1C, `waypoints.md` §2).
    pub waypoints: BTreeMap<UnitId, WaypointRecords>,
    /// Seams with no provider yet.
    pub x: X,
    /// Scratch seed handed out for a unit without a record (an error is
    /// logged with it).
    pub orphan_seed: Seed,
    pub errors: Vec<WiringError>,
}

impl<X> ActionHooks<X> {
    pub fn new(tables: Arc<ActionTables>, drlg: DrlgWorld, game_seed: Seed, x: X) -> Self {
        Self {
            tables,
            drlg,
            missiles: Some(MissileStore::new()),
            ai: Some(AiStore::new()),
            ai_info: GameInfo::default(),
            combat_lists: BTreeMap::new(),
            hit_class: 0,
            game_seed,
            waypoints: BTreeMap::new(),
            x,
            orphan_seed: Seed::init(),
            errors: Vec::new(),
        }
    }

    /// The missile store (outside a missile call).
    pub fn missile_store(&self) -> &MissileStore {
        self.missiles.as_ref().expect("not lent out")
    }

    /// The AI store (outside a think).
    pub fn ai_store(&mut self) -> &mut AiStore {
        self.ai.as_mut().expect("not lent out")
    }
}

/// The unit side of a seam call: unit records, stat lists, unit tables
/// and the shared state. Missile and AI seams are implemented on it
/// ([`missiles`], [`ai`]); the game comes with each call.
pub struct View<'a, X> {
    pub units: &'a mut Units,
    pub stats: &'a mut StatLists,
    pub data: &'a UnitData,
    pub h: &'a mut ActionHooks<X>,
}

impl<'a, X> View<'a, X> {
    /// A view over a [`Sim`]'s parts (the game stays with the caller).
    pub fn of(
        units: &'a mut Units,
        stats: &'a mut StatLists,
        data: &'a UnitData,
        h: &'a mut ActionHooks<X>,
    ) -> Self {
        Self {
            units,
            stats,
            data,
            h,
        }
    }

    /// The [`Sim`] of a unit operation on `game`.
    pub fn sim<'b>(&'b mut self, game: &'b mut Game) -> Sim<'b> {
        Sim {
            game,
            units: self.units,
            stats: self.stats,
            data: self.data,
        }
    }
}
