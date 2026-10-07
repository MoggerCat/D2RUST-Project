// Spec: specs/drlg/levels.md, specs/drlg/rooms.md, specs/data/runtime-maps.md §9 (portal level list)
//! The table view the DRLG reads, built from `d2_data::tables` records
//! (`leveldefs`, `lvlwarp`, `lvltypes`, `objects`). Only the columns the
//! two specs name are kept. Plus the code tables of `rooms.md` §9.5–§9.6:
//! [`WallRemap`] embedded from `specs/drlg/wall-remap.tsv`
//! (`drlg/wall-remap.md`), and [`DoorTables`], not transcribed yet (an
//! input with no built-in values; open question in the notes).

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
    /// `Portal` (+0x8C): ≠ 0 puts the level in the portal level list
    /// (`data/runtime-maps.md` §9, [`DrlgData::portal_levels`]).
    pub portal: u32,
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
            portal: r.portal,
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

/// `specs/drlg/wall-remap.tsv` (`drlg/wall-remap.md` §1).
pub const WALL_REMAP_TSV: &str = include_str!("../../../../specs/drlg/wall-remap.tsv");

/// Number of new cell types the index table `0x006EF620` covers
/// (`wall-remap.md` §2 rule 1).
pub const WALL_REMAP_TYPES: usize = 20;

/// The class of a new cell type (`wall-remap.md` §1, column `class`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WallClass {
    /// Merged type = entry `R.type` (columns `r0`..`r7`) when R's type is
    /// ≤ 7, else stop.
    Table([u32; 8]),
    /// Merged type = the new type, whatever R's type.
    Keep,
    /// No merge: the record is left as it is, no flag rules.
    Stop,
}

/// Wall-type remap of linked cells (`rooms.md` §9.6 step 3 rule 4): the
/// index table `0x006EF620` and the 6×7 table `0x006EF578` as
/// `drlg/wall-remap.md` transcribes them, one class per new type 0..19.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WallRemap {
    /// Row = new cell type.
    pub classes: [WallClass; WALL_REMAP_TYPES],
}

impl Default for WallRemap {
    /// The 1.14d table ([`WallRemap::original`]).
    fn default() -> Self {
        Self::original()
    }
}

impl WallRemap {
    /// The 1.14d table, parsed from [`WALL_REMAP_TSV`].
    pub fn original() -> Self {
        Self::parse(WALL_REMAP_TSV).expect("specs/drlg/wall-remap.tsv is well-formed")
    }

    /// Strict parse of the `wall-remap.tsv` layout (`wall-remap.md`
    /// "Constants"): the header, then 20 rows of 10 columns, `new_type`
    /// equal to the row index; `table` rows carry 8 values, `keep` and
    /// `stop` rows 8 empty cells.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let header = "new_type\tclass\tr0\tr1\tr2\tr3\tr4\tr5\tr6\tr7";
        if lines.next() != Some(header) {
            return Err("bad header".into());
        }
        let mut classes = [WallClass::Stop; WALL_REMAP_TYPES];
        let mut n = 0;
        for line in lines {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() != 10 {
                return Err(format!("row {n}: {} columns", f.len()));
            }
            if n >= WALL_REMAP_TYPES || f[0] != n.to_string() {
                return Err(format!("row {n}: new_type {:?}", f[0]));
            }
            let empty = f[2..].iter().all(|c| c.is_empty());
            classes[n] = match f[1] {
                "keep" if empty => WallClass::Keep,
                "stop" if empty => WallClass::Stop,
                "table" => {
                    let mut r = [0u32; 8];
                    for (v, c) in r.iter_mut().zip(&f[2..]) {
                        *v = c.parse().map_err(|_| format!("row {n}: value {c:?}"))?;
                    }
                    WallClass::Table(r)
                }
                c => return Err(format!("row {n}: class {c:?}")),
            };
            n += 1;
        }
        if n != WALL_REMAP_TYPES {
            return Err(format!("{n} rows"));
        }
        Ok(Self { classes })
    }

    /// The 1.14d classes with `table` as the 6×7 table `0x006EF578`
    /// (rows for new types 1, 2, 3, 5, 6, 7; column = R's type − 1), `r0`
    /// read as the dword before the row (`wall-remap.md` §2 rule 3).
    #[cfg(test)]
    pub(crate) fn with_rows(table: [[u32; 7]; 6]) -> Self {
        let mut m = Self::original();
        let mut before = 0;
        for (i, t) in [1, 2, 3, 5, 6, 7].into_iter().enumerate() {
            let mut r = [before; 8];
            r[1..].copy_from_slice(&table[i]);
            before = table[i][6];
            m.classes[t] = WallClass::Table(r);
        }
        m
    }

    /// The class of new cell type `t`; `None` beyond the 20 rows.
    pub fn class(&self, t: u32) -> Option<WallClass> {
        self.classes.get(t as usize).copied()
    }
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
    /// See [`WallRemap`] (the 1.14d table by default).
    pub wall_remap: WallRemap,
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
            wall_remap: WallRemap::original(),
            doors: DoorTables::default(),
        }
    }

    /// The portal level list (`leveldefs_portals`, `data/runtime-maps.md`
    /// §9, `0x0061DD00`): the level ids whose `Portal` ≠ 0, in order.
    pub fn portal_levels(&self) -> Vec<u32> {
        (0u32..)
            .zip(&self.levels)
            .filter(|(_, l)| l.portal != 0)
            .map(|(i, _)| i)
            .collect()
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
