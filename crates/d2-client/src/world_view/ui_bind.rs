// Spec: specs/client/ui.md (A2, A4), specs/client/render-pipeline.md (A3, A6), specs/ui/text.md (§1, §4, §12, §13)
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
//! rules, glyph look) is a [`TextHooks`] hook, answered by `ui/text.md`
//! in [`OriginalTextHooks`].

use crate::assets::path::CanonicalPath;
use crate::bridge::link::ServerLink;
use crate::bridge::{Bridge, BridgeError};
use crate::composite::ComponentFrame;
use crate::frames::{FramePart, FrameSetKey};
use crate::rules::placement::draw_position;
use crate::scene::{BlendOp, DrawItem, DrawKey, ItemTag, MapId, MapTable, Rect, ShadeChain};
use crate::ui::text::{TEXT_COLORS, TEXT_COLOR_MAP_OFFSET, TEXT_DRAW_MODE};
use crate::ui::{
    font_info, layout_text, ImageRequest, OriginalText, StringLookup, TextRequest, TextRules,
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

    /// One sprite per drawn glyph, in drawing order (`ui/text.md`).
    /// [`text_sprites`] answers it through `layout_text` from
    /// [`TextHooks`]; [`Unspecified`] does so.
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
/// question. [`OriginalTextHooks`] answers them from `ui/text.md`.
pub trait TextHooks {
    /// The font a style's `font` id names: glyph table and DC6.
    fn text_font(&self, style: TextStyle) -> Result<TextFont, ViewError>;

    /// The layout rules (advance, line step, color codes, centering).
    fn text_rules(&self) -> &dyn TextRules;

    /// Shade and blend of a glyph in text color `color` drawn with draw
    /// mode `mode` (`ui/text.md` §4.3).
    fn glyph_look(&self, color: i32, mode: u8) -> Result<(ShadeChain, BlendOp), ViewError>;
}

/// UI text through `ui::text::layout_text` (`ui.md` §A3): the style's
/// font from `assets`, the rules' placements resolved to glyph records,
/// one sprite per placed glyph in placement order. A glyph is its DC6
/// frame drawn at the pen (`ui/text.md` §4.2): the sprite top-left is
/// `sprite-placement.md` §2/§8 of that frame. The request's `clip` is the
/// item clip; the original's text has no other clip (§12, CG2).
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
        &req.opts,
        hooks.text_rules(),
    )?;
    glyphs
        .into_iter()
        .map(|g| {
            let (shade, blend) = hooks.glyph_look(g.color, req.opts.mode())?;
            let index = usize::from(g.frame);
            let (x, y) = draw_position(assets.frame(&font.glyphs, index)?, g.at.x, g.at.y);
            Ok(UiSprite {
                frame: ComponentFrame {
                    set: font.glyphs.clone(),
                    index,
                },
                x,
                y,
                shade,
                blend,
            })
        })
        .collect()
}

/// The 12 text-color maps of the frame's act `pal.pl2` in the map table
/// (`ui/text.md` §4.4): `maps[k − 1]` is map `k`. Map 0 is never drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextColors {
    pub maps: [MapId; TEXT_COLORS - 1],
}

impl TextColors {
    /// Pushes maps 1–12 of `pl2` (file offset `439,847 + 256k`) onto
    /// `table`. The loader copies 13 maps, so the file must hold all 13.
    pub fn push(table: &mut MapTable, pl2: &[u8]) -> Result<TextColors, ViewError> {
        let end = TEXT_COLOR_MAP_OFFSET + 256 * TEXT_COLORS;
        let bytes = pl2
            .get(TEXT_COLOR_MAP_OFFSET..end)
            .ok_or_else(|| ViewError::Unresolved {
                what: "UI text color maps",
                spec: "ui/text.md",
                message: format!(
                "PL2 of {} bytes ends before its text-color maps (offset {TEXT_COLOR_MAP_OFFSET}, \
                 {} bytes)",
                pl2.len(),
                256 * TEXT_COLORS
            ),
            })?;
        let mut maps = [MapId(0); TEXT_COLORS - 1];
        for (k, map) in maps.iter_mut().enumerate() {
            let row = &bytes[256 * (k + 1)..256 * (k + 2)];
            *map = table.push(row.try_into().expect("256-byte row"));
        }
        Ok(TextColors { maps })
    }
}

