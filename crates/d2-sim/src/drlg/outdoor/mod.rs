// Spec: specs/drlg/outdoor.md
//! Outdoor levels (leveldefs `DrlgType` 3): the act-wide placer that
//! positions an act's outdoor chain at DRLG creation (§2, §9.1–§9.2), and
//! level generation (§3–§12): coarse 8×8-tile cell grids, the vertex
//! polygon, borders, cliffs, rivers, paths, fixed presets, lvlsub
//! substitution ([`tilesub`]) and one DRLG room per cell.
//!
//! State lives in [`Outdoor`] (one per act DRLG, beside the [`Drlg`]):
//! the original's outdoor info (level +0x14) per level and the outdoor
//! room data (room +0x20) per room. Table data comes from
//! `d2_data::tables` through [`OutdoorData`]. Two narrow seams reach code
//! other specs own:
//!
//! | Seam | What | Provider |
//! |---|---|---|
//! | [`OutdoorPresets`] | preset map + build area of a preset cell (`preset.md` §4, §6) | `drlg::preset` (parallel session) |
//! | [`SubFiles`] | parsed lvlsub DS1 files (`preset.md` §5 loader, `formats/ds1.md`) | the caller holding parsed DS1s (d2-server / world) |
//!
//! [`OutdoorTypes`] adapts this module to [`LevelTypes`] for outdoor
//! levels and forwards every other level to another provider.
//!
//! Module map:
//! - [`grid`]: cell grids, stamping, fit tests, shuffles, placers (§1, §5);
//! - [`place`]: act-wide placement (§2, §9.1, §9.2);
//! - [`vertex`]: vertex polygon, link flags, borders (§4, §5.5, §6);
//! - [`wild`]: Act I (§7);
//! - [`acts`]: Acts II–V (§8, §9.3, §10, §11);
//! - [`rooms`]: cells to rooms, outdoor room grids (§12);
//! - [`tilesub`]: lvlsub substitution (`outdoor-tilesub.md`).

pub mod acts;
pub mod grid;
pub mod place;
pub mod rooms;
pub mod tilesub;
pub mod vertex;
pub mod wild;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use d2_data::tables::{text, Leveldefs, Lvlprest, Lvlsub};
use thiserror::Error;

use super::data::DrlgData;
use super::level::Drlg;
use super::seams::{LevelTypes, PresetUnit};
use super::tiles::{CellGrid, RoomGrids};
use super::{act_of_level, DrlgError, DrlgRoomId, LevelIdx};

pub use grid::Grid;
pub use tilesub::SubRow;

/// DRLG type of outdoor levels (leveldefs `DrlgType`).
pub const DRLG_OUTDOOR: u32 = 3;

/// Fatal errors of the outdoor code (the original asserts) and seam
/// failures. [`OutdoorTypes`] reports them through [`DrlgError`] (see
/// there).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum OutdoorError {
    #[error(transparent)]
    Drlg(#[from] DrlgError),
    #[error("lvlprest row {0} does not exist")]
    UnknownPreset(u32),
    #[error("build list of lvlprest {0} has Files = 0 (division by zero, outdoor.md edge case 3)")]
    NoFiles(u32),
    #[error("neighbour entry with direction {0} (fatal, outdoor.md §4)")]
    UnknownDirection(i32),
    #[error("Act II exit preset {0} not placed (fatal, outdoor.md §8)")]
    ExitNotPlaced(u32),
    #[error("lvlsub DS1 file not supplied: {0:?}")]
    MissingSubFile(Vec<u8>),
    #[error("lvlsub DS1 file without groups: {0:?} (fatal, outdoor-tilesub.md §1)")]
    SubFileNoGroups(Vec<u8>),
    #[error("no lvlsub rows of type {0} (fatal, outdoor-tilesub.md §1)")]
    NoSubRows(i32),
    #[error("Act V style map has no row for style {0}, value {1} (fatal)")]
    StyleMap(u32, u32),
    #[error("sub theme {0} out of range 0..4")]
    SubTheme(i32),
    #[error("siege strip piece {0} at x < 0 (fatal, outdoor.md §11)")]
    SiegeStrip(u32),
    #[error("link driver backtracked past its first row")]
    DriverUnderflow,
    #[error("level {0} is not allocated")]
    LevelMissing(u32),
}

// ---- data ----------------------------------------------------------------

