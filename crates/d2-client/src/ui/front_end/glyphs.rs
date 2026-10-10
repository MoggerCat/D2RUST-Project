// Spec: specs/ui/frontend-menus.md (§F1.1 r5 button label, r8 text control), specs/ui/text.md (§3, §6, §7)
//! Glyph quads of a front-end text item: the host's text path. The glyph
//! walk is the in-game one ([`layout_text`] with [`OriginalText`]); the
//! pens come from the control: a button label (§F1.1 r5), a text
//! control's rows (§F1.1 r8), or a plain pen.

use d2_formats::font::FontTable;

use super::{control, DrawItem, Label, TextBox};
use crate::ui::draw::TextStyle;
use crate::ui::geom::Point;
use crate::ui::text::{
    layout_text, max_width, width_a, width_b, GlyphLookup, OriginalText, TextOpts,
};

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
/// string id (used when the item carries no literal text; id 0 with no
/// text is an empty row and draws nothing).
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
        color,
        boxed,
    } = item
    else {
        return Vec::new();
    };
    let units: Vec<u16> = if label.is_some() && *string_id != 0 && !text.is_empty() {
        // A label formatted from a string: its "%s" takes `text`.
        let arg: Vec<u16> = text.encode_utf16().collect();
        let fmt = resolve(*string_id);
        match fmt.iter().position(|&u| u == u16::from(b'%')) {
            Some(i) if fmt.get(i + 1) == Some(&u16::from(b's')) => {
                [&fmt[..i], &arg[..], &fmt[i + 2..]].concat()
            }
            _ => fmt,
        }
    } else if !text.is_empty() {
        text.encode_utf16().collect()
    } else if *string_id != 0 {
        resolve(*string_id)
    } else {
        return Vec::new();
    };
    let Some(t) = table(*font) else {
        return Vec::new();
    };
    let g = GlyphLookup::new(&t);
    let pens = match (label, boxed) {
        (Some(l), _) => {
            let second = (l.second != 0).then(|| resolve(l.second));
            label_pens(&g, units, second, *at, l)
        }
        (None, Some(b)) => box_pens(&g, &units, *at, b),
        (None, None) => vec![(units, *at)],
    };
    let style = TextStyle {
        font: *font,
        color: (*color).clamp(0, i32::from(u16::MAX)) as u16,
    };
    pens.into_iter()
        .flat_map(|(row, pen)| {
            layout_text(&t, &row, pen, style, &TextOpts::default(), &OriginalText)
                .unwrap_or_default()
        })
        .map(|g| GlyphQuad {
            font: *font,
            frame: g.frame,
            at: g.at,
            color: g.color,
        })
        .collect()
}

/// Width A of `row` (`ui/text.md` §6); 0 for a glyph the font lacks.
fn width_of(g: &GlyphLookup<'_>, row: &[u16]) -> i32 {
    width_a(g, row).unwrap_or(0)
}

/// Button label pens (§F1.1 r5, `0x00500C50`): pen x = x + max(0, (w −
/// width A)/2), pen y = y − (h − font height)/2 + k; pressed: x − 2, y + 2.
fn label_pens(
    g: &GlyphLookup<'_>,
    first: Vec<u16>,
    second: Option<Vec<u16>>,
    at: Point,
    l: &Label,
) -> Vec<(Vec<u16>, Point)> {
    let (w, h) = (i32::from(l.w), i32::from(l.h));
    let y = at.y - (h - g.height()) / 2 + control::label_k(l.h);
    let x = |row: &[u16]| at.x + ((w - width_of(g, row)) / 2).max(0);
    let (dx, dy) = if l.pressed { (-2, 2) } else { (0, 0) };
    let mut rows = vec![first];
    rows.extend(second);
    // Flag 0x40 (two strings, `0x00500CB4`–`0x00500D7E`, named only in
    // the spec). PROVISIONAL (§F1.1 r5): each line is centred as a single
    // label; the first sits half a line step minus one row above the
    // single-label baseline, the second one line step below it (fitted to
    // the 1.14d character-select buttons "CREATE NEW / CHARACTER",
    // 168 × 60: baselines 500 and 516 against 507).
    let two = rows.len() > 1;
    let step = g.line_step();
    rows.into_iter()
        .enumerate()
        .map(|(i, row)| {
            let off = if two {
                -step / 2 + 1 + i as i32 * step
            } else {
                0
            };
            let pen = Point::new(x(&row) + dx, y + off + dy);
            (row, pen)
        })
        .collect()
}

/// White space dropped at the start of an added row (§F1.1 r8 a: units
/// < 0x100 that CRT `isspace` accepts).
fn trim_start(u: &[u16]) -> &[u16] {
    let n = u
        .iter()
        .take_while(|&&c| matches!(c, 0x09..=0x0D | 0x20))
        .count();
    &u[n..]
}

/// The rows of one added string (§F1.1 r8 a, d): leading white space
/// dropped; unless flag 0x20 (or 1) the string is split while width B of
/// its first 255 units is ≥ w − 2·mx (`0x004FCDA0`).
fn split_rows(g: &GlyphLookup<'_>, units: &[u16], w: i32, mx: i32, flags: u16) -> Vec<Vec<u16>> {
    let wrap = flags & 0x21 == 0;
    let limit = w - 2 * mx;
    let mut rest = trim_start(units);
    let mut rows = Vec::new();
    while wrap && !rest.is_empty() && width_b(g, rest, 255).unwrap_or(0) >= limit {
        let mut space = None;
        let mut stop = rest.len();
        for i in 0..rest.len() {
            if rest[i] == 0x20 {
                space = Some(i);
            }
            if width_b(g, rest, i + 1).unwrap_or(0) >= limit {
                stop = i;
                break;
            }
        }
        // d2rs-own: at least one unit per row, so the split always ends.
        let cut = space.unwrap_or(stop).max(1).min(rest.len());
        rows.push(rest[..cut].to_vec());
        rest = trim_start(&rest[cut..]);
    }
    if !rest.is_empty() || rows.is_empty() {
        rows.push(rest.to_vec());
    }
    rows
}

/// Text control pens (§F1.1 r8, draw `0x004FBF30`): row r's baseline is
/// (y − h) + Hf + my + r·(Hf + gap), gap 4 (English); a row after the first
/// only while (h − 2·my) − r·pitch ≥ pitch. Each row is cut to its longest
/// prefix of width B < w, then placed: flag 2 centred by width A, flag 0x10
/// ending at x, else at x + mx.
fn box_pens(g: &GlyphLookup<'_>, units: &[u16], at: Point, b: &TextBox) -> Vec<(Vec<u16>, Point)> {
    const GAP: i32 = 4;
    let (w, h) = (i32::from(b.w), i32::from(b.h));
    let hf = g.height();
    let pitch = hf + GAP;
    let mut out = Vec::new();
    for (i, mut row) in split_rows(g, units, w, b.mx, b.flags)
        .into_iter()
        .enumerate()
    {
        let r = i32::from(b.row) + i as i32;
        if r > 0 && (h - 2 * b.my) - r * pitch < pitch {
            break;
        }
        while !row.is_empty() && width_b(g, &row, row.len()).unwrap_or(0) >= w {
            row.pop();
        }
        let wa = width_of(g, &row);
        let x = if b.flags & 2 != 0 {
            at.x + ((w - wa) / 2).max(0)
        } else if b.flags & 0x10 != 0 {
            at.x - wa
        } else {
            at.x + b.mx
        };
        let y = (at.y - h) + hf + b.my + r * pitch;
        out.push((row, Point::new(x, y)));
    }
    out
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
