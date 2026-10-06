// Spec: specs/render/camera.md, specs/render/sprite-placement.md
//! Original-behavior answers to the world-view hooks, one owner spec per
//! module. Plain Rust, integer math, no Bevy types.
//!
//! - [`camera`]: frame size and play area, world → client pixels, the two
//!   camera origins of a frame, unit and tile draw positions, view
//!   culling, screen shake, the time base (`render/camera.md`).
//! - [`placement`]: what a draw at (X, Y) covers, row clipping, DT1
//!   blocks, and the `DrawItem` top-left of an `IndexFrame`
//!   (`render/sprite-placement.md`).
//! - [`view`]: [`view::OriginalView`], the `ViewRules` implementation that
//!   answers `tiles`, `unit_params` and `place` with the two modules above
//!   and hands every other hook (pose, component frame, draw keys,
//!   shading, blend, UI) to the rules it wraps.

pub mod camera;
pub mod placement;
pub mod view;

#[cfg(test)]
mod gaps_numbered_tests;
#[cfg(test)]
mod tests;

pub use camera::{Camera, ClientPos, FrameSize, OpenMode, Shake, TileList, UnitPosition, ViewRect};
pub use placement::{Cel, Placed, RowPlan};
pub use view::{BlockRect, MapTile, OriginalView, ViewSource};
