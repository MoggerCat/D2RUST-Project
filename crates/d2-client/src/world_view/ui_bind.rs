// Spec: specs/client/ui.md (A2, A4), specs/client/render-pipeline.md (A3, A6)
//! The C8 UI core bound to the bridge: each frame the panels see the
//! client world model read-only ([`UiCtx`]), route the frame's input
//! events, hand their intents to the bridge (only the root forwards, §A2),
//! and draw requests that become scene items here.
//!
//! No panel is defined here: `ui.md` defines no d2rs-owned panel, and the
//! original's panels (art, layout, open/close rules) are `ui/panels.md`,
//! `ui/inventory.md` and `ui/text.md` (§B1–§B7). What a request draws and
//! where is a [`UiRules`] hook. Text goes through `ui::text::layout_text`
//! ([`text_sprites`]); what the request does not carry (font, layout
//! rules, glyph look) is a [`TextHooks`] hook.

use crate::assets::path::CanonicalPath;
use crate::bridge::link::ServerLink;
use crate::bridge::{Bridge, BridgeError};
use crate::composite::ComponentFrame;
use crate::frames::FrameSetKey;
use crate::scene::{BlendOp, DrawItem, DrawKey, ItemTag, Rect, ShadeChain};
use crate::ui::{
    layout_text, ImageRequest, NoTextRules, StringLookup, TextOpts, TextRequest, TextRules,
    TextStyle, UiCtx, UiDraw, UiEvent, UiInput, UiRoot,
};

use super::{Unspecified, ViewAssets, ViewError};

/// One sprite of a UI request: an image, or one glyph of laid-out text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiSprite {
    pub frame: ComponentFrame,
    /// Screen top-left.
    pub x: i32,
    pub y: i32,
    pub shade: ShadeChain,
    pub blend: BlendOp,
}

/// The original-behavior questions of UI drawing, one hook per question.
pub trait UiRules {
    /// TODO(spec: ui/panels.md) (§B1, §B6): the DC6 file and frame an
    /// [`crate::ui::ImageRef`] names, how the frame offsets combine with
    /// `req.at`, shading and blend. `image` reads a resident frame.
    fn ui_image(&self, req: &ImageRequest, assets: &ViewAssets) -> Result<UiSprite, ViewError>;

    /// TODO(spec: ui/text.md) (§B3): one sprite per drawn glyph, in
    /// drawing order. [`text_sprites`] answers it through `layout_text`
    /// from [`TextHooks`]; [`Unspecified`] does so.
    fn ui_text(&self, req: &TextRequest, assets: &ViewAssets) -> Result<Vec<UiSprite>, ViewError>;

    /// TODO(spec: render/draw-order.md) (§B6): the pass number of UI items.
    /// Every UI item gets key `(pass, 0, 0, 0)`, so the stable sort keeps
    /// emission order (draw order = list order).
    fn ui_pass(&self) -> Result<u32, ViewError>;
}

/// The font of a text style: its glyph table (`.tbl`, `formats/font-tbl.md`)
/// and the DC6 frame set its glyph frames index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextFont {
    pub table: CanonicalPath,
    pub glyphs: FrameSetKey,
}

/// What UI text needs besides the request and `layout_text`, one hook per
/// question.
pub trait TextHooks {
    /// TODO(spec: ui/text.md) (§B3): the font a style's `font` number
    /// names (glyph table and DC6).
    fn text_font(&self, style: TextStyle) -> Result<TextFont, ViewError>;

    /// TODO(spec: ui/text.md) (§B3): the layout rules (advance, wrap,
    /// alignment, color codes).
    fn text_rules(&self) -> &dyn TextRules;

    /// TODO(spec: ui/text.md, render/shading.md) (§B3): shade and blend of
    /// a glyph in text color `color` (the PL2 text-color map).
    fn glyph_look(&self, color: u16) -> Result<(ShadeChain, BlendOp), ViewError>;
}

/// UI text through `ui::text::layout_text` (`ui.md` §A3): the style's
/// font from `assets`, the rules' placements resolved to glyph records,
/// one sprite per placed glyph (its DC6 frame at the placed point) in
/// placement order. TODO(spec: ui/text.md) (§B3): the layout options are
/// the defaults until the spec says what the original's text call takes.
pub fn text_sprites<H: TextHooks + ?Sized>(
    hooks: &H,
    req: &TextRequest,
    assets: &ViewAssets,
) -> Result<Vec<UiSprite>, ViewError> {
    let font = hooks.text_font(req.style)?;
    let table = assets
        .fonts
        .get(&font.table)
        .ok_or_else(|| ViewError::FontMissing(font.table.clone()))?;
    let glyphs = layout_text(
        table,
        &req.text,
        req.at,
        req.style,
        &TextOpts::default(),
        hooks.text_rules(),
    )?;
    glyphs
        .into_iter()
        .map(|g| {
            let (shade, blend) = hooks.glyph_look(g.color)?;
            Ok(UiSprite {
                frame: ComponentFrame {
                    set: font.glyphs.clone(),
                    index: usize::from(g.frame),
                },
                x: g.at.x,
                y: g.at.y,
                shade,
                blend,
            })
        })
        .collect()
}