/// The leveldefs columns this spec reads beyond [`DrlgData`] (`SubType`,
/// `SubTheme`, `SubWaypoint`, `SubShrine`; −1 = none).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubDefs {
    pub sub_type: i32,
    pub sub_theme: i32,
    pub sub_waypoint: i32,
    pub sub_shrine: i32,
}

/// One lvlprest row (row index = `Def`, `preset.md` §2.2): the columns
/// this spec reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PresetDef {
    pub size_x: i32,
    pub size_y: i32,
    /// `Files`: drives every build-list roll (§5.1).
    pub files: i32,
}

/// The table view of this spec: leveldefs sub columns, lvlprest sizes
/// and file counts, lvlsub rows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutdoorData {
    /// Row = level id.
    pub levels: Vec<SubDefs>,
    /// Row = lvlprest index (`Def`).
    pub presets: Vec<PresetDef>,
    /// lvlsub rows in file order.
    pub subs: Vec<SubRow>,
}

impl OutdoorData {
    pub fn from_tables(leveldefs: &[Leveldefs], lvlprest: &[Lvlprest], lvlsub: &[Lvlsub]) -> Self {
        let i = |v: u32| v as i32;
        Self {
            levels: leveldefs
                .iter()
                .map(|r| SubDefs {
                    sub_type: i(r.subtype),
                    sub_theme: i(r.subtheme),
                    sub_waypoint: i(r.subwaypoint),
                    sub_shrine: i(r.subshrine),
                })
                .collect(),
            presets: lvlprest
                .iter()
                .map(|r| PresetDef {
                    size_x: i(r.sizex),
                    size_y: i(r.sizey),
                    files: i(r.files),
                })
                .collect(),
            subs: lvlsub
                .iter()
                .map(|r| SubRow {
                    type_: i(r.type_),
                    file: text(&r.file[..]).to_vec(),
                    check_all: i(r.checkall),
                    bord_type: i(r.bordtype),
                    dt1_mask: r.dt1mask,
                    grid_size: i(r.gridsize),
                    prob: [r.prob0, r.prob1, r.prob2, r.prob3, r.prob4].map(i),
                    trials: [r.trials0, r.trials1, r.trials2, r.trials3, r.trials4].map(i),
                    max: [r.max0, r.max1, r.max2, r.max3, r.max4].map(i),
                })
                .collect(),
        }
    }

    pub fn preset(&self, p: u32) -> Result<&PresetDef, OutdoorError> {
        self.presets
            .get(p as usize)
            .ok_or(OutdoorError::UnknownPreset(p))
    }

    /// Leveldefs sub columns of a level (all −1 for a missing row).
    pub fn sub_defs(&self, id: u32) -> SubDefs {
        self.levels.get(id as usize).copied().unwrap_or(SubDefs {
            sub_type: -1,
            sub_theme: -1,
            sub_waypoint: -1,
            sub_shrine: -1,
        })
    }
}

// ---- seams ----------------------------------------------------------------

/// The preset-room side of a preset cell (§12.1). Provider:
/// `drlg::preset` (`preset.md` §4 and §6).
pub trait OutdoorPresets {
    /// One preset cell of an outdoor level, in this order: allocate a
    /// preset map for lvlprest `def` at tile `(x, y)` (`0x00666ED0`,
    /// `preset.md` §4) with **the level seed** — it draws
    /// `roll(Files(def))` (site `0x00666F33`, edge case 4) on
    /// `drlg.level_mut(level).seed`; set the map's file := `file`
    /// (`0x00666EC0`); build the area with room flags `room_flags`
    /// (`0x00667ED0`, `preset.md` §6), which creates and links the rooms.
    #[allow(clippy::too_many_arguments)]
    fn build_preset_cell(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        level: LevelIdx,
        def: u32,
        x: i32,
        y: i32,
        file: u32,
        room_flags: u32,
    ) -> Result<(), DrlgError>;
}

/// One group of a substitution DS1 (`outdoor-tilesub.md` §1.4): box and
/// variant count N (DS1 group `unknown`, tilesub open question 1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubGroup {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub variants: i32,
}

/// A loaded substitution DS1 (`outdoor-tilesub.md` §1.3–§1.4). Grids
/// are (DS1 width + 1) × (height + 1) packed cells as `formats/ds1.md`
/// decodes them. Wall layer `k ≥ 1` is read ORed with `k << 18` (§1.3);
/// the provider supplies the raw values.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SubFile {
    /// DS1 substitution method (+0x00): 1 fixed, 2 random.
    pub method: u32,
    pub groups: Vec<SubGroup>,
    /// Floor layer 0, if the file has floor layers.
    pub floor: Option<CellGrid>,
    /// Wall layers in order.
    pub walls: Vec<CellGrid>,
    /// Tile-type (orientation) grid per wall layer.
    pub tile_types: Vec<CellGrid>,
    pub shadow: Option<CellGrid>,
    /// Preset units, positions in sub-tiles.
    pub units: Vec<PresetUnit>,
}

