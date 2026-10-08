// Spec: specs/ui/frontend-menus.md (§F1.1 r5 button label), specs/ui/text.md (§3, §6, §7)
//! Glyph quads of a front-end text item: the host's text path. The layout
//! is the in-game one ([`layout_text`] with [`OriginalText`]); nothing is
//! copied. A button label is centered in the button and its baseline is
//! `y − (h − text_height)/2 + k`, +2 while pressed (§F1.1 r5).

use d2_formats::font::FontTable;

use super::{control, DrawItem};
use crate::ui::draw::TextStyle;
use crate::ui::geom::Point;
use crate::ui::text::{layout_text, text_height, GlyphLookup, OriginalText, TextOpts};

/// One glyph to draw: frame `frame` of font `font`'s DC6, whose bottom row
/// is the pen row `at.y` (`ui/text.md` §4.2); colour `color` (0: none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphQuad {
    pub font: u16,
    pub frame: u16,
    pub at: Point,
    pub color: i32,
}

/// The glyphs of a [`DrawItem::Text`]; empty for other items, a missing
/// font or a string without text. `resolve` gives the UTF-16 text of a
/// string id (used when the item carries no literal text).
pub fn text_quads(
    item: &DrawItem,
    resolve: &dyn Fn(u32) -> Vec<u16>,
    table: &dyn Fn(u16) -> Option<FontTable>,
) -> Vec<GlyphQuad> {
    let DrawItem::Text {
        string_id,
        text,
        font,
        at,
        label,
    } = item
    else {
        return Vec::new();
    };
    let units: Vec<u16> = if text.is_empty() {
        resolve(*string_id)
    } else {
        text.encode_utf16().collect()
    };
    let Some(t) = table(*font) else {
        return Vec::new();
    };
    let (origin, opts) = match label {
        Some(l) => {
            let th = text_height(&GlyphLookup::new(&t), &units);
            let y = at.y - (i32::from(l.h) - th) / 2
                + control::label_k(l.h)
                + if l.pressed { 2 } else { 0 };
            (
                Point::new(at.x, y),
                TextOpts::Draw {
                    centered: true,
                    block_w: Some(i32::from(l.w)),
                    mode: crate::ui::text::TEXT_DRAW_MODE,
                },
            )
        }
        None => (*at, TextOpts::default()),
    };
    let style = TextStyle {
        font: *font,
        color: 0,
    };
    layout_text(&t, &units, origin, style, &opts, &OriginalText)
        .unwrap_or_default()
        .into_iter()
        .map(|g| GlyphQuad {
            font: *font,
            frame: g.frame,
            at: g.at,
            color: g.color,
        })
        .collect()
}
