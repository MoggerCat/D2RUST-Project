// Spec: specs/client/assets.md
//! `.tbl` assets (§A2): font tables and string tables share the extension;
//! the "Woo!" magic (`formats/font-tbl.md`) picks the parser.

use bevy::prelude::*;
use d2_formats::font::FontTable;
use d2_formats::tbl::StringTable;
use d2_formats::FormatError;

#[derive(Asset, TypePath, Debug)]
pub enum TblAsset {
    Font(FontTable),
    Strings(StringTable),
}

impl TblAsset {
    /// A file with the font magic is a font; one that then fails to parse
    /// is an error, never retried as a string table (§Edge cases).
    pub fn parse(data: &[u8]) -> Result<TblAsset, FormatError> {
        if FontTable::is_font_table(data) {
            FontTable::parse(data).map(TblAsset::Font)
        } else {
            StringTable::parse(data).map(TblAsset::Strings)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Font header (`Woo!`, version 1, 4 unknown bytes, height, width) and
    /// no glyphs.
    fn font() -> Vec<u8> {
        let mut v = b"Woo!".to_vec();
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&[0; 4]);
        v.extend_from_slice(&[16, 12]);
        v
    }

    /// Empty string table: a 21-byte header with no elements or slots.
    fn strings() -> Vec<u8> {
        vec![0; 21]
    }

    // Covers: specs/client/assets.md §a2-loaders
    #[test]
    fn font_magic_gives_font() {
        // Test vector §A2.
        match TblAsset::parse(&font()).unwrap() {
            TblAsset::Font(f) => assert_eq!((f.height, f.width), (16, 12)),
            other => panic!("expected a font, got {other:?}"),
        }
    }

    // Covers: specs/client/assets.md §a2-loaders
    #[test]
    fn no_magic_gives_strings() {
        // Test vector §A2.
        assert!(matches!(
            TblAsset::parse(&strings()).unwrap(),
            TblAsset::Strings(_)
        ));
    }

    // Covers: specs/client/assets.md §edge-cases-original-bugs
    #[test]
    fn broken_font_is_an_error_not_strings() {
        // Perturbation (M08): version 2 breaks the font; the result is the
        // font parser's error, not a string-table fallback.
        let mut f = font();
        f[4] = 2;
        let err = TblAsset::parse(&f).unwrap_err();
        assert!(err.to_string().contains("version 2"), "{err}");
    }
}
