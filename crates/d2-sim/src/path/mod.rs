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
//!
//! No function here draws (spec "Randomness").

pub mod collision;
pub mod coords;
pub mod footprint;
pub mod record;
pub mod tables;

#[cfg(test)]
mod tests;

pub use collision::CollisionRooms;
pub use footprint::{FootShape, Footprint, PathMotion, RemoveRule};
pub use record::{
    DynamicKind, DynamicPath, MonsterShape, ObjectShape, PathKind, PathPoint, StaticPath, UnitPath,
    UnitShape,
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
}
