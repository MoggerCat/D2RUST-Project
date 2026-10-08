// Spec: specs/formats/tbl.md, specs/formats/loading.md (§10.1), specs/ui/text.md (§2)
//! The play app's string tables: `string.tbl`, `patchstring.tbl` and
//! `expansionstring.tbl` loaded through `d2_data::strings`, held as UTF-16
//! by id and by key, and bound as the world view's [`StringLookup`]
//! (replacing [`crate::ui::NoStrings`]). Panels that take a closure
//! (`Fn(u16) -> Vec<u16>`) use [`TableStrings::by_id`].
// d2rs-own, unverified: the id → text map is built once at load (every
// element of the three tables), not read per call as the original does.

use std::collections::HashMap;

use bevy::prelude::*;
use d2_data::bin::TableFiles;
use d2_data::strings::{StringTables, StringsError, EXPANSION_BASE, PATCH_BASE};
use d2_formats::tbl::StringTable;

use crate::ui::StringLookup;
use crate::world_view::WorldViewUi;

/// The language of the play preview (`UiParts::live` loads `eng` too).
pub const LANG: &str = "eng";

/// All string-table text, UTF-16 as the fonts take it.
#[derive(Debug, Clone, Default)]
pub struct TableStrings {
    by_id: HashMap<u16, Vec<u16>>,
    by_key: HashMap<String, Vec<u16>>,
}

fn add(
    t: &StringTable,
    base: u32,
    ids: &mut HashMap<u16, Vec<u16>>,
    keys: &mut Vec<(Vec<u8>, Vec<u16>)>,
) {
    for n in 0..usize::from(t.header.num_elements) {
        let Some(e) = t.element(n) else { continue };
        let text = d2_data::strings::decode_utf8_units(&e.value);
        if let Ok(id) = u16::try_from(base + n as u32) {
            ids.insert(id, text.clone());
        }
        if !e.key.is_empty() {
            keys.push((e.key.clone(), text));
        }
    }
}

impl TableStrings {
    /// Reads the tables of `lang` from `files`; the expansion table only
    /// when `files.lod()`.
    pub fn load(files: &dyn TableFiles, lang: &str) -> Result<Self, StringsError> {
        Ok(Self::from_tables(&StringTables::load_from(
            files,
            lang,
            files.lod(),
        )?))
    }

    /// Every element of `t`. A key found in several tables resolves in the
    /// order of `StringTables::id`: patch, then expansion, then base.
    pub fn from_tables(t: &StringTables) -> Self {
        let mut by_id = HashMap::new();
        let mut keys = Vec::new();
        // Lowest priority first, later inserts win.
        for (table, base) in [
            (&t.base, 0),
            (&t.expansion, EXPANSION_BASE),
            (&t.patch, PATCH_BASE),
        ] {
            if let Some(table) = table {
                add(table, base, &mut by_id, &mut keys);
            }
        }
        let by_key = keys
            .into_iter()
            .map(|(k, v)| (String::from_utf8_lossy(&k).into_owned(), v))
            .collect();
        TableStrings { by_id, by_key }
    }

    /// The text of string id `id`, empty when absent (the closure form
    /// the HUD / NPC / message panels take).
    pub fn by_id(&self, id: u16) -> Vec<u16> {
        self.by_id.get(&id).cloned().unwrap_or_default()
    }
}

impl StringLookup for TableStrings {
    fn get(&self, key: &str) -> Option<&[u16]> {
        self.by_key.get(key).map(Vec::as_slice)
    }

    fn get_id(&self, id: u16) -> Option<&[u16]> {
        self.by_id.get(&id).map(Vec::as_slice)
    }
}

/// Binds `strings` as the play UI's lookup (no-op without a UI).
pub fn install_strings(app: &mut App, strings: TableStrings) {
    if let Some(mut ui) = app.world_mut().get_non_send_mut::<WorldViewUi>() {
        ui.strings = Box::new(strings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use d2_formats::tbl::{TblEntry, TblHeader};

    fn table(vals: &[(&str, &str)]) -> StringTable {
        let n = vals.len();
        StringTable {
            header: TblHeader {
                crc: 0,
                num_elements: n as u16,
                hash_table_size: n as u32,
                version: 0,
                data_start: 0,
                max_tries: 1,
                file_size: 0,
            },
            indices: (0..n as u16).collect(),
            entries: vals
                .iter()
                .enumerate()
                .map(|(i, (k, v))| TblEntry {
                    used: true,
                    index: i as u16,
                    hash: 0,
                    key: k.as_bytes().to_vec(),
                    value: v.as_bytes().to_vec(),
                })
                .collect(),
        }
    }

    fn u16s(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn labels_resolve_by_id_and_key_across_tables() {
        let tables = StringTables {
            base: Some(table(&[("a", "Strength"), ("b", "Dexterity")])),
            patch: Some(table(&[("b", "Patched")])),
            expansion: Some(table(&[("x", "Expansion")])),
        };
        let s = TableStrings::from_tables(&tables);
        assert_eq!(s.get_id(0), Some(u16s("Strength").as_slice()));
        assert_eq!(s.get_id(1), Some(u16s("Dexterity").as_slice()));
        assert_eq!(s.get_id(10_000), Some(u16s("Patched").as_slice()));
        assert_eq!(s.get_id(20_000), Some(u16s("Expansion").as_slice()));
        assert_eq!(s.get("b"), Some(u16s("Patched").as_slice()));
        assert_eq!(s.get("x"), Some(u16s("Expansion").as_slice()));
        assert_eq!(s.get_id(7), None);
        assert_eq!(s.by_id(0), u16s("Strength"));
        assert!(s.by_id(7).is_empty());
    }

    #[test]
    fn install_replaces_no_strings_in_the_ui() {
        use crate::ui::{NoPanelRules, UiRoot};
        let mut app = App::new();
        app.insert_non_send(WorldViewUi::new(
            UiRoot::new(Box::new(NoPanelRules)),
            Box::new(crate::ui::NoStrings),
        ));
        let tables = StringTables {
            base: Some(table(&[("a", "Strength")])),
            ..Default::default()
        };
        install_strings(&mut app, TableStrings::from_tables(&tables));
        let ui = app.world().non_send::<WorldViewUi>();
        assert_eq!(ui.strings.get_id(0), Some(u16s("Strength").as_slice()));
    }
}
