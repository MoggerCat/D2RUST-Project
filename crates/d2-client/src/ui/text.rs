// Spec: specs/client/ui.md
//! Text layout entry point (spec §A3).
//!
//! [`layout_text`] splits the work in two:
//! - format-level glyph lookup (ours, from `formats/font-tbl.md`): a code
//!   unit names the font-table record whose `code` field equals it, and
//!   that record names the DC6 frame. A code with no record is an error,
//!   never a fallback glyph;
//! - everything else (advance, kerning, line height, baseline, word wrap,
//!   alignment, `ÿc` color codes, the PL2 text-color map) is original
//!   behavior and lives in one [`TextRules`] implementation:
//!   TODO(spec: ui/text.md §B3). The placeholder [`NoTextRules`] refuses.
//!
//! Strings stay UTF-16 code units as the string tables hold them
//! (`tbl.md`): the rules see the caller's slice as given.

use d2_formats::font::FontTable;

use super::draw::TextStyle;
use super::geom::Point;

/// Layout options. Which options the original's text drawing takes
/// (wrap width, alignment, …) is TODO(spec: ui/text.md §B3); none yet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextOpts {}

/// One glyph the rules place: the code unit to draw, where and with which
/// text color. The rules decide which code units are drawn (color codes
/// are not) and where.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphPlacement {
    pub code: u16,
    pub at: Point,
    pub color: u16,
}

/// A placed glyph resolved to its font record and DC6 frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlyphDraw {
    pub code: u16,
    /// Index of the record in [`FontTable::glyphs`].
    pub record: usize,
    /// Frame in the font's DC6 (`font-tbl.md` `frame`).
    pub frame: u8,
    pub at: Point,
    pub color: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TextError {
    /// TODO(spec: ui/text.md §B3): the original's handling of a code the
    /// font has no record for is not specified; until it is, an error.
    #[error("font has no glyph record for code {0:#06x}")]
    MissingGlyph(u16),
    /// Two records share one code; which one the original uses is not
    /// specified (TODO(spec: ui/text.md §B3)).
    #[error("font has {count} glyph records for code {code:#06x}")]
    AmbiguousGlyph { code: u16, count: usize },
    /// A rule the owner spec has not written yet.
    #[error("text layout not specified: TODO(spec: {0})")]
    Unspecified(&'static str),
}

/// Format-level lookup the rules may use for metrics (`width`, …).
pub struct GlyphLookup<'a> {
    font: &'a FontTable,
}

impl<'a> GlyphLookup<'a> {
    pub fn new(font: &'a FontTable) -> Self {
        GlyphLookup { font }
    }

    pub fn font(&self) -> &'a FontTable {
        self.font
    }

    /// The record whose `code` equals `code` (index into `glyphs`).
    pub fn record(&self, code: u16) -> Result<usize, TextError> {
        let mut found = self
            .font
            .glyphs
            .iter()
            .enumerate()
            .filter(|(_, g)| g.code == code)
            .map(|(i, _)| i);
        let first = found.next().ok_or(TextError::MissingGlyph(code))?;
        let more = found.count();
        if more > 0 {
            return Err(TextError::AmbiguousGlyph {
                code,
                count: more + 1,
            });
        }
        Ok(first)
    }
}

/// The original's text layout (spec §A3, §B3).
///
/// TODO(spec: ui/text.md §B3): one implementation, written from that spec.
pub trait TextRules {
    fn place(
        &self,
        glyphs: &GlyphLookup<'_>,
        text: &[u16],
        origin: Point,
        style: TextStyle,
        opts: &TextOpts,
    ) -> Result<Vec<GlyphPlacement>, TextError>;
}

/// Placeholder until `ui/text.md` exists: every layout is an error.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoTextRules;

impl TextRules for NoTextRules {
    fn place(
        &self,
        _: &GlyphLookup<'_>,
        _: &[u16],
        _: Point,
        _: TextStyle,
        _: &TextOpts,
    ) -> Result<Vec<GlyphPlacement>, TextError> {
        Err(TextError::Unspecified("ui/text.md §B3"))
    }
}

/// Lays out `text` (UTF-16 code units, unchanged) at `origin`: `rules`
/// place the glyphs, then each placed code is resolved to its font record
/// and DC6 frame. Output order is the rules' placement order.
pub fn layout_text(
    font: &FontTable,
    text: &[u16],
    origin: Point,
    style: TextStyle,
    opts: &TextOpts,
    rules: &dyn TextRules,
) -> Result<Vec<GlyphDraw>, TextError> {
    let lookup = GlyphLookup::new(font);
    rules
        .place(&lookup, text, origin, style, opts)?
        .into_iter()
        .map(|p| {
            let record = lookup.record(p.code)?;
            Ok(GlyphDraw {
                code: p.code,
                record,
                frame: font.glyphs[record].frame,
                at: p.at,
                color: p.color,
            })
        })
        .collect()
}
