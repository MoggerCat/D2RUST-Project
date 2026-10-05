// Spec: specs/data/field-types.md §7 (string keys); load order from specs/data/loading.md §10.1
//! The three string tables and the `strkey` resolver.

use d2_formats::mpq::{ArchiveSet, MpqError};
use d2_formats::tbl::StringTable;
use d2_formats::FormatError;

/// The no-string ID (`string.tbl` element 5382, key `dummy`).
pub const NO_STRING: u16 = 5382;
/// ID bases (`field-types.md` §7).
pub const PATCH_BASE: u32 = 10_000;
pub const EXPANSION_BASE: u32 = 20_000;

/// A string table that failed to load.
#[derive(Debug, thiserror::Error)]
pub enum StringsError {
    #[error("{file}: {source}")]
    Mpq { file: String, source: MpqError },
    #[error("{file}: {source}")]
    Format { file: String, source: FormatError },
}

/// `string.tbl`, `patchstring.tbl` and (LoD only) `expansionstring.tbl`.
#[derive(Debug, Clone, Default)]
pub struct StringTables {
    pub base: Option<StringTable>,
    pub patch: Option<StringTable>,
    /// Present exactly when the expansion is installed ("LoD").
    pub expansion: Option<StringTable>,
}

/// Element number of `key` in `t`.
fn element(t: &StringTable, key: &[u8]) -> Option<u32> {
    t.find_slot(key).map(|s| u32::from(t.entries[s].index))
}

impl StringTables {
    /// Loads the tables of language `lang` (e.g. `eng`) from the archive
    /// set: `string.tbl`, `patchstring.tbl`, then `expansionstring.tbl`
    /// when `lod` (`loading.md` §10.1).
    pub fn load(set: &ArchiveSet, lang: &str, lod: bool) -> Result<StringTables, StringsError> {
        let read = |name: &str| -> Result<StringTable, StringsError> {
            let file = format!("data\\local\\lng\\{lang}\\{name}");
            let bytes = set.read(&file).map_err(|source| StringsError::Mpq {
                file: file.clone(),
                source,
            })?;
            StringTable::parse(&bytes).map_err(|source| StringsError::Format { file, source })
        };
        Ok(StringTables {
            base: Some(read("string.tbl")?),
            patch: Some(read("patchstring.tbl")?),
            expansion: if lod {
                Some(read("expansionstring.tbl")?)
            } else {
                None
            },
        })
    }

    /// The string ID of `text`, without the no-string substitution: 0 for
    /// an empty or unknown key (`loading.md` §7.4 "→ id").
    pub fn id(&self, text: &[u8]) -> u32 {
        if text.is_empty() {
            return 0;
        }
        if let Some(e) = self.patch.as_ref().and_then(|t| element(t, text)) {
            return e + PATCH_BASE;
        }
        if let Some(e) = self.expansion.as_ref().and_then(|t| element(t, text)) {
            return e + EXPANSION_BASE;
        }
        self.base
            .as_ref()
            .and_then(|t| element(t, text))
            .unwrap_or(0)
    }

    /// The `strkey` value stored for a bound cell: the ID, or 5382 when it
    /// is 0 (`field-types.md` §7). `text` is the first 256 cell bytes.
    pub fn strkey(&self, text: &[u8]) -> u16 {
        match self.id(text) {
            0 => NO_STRING,
            id => id as u16,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tables_give_no_string() {
        let t = StringTables::default();
        assert_eq!(t.id(b""), 0);
        assert_eq!(t.strkey(b""), NO_STRING);
        assert_eq!(t.strkey(b"cap"), NO_STRING);
    }

    fn game_strings() -> Option<StringTables> {
        let dir = std::env::var("D2_GAME_DIR").ok()?;
        let set = ArchiveSet::open_dir(dir).ok()?;
        Some(StringTables::load(&set, "eng", true).expect("string tables"))
    }

    /// `field-types.md` §7 test vectors (1.14d ENG, LoD).
    #[test]
    #[ignore = "needs original game files in D2_GAME_DIR"]
    fn strkey_vectors() {
        let t = game_strings().expect("D2_GAME_DIR must be set");
        let cases: [(&[u8], u16); 12] = [
            (b"", 5382),
            (b"cap", 1930),
            (b"Cap", 5382),
            (b"CAP", 5382),
            (b"x", 10016),
            (b"Cutthroat1", 10000),
            (b"ModStr5d", 10001),
            (b"A4Q2ExpansionSuccessTyrael", 20000),
            (b"ob1", 20281),
            (b"Lilith", 11154),
            (b"WarrivAct1IntroGossip1", 5382),
            (b"nonexistent_key_zz", 5382),
        ];
        for (text, id) in cases {
            assert_eq!(t.strkey(text), id, "{}", String::from_utf8_lossy(text));
        }
        assert_eq!(t.strkey(b"dummy"), 5382);
    }
}
