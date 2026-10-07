// Spec: specs/drlg/preset.md §5.3, §13; specs/drlg/levels.md; specs/drlg/maze.md §1; specs/drlg/outdoor-tilesub.md §1; specs/world/hirelings.md Inputs (the `hireling`, `pettype`, `experience` tables), §1.2 r2, §10 r2; specs/items/treasure.md Inputs, §1; specs/formats/d2s.md Inputs, §2.5 r2, §8.1 r2, §8.4 r2
//! The table views of the level types, from the fixed-up table set
//! (`d2_data::fixup::FixedSet`): each view's own `from_tables` /
//! `from_record`, plus the preset counts of `preset.md` §5.3 (monstats
//! and superuniques row counts, monpreset rows and act ranges, the item
//! class of `hdm `). Also the hireling tables of the interaction desk
//! ([`hireling_tables`]), the drop tables of the treasure walk
//! ([`drop_tables`], `ActionHooks::object_drops`) and the tables of the
//! `.d2s` reader and writer ([`SaveData`], `d2s::SaveTables`).

use d2_data::bin::BinTable;
use d2_data::fixup::FixedSet;
use d2_data::tables::{
    decode_all, Armor, Itemstatcost, Itemtypes, Leveldefs, Lvlmaze, Lvlprest, Lvlsub, Lvltypes,
    Lvlwarp, Misc, Objects, Record, Setitems, Superuniques, Treasureclassex, Uniqueitems, Weapons,
};
use d2_formats::d2s::{self, Hireling, StatSave};
use d2_sim::drlg::maze::MazeData;
use d2_sim::drlg::outdoor::OutdoorData;
use d2_sim::drlg::preset::{MonPresetRow, PresetData, PresetDef, PresetTables};
use d2_sim::drlg::DrlgData;
use d2_sim::items::bitstream::read::read_save_entry;
use d2_sim::items::ItemTables;
use d2_sim::treasure::{item_list, TcSources, TreasureClasses};
use d2_sim::wiring::economy::DropTables;
use d2_sim::world::hirelings::{HirelingRows, HirelingTables};

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

/// The hireling tables (`hirelings.md` Inputs: `hireling` rows, `pettype`
/// row 7, `experience` `MaxLvl` and `ExpRatio`) from the fixed-up set,
/// for `InteractionState::hireling_tables`. A missing or malformed table
/// is an error (M07).
pub fn hireling_tables(set: &FixedSet) -> Result<HirelingTables, WorldDataError> {
    hireling_tables_by(|name| set.table(name))
}

/// [`hireling_tables`] over a table lookup.
pub fn hireling_tables_by<'a>(
    lookup: impl Fn(&str) -> Option<&'a BinTable>,
) -> Result<HirelingTables, WorldDataError> {
    let get = |name: &str| lookup(name).ok_or_else(|| table_err(name, "not loaded"));
    HirelingTables::from_tables(get("hireling")?, get("pettype")?, get("experience")?)
        .map_err(|e| table_err("hireling", e))
}

/// The drop tables of the treasure walk (`treasure.md` Inputs; the drop
/// state of `ActionHooks::object_drops`, `d2_sim::wiring::economy::
/// DeathDrops`): the item tables (`ItemTables::from_fixed`), the runtime
/// treasure classes (`treasure.md` §1, `TreasureClasses::build` over
/// `treasureclassex`, `itemtypes`, the item list, the itemtypes
/// equivalence, `uniqueitems` and `setitems`), the item list in its
/// index order (`item_list`: weapons, armor, misc) and the
/// `superuniques` rows. A missing or malformed table, or treasure
/// classes that do not build, is an error (M07).
pub fn drop_tables(set: &FixedSet) -> Result<DropTables, WorldDataError> {
    let items = ItemTables::from_fixed(set).map_err(|e| table_err("items", e))?;
    let treasure_items = item_list(
        &records::<Weapons>(set)?,
        &records::<Armor>(set)?,
        &records::<Misc>(set)?,
    );
    let tcs = TreasureClasses::build(&TcSources {
        treasureclassex: &records::<Treasureclassex>(set)?,
        itemtypes: &records::<Itemtypes>(set)?,
        items: &treasure_items,
        equiv: &set.itemtypes_equiv,
        uniqueitems: &records::<Uniqueitems>(set)?,
        setitems: &records::<Setitems>(set)?,
    })
    .map_err(|e| table_err(Treasureclassex::TABLE, e))?;
    Ok(DropTables {
        items,
        tcs,
        treasure_items,
        superuniques: records::<Superuniques>(set)?,
    })
}

/// The tables the `.d2s` reader and writer read (`formats/d2s.md` Inputs,
/// `d2s::SaveTables`): the `itemstatcost` save columns (`CSvBits`,
/// `CSvParam`, `CSvSigned`), the item tables of the item stream
/// (`items/bitstream.md`, `read_save_entry`: an entry's length with its
/// socketed children, §8.1 rule 2) and the hireling rows of the restore
/// test (§8.4 rule 2: present, `hirelings.md` §10 rule 1, and its row
/// found, rule 2: `Id` at level 1 in the game's version).
#[derive(Clone, Debug)]
pub struct SaveData {
    pub stats: Vec<StatSave>,
    pub items: ItemTables,
    pub hirelings: HirelingRows,
    /// Expansion game: the hireling rows of the expansion version
    /// (`hirelings.md` §1.2 rule 2).
    pub expansion: bool,
}

impl SaveData {
    /// From the fixed-up set for a game of `expansion`; a missing or
    /// malformed table is an error (M07).
    pub fn from_fixed(set: &FixedSet, expansion: bool) -> Result<Self, WorldDataError> {
        let stats = records::<Itemstatcost>(set)?
            .iter()
            .map(|r| StatSave {
                bits: r.csvbits,
                param: r.csvparam,
                signed: r.csvsigned,
            })
            .collect();
        Ok(Self {
            stats,
            items: ItemTables::from_fixed(set).map_err(|e| table_err("items", e))?,
            hirelings: HirelingRows::from_table(table(set, "hireling")?)
                .map_err(|e| table_err("hireling", e))?,
            expansion,
        })
    }
}

impl d2s::SaveTables for SaveData {
    fn stat_save(&self, id: u16) -> Option<StatSave> {
        self.stats.get(usize::from(id)).copied()
    }
    fn item_entry_len(&self, buf: &[u8]) -> Result<usize, String> {
        read_save_entry(buf, &self.items)
            .map(|e| e.len)
            .map_err(|e| e.to_string())
    }
    fn hireling_restored(&self, h: &Hireling) -> bool {
        h.is_present()
            && self
                .hirelings
                .row_at(self.expansion, u32::from(h.id), 1)
                .is_some()
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
