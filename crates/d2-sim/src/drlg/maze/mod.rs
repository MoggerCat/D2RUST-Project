// Spec: specs/drlg/maze.md
//! Maze levels (DrlgType 1): a grid of equal-sized preset cells grown
//! from the middle of the level rectangle, linked by shape, stamped with
//! special cells, moved into the level rect, themed, then handed to the
//! preset builder with a DS1 file index.
//!
//! Module map:
//! - [`specials`]: the special-room tables, parsed from
//!   `specs/drlg/maze-specials.tsv` (§3.6);
//! - [`cells`]: cells, links and the cell primitives (§2–§3);
//! - [`layout`]: the generation sequence, layout builders, special cells,
//!   neighbour placement, theme pass and build (§4–§9).
//!
//! Cells are real DRLG rooms of the level ([`Drlg::alloc_room`], which
//! draws the level-seed and room-seed steps of §3.1), kept in the level's
//! room list newest first. Their maze data (def, file, lock, links) lives
//! here beside them: `drlg::room` has no orth links.
//!
//! Seams: [`MazePresets`] reaches `drlg::preset` (DS1 map alloc and its
//! file draw, building a map into rooms, a preset level's direction) and
//! the act's get-or-allocate. [`Maze`] is not a [`super::LevelTypes`]:
//! the composite provider dispatching by DrlgType calls
//! [`Maze::init_level`], [`Maze::generate`] and [`Maze::reset_level`] and
//! maps [`MazeError`] (DrlgError has no variant for it yet).

pub mod cells;
pub mod layout;
pub mod specials;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use d2_data::tables::{Lvlmaze, Lvlprest};
use thiserror::Error;

use super::data::DrlgData;
use super::level::Drlg;
use super::{DrlgError, LevelIdx, TileRect};

pub use cells::{Cell, LinkTarget, MazeLink};
pub use specials::{SpecialRow, Specials, SPECIALS_TSV};

/// One `lvlmaze.bin` record (§1.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MazeRow {
    /// `Level` (+0).
    pub level: u32,
    /// `Rooms[d]` (+4 + 4·d).
    pub rooms: [u32; 3],
    /// `SizeX`, `SizeY` (+16, +20): cell size in tiles.
    pub size_x: i32,
    pub size_y: i32,
    /// `Merge` (+24): per-mille link chance (compared signed).
    pub merge: i32,
}

impl MazeRow {
    pub fn from_record(r: &Lvlmaze) -> Self {
        Self {
            level: r.level,
            rooms: [r.rooms, r.rooms_n, r.rooms_h],
            size_x: r.sizex as i32,
            size_y: r.sizey as i32,
            merge: r.merge as i32,
        }
    }
}

/// The tables maze code reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MazeData {
    /// lvlmaze records in file order.
    pub rows: Vec<MazeRow>,
    /// lvlprest `Def` → `Files` (+64), first row of each def.
    pub prest_files: BTreeMap<u32, u32>,
    /// The special-room tables.
    pub specials: Specials,
}

impl MazeData {
    /// The view from decoded records (`d2_data::tables::decode_all`).
    pub fn from_tables(lvlmaze: &[Lvlmaze], lvlprest: &[Lvlprest]) -> Self {
        let mut prest_files = BTreeMap::new();
        for p in lvlprest {
            prest_files.entry(p.def).or_insert(p.files);
        }
        Self {
            rows: lvlmaze.iter().map(MazeRow::from_record).collect(),
            prest_files,
            specials: Specials::shipped(),
        }
    }

    /// `0x0061F490` (§1.1): the first record whose `Level` is `id`, by
    /// file order. No match is fatal.
    pub fn row_index(&self, id: u32) -> Result<usize, MazeError> {
        self.rows
            .iter()
            .position(|r| r.level == id)
            .ok_or(MazeError::NoMazeRow(id))
    }

    /// lvlprest `Files` of a def.
    pub fn files(&self, def: u32) -> Result<u32, MazeError> {
        self.prest_files
            .get(&def)
            .copied()
            .ok_or(MazeError::NoPrest(def))
    }
}

/// A DS1 map allocated by [`MazePresets::alloc_map`] (provider handle).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MapId(pub u32);

/// What maze code needs from `drlg::preset` and the act (narrow seam;
/// provider: the preset level type, `drlg/preset.md`, parallel session).
pub trait MazePresets {
    /// Get-or-allocate `0x00642BB0` through the act's level types
    /// (§7: Outer Cloister 27, Chaos Sanctum 108).
    fn level(&mut self, drlg: &mut Drlg, data: &DrlgData, id: u32) -> Result<LevelIdx, DrlgError>;

    /// A preset level's direction (its preset data +4, §7.1).
    fn preset_direction(&self, drlg: &Drlg, level: LevelIdx) -> Result<u32, DrlgError>;

    /// `0x00666ED0` (§9 step 1): allocate a DS1 map for `def` at `rect`
    /// in `level`. Draws `roll(Files)` on the level seed as the map's
    /// default file.
    fn alloc_map(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        level: LevelIdx,
        def: u32,
        rect: TileRect,
    ) -> Result<MapId, DrlgError>;

