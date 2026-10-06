// Spec: specs/render/camera.md, specs/render/sprite-placement.md, specs/render/unit-composite.md
//! Original-behavior answers to the world-view hooks, one owner spec per
//! module. Plain Rust, integer math, no Bevy types.
//!
//! - [`camera`]: frame size and play area, world → client pixels, the two
//!   camera origins of a frame, unit and tile draw positions, view
//!   culling, screen shake, the time base (`render/camera.md`).
//! - [`placement`]: what a draw at (X, Y) covers, row clipping, DT1
//!   blocks, and the `DrawItem` top-left of an `IndexFrame`
//!   (`render/sprite-placement.md`).
//! - [`unit_composite`]: COF and component file names, directions and
//!   cels, component requests, colormap sources, COF box culling, extra
//!   offsets and single-cel units (`render/unit-composite.md`).
//! - [`view`]: [`view::OriginalView`], the `ViewRules` implementation that
//!   answers `tiles`, `unit_params` and `place` with the two modules above
//!   and hands every other hook (pose, component frame, draw keys,
//!   shading, blend, UI) to the rules it wraps.

pub mod camera;
pub mod placement;
pub mod unit_composite;
pub mod view;

#[cfg(test)]
mod tests;

pub use camera::{Camera, ClientPos, FrameSize, OpenMode, Shake, TileList, UnitPosition, ViewRect};
pub use placement::{Cel, Placed, RowPlan};
pub use view::{BlockRect, MapTile, OriginalView, ViewSource};
