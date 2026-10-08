// Spec: specs/data/field-types.md §7 (string keys); load order from specs/data/loading.md §10.1; specs/ui/text.md §2 (decode, lookup by id)
//! The three string tables and the `strkey` resolver.

use d2_formats::mpq::{ArchiveSet, MpqError};
use d2_formats::tbl::StringTable;
use d2_formats::FormatError;

/// The no-string ID (`string.tbl` element 5382, key `dummy`).
pub const NO_STRING: u16 = 5382;
/// ID bases (`field-types.md` §7).
pub const PATCH_BASE: u32 = 10_000;
pub const EXPANSION_BASE: u32 = 20_000;

/// Decodes string-table bytes as UTF-8 into UTF-16 units; an invalid
/// sequence ends the string at that point (`ui/text.md` §2 r1).
pub fn decode_utf8_units(bytes: &[u8]) -> Vec<u16> {
    let valid = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => std::str::from_utf8(&bytes[..e.valid_up_to()]).unwrap_or(""),
    };
    valid.encode_utf16().collect()
}

/// Lookup by id failed: the original ends the process (`ui/text.md` §2
/// r2.4).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("string id {0}: element missing (fatal in the original)")]
pub struct NoSuchString(pub u32);

/// The element within one table (`0x00524930`, `ui/text.md` §2 r2.4): a
/// number >= `num_elements` reads element 500.
fn table_element(t: &StringTable, n: u32) -> Option<Vec<u16>> {
    let n = if n >= u32::from(t.header.num_elements) {
        500
    } else {
        n
    };
    t.element(n as usize).map(|e| decode_utf8_units(&e.value))
}

/// A string table that failed to load.
#[derive(Debug, thiserror::Error)]
pub enum StringsError {
    #[error("{file}: {source}")]
    Mpq { file: String, source: MpqError },
    #[error("{file}: {source}")]
    Format { file: String, source: FormatError },
    /// A read error from a native source (`native-assets.md` §5.3).
    #[error("{file}: {detail}")]
    Source { file: String, detail: String },
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
        StringTables::load_from(set, lang, lod)
    }

    /// [`StringTables::load`] over any table source: the archive set or a
    /// native folder (`native-assets.md` §5.3).
    pub fn load_from(
        set: &dyn crate::bin::TableFiles,
        lang: &str,
        lod: bool,
    ) -> Result<StringTables, StringsError> {
        let read = |name: &str| set.string_table(&format!("data\\local\\lng\\{lang}\\{name}"));
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

    /// `D2Lang_GetStringByIndex` (`0x00524A30`, `ui/text.md` §2 r2): id >=
    /// 20,000 reads `expansionstring.tbl` element `id - 20,000` when it is
    /// loaded (else the id becomes 11,078 and goes on), id >= 10,000 reads
    /// `patchstring.tbl` element `id - 10,000` when loaded, else element
    /// `id` of `string.tbl`.
    pub fn by_index(&self, id: u32) -> Result<Vec<u16>, NoSuchString> {
        let mut id = id;
        if id >= EXPANSION_BASE {
            if let Some(t) = &self.expansion {
                return table_element(t, id - EXPANSION_BASE).ok_or(NoSuchString(id));
            }
            id = 11_078;
        }
        if id >= PATCH_BASE {
            if let Some(t) = &self.patch {
                return table_element(t, id - PATCH_BASE).ok_or(NoSuchString(id));
            }
        }
        let t = self.base.as_ref().ok_or(NoSuchString(id))?;
        table_element(t, id).ok_or(NoSuchString(id))
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

    /// A table of `n` elements where element `i` has value `vals[i]`
    /// (slot = element number).
    fn table(vals: &[&[u8]]) -> StringTable {
        use d2_formats::tbl::{TblEntry, TblHeader};
        StringTable {
            header: TblHeader {
                crc: 0,
                num_elements: vals.len() as u16,
                hash_table_size: vals.len() as u32,
                version: 0,
                data_start: 0,
                max_tries: 1,
                file_size: 0,
            },
            indices: (0..vals.len() as u16).collect(),
            entries: vals
                .iter()
                .enumerate()
                .map(|(i, v)| TblEntry {
                    used: true,
                    index: i as u16,
                    hash: 0,
                    key: Vec::new(),
                    value: v.to_vec(),
                })
                .collect(),
        }
    }

    // Covers: specs/ui/text.md §2 r1
    #[test]
    fn utf8_decode_ends_at_invalid() {
        // ÿ is stored as C3 BF and decodes to U+00FF.
        assert_eq!(
            decode_utf8_units(b"\xC3\xBFc4Gold"),
            [0xFF, 0x63, 0x34, 0x47, 0x6F, 0x6C, 0x64]
        );
        assert_eq!(decode_utf8_units(b"ab\xFFcd"), [0x61, 0x62]);
        assert_eq!(decode_utf8_units(b"ab\xC3"), [0x61, 0x62]);
    }

    // Covers: specs/ui/text.md §2 r2
    #[test]
    fn lookup_by_id() {
        let mut base_vals: Vec<&[u8]> = vec![b"b"; 600];
        base_vals[500] = b"fallback500";
        base_vals[3] = b"three";
        let mut patch_vals: Vec<&[u8]> = vec![b"p"; 1100];
        patch_vals[1078] = b"patch1078";
        let mut t = StringTables {
            base: Some(table(&base_vals)),
            patch: Some(table(&patch_vals)),
            expansion: Some(table(&[b"exp0", b"exp1"])),
        };
        let s = |v: Vec<u16>| String::from_utf16(&v).unwrap();
        assert_eq!(s(t.by_index(3).unwrap()), "three");
        assert_eq!(s(t.by_index(10_000 + 1078).unwrap()), "patch1078");
        assert_eq!(s(t.by_index(20_000).unwrap()), "exp0");
        // element number >= num_elements reads element 500
        assert_eq!(s(t.by_index(599 + 1).unwrap()), "fallback500");
        // ... which a two-element table does not have: fatal in the original
        assert_eq!(t.by_index(20_007), Err(NoSuchString(20_007)));
        // expansion not loaded: 20,000 becomes 11,078 -> patch element 1,078
        t.expansion = None;
        assert_eq!(s(t.by_index(20_000).unwrap()), "patch1078");
        // patch not loaded either: element 11,078 of string.tbl -> 500
        t.patch = None;
        assert_eq!(s(t.by_index(10_005).unwrap()), "fallback500");
    }

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
    // Covers: specs/data/field-types.md §7
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
