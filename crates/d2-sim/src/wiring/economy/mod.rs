// Spec: specs/items/generation.md; specs/items/treasure.md §3; specs/items/properties.md §2, §4.2; specs/sim/stat-lists.md §4, §8.1, §9.3; specs/sim/units.md §3; specs/items/treasure.md §7, §8; specs/world/cube.md §7; specs/world/quests.md §9
//! The economy seams on their real providers:
//!
//! - [`game_fields`]: [`crate::items::ItemGame`] on [`GameFields`], the
//!   game-creation fields no written spec puts in [`crate::game::Game`]
//!   yet (game seed, difficulty, expansion, ladder, unique bits); also
//!   the treasure [`crate::treasure::GameFacts`].
//! - [`item_stats`]: [`crate::items::ItemStats`] on
//!   [`crate::stats::StatLists`] ([`UnitStats`]).
//! - [`item_units`]: item units: allocation (`sim/units.md` §3.1),
//!   creation (`items/generation.md` §3) and the per-item fields
//!   ([`ItemStore`], [`Economy`]); the request unit from unit and stat
//!   fields.
//! - [`treasure_items`]: [`crate::treasure::DropSink`] on item creation
//!   ([`ItemDrops`]); dropper and recipient from unit and stat fields.
//! - [`cube_items`]: [`crate::world::cube::CubeWorld`] on items, stats and
//!   units ([`EconomyCube`]); the rest stays a seam ([`CubeRest`]).
//! - [`quest_items`]: [`crate::world::quests::QuestWorld`] likewise
//!   ([`EconomyQuests`], [`QuestRest`]).
//! - [`death`]: a dead monster's drop (`treasure.md` §3) on the action
//!   wiring's units and DRLG, through [`ItemDrops`] ([`DeathDrops`],
//!   [`monster_death_drop`]); the free-spot search is the path
//!   provider's floor drop when it is on with its field, else a seam
//!   ([`FreeSpot`]); [`DropPlacer`] receives the economy (its hooks
//!   hold the rooms).
//! - [`chest_drop`]: the chest drop `D(Q)` of the object code
//!   (`treasure.md` §4) the same way, the object the dropper
//!   ([`object_chest_drop`]).
//! - [`drop_helpers`]: the object and quest drop helpers
//!   (`objects-2.md` §20: armor, weapon, gold, by source unit; the code
//!   drop) the same way.
//! - [`quest_host`]: the quests' world on the wired host ([`HostQuests`]:
//!   [`EconomyQuests`] with the object, level, interaction and identify
//!   calls the action wiring provides).
//! - [`quest_reward`]: the quest reward `0x005466B0` on the host's
//!   inventory model ([`QuestInventory`], [`QuestInv`]).
//! - [`quest_objects`]: the object module's quest routes on the quest
//!   control (init / operate functions by index, object event 7).
//! - [`quest_tick`]: tick step 8, the quest updater, on the same quest
//!   world, wrapped around a game's tick hooks ([`QuestTick`]).
//!
//! Status: wired, unverified (every spec involved is a draft).

pub mod chest_drop;
pub mod cube_items;
pub mod death;
pub mod drop_helpers;
pub mod game_fields;
pub mod item_records;
pub mod item_stats;
pub mod item_units;
pub mod quest_host;
pub mod quest_items;
pub mod quest_objects;
pub mod quest_reward;
pub mod quest_tick;
pub mod treasure_items;

#[cfg(test)]
mod tests;

pub use chest_drop::{object_chest_drop, NoSpot};
pub use cube_items::{CubeRest, EconomyCube};
pub use death::{monster_death_drop, DeathDrops, DropTables, FreeSpot};
pub use game_fields::GameFields;
pub use item_stats::{find_list, StatCtx, UnitStats};
pub use item_units::{Economy, ItemScope, ItemSpawn, ItemStore};
pub use quest_host::HostQuests;
pub use quest_items::{EconomyQuests, QuestDeferred, QuestRest};
pub use quest_objects::{QuestLoan, QuestObjectRun};
pub use quest_reward::{QuestInv, QuestInventory};
pub use quest_tick::{QuestTick, UnitSide};
pub use treasure_items::{dropper, recipient, DropPlacer, DropSpot, ItemDrops};

use crate::game::GameError;
use crate::items::{CreateError, Fatal};
use crate::units::modes::UnitError;
use crate::units::UnitId;

/// Errors of the economy wiring.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum EconomyError {
    #[error(transparent)]
    Create(#[from] CreateError),
    #[error(transparent)]
    Fatal(#[from] Fatal),
    #[error(transparent)]
    Unit(#[from] UnitError),
    #[error(transparent)]
    Game(#[from] GameError),
    /// The allocator rejected an item unit (it never does: `sim/units.md`
    /// §3.1 step 1 tests players and monsters only).
    #[error("item unit not allocated")]
    NotAllocated,
    /// The allocator and item creation stepped the game seed differently
    /// (both derive the unit and item seeds, `sim/rng.md` §5.3).
    #[error("allocation and creation stepped the game seed differently")]
    SeedMismatch,
    #[error("unit {0:?} is not an item of the store")]
    NotAnItem(UnitId),
    #[error("unit {0:?} has no unit record")]
    NoRecord(UnitId),
}
