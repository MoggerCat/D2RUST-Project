// Spec: specs/sim/path-placement.md
//! Unit position, collision footprints and placement
//! (`sim/path-placement.md`), and walk / run movement (`sim/pathing.md`).
//!
//! | Module | Spec |
//! |---|---|
//! | [`coords`] | §1 coordinates |
//! | [`record`] | §2 path records, §3 size / pattern / footprint mask |
//! | [`collision`] | §4 collision queries ([`collision::CollisionRooms`] seam) |
//! | [`footprint`] | §5 footprints, §6 moving a footprint ([`footprint::PathMotion`] seam) |
//! | [`tables`] | `sim/path-tables.tsv` |
//! | [`search`] | §7 nearest free point, §8 coarse free-box search |
//! | [`place`] | §9 floor drop, §10 placing a unit, §11 level spawn ([`place_seams`]: its seams) |
//! | [`warp`] | §12 warp tiles and arrival |
//! | [`history`] | §10 rule 7 player position history |
//! | [`walk`] | `sim/pathing.md`: walk / run requests, path compute, per-tick movement, missile paths, the 0x5F resync |
//!
//! No function of `path-placement.md` draws (spec "Randomness"); in
//! [`walk`], the concentration roll (`pathing.md` §1.4), the charged-bolt
//! path (§11.2) and the 0x5F lock (§1.6) draw on the unit seed.

pub mod collision;
pub mod coords;
pub mod footprint;
pub mod history;
pub mod place;
pub mod place_seams;
pub mod record;
pub mod search;
pub mod tables;
pub mod walk;
pub mod warp;

#[cfg(test)]
mod gap_tests;
#[cfg(test)]
mod tests;

pub use collision::CollisionRooms;
pub use coords::Point;
pub use footprint::{FootShape, Footprint, PathMotion, RemoveRule};
pub use record::{
    DynamicKind, DynamicPath, MonsterShape, ObjectShape, PathKind, PathPoint, StaticPath,
    TargetUnit, UnitPath, UnitShape,
};
pub use tables::PathTables;

/// Errors of the path code: bad embedded tables, and the original's
/// fatal asserts.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    #[error("path-tables.tsv: {0}")]
    Tsv(String),
    /// `0x00648CF0` fatal assert (`pathing.md` §2).
    #[error("path type {0}: fatal assert")]
    PathType(u32),
    /// `0x00650910` fatal assert: a non-zero point without a room (§6 rule 4).
    #[error("teleport to ({x}, {y}) without a room")]
    TeleportNoRoom { x: i32, y: i32 },
    /// `0x006490E0` fatal assert: a re-path budget above 255
    /// (`pathing.md` §9.10).
    #[error("re-path budget {0} > 255: fatal assert")]
    RepathBudget(u32),
}
