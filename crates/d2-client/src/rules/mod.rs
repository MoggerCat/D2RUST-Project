// Spec: specs/render/camera.md, specs/render/sprite-placement.md, specs/render/draw-order.md, specs/render/unit-composite.md, specs/render/shading.md, specs/render/blend-modes.md, specs/render/lighting.md, specs/monsters/umod-callbacks.md (§28.1)
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
//! - [`draw_order`]: the draw-cell grid, its lists, the world passes and
//!   the draw keys of a frame (`render/draw-order.md`), wired in through
//!   [`draw_order::source::OrderedSource`].
//! - [`shading`]: the act palette-table block as map-table rows (light
//!   maps, the computed highlight and red maps, remaps), cel light maps,
//!   DT1 wall and floor block light (`render/shading.md`).
//! - [`blend`]: draw mode → blend table and the pixel ops of a cel
//!   (through `scene::PixelTables`), component, missile, item and overlay
//!   draw modes, shadows, translucent walls (`render/blend-modes.md`).
//! - [`umod_hooks`]: the client umod hook table and its dispatch order
//!   (`monsters/umod-callbacks.md` §28.1).
//! - [`lighting`]: the light map, light records and sources, the day /
//!   night environment and the light value of each draw
//!   (`render/lighting.md`).

pub mod blend;
pub mod camera;
pub mod draw_order;
pub mod lighting;
pub mod placement;
pub mod shading;
pub mod umod_hooks;
pub mod unit_composite;
pub mod view;

#[cfg(test)]
mod blend_gdi_tests;
#[cfg(test)]
mod gaps_numbered_tests;
#[cfg(test)]
mod shading_blend_tests;
#[cfg(test)]
mod shading_shift_tests;
#[cfg(test)]
mod tests;

pub use camera::{Camera, ClientPos, FrameSize, OpenMode, Shake, TileList, UnitPosition, ViewRect};
pub use placement::{Cel, Placed, RowPlan};
pub use view::{BlockRect, BlockShade, MapTile, OriginalView, ViewSource};