    /// Overwrite a map's file (§9 step 2).
    fn set_map_file(&mut self, map: MapId, file: i32);

    /// `0x00667ED0` (§9 steps 3–4): build the map into room(s), `small`
    /// when the cell is at most 12 × 12, then link the built room to each
    /// of `links` (the cell's init-flag links, in list order) with the
    /// same direction. The cells named by `links` may already be freed
    /// (built earlier in list order).
    // TODO(spec: maze.md §9 step 4): which built room carries the links
    // when one DS1 builds several rooms, and what a link to an already
    // freed cell resolves to, belong to drlg/preset.md; the provider
    // decides.
    fn build_map(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        level: LevelIdx,
        map: MapId,
        small: bool,
        links: &[MazeLink],
    ) -> Result<(), DrlgError>;
}

/// Maze generation errors: the original's fatal paths (edge cases 3, 7)
/// and missing data.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MazeError {
    #[error(transparent)]
    Drlg(#[from] DrlgError),
    #[error("no lvlmaze record for level {0} (fatal in the original)")]
    NoMazeRow(u32),
    #[error("level {0} has no lvlmaze record stored (init_level not run)")]
    NotInitialized(u32),
    #[error("lvlmaze record has no Rooms entry for difficulty {0}")]
    BadDifficulty(u8),
    #[error("level type {0} is not a maze type (\"Some really bad voodoo\")")]
    BadLevelType(u32),
    #[error("no lvlprest row with Def {0}")]
    NoPrest(u32),
    #[error("lvlprest Def {0} has Files 0 (division by zero in the original)")]
    ZeroFiles(u32),
    #[error("placement rejected where the original dereferences the null cell: {0}")]
    NullCell(&'static str),
    #[error("cells do not fit the level rect (fatal in 1.14d)")]
    DoesNotFit,
    #[error("Act 3 Sewers 1 corner swap: no unlocked cell with def {0} (fatal in 1.14d)")]
    MissingSwap(u32),
    #[error("tomb hub def {0:?} is not 444..447 (fatal in the original)")]
    TombHub(Option<u32>),
    #[error("Outer Cloister preset direction {0} outside the finder table")]
    BarracksDirection(u32),
    #[error("special table {0} has no row {1}")]
    NoSpecialRow(&'static str, usize),
}

/// A rotation record (level +0x1CC, `0x0045C3E0`): def, file count n,
/// last value v.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rotation {
    pub def: u32,
    pub n: u32,
    pub v: i32,
}

/// Maze type data of one level: the lvlmaze record (+0x14) and the file
/// rotation list (+0x1CC, head first).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MazeLevel {
    pub row: Option<usize>,
    pub rotation: Vec<Rotation>,
}

/// The maze level type: its tables and the per-level type data of every
/// act (keyed by act and level slot).
#[derive(Clone, Debug, Default)]
pub struct Maze {
    pub data: MazeData,
    levels: BTreeMap<(u8, LevelIdx), MazeLevel>,
}

impl Maze {
    pub fn new(data: MazeData) -> Self {
        Self {
            data,
            levels: BTreeMap::new(),
        }
    }

    /// `0x00673B10` (§1.1): store the level's lvlmaze record. The DRLG
    /// then sets the level's position and size (`levels.md` §6.1).
    pub fn init_level(&mut self, drlg: &Drlg, level: LevelIdx) -> Result<(), MazeError> {
        let row = self.data.row_index(drlg.level(level).id)?;
        self.levels.entry((drlg.act, level)).or_default().row = Some(row);
        Ok(())
    }

    /// The type data of a level, if any.
    pub fn level_data(&self, drlg: &Drlg, level: LevelIdx) -> Option<&MazeLevel> {
        self.levels.get(&(drlg.act, level))
    }

    /// Type data reset when the level's rooms are freed and the level is
    /// kept (`levels.md` §9.4): the DRLG frees the build list (+0x1CC),
    /// i.e. the rotation records; `0x00673FE0` keeps the lvlmaze record
    /// (it clears it only when the level itself is freed,
    /// [`Maze::free_level`]).
    pub fn reset_level(&mut self, drlg: &Drlg, level: LevelIdx) {
        if let Some(m) = self.levels.get_mut(&(drlg.act, level)) {
            m.rotation.clear();
        }
    }

    /// `0x00673FE0` when the level is freed without keeping it (§1.1).
    pub fn free_level(&mut self, drlg: &Drlg, level: LevelIdx) {
        self.levels.remove(&(drlg.act, level));
    }

    /// `0x00673B30` (§4): generate the level's cells and build them.
    pub fn generate(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        presets: &mut dyn MazePresets,
        level: LevelIdx,
    ) -> Result<(), MazeError> {
        let id = drlg.level(level).id;
        let key = (drlg.act, level);
        let row = self
            .levels
            .get(&key)
            .and_then(|m| m.row)
            .ok_or(MazeError::NotInitialized(id))?;
        let row = self.data.rows[row];
        let rotation = &mut self.levels.get_mut(&key).expect("stored above").rotation;
        let mut gen = cells::Gen::new(drlg, level, row, &self.data)?;
        layout::generate(&mut gen, data, presets, rotation)
    }
}