impl TextHooks for Unspecified {
    fn text_font(&self, _: TextStyle) -> Result<TextFont, ViewError> {
        Err(ViewError::unresolved("UI text font", "ui/text.md"))
    }

    fn text_rules(&self) -> &dyn TextRules {
        &NoTextRules
    }

    fn glyph_look(&self, _: u16) -> Result<(ShadeChain, BlendOp), ViewError> {
        Err(ViewError::unresolved("UI text color", "ui/text.md"))
    }
}

impl UiRules for Unspecified {
    fn ui_image(&self, _: &ImageRequest, _: &ViewAssets) -> Result<UiSprite, ViewError> {
        Err(ViewError::unresolved("UI image", "ui/panels.md"))
    }

    fn ui_text(&self, req: &TextRequest, assets: &ViewAssets) -> Result<Vec<UiSprite>, ViewError> {
        text_sprites(self, req, assets)
    }

    fn ui_pass(&self) -> Result<u32, ViewError> {
        Err(ViewError::unresolved("UI pass", "render/draw-order.md"))
    }
}

/// A UI rectangle as a scene clip rectangle (same integer edges).
pub fn clip_rect(r: crate::ui::Rect) -> Rect {
    Rect::new(r.x, r.y, u32::from(r.w), u32::from(r.h))
}

/// Appends the items of `draws` (emission order) to `items`. Asks for the
/// pass only when there is something to draw.
pub(super) fn ui_items<R: UiRules + ?Sized>(
    draws: &[UiDraw],
    rules: &R,
    assets: &ViewAssets,
    items: &mut Vec<DrawItem>,
) -> Result<(), ViewError> {
    if draws.is_empty() {
        return Ok(());
    }
    let key = DrawKey::new(rules.ui_pass()?, 0, 0, 0)?;
    for (index, d) in draws.iter().enumerate() {
        let at = |error| ViewError::Ui {
            index,
            error: Box::new(error),
        };
        let (sprites, clip) = match d {
            UiDraw::Image(r) => (vec![rules.ui_image(r, assets).map_err(at)?], r.clip),
            UiDraw::Text(r) => (rules.ui_text(r, assets).map_err(at)?, r.clip),
        };
        for s in sprites {
            let id = assets.id(&s.frame.set, s.frame.index).map_err(at)?;
            let mut item = DrawItem::new(id, s.x, s.y);
            item.clip = clip_rect(clip);
            item.shade = s.shade;
            item.blend = s.blend;
            item.key = key;
            item.tag = ItemTag::Ui(index as u32);
            items.push(item);
        }
    }
    Ok(())
}

/// The frame's UI input, filled by the input edge and drained by the root.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UiQueue(pub Vec<UiEvent>);

impl UiInput for UiQueue {
    fn drain(&mut self, out: &mut Vec<UiEvent>) {
        out.append(&mut self.0);
    }
}

/// What one UI frame did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UiFrame {
    /// Events no panel took, in order. Turning them into world intents is
    /// TODO(spec: ui/controls.md) (§B4): they are reported, never acted on.
    pub unhandled: Vec<UiEvent>,
    /// Intents handed to the bridge this frame.
    pub sent: usize,
    /// The open panels' draw requests, bottom-most panel first.
    pub draws: Vec<UiDraw>,
}

/// One UI frame against the bridge (§A2): route `input` with the world
/// model as context, forward the queued intents through the bridge (sent
/// this frame, drained by the server's next pump, `bridge.md` §8 rule 3),
/// then collect the draw requests. `tick` is the bridge frame count.
pub fn run_ui<L: ServerLink>(
    root: &mut UiRoot,
    input: &mut dyn UiInput,
    bridge: &mut Bridge<L>,
    strings: &dyn StringLookup,
) -> Result<UiFrame, BridgeError> {
    let unhandled = {
        let world = bridge.world();
        let ctx = UiCtx {
            tick: world.frames,
            world,
            strings,
        };
        root.pump(input, &ctx)
    };
    let sent = root.forward(bridge)?;
    let mut draws = Vec::new();
    let world = bridge.world();
    let ctx = UiCtx {
        tick: world.frames,
        world,
        strings,
    };
    root.draw(&ctx, &mut draws);
    Ok(UiFrame {
        unhandled,
        sent,
        draws,
    })
}
