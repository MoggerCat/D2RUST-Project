// Spec: specs/client/ui.md
//! UI core (spec §A2–§A4): our integer-pixel panel framework, not
//! `bevy_ui` (§A1). Panels live in a [`UiRoot`] in a fixed order, answer
//! integer hit tests, take [`UiEvent`]s in 800×600 frame coordinates and
//! emit plain draw requests ([`UiDraw`]) to a [`UiDrawSink`]. A panel never
//! decides an outcome: a click becomes a [`ClientIntent`] (an encoded C→S
//! message) that only the root forwards to the bridge.
//!
//! Everything here is plain Rust without Bevy types except [`edge`], the
//! input edge that reads the cursor from a Bevy `Window`.
//!
//! Original behavior (panel art and layout, open/close rules, text layout,
//! grid cells, cursor; spec §B) is not written here: each place that needs
//! it carries a `TODO(spec: <owner spec>)` hook with the narrowest neutral
//! behavior.

pub mod automap;
pub mod cursor;
pub mod draw;
pub mod edge;
pub mod edit_box;
pub mod frame;
pub mod geom;
pub mod gold;
pub mod inv_grid;
pub mod layout;
pub mod original;
pub mod panel;
pub mod panels;
pub mod root;
pub mod states;
pub mod text;
pub mod wformat;
pub mod widget;

#[cfg(test)]
mod tests;

pub use draw::{ImageRef, ImageRequest, TextRequest, TextStyle, UiDraw, UiDrawSink};
pub use frame::{FrameError, FramePos, Presentation};
pub use geom::{Point, Rect, FRAME, FRAME_H, FRAME_W};
pub use panel::{
    ActionId, ClientIntent, NoStrings, Panel, PanelId, PointerButton, StringLookup, UiCtx, UiEvent,
    UiInput, UiResponse, WidgetId,
};
pub use root::{NoPanelRules, PanelRules, Routed, UiError, UiHit, UiRoot};
pub use text::{
    font_info, layout_text, FontInfo, GlyphDraw, GlyphLookup, GlyphPlacement, OriginalText,
    TextError, TextOpts, TextRules, FONTS,
};