impl SubFile {
    /// Pattern floor value (0 without floor layers).
    pub fn floor_at(&self, x: i32, y: i32) -> u32 {
        self.floor.as_ref().map_or(0, |g| g.get(x, y))
    }

    /// Pattern wall value of layer `k` (0 without that layer), with
    /// `k << 18` for `k ≥ 1`.
    pub fn wall_at(&self, k: usize, x: i32, y: i32) -> u32 {
        self.walls
            .get(k)
            .map_or(0, |g| g.get(x, y) | ((k as u32) << 18))
    }

    pub fn tile_type_at(&self, k: usize, x: i32, y: i32) -> u32 {
        self.tile_types.get(k).map_or(0, |g| g.get(x, y))
    }

    pub fn shadow_at(&self, x: i32, y: i32) -> u32 {
        self.shadow.as_ref().map_or(0, |g| g.get(x, y))
    }
}

/// Parsed lvlsub DS1 files by lvlsub `File` string (no I/O in the sim).
/// Files stay loaded for the session (`outdoor-tilesub.md` §1.3).
pub trait SubFiles {
    fn sub_file(&self, file: &[u8]) -> Option<&SubFile>;
}

/// A [`SubFiles`] by name.
#[derive(Clone, Debug, Default)]
pub struct SubFileMap(pub BTreeMap<Vec<u8>, SubFile>);

impl SubFiles for SubFileMap {
    fn sub_file(&self, file: &[u8]) -> Option<&SubFile> {
        self.0.get(file)
    }
}

// ---- state ----------------------------------------------------------------

/// Vertex flag bit 0: the edge from this vertex is a level link (§1.4).
pub const VERTEX_LINK: u32 = 0x1;
/// Vertex flag bit 1: link to a preset level.
pub const VERTEX_PRESET_LINK: u32 = 0x2;

/// A polygon vertex (§1.4): cell coordinates after §3 step 2.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Vertex {
    pub x: i32,
    pub y: i32,
    pub direction: u8,
    pub flags: u32,
}

impl Vertex {
    pub fn is_link(&self) -> bool {
        self.flags & VERTEX_LINK != 0
    }

    pub fn is_preset_link(&self) -> bool {
        self.flags & VERTEX_PRESET_LINK != 0
    }
}

/// A neighbour ("orth") entry (§1.4): level, direction (0 W, 1 N, 2 E,
/// 3 S, −1 none from `0x00642240`), init flag, box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Orth {
    pub level_id: u32,
    pub direction: i32,
    pub init: bool,
    pub rect: super::TileRect,
    /// The neighbour is a preset level (DrlgType 2).
    pub preset: bool,
}

/// A build-list node (§1.5): preset id, file count, current file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildNode {
    pub preset: u32,
    pub files: i32,
    pub current: i32,
}

/// A path end-point (§1: x, y, direction).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PathPoint {
    pub x: i32,
    pub y: i32,
    pub direction: i32,
}

/// The four end-points of one dirt path (§7.5): start, start-adjusted,
/// join-adjusted, join.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PathEnds {
    pub start: PathPoint,
    pub start_adjusted: PathPoint,
    pub join_adjusted: PathPoint,
    pub join: PathPoint,
}

/// The outdoor info of a level (level +0x14, §1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutdoorLevel {
    /// Flags (+0x00, §1.3).
    pub flags: u32,
    /// Grids 0..3 (gw × gh).
    pub grids: [Grid; 4],
    /// Vertex polygon, head first (circular).
    pub vertices: Vec<Vertex>,
    /// Path end-points per path (§7.5).
    pub path_ends: Vec<PathEnds>,
    /// Path vertex lists (tile coordinates after jitter).
    pub paths: Vec<Vec<(i32, i32)>>,
    /// Neighbour entries, head first.
    pub orth: Vec<Orth>,
    /// Build list (level +0x1CC), head first.
    pub build_list: Vec<BuildNode>,
}

impl OutdoorLevel {
    pub fn gw(&self) -> i32 {
        self.grids[0].w
    }

    pub fn gh(&self) -> i32 {
        self.grids[0].h
    }
}

