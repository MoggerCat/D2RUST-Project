// Spec: specs/client/render-pipeline.md
// Spec: specs/render/composition.md
//! Scene: the draw list and the CPU reference compositor (§A3–A8, bins of
//! §A9). Plain Rust, no Bevy types, integer math only: the GPU compositor
//! (`render`) and the verify harness consume exactly these types.
//!
//! Flow: build `Vec<DrawItem>` → [`order`] (stable sort by [`DrawKey`]) →
//! [`cpu::compose`] (or [`bins::bin`] + [`cpu::compose_binned`], the GPU's
//! model) → index framebuffer → [`cpu::to_rgba`].
//!
//! Original-game behavior (§B) is not decided here. Each place that needs
//! it is marked `TODO(spec: …)` and does the narrowest neutral thing,
//! documented on the item it concerns.

pub mod bins;
pub mod cpu;
pub mod frame;
pub mod item;
pub mod order;

#[cfg(test)]
mod tests;

pub use bins::{bin, Bins};
pub use cpu::{
    compose, compose_binned, compose_binned_frame, compose_frame, compose_rgba, to_rgba,
};
pub use frame::{present_palette, FrameCycle, FramePlan, PL2_PALETTE_BYTES, UNCLEARED_ROWS};
pub use item::{
    BlendOp, DrawItem, FrameId, FrameImage, FrameSource, FrameView, ItemTag, MapId, MapTable,
    PixelTables, ShadeChain,
};
pub use order::{order, DrawKey};

/// Width of the composed frame (EARLY_DECISIONS 9).
pub const FRAME_WIDTH: u32 = 800;
/// Height of the composed frame.
pub const FRAME_HEIGHT: u32 = 600;
/// Side of a square bin in pixels (§A9).
pub const BIN_SIZE: u32 = 32;
/// Longest shade chain (§A4).
pub const MAX_SHADE: usize = 4;

/// An integer screen rectangle, y down. Edges are computed in `i64`, so no
/// position or size overflows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    /// The full 800×600 frame at the origin.
    pub const FRAME: Rect = Rect {
        x: 0,
        y: 0,
        width: FRAME_WIDTH,
        height: FRAME_HEIGHT,
    };

    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    fn right(&self) -> i64 {
        i64::from(self.x) + i64::from(self.width)
    }

    fn bottom(&self) -> i64 {
        i64::from(self.y) + i64::from(self.height)
    }

    /// The overlap of two rectangles, or `None` when it is empty.
    pub fn intersect(&self, other: &Rect) -> Option<Rect> {
        let x0 = self.x.max(other.x);
        let y0 = self.y.max(other.y);
        let x1 = self.right().min(other.right());
        let y1 = self.bottom().min(other.bottom());
        if x1 <= i64::from(x0) || y1 <= i64::from(y0) {
            return None;
        }
        // Width and height are bounded by one input's u32 size.
        Some(Rect {
            x: x0,
            y: y0,
            width: (x1 - i64::from(x0)) as u32,
            height: (y1 - i64::from(y0)) as u32,
        })
    }

    /// Whether the screen pixel `(x, y)` lies inside.
    pub fn contains(&self, x: i64, y: i64) -> bool {
        x >= i64::from(self.x) && x < self.right() && y >= i64::from(self.y) && y < self.bottom()
    }

    /// A view's pixels must be screen points (i32): items are placed in
    /// i32, and bin rectangles ([`Bins::rect`]) are `Rect`s.
    pub(crate) fn check_view(&self) -> Result<(), SceneError> {
        let end = i64::from(i32::MAX) + 1;
        if self.right() > end || self.bottom() > end {
            return Err(SceneError::View(*self));
        }
        Ok(())
    }
}

/// Errors of draw-list construction and composition. Inputs are strict
/// (M07): anything the compositor cannot draw exactly is an error, never a
/// skipped or defaulted draw.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SceneError {
    #[error("shade chain of {len} maps (at most {MAX_SHADE})")]
    ShadeChainTooLong { len: usize },
    #[error("draw key field {field} = {value} exceeds {max}")]
    KeyField {
        field: &'static str,
        value: u32,
        max: u32,
    },
    #[error("frame {0:?} is not resident")]
    FrameMissing(FrameId),
    #[error("frame {width}x{height} has {len} pixels")]
    FrameSize { width: u32, height: u32, len: usize },
    #[error("map {0:?} is not in the map table")]
    MapMissing(MapId),
    #[error("blend table at {0:?} needs 256 rows in the map table")]
    BlendTable(MapId),
    #[error("flip_x is reserved until an owner spec defines it")]
    FlipX,
    #[error("view {0:?} has pixels past the i32 screen range")]
    View(Rect),
    #[error("bins were built for {built:?}, composing {view:?}")]
    BinsView { built: Rect, view: Rect },
    #[error("bins were built for {built} items, composing {items}")]
    BinsItems { built: usize, items: usize },
    #[error("base framebuffer has {len} bytes, the view {pixels} pixels")]
    BaseSize { len: usize, pixels: u64 },
    #[error(
        "framebuffer height {height} leaves no rows for the frame clear (more than 47 needed)"
    )]
    FramebufferHeight { height: u32 },
    #[error("frame plan {0:?} does not fit the framebuffer or the frame cycle")]
    FramePlan(frame::FramePlan),
    #[error("PL2 data of {len} bytes has no 1,024-byte palette")]
    Pl2Size { len: usize },
    #[error("draw item {index}: {error}")]
    Item {
        index: usize,
        error: Box<SceneError>,
    },
}

impl SceneError {
    fn at(self, index: usize) -> SceneError {
        SceneError::Item {
            index,
            error: Box::new(self),
        }
    }
}
