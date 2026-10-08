// Spec: specs/ui/frontend-menus.md (§F1.1 r5 button label), specs/ui/text.md (§3, §6, §7)
//! Glyph quads of a front-end text item: the host's text path. The layout
//! is the in-game one ([`layout_text`] with [`OriginalText`]); nothing is
//! copied. A button label is centered in the button and its baseline is
//! `y − (h − text_height)/2 + k`, +2 while pressed (§F1.1 r5).

use d2_formats::font::FontTable;

use super::{control, DrawItem};
use crate::ui::draw::TextStyle;
use crate::ui::geom::Point;
use crate::ui::text::{layout_text, max_width, text_height, GlyphLookup, OriginalText, TextOpts};

/// One glyph to draw: frame `frame` of font `font`'s DC6, whose bottom row
/// is the pen row `at.y` (`ui/text.md` §4.2); colour `color` (0: none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphQuad {
    pub font: u16,
    pub frame: u16,
    pub at: Point,
    pub color: i32,
}

/// Width in pixels of `text` in the font `table` (the widest `LF` line, the
/// font table's advances: `ui/text.md` §6). A unit without a glyph record
/// counts 0, as it draws nothing.
pub fn text_width(table: &FontTable, text: &[u16]) -> i32 {
    max_width(&GlyphLookup::new(table), text).unwrap_or(0)
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

#[cfg(test)]
mod tests {
    use super::*;
    use d2_formats::font::Glyph;

    /// 256 records; record `c` advances `c % 10 + 1`.
    pub(crate) fn table() -> FontTable {
        FontTable {
            version: 1,
            unknown: 0,
            count: 256,
            height: 16,
            width: 12,
            glyphs: (0..256u16)
                .map(|c| Glyph {
                    code: c,
                    unknown1: 0,
                    width: (c % 10 + 1) as u8,
                    height: 16,
                    unknown2: 0,
                    unknown3: 0,
                    frame: c,
                    unknown5: 0,
                })
                .collect(),
        }
    }

    #[test]
    fn width_is_the_sum_of_advances() {
        let t = table();
        let u: Vec<u16> = "AB".encode_utf16().collect(); // 65, 66 -> 6 + 7
        assert_eq!(text_width(&t, &u), 13);
        assert_eq!(text_width(&t, &[]), 0);
    }

    #[test]
    fn credits_rows_centre_with_the_font_advances() {
        use crate::ui::front_end::screens::credits::{layout, parse, Col, Scroll};
        let t = table();
        let raw = crate::ui::front_end::screens::credits::decode(b"AB\r\nCD\r\n").unwrap();
        let cr = parse(&raw);
        let adv = |text: &[u16]| text_width(&t, text);
        let rows = layout(&cr, Scroll { top: 50, off: 0 }, true, &adv);
        let a = rows.iter().find(|d| d.col == Col::A).expect("column A row");
        // Column A is right-aligned to x 400 (expansion), by the real width.
        assert_eq!(a.x + adv(&a.text), 400);
        assert_ne!(adv(&a.text), 8 * a.text.len() as i32);
    }
}