/// The text hooks of `ui/text.md`: fonts from `text-fonts.tsv` (§1.3),
/// the [`OriginalText`] rules, glyph look per §4.3. `colors` are the
/// frame's text-color maps; without them only color 0 draws.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OriginalTextHooks {
    pub colors: Option<TextColors>,
}

/// The font of id `style.font` (`text-fonts.tsv`).
pub fn original_text_font(style: TextStyle) -> Result<TextFont, ViewError> {
    let info = font_info(style.font).ok_or_else(|| ViewError::Unresolved {
        what: "UI text font",
        spec: "ui/text.md",
        message: format!("font id {} is not one of the 14 fonts (0–13)", style.font),
    })?;
    let bad = |e: String| ViewError::Unresolved {
        what: "UI text font",
        spec: "ui/text.md",
        message: e,
    };
    Ok(TextFont {
        table: CanonicalPath::new(info.tbl_path).map_err(|e| bad(e.to_string()))?,
        glyphs: FrameSetKey::new(
            CanonicalPath::new(info.dc6_path)
                .map_err(|e| bad(e.to_string()))?
                .as_str(),
            FramePart::Dir(0),
        )
        .map_err(|e| bad(e.to_string()))?,
    })
}

/// Glyph look of `ui/text.md` §4.3: no light, blend none (draw mode 5),
/// remap = text-color map `k` (none for 0).
fn original_glyph_look(
    colors: Option<&TextColors>,
    color: i32,
    mode: u8,
) -> Result<(ShadeChain, BlendOp), ViewError> {
    if mode != TEXT_DRAW_MODE {
        return Err(ViewError::unresolved(
            "UI text draw mode",
            "render/blend-modes.md",
        ));
    }
    if color == 0 {
        return Ok((ShadeChain::EMPTY, BlendOp::Opaque));
    }
    if !(1..TEXT_COLORS as i32).contains(&color) {
        return Err(ViewError::Unresolved {
            what: "UI text color",
            spec: "ui/text.md",
            message: format!("color {color} outside 0–12 (open question 2)"),
        });
    }
    let colors = colors.ok_or_else(|| ViewError::Unresolved {
        what: "UI text color",
        spec: "ui/text.md",
        message: "the frame's PL2 text-color maps are not loaded".into(),
    })?;
    let map = colors.maps[color as usize - 1];
    Ok((ShadeChain::new(&[map])?, BlendOp::Opaque))
}

impl TextHooks for OriginalTextHooks {
    fn text_font(&self, style: TextStyle) -> Result<TextFont, ViewError> {
        original_text_font(style)
    }

    fn text_rules(&self) -> &dyn TextRules {
        &OriginalText
    }

    fn glyph_look(&self, color: i32, mode: u8) -> Result<(ShadeChain, BlendOp), ViewError> {
        original_glyph_look(self.colors.as_ref(), color, mode)
    }
}

/// `ui/text.md` without the frame's text-color maps (they are frame data,
/// not rules): colored glyphs are errors.
impl TextHooks for Unspecified {
    fn text_font(&self, style: TextStyle) -> Result<TextFont, ViewError> {
        original_text_font(style)
    }

    fn text_rules(&self) -> &dyn TextRules {
        &OriginalText
    }

