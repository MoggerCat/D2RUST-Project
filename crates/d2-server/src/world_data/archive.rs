// Spec: specs/formats/mpq.md (archive set); specs/data/loading.md
//! The server's reads from the user's archives: the fixed-up table set
//! and the DRLG files, through `d2_formats::mpq::ArchiveSet`.

use d2_data::bin;
use d2_data::fixup::{self, FixedSet};
use d2_formats::mpq::ArchiveSet;

use super::tables::LevelTables;
use super::{WorldDataError, WorldFiles};

/// A reader for [`WorldFiles::load`] over an archive set.
pub fn reader(set: &ArchiveSet) -> impl FnMut(&str) -> Result<Vec<u8>, String> + '_ {
    move |name| set.read(name).map_err(|e| e.to_string())
}

/// Loads, fixes up and returns the live table set (`bin::load`,
/// `AnimData.d2`, `fixup::apply`).
pub fn fixed_tables(set: &ArchiveSet) -> Result<FixedSet, WorldDataError> {
    let err = |table: &str, detail: String| WorldDataError::Table {
        table: table.to_owned(),
        detail,
    };
    let data = bin::load(set, bin::DEFAULT_LANGUAGE).map_err(|e| err("bin", e.to_string()))?;
    let anim = fixup::read_animdata(set).map_err(|e| err("AnimData.d2", e.to_string()))?;
    fixup::apply(&data, &anim).map_err(|e| err("fixup", e.to_string()))
}

/// The level-type table views and every DRLG file, from the archives.
pub fn load(set: &ArchiveSet) -> Result<(LevelTables, WorldFiles), WorldDataError> {
    let fixed = fixed_tables(set)?;
    let tables = LevelTables::from_fixed(&fixed)?;
    let files = WorldFiles::load(&tables.drlg, &tables.preset, &tables.outdoor, reader(set))?;
    Ok((tables, files))
}
