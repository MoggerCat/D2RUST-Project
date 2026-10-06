// Spec: specs/client/ui.md
//! Plain draw requests a panel emits (spec §A2 "panels produce
//! `DrawItem`s").
//!
//! The scene's `DrawItem` (`render-pipeline.md` §A3) belongs to
//! `d2-client::scene` (task C4). Until it lands, panels emit these neutral
//! requests through [`UiDrawSink`]; the scene side implements the sink and
//! turns each request into `DrawItem`s in pass `ui` (§A6), keeping the
//! emission order (draw order is list order; equal keys keep build order).

use super::geom::{Point, Rect};

/// A UI image: a frame of a file the panel registry names. Both are
/// opaque ids here; which DC6 files and frames each panel draws is
/// `TODO(spec: ui/panels.md §B1)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageRef {
    pub file: u32,
    pub frame: u32,
}

/// Opaque font and text-color ids; their meaning (font file, PL2 text
/// color map) is `TODO(spec: ui/text.md §B3)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextStyle {
    pub font: u16,
    pub color: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageRequest {
    pub image: ImageRef,
    /// The widget's placement point. How a frame's own offsets combine with
    /// it is `TODO(spec: ui/panels.md §B1)`; the sink resolves it.
    pub at: Point,
    pub clip: Rect,
}

/// Text as UTF-16 code units, the way the string tables hold them
/// (spec §A3); layout (advance, wrap, alignment, color codes) is done by
/// the sink through `layout_text` (§A3, `TODO(spec: ui/text.md §B3)`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextRequest {
    pub text: Vec<u16>,
    pub at: Point,
    pub style: TextStyle,
    pub clip: Rect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiDraw {
    Image(ImageRequest),
    Text(TextRequest),
}

/// Receives a panel's draw requests in order.
pub trait UiDrawSink {
    fn push(&mut self, d: UiDraw);
}

impl UiDrawSink for Vec<UiDraw> {
    fn push(&mut self, d: UiDraw) {
        Vec::push(self, d);
    }
}