    fn glyph_look(&self, color: i32, mode: u8) -> Result<(ShadeChain, BlendOp), ViewError> {
        original_glyph_look(None, color, mode)
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

#[cfg(test)]
mod text_tests {
    use d2_formats::font::{FontTable, Glyph};
    use d2_formats::palette::{Palette, Rgb};

    use super::*;
    use crate::frames::{FrameAnchor, FrameSet, IndexFrame};
    use crate::ui::{Point, TextOpts, FRAME};

    // Covers: specs/ui/text.md §1 r3, §13
    #[test]
    fn font_ids_name_the_tsv_files() {
        let f = original_text_font(TextStyle { font: 1, color: 0 }).unwrap();
        assert_eq!(f.table.as_str(), "data/local/font/latin/font16.tbl");
        assert_eq!(
            f.glyphs,
            FrameSetKey::new("data/local/font/latin/font16.dc6", FramePart::Dir(0)).unwrap()
        );
        assert!(original_text_font(TextStyle { font: 14, color: 0 }).is_err());
    }

    fn pl2() -> Vec<u8> {
        let mut p = vec![0u8; TEXT_COLOR_MAP_OFFSET + 256 * TEXT_COLORS];
        for k in 0..TEXT_COLORS {
            for s in 0..256 {
                p[TEXT_COLOR_MAP_OFFSET + 256 * k + s] = (k * 16 + s % 16) as u8;
            }
        }
        p
    }

    // Covers: specs/ui/text.md §4 r3, §4 r4
    #[test]
    fn glyph_look_is_text_color_map_k() {
        let mut table = MapTable::new();
        table.push([0xEE; 256]);
        let colors = TextColors::push(&mut table, &pl2()).unwrap();
        assert_eq!(table.len(), 13);
        for k in 1..TEXT_COLORS {
            let row = table.get(colors.maps[k - 1]).unwrap();
            assert_eq!(row[3], (k * 16 + 3) as u8, "map {k}");
        }
        let hooks = OriginalTextHooks {
            colors: Some(colors),
        };
        assert_eq!(
            hooks.glyph_look(0, 5).unwrap(),
            (ShadeChain::EMPTY, BlendOp::Opaque)
        );
        let (shade, blend) = hooks.glyph_look(3, 5).unwrap();
        assert_eq!(
            (shade.maps(), blend),
            (&[colors.maps[2]][..], BlendOp::Opaque)
        );
        // Outside 0–12 (open question 2), another draw mode, no maps.
        assert!(hooks.glyph_look(13, 5).is_err());
        assert!(hooks.glyph_look(-1, 5).is_err());
        assert!(hooks.glyph_look(0, 3).is_err());
        assert!(OriginalTextHooks::default().glyph_look(1, 5).is_err());
        assert_eq!(
            Unspecified.glyph_look(0, 5).unwrap(),
            (ShadeChain::EMPTY, BlendOp::Opaque)
        );
        // The file must hold all 13 maps.
        let short = &pl2()[..TEXT_COLOR_MAP_OFFSET + 256 * 12];
        assert!(TextColors::push(&mut MapTable::new(), short).is_err());
    }

    // Covers: specs/ui/text.md §4 r1, §4 r2
    #[test]
    fn glyph_cell_ends_at_the_pen_row() {
        let mut a = ViewAssets::new(Palette {
            colors: [Rgb::default(); 256],
        });
        let font = original_text_font(TextStyle { font: 1, color: 0 }).unwrap();
        let frame = IndexFrame {
            width: 14,
            height: 16,
            x_off: 0,
            y_off: 0,
            anchor: FrameAnchor::Bottom,
            pixels: vec![1; 14 * 16],
        };
        a.frames
            .insert(
                font.glyphs.clone(),
                FrameSet {
                    frames: vec![frame; 256],
                },
            )
            .unwrap();
        let glyphs = (0..256u16)
            .map(|i| Glyph {
                code: i,
                unknown1: 0,
                width: 12,
                height: 10,
                unknown2: 1,
                unknown3: 0,
                frame: i,
                unknown5: 0,
            })
            .collect();
        a.fonts.insert(
            font.table.clone(),
            FontTable {
                version: 1,
                unknown: 0,
                count: 256,
                height: 10,
                width: 0,
                glyphs,
            },
        );
        let req = TextRequest {
            text: vec![u16::from(b'A')],
            at: Point::new(104, 200),
            style: TextStyle { font: 1, color: 0 },
            opts: TextOpts::default(),
            clip: FRAME,
        };
        let s = text_sprites(&OriginalTextHooks::default(), &req, &a).unwrap();
        // Font16 `A` at pen (104, 200): columns 104–117, rows 185–200.
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].x, s[0].y, s[0].frame.index), (104, 185, 65));
    }
}
