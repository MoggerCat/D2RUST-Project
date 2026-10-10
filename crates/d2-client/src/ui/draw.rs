// Spec: specs/client/ui.md, specs/ui/panels.md (§1.4, §1.6), specs/render/blend-modes.md (§1, §8 r2)
//! Plain draw requests a panel emits (spec §A2 "panels produce
//! `DrawItem`s").
//!
//! The scene's `DrawItem` (`render-pipeline.md` §A3) belongs to
//! `d2-client::scene` (task C4). Until it lands, panels emit these neutral
//! requests through [`UiDrawSink`]; the scene side implements the sink and
//! turns each request into `DrawItem`s in pass `ui` (§A6), keeping the
//! emission order (draw order is list order; equal keys keep build order).

use super::geom::{Point, Rect};
use super::text::TextOpts;

/// A UI image: a frame of a file the panel registry names. Both are
/// opaque ids here; the original panels' files and frames are
/// `ui/panels.md` §16.2, named by [`super::panels::UiFiles`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ImageRef {
    pub file: u32,
    pub frame: u32,
}

/// Font id 0–13 (`ui/text.md` §1, `text-fonts.tsv`) and the caller's
/// text color `k` (§5: 0 = no remap, else PL2 text-color map `k`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextStyle {
    pub font: u16,
    pub color: u16,
}

/// The palette argument of a cel draw (`ui/panels.md` §1.6): the plain
/// cel draw `0x004F6480` has none; the colored cel draw `0x004F64B0`
/// passes `k`, read as pointer `+0xD0 + 4k` of the palette-table block
/// (`ui/text.md` §4.3–§4.5); an item picture passes the item's colour
/// map (`render/shading.md` §6 r4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Remap {
    /// No remap (`P` none).
    #[default]
    None,
    /// The colored cel draw's `k`: 0 = no remap, 1–12 = PL2 text-colour
    /// map `k`, negative = the block entries of `ui/text.md` §4.5.
    Palette(i32),
    /// Map `c` (0–20) of item palette file `t` (1–8), as
    /// `render/shading.md` §6 r4 selects it (`rules::shading::item_color`).
    ItemColor { t: u8, c: u8 },
}

/// How a cel is written (`render/blend-modes.md` §1, `ui/panels.md`
/// §1.4): draw mode 0–7 (5 opaque; other values draw as 5) and the
/// palette argument. The UI's cel draws pass light 0xFF (no `L`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CelLook {
    pub mode: u8,
    pub remap: Remap,
}

impl CelLook {
    /// The plain panel cel draw: mode 5, no remap (`ui/panels.md` §1.4).
    pub const PLAIN: CelLook = CelLook {
        mode: DRAW_MODE_OPAQUE,
        remap: Remap::None,
    };
}

impl Default for CelLook {
    fn default() -> Self {
        CelLook::PLAIN
    }
}

/// Draw mode 5, the opaque copy (`render/blend-modes.md` §1).
pub const DRAW_MODE_OPAQUE: u8 = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageRequest {
    pub image: ImageRef,
    /// The cel draw position (X, Y) (`ui/panels.md` §1.3): the frame covers
    /// columns `X … X + w − 1`, rows `Y − h + 1 … Y`
    /// (`sprite-placement.md` §2); the sink resolves it.
    pub at: Point,
    pub clip: Rect,
    /// Draw mode and remap ([`CelLook::PLAIN`] for the plain cel draw).
    pub look: CelLook,
    /// The 1.14d cel wrapper the original calls for it (the rendering
    /// facts' `op`, `tools/facts-render.md` §5 r18).
    pub call: CelCall,
}

/// Which 1.14d cel draw wrapper a UI cel is (`render/capture.md` §3.5
/// wrapper names; `tools/facts-render.md` §5 r18): the plain draw, the
/// row-window draw of the globes, the colour (palette) draw of the skill
/// icons and font glyphs, the clipped draw of the automap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CelCall {
    #[default]
    Draw,
    Ex,
    Color,
    Clipped,
}

/// What the rendering facts print of a UI draw besides its wrapper: the
/// draw mode and the palette / colour argument (`tools/facts-render.md`
/// §2 r3), kept per UI draw in `WorldFrame::ui_calls`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiCelInfo {
    pub call: CelCall,
    pub mode: u8,
    /// The palette argument: `Some(0)` null, `Some(k)` the colour index of
    /// the colour draw; `None` not known (an item palette pointer).
    pub pal: Option<i32>,
    /// A text draw: each glyph's colour index is read from its shade map.
    pub text: bool,
}

impl CelCall {
    /// The `draws.tsv` op name.
    pub fn op(self) -> &'static str {
        match self {
            CelCall::Draw => "CelDraw",
            CelCall::Ex => "CelDrawEx",
            CelCall::Color => "CelDrawColor",
            CelCall::Clipped => "CelDrawClipped",
        }
    }
}

/// A filled rectangle: the arguments of `D2GFX_DrawRectangle`
/// (`0x004F6300`, `render/blend-modes.md` §8 r2) as the caller passes
/// them. Columns `x0 … x1 − 1`, rows `y0 … y1 − 1` after the clamp to the
/// surface; `color` is a palette index; `mode` picks the write (blend
/// kind 0: `color`, 1: `T[d]`, 2: `T[256·d + color]`). No clip: the
/// original clamps to the surface only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RectRequest {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub color: u8,
    pub mode: u8,
}

impl RectRequest {
    /// The UI's rectangle primitive `0x0046EFD0(x, y, w, h, color, mode)`
    /// = `DrawRectangle(x, y, x + w, y + h, color, mode)`
    /// (`ui/control-panel.md` §5 r4, `ui/inventory.md` §2 r2).
    pub fn sized(x: i32, y: i32, w: i32, h: i32, color: u8, mode: u8) -> Self {
        RectRequest {
            x0: x,
            y0: y,
            x1: x + w,
            y1: y + h,
            color,
            mode,
        }
    }
}

/// Text as UTF-16 code units, the way the string tables hold them
/// (spec §A3); layout is done by the sink through
/// [`super::text::layout_text`] (`ui/text.md` §5–§9). `at` is the pen: the
/// bottom row of the first-drawn line (`ui/text.md` §4.2). `opts` is the
/// text call and its arguments (§7–§9). The original's text calls take no
/// clip rectangle (`ui/text.md` §12, decision CG2): `clip` is the frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextRequest {
    pub text: Vec<u16>,
    pub at: Point,
    pub style: TextStyle,
    pub opts: TextOpts,
    pub clip: Rect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiDraw {
    Image(ImageRequest),
    Text(TextRequest),
    Rect(RectRequest),
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