/// The outdoor room data (room +0x20, §12.2) and what the room's
/// substitutions produced at its last grid build.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutdoorRoom {
    /// Outdoor flags (+0x54) = the cell's grid-2 value.
    pub flags: u32,
    /// Flags ex (+0x58) = grid 3.
    pub flags_ex: u32,
    pub sub_type: i32,
    pub sub_theme: i32,
    /// Picked sub-theme rows (+0x6C).
    pub picked: u32,
    /// Grids of the last build (9 × 9): tile type, wall, floor.
    pub tile_type: CellGrid,
    pub wall: CellGrid,
    pub floor: CellGrid,
    /// Roof-list growth of the last build (`0x0066EFB0`).
    pub roof_count: u32,
    /// Shadow tiles of the last build: world tile (x, y) and cell value
    /// (`0x0066E060`).
    pub shadows: Vec<(i32, i32, u32)>,
    /// Preset units added by the last build (`0x0066BF30`), sub-tiles
    /// relative to the room.
    pub units: Vec<PresetUnit>,
}

/// The outdoor state of one act DRLG.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outdoor {
    pub levels: BTreeMap<LevelIdx, OutdoorLevel>,
    pub rooms: BTreeMap<DrlgRoomId, OutdoorRoom>,
    /// Preset direction (preset info +0x04) written by the placer for
    /// levels 1, 27 and 40 (§2.3 step 4), by level id. Read by
    /// `drlg::preset` (seam: the preset info is that module's).
    pub preset_direction: BTreeMap<u32, i32>,
}

impl Outdoor {
    pub fn level(&self, l: LevelIdx) -> Option<&OutdoorLevel> {
        self.levels.get(&l)
    }

    pub fn room(&self, r: DrlgRoomId) -> Option<&OutdoorRoom> {
        self.rooms.get(&r)
    }

    /// The info of a level, created zeroed on first use: the outdoor
    /// init at allocation (`0x00675320`) only zeroes it, so creating it
    /// lazily is the same state.
    pub fn info_mut(&mut self, l: LevelIdx) -> &mut OutdoorLevel {
        self.levels.entry(l).or_default()
    }

    /// Outdoor init at allocation (`0x00675320`, `levels.md` §4.3).
    pub fn init_level(&mut self, l: LevelIdx) {
        self.levels.insert(l, OutdoorLevel::default());
    }

    /// Type data reset when the level's rooms are freed (`0x006754C0`,
    /// `levels.md` §9.4): grids, polygon, paths and the build list.
    pub fn reset_level(&mut self, drlg: &Drlg, l: LevelIdx) {
        // TODO(outdoor.md §3, levels.md §9.4): what `0x006754C0` frees is
        // not stated. Kept: flags (+0x00) and neighbour entries, which
        // only act creation writes; a regeneration then starts from the
        // flags the last generation left (0x20, 0x40 included).
        let rooms: Vec<DrlgRoomId> = drlg.level_rooms(l);
        for r in rooms {
            self.rooms.remove(&r);
        }
        if let Some(info) = self.levels.get_mut(&l) {
            info.grids = Default::default();
            info.vertices.clear();
            info.path_ends.clear();
            info.paths.clear();
            info.build_list.clear();
        }
    }

    /// Free the type data of a room's tiles (`0x0067D680`).
    pub fn free_room_tiles(&mut self, room: DrlgRoomId) {
        if let Some(r) = self.rooms.get_mut(&room) {
            r.tile_type = CellGrid::default();
            r.wall = CellGrid::default();
            r.floor = CellGrid::default();
        }
    }
}

/// The act of a level for dispatch (§3 step 3): level 134 counts as
/// Act II.
pub fn dispatch_act(id: u32) -> u8 {
    if id == 134 {
        1
    } else {
        act_of_level(id)
    }
}

// ---- LevelTypes adapter ---------------------------------------------------

/// [`LevelTypes`] for outdoor levels (DrlgType 3); every other level and
/// every other hook goes to `others`.
///
/// Errors: [`LevelTypes`] returns [`DrlgError`], which has no outdoor
/// variants (this module may not edit it). An [`OutdoorError::Drlg`] is
/// returned as is; any other outdoor error is kept in `last_error` and
/// reported as `DrlgError::UnknownLevel(level id)`.
/// TODO(seam request): add a `DrlgError::Outdoor` variant.
pub struct OutdoorTypes<'a> {
    pub outdoor: &'a mut Outdoor,
    pub od: &'a OutdoorData,
    pub subs: &'a dyn SubFiles,
    pub presets: &'a mut dyn OutdoorPresets,
    pub others: &'a mut dyn LevelTypes,
    pub last_error: Option<OutdoorError>,
}

