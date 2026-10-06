// Spec: specs/drlg/levels.md, specs/drlg/rooms.md
//! The table view the DRLG reads, built from `d2_data::tables` records
//! (`leveldefs`, `lvlwarp`, `lvltypes`, `objects`). Only the columns the
//! two specs name are kept. Plus the code tables of `rooms.md` §9.5–§9.6
//! that the spec has not transcribed yet ([`WallRemap`], [`DoorTables`]):
//! inputs with no built-in values (open questions in the notes).

use d2_data::tables::{text, Leveldefs, Lvltypes, Lvlwarp, Objects};

use super::DrlgError;

/// One `leveldefs` row (row index = level id), the columns of
/// `levels.md` "Data".
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LevelDef {
    /// `DrlgType` (+0x30): 1 maze, 2 preset, 3 outdoor; others nothing.
    pub drlg_type: u32,
    /// `LevelType` (+0x34): `lvltypes` row.
    pub level_type: u32,
    /// `SizeX`, `SizeY` per difficulty (Normal, Nightmare, Hell).
    pub size: [(i32, i32); 3],
    /// `OffsetX`, `OffsetY`.
    pub offset: (i32, i32),
    /// `Depend`: level whose position is added (0 = none).
    pub depend: u32,
    /// `Vis0..7` (+0x48): neighbouring level ids, 0 = none.
    pub vis: [u32; 8],
    /// `Warp0..7` (+0x68): lvlwarp `Id`, −1 = none.
    pub warp: [i32; 8],
    /// `Position` (+0x90).
    pub position: u32,
}

impl LevelDef {
    pub fn from_record(r: &Leveldefs) -> Self {
        let i = |v: u32| v as i32;
        Self {
            drlg_type: r.drlgtype,
            level_type: r.leveltype,
            size: [
                (i(r.sizex), i(r.sizey)),
                (i(r.sizex_n), i(r.sizey_n)),
                (i(r.sizex_h), i(r.sizey_h)),
            ],
            offset: (i(r.offsetx), i(r.offsety)),
            depend: r.depend,
            vis: [
                r.vis0, r.vis1, r.vis2, r.vis3, r.vis4, r.vis5, r.vis6, r.vis7,
            ],
            warp: [
                i(r.warp0),
                i(r.warp1),
                i(r.warp2),
                i(r.warp3),
                i(r.warp4),
                i(r.warp5),
                i(r.warp6),
                i(r.warp7),
            ],
            position: r.position,
        }
    }
}

/// One `lvlwarp` row (file order kept).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WarpDef {
    pub id: i32,
    /// `Direction`: `b'b'`, `b'l'`, `b'r'`, ...
    pub direction: u8,
    pub lit_version: u32,
    pub tiles: u32,
}

impl WarpDef {
    pub fn from_record(r: &Lvlwarp) -> Self {
        Self {
            id: r.id as i32,
            direction: r.direction[0],
            lit_version: r.litversion,
            tiles: r.tiles,
        }
    }
}

/// Wall-type remap of linked cells (`rooms.md` §9.6): the 6×7 table
/// `0x006EF578` (row = the index of the new type from table `0x006EF620`,
/// which the spec gives and [`super::tiles`] holds; column = existing
/// record type 1..7). The values are not transcribed in the spec
/// ("identical to D2MOO's `nWallTileTypeRemap`"): supplied by the caller.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WallRemap {
    /// Merged type for row `i`, column `R.type − 1`.
    pub table: [[u32; 7]; 6],
}

/// Door unit tables `0x006EEFD0` / `0x006EF18C` (`rooms.md` §9.5,
/// open question 10). Not transcribed: kept for the door hook
/// ([`super::LevelTypes::door_unit`]); empty by default.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DoorTables {
    /// (level id, first row, last row).
    pub levels: Vec<(u32, u32, u32)>,
    /// main, sub, right-door flag, unit id, unit type, dx, dy.
    pub rows: Vec<[i32; 7]>,
}

/// Everything the DRLG reads from tables.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DrlgData {
    /// `leveldefs`, row = level id.
    pub levels: Vec<LevelDef>,
    /// `lvlwarp` rows in file order.
    pub warps: Vec<WarpDef>,
    /// `lvltypes` `File 1..32` per row (level type), NUL-trimmed.
    pub lvltypes: Vec<Vec<Vec<u8>>>,
    /// `objects` subclass per object class (row).
    pub object_subclass: Vec<u8>,
    /// See [`WallRemap`]; `None` until a spec transcribes it.
    pub wall_remap: Option<WallRemap>,
    /// See [`DoorTables`].
    pub doors: DoorTables,
}

impl DrlgData {
    /// The view from decoded records (`d2_data::tables::decode_all`).
    pub fn from_tables(
        leveldefs: &[Leveldefs],
        lvlwarp: &[Lvlwarp],
        lvltypes: &[Lvltypes],
        objects: &[Objects],
    ) -> Self {
        Self {
            levels: leveldefs.iter().map(LevelDef::from_record).collect(),
            warps: lvlwarp.iter().map(WarpDef::from_record).collect(),
            lvltypes: lvltypes.iter().map(lvltype_files).collect(),
            object_subclass: objects.iter().map(|o| o.subclass).collect(),
            wall_remap: None,
            doors: DoorTables::default(),
        }
    }

    /// The leveldefs row of a level id.
    pub fn level(&self, id: u32) -> Result<&LevelDef, DrlgError> {
        self.levels
            .get(id as usize)
            .ok_or(DrlgError::UnknownLevel(id))
    }

    /// `0x0061F310` (`levels.md` §7.4): the first lvlwarp row (file order)
    /// with `Id` = `id` whose direction matches `dir` (`'b'` requests or
    /// rows match anything). Returns the row index.
    pub fn lvlwarp_row(&self, id: i32, dir: u8) -> Result<usize, DrlgError> {
        self.warps
            .iter()
            .position(|w| w.id == id && (dir == b'b' || w.direction == b'b' || w.direction == dir))
            .ok_or(DrlgError::NoLvlWarp(id))
    }

    /// `lvltypes` `File(i+1)` of a level type, if non-empty.
    pub fn lvltype_file(&self, level_type: u32, i: usize) -> Option<&[u8]> {
        let f = self.lvltypes.get(level_type as usize)?.get(i)?;
        (!f.is_empty()).then_some(f.as_slice())
    }

    /// objects.txt subclass bit 0x40 (waypoint) of an object class.
    pub fn is_waypoint_object(&self, class: u32) -> bool {
        self.object_subclass
            .get(class as usize)
            .is_some_and(|s| s & 0x40 != 0)
    }
}

fn lvltype_files(r: &Lvltypes) -> Vec<Vec<u8>> {
    [
        &r.file_1, &r.file_2, &r.file_3, &r.file_4, &r.file_5, &r.file_6, &r.file_7, &r.file_8,
        &r.file_9, &r.file_10, &r.file_11, &r.file_12, &r.file_13, &r.file_14, &r.file_15,
        &r.file_16, &r.file_17, &r.file_18, &r.file_19, &r.file_20, &r.file_21, &r.file_22,
        &r.file_23, &r.file_24, &r.file_25, &r.file_26, &r.file_27, &r.file_28, &r.file_29,
        &r.file_30, &r.file_31, &r.file_32,
    ]
    .iter()
    .map(|f| text(&f[..]).to_vec())
    .collect()
}
