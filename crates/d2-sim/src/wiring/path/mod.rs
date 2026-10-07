// Spec: specs/sim/path-placement.md §2–§12; specs/sim/pathing.md §1, §9 (wiring of the path seams)
//! The path seams wired to `d2_sim::path`: unit path records kept with
//! the units, collision rooms on the DRLG, walk / run (`path::walk`) on
//! the unit system, and placement / warps (`path::place`, `path::warp`)
//! on the DRLG levels.
//!
//! - [`rooms`]: [`crate::path::CollisionRooms`] on
//!   [`crate::wiring::action::DrlgWorld`].
//! - [`units`]: the unit path record of `units.md` §2 (+0x2C) in
//!   [`PathState`]; the path part of `SUNIT_Add` (§2.5) at allocation,
//!   size and shape (§3), footprints (§5), the path setters the missile
//!   and AI adapters call, and the path free at removal.
//! - [`walk`]: [`PathCtx`], the one context `path::walk` runs on
//!   (`PathWorld` + `WalkUnits`) and `path::footprint::PathMotion`;
//!   the walk request, the per-tick player step (`tick.md` §3 step 4,
//!   event 0) and the unit step.
//! - [`missiles`]: the missile bodies' path seams (target point and
//!   position, set type, step counts, teleport).
//! - [`monsters`]: monster walk / run: the path part of the monster mode
//!   set, a monster mode's velocity, the walk event 0 and the mode end.
//! - [`place`]: `CollisionView`, `PlaceHost` and `LevelView` on a shared
//!   [`PathCtx`]; placing a unit (§10), the level warp (§11) and the
//!   floor drop (§9).
//!
//! The provider is opt-in per game: [`crate::wiring::action::ActionHooks::paths`]
//! is `None` until a host calls `enable_paths`; then every path seam of
//! the action and worldgen adapters is answered here, and
//! [`crate::wiring::action::Pending`]'s path methods are no longer
//! called. Without it the adapters keep their `Pending` answers (hosts
//! and tests that fake positions). Nothing here decides game behaviour:
//! each adapter maps one seam call to one call of the path code.

pub mod missiles;
pub mod monsters;
pub mod place;
pub mod rooms;
pub mod units;
pub mod walk;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod motion_tests;

#[cfg(test)]
mod init_cb_tests;

#[cfg(test)]
mod mutant_tests;

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::path::history::PositionHistory;
use crate::path::search::ExpField;
use crate::path::{PathTables, UnitPath};
use crate::units::UnitId;

pub use walk::PathCtx;

/// The path records of a game's units (unit +0x2C, `units.md` §2, freed
/// at removal) and the constant tables of `sim/path-tables.tsv`.
#[derive(Clone, Debug)]
pub struct PathState {
    pub tables: Arc<PathTables>,
    pub records: BTreeMap<UnitId, UnitPath>,
    /// `data\global\ExpField.D2` (d2data.mpq, `path-placement.md` §7.3),
    /// loaded by the host: the floor drop's walk-back field. `None`: the
    /// floor-drop seams keep their pending answers.
    pub field: Option<Arc<ExpField>>,
    /// The players' position history (player data +0xA0..+0x14C,
    /// `path-placement.md` §10 rule 7), written by the placement and the
    /// walk step; read by monster AI (`monsters/ai.md`).
    pub history: BTreeMap<UnitId, PositionHistory>,
    /// The AI's mode request staged for the monster mode set that follows
    /// (the request record's target, `monsters/ai.md` §7.1), taken by
    /// [`ActionHooks::monster_path_setup`](crate::wiring::action::ActionHooks).
    pub mode_request: Option<(UnitId, crate::monsters::ai::ModeTarget)>,
    /// The monster's velocity request (`monsters/ai.md` §7.3) staged by
    /// an AI mode request for the movement set-up that consumes it
    /// (§7.5 rule 4.1; the AI store is lent out during the think).
    pub mode_velocity: Option<(UnitId, crate::monsters::ai::VelocityRequest)>,
    /// The movement set-up's fields of the AI param record (monster data
    /// +0x2C, `monsters/ai.md` §7.5 rules 4–5), per monster.
    pub setup: BTreeMap<UnitId, monsters::MoveSetup>,
    /// The staged request's path-type byte (+0x15) when the AI
    /// overwrote the builder's value (`ai.md` §7.1).
    pub mode_request_byte: Option<(UnitId, u8)>,
}

impl PathState {
    /// No records, the embedded tables.
    pub fn new() -> Result<Self, crate::path::PathError> {
        Ok(Self {
            tables: Arc::new(PathTables::spec()?),
            records: BTreeMap::new(),
            field: None,
            history: BTreeMap::new(),
            mode_request: None,
            mode_velocity: None,
            setup: BTreeMap::new(),
            mode_request_byte: None,
        })
    }

    /// The unit's path record (`None`: unit +0x2C null).
    pub fn record(&self, unit: UnitId) -> Option<&UnitPath> {
        self.records.get(&unit)
    }

    /// The unit's dynamic path, if it has one.
    pub fn dynamic(&self, unit: UnitId) -> Option<&crate::path::DynamicPath> {
        match self.records.get(&unit)? {
            UnitPath::Dynamic(p) => Some(p),
            UnitPath::Static(_) => None,
        }
    }

    /// The unit's dynamic path, mutable.
    pub fn dynamic_mut(&mut self, unit: UnitId) -> Option<&mut crate::path::DynamicPath> {
        match self.records.get_mut(&unit)? {
            UnitPath::Dynamic(p) => Some(p),
            UnitPath::Static(_) => None,
        }
    }
}
