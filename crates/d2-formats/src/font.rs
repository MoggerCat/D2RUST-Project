// Spec: specs/formats/font-tbl.md
//! Font tables: per-character metrics for a font's DC6 glyph sheet.

use crate::cursor::{invalid, Cursor, FormatError};

const FORMAT: &str = "font tbl";
pub const MAGIC: [u8; 4] = *b"Woo!";
const HEADER_LEN: usize = 12;
const GLYPH_LEN: usize = 14;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    pub code: u16,
    pub unknown1: u8,
    pub width: u8,
    pub height: u8,
    pub unknown2: u8,
    pub unknown3: u16,
    /// Frame index in the font's DC6.
    pub frame: u8,
    pub unknown4: u8,
    pub unknown5: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontTable {
    pub version: u16,
    pub unknown: [u8; 4],
    pub height: u8,
    pub width: u8,
    pub glyphs: Vec<Glyph>,
}

impl FontTable {
    /// True if `data` starts with the font-table magic. String tables share
    /// the `.tbl` extension but not the magic.
    pub fn is_font_table(data: &[u8]) -> bool {
        data.starts_with(&MAGIC)
    }

    pub fn parse(data: &[u8]) -> Result<FontTable, FormatError> {
        let mut c = Cursor::new(data, FORMAT);
        if c.bytes(4)? != MAGIC {
            return Err(invalid(FORMAT, "missing \"Woo!\" magic"));
        }
        let version = c.u16()?;
        if version != 1 {
            return Err(invalid(FORMAT, format!("version {version}, expected 1")));
        }
        let mut unknown = [0u8; 4];
        unknown.copy_from_slice(c.bytes(4)?);
        let height = c.u8()?;
        let width = c.u8()?;

        let body = data.len() - HEADER_LEN;
        if !body.is_multiple_of(GLYPH_LEN) {
            return Err(invalid(
                FORMAT,
                format!("{body} bytes of records is not a multiple of {GLYPH_LEN}"),
            ));
        }
        let glyphs = (0..body / GLYPH_LEN)
            .map(|_| {
                Ok(Glyph {
                    code: c.u16()?,
                    unknown1: c.u8()?,
                    width: c.u8()?,
                    height: c.u8()?,
                    unknown2: c.u8()?,
                    unknown3: c.u16()?,
                    frame: c.u8()?,
                    unknown4: c.u8()?,
                    unknown5: c.u32()?,
                })
            })
            .collect::<Result<Vec<_>, FormatError>>()?;
        Ok(FontTable {
            version,
            unknown,
            height,
            width,
            glyphs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(records: &[[u8; 14]]) -> Vec<u8> {
        let mut d = b"Woo!".to_vec();
        d.extend_from_slice(&1u16.to_le_bytes());
        d.extend_from_slice(&[0, 0, 0, 0, 16, 12]);
        for r in records {
            d.extend_from_slice(r);
        }
        d
    }

    // Covers: specs/formats/font-tbl.md §header-12-bytes, §glyph-records-14-bytes-each
    #[test]
    fn parses_records() {
        let a = [b'A', 0, 0, 7, 16, 1, 0, 0, 33, 0, 0, 0, 0, 0];
        let b = [0x10, 0xAC, 0, 9, 16, 1, 0, 0, 200, 0, 0, 0, 0, 0];
        let f = FontTable::parse(&file(&[a, b])).unwrap();
        assert_eq!((f.height, f.width), (16, 12));
        assert_eq!(f.glyphs.len(), 2);
        assert_eq!(f.glyphs[0].code, u16::from(b'A'));
        assert_eq!(f.glyphs[0].width, 7);
        assert_eq!(f.glyphs[0].frame, 33);
        assert_eq!(f.glyphs[1].code, 0xAC10);
        assert_eq!(f.glyphs[1].frame, 200);
    }

    // Covers: specs/formats/font-tbl.md §header-12-bytes, §glyph-records-14-bytes-each
    #[test]
    fn errors() {
        let mut bad = file(&[]);
        bad[0] = b'X';
        assert!(FontTable::parse(&bad).is_err(), "magic");
        let mut partial = file(&[]);
        partial.extend_from_slice(&[0; 15]);
        assert!(FontTable::parse(&partial).is_err(), "partial record");
        assert!(FontTable::is_font_table(&file(&[])));
    }

    mod robust {
        use super::*;
        use crate::robust::mutated;
        use crate::robust_tests::{check, config};
        use proptest::prelude::*;

        fn valid() -> Vec<u8> {
            file(&[[b'A', 0, 0, 7, 16, 1, 0, 0, 33, 0, 0, 0, 0, 0]; 2])
        }

        #[test]
        fn builder_is_valid() {
            assert!(FontTable::parse(&valid()).is_ok());
        }

        proptest! {
            #![proptest_config(config(64))]

            #[test]
            fn mutated_file(data in mutated(valid())) {
                check(data, FontTable::parse);
            }
        }
    }
}