/// Allocation view used by the act placer: outdoor inits are lazy
/// ([`Outdoor::info_mut`]), everything else goes to the other provider.
struct NonOutdoor<'a> {
    others: &'a mut dyn LevelTypes,
}

impl LevelTypes for NonOutdoor<'_> {
    fn init_level(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        l: LevelIdx,
    ) -> Result<(), DrlgError> {
        if drlg.level(l).drlg_type == DRLG_OUTDOOR {
            return Ok(());
        }
        self.others.init_level(drlg, data, l)
    }

    fn generate(&mut self, drlg: &mut Drlg, data: &DrlgData, l: LevelIdx) -> Result<(), DrlgError> {
        self.others.generate(drlg, data, l)
    }
}

impl OutdoorTypes<'_> {
    fn report(&mut self, id: u32, e: OutdoorError) -> DrlgError {
        match e {
            OutdoorError::Drlg(d) => d,
            other => {
                self.last_error = Some(other);
                DrlgError::UnknownLevel(id)
            }
        }
    }

    fn is_outdoor(drlg: &Drlg, l: LevelIdx) -> bool {
        drlg.level(l).drlg_type == DRLG_OUTDOOR
    }
}

impl LevelTypes for OutdoorTypes<'_> {
    fn create_act_levels(&mut self, drlg: &mut Drlg, data: &DrlgData) -> Result<(), DrlgError> {
        let mut alloc = NonOutdoor {
            others: &mut *self.others,
        };
        let r = self
            .outdoor
            .create_act_levels(drlg, data, self.od, &mut alloc);
        r.map_err(|e| self.report(0, e))
    }

    fn init_level(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        l: LevelIdx,
    ) -> Result<(), DrlgError> {
        if Self::is_outdoor(drlg, l) {
            self.outdoor.init_level(l);
            Ok(())
        } else {
            self.others.init_level(drlg, data, l)
        }
    }

    fn generate(&mut self, drlg: &mut Drlg, data: &DrlgData, l: LevelIdx) -> Result<(), DrlgError> {
        if !Self::is_outdoor(drlg, l) {
            return self.others.generate(drlg, data, l);
        }
        let id = drlg.level(l).id;
        let r = self
            .outdoor
            .generate(drlg, data, self.od, self.subs, &mut *self.presets, l);
        r.map_err(|e| self.report(id, e))
    }

    fn reset_level(&mut self, drlg: &mut Drlg, l: LevelIdx) {
        if Self::is_outdoor(drlg, l) {
            self.outdoor.reset_level(drlg, l);
        } else {
            self.others.reset_level(drlg, l);
        }
    }

    fn add_preset_units(&mut self, drlg: &mut Drlg, room: DrlgRoomId) -> Result<(), DrlgError> {
        self.others.add_preset_units(drlg, room)
    }

    fn preset_units(&self, drlg: &Drlg, room: DrlgRoomId) -> Vec<PresetUnit> {
        self.others.preset_units(drlg, room)
    }

    fn room_grids(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        room: DrlgRoomId,
    ) -> Result<RoomGrids, DrlgError> {
        if !self.outdoor.rooms.contains_key(&room) {
            return self.others.room_grids(drlg, data, room);
        }
        let id = drlg.level(drlg.room(room).level).id;
        let r = self.outdoor.room_grids(drlg, self.od, self.subs, room);
        r.map_err(|e| self.report(id, e))
    }

    fn free_room_tiles(&mut self, drlg: &mut Drlg, room: DrlgRoomId) {
        if self.outdoor.rooms.contains_key(&room) {
            self.outdoor.free_room_tiles(room);
        } else {
            self.others.free_room_tiles(drlg, room);
        }
    }

    fn door_unit(
        &mut self,
        drlg: &mut Drlg,
        data: &DrlgData,
        room: DrlgRoomId,
        wx: i32,
        wy: i32,
        cell: u32,
        orientation: u32,
    ) {
        self.others
            .door_unit(drlg, data, room, wx, wy, cell, orientation);
    }

    fn warp_unit(&mut self, drlg: &mut Drlg, room: DrlgRoomId, wx: i32, wy: i32, cell: u32) {
        self.others.warp_unit(drlg, room, wx, wy, cell);
    }
}
