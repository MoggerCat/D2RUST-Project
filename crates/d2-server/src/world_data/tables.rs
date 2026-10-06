// Spec: specs/drlg/preset.md §5.3, §13; specs/drlg/levels.md; specs/drlg/maze.md §1; specs/drlg/outdoor-tilesub.md §1
//! The table views of the level types, from the fixed-up table set
//! (`d2_data::fixup::FixedSet`): each view's own `from_tables` /
//! `from_record`, plus the preset counts of `preset.md` §5.3 (monstats
//! and superuniques row counts, monpreset rows and act ranges, the item
//! class of `hdm `).

use d2_data::bin::BinTable;
use d2_data::fixup::FixedSet;
use d2_data::tables::{
    decode_all, Leveldefs, Lvlmaze, Lvlprest, Lvlsub, Lvltypes, Lvlwarp, Objects, Record,
};
use d2_sim::drlg::maze::MazeData;
use d2_sim::drlg::outdoor::OutdoorData;
use d2_sim::drlg::preset::{MonPresetRow, PresetData, PresetDef, PresetTables};
use d2_sim::drlg::DrlgData;

use super::WorldDataError;

/// The item code the one-entry DS1 item table holds (`preset.md` §5.3).
pub const HDM_CODE: [u8; 4] = *b"hdm ";

/// The four table views of the level types.
#[derive(Clone, Debug)]
pub struct LevelTables {
    pub drlg: DrlgData,
    pub preset: PresetData,
    pub outdoor: OutdoorData,
    pub maze: MazeData,
}

impl LevelTables {
    pub fn from_fixed(set: &FixedSet) -> Result<Self, WorldDataError> {
        let leveldefs: Vec<Leveldefs> = records(set)?;
        let lvlprest: Vec<Lvlprest> = records(set)?;
        let lvlsub: Vec<Lvlsub> = records(set)?;
        let lvlmaze: Vec<Lvlmaze> = records(set)?;
        let lvltypes: Vec<Lvltypes> = records(set)?;
        let lvlwarp: Vec<Lvlwarp> = records(set)?;
        let objects: Vec<Objects> = records(set)?;
        let drlg = DrlgData::from_tables(&leveldefs, &lvlwarp, &lvltypes, &objects);
        let monpreset = table(set, "monpreset")?
            .iter()
            .map(|r| {
                <[u8; 4]>::try_from(r)
                    .map(|b| MonPresetRow::from_record_bytes(&b))
                    .map_err(|_| table_err("monpreset", "record is not 4 bytes"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let preset = PresetData {
            defs: lvlprest.iter().map(PresetDef::from_record).collect(),
            monpreset_acts: set.monpreset,
            monpreset,
            monstats_count: table(set, "monstats")?.count as u32,
            superuniques_count: table(set, "superuniques")?.count as u32,
            hdm_item: set
                .item_codes
                .find(u32::from_le_bytes(HDM_CODE))
                .map_or(-1, |i| i as i32),
            tables: PresetTables::spec().map_err(|e| table_err("preset-tables.tsv", e))?,
        };
        Ok(Self {
            outdoor: OutdoorData::from_tables(&leveldefs, &lvlprest, &lvlsub),
            maze: MazeData::from_tables(&lvlmaze, &lvlprest),
            drlg,
            preset,
        })
    }
}

fn table_err(table: &str, detail: impl ToString) -> WorldDataError {
    WorldDataError::Table {
        table: table.to_owned(),
        detail: detail.to_string(),
    }
}

fn table<'a>(set: &'a FixedSet, name: &str) -> Result<&'a BinTable, WorldDataError> {
    set.table(name).ok_or_else(|| table_err(name, "not loaded"))
}

fn records<T: Record>(set: &FixedSet) -> Result<Vec<T>, WorldDataError> {
    decode_all(table(set, T::TABLE)?).map_err(|e| table_err(T::TABLE, e))
}
