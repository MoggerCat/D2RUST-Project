// Spec: specs/sim/pathing.md
//! Walk and run: the mode request (C→S 0x01–0x04), path types 1, 2 and 7,
//! path compute, target preparation, toward / straight / A*, velocity,
//! direction and facing, per-tick movement, and the S→C byte builders of
//! §10.
//!
//! Seams: [`seams::PathWorld`] (path records, collision, footprints,
//! rooms; provider: the path core of `sim/path-placement.md` §1–§6) and
//! [`seams::WalkUnits`] (units, modes, timers, stats, skills, messages).
//! Integer and 16.16 fixed-point arithmetic only (CLAUDE.md rule 6); the
//! x87 target lead (pathing.md open question 4) is a seam.

pub mod find;
pub mod geom;
pub mod messages;
pub mod request;
pub mod seams;
pub mod step;
pub mod tables;
pub mod velocity;

#[cfg(test)]
mod mutant_tests;
#[cfg(test)]
mod tests;

pub use request::{handle_message, request, Outcome, WalkTarget};
pub use seams::{PathInfo, PathWorld, Point, WalkError, WalkPath, WalkUnits};
pub use step::{Step, Walk};
pub use tables::PathTables;
