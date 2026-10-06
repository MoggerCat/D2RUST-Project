// Spec: specs/drlg/preset.md §5; specs/drlg/outdoor-tilesub.md §1; specs/drlg/rooms.md §9.3; specs/data/fixups.md §12
//! Real-file providers of the DRLG data seams: parsed DS1 files for
//! [`Ds1Source`] (lvlprest) and [`SubFiles`] (lvlsub), parsed DT1 tile
//! headers for [`TileSource`], and the table views the level types read
//! ([`tables`]).
//!
//! The sim does no I/O: the server reads the bytes (from the archive set,
//! [`archive`]), parses them with `d2_formats::{ds1, dt1}` and hands the
//! sim the seam types. This module adds no DRLG rule: the 1.14d parser
//! rules of `preset.md` §5.2–§5.3 stay in `d2_sim::drlg::preset`
//! ([`Ds1File::from_input`]); here the parsed file is only re-expressed
//! as the seam's plain data (stored width / height, orientations as
//! stored).
//!
//! Everything is loaded up front ([`WorldFiles::load`]): every DS1 a
//! lvlprest or lvlsub row names and every DT1 a lvltypes row names, plus
//! the fixed tile library (`rooms.md` §9.3). A named file that is missing
//! or does not parse is a load error (M07); nothing falls back.

pub mod archive;
pub mod tables;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use d2_formats::ds1::Ds1;
use d2_formats::dt1::{Dt1, Dt1Tile};
use d2_sim::drlg::outdoor::{OutdoorData, SubFile, SubFileMap, SubFiles, SubGroup};
use d2_sim::drlg::preset::{
    Ds1File, Ds1Input, Ds1ObjectInput, Ds1PathInput, Ds1Source, PresetData, PresetError,
};
use d2_sim::drlg::tiles::{CellGrid, FIXED_LIBRARY};
use d2_sim::drlg::{DrlgData, TileInfo, TileSource};

/// The tile prefix of `fixups.md` §12 (`DATA\GLOBAL\TILES\`).
pub const TILE_PREFIX: &[u8] = b"DATA\\GLOBAL\\TILES\\";

/// A file the level types need could not be provided.
#[derive(Debug, thiserror::Error)]
pub enum WorldDataError {
    /// The reader could not supply the file (missing from the archives,
    /// or a read error).
    #[error("{path}: {detail}")]
    Read { path: String, detail: String },
    #[error("{path}: {source}")]
    Parse {
        path: String,
        source: d2_formats::FormatError,
    },
    /// `d2_formats::ds1` already remaps the orientations of v < 7 files
    /// with its 25-entry table; the stored values the seam takes
    /// (`Ds1Input::orientations`, mapped by the sim's 42-entry table,
    /// `preset.md` §5.2 step 6) are lost. No such file is supported.
    #[error("{path}: DS1 version {version} < 7: stored orientations are not available")]
    OldOrientations { path: String, version: u32 },
    /// A path is not ASCII (archive names are).
    #[error("path {0:?} is not ASCII")]
    BadPath(Vec<u8>),
    /// A lvlsub file's preset units (`preset.md` §5.3) failed.
    #[error("{path}: {source}")]
    Preset { path: String, source: PresetError },
    /// A table the views need is absent or of the wrong size.
    #[error("table {table}: {detail}")]
    Table { table: String, detail: String },
}

/// The archive name of a table tile path (`fixups.md` §12): a string that
/// already carries [`TILE_PREFIX`] (ASCII case-insensitive; a fixed-up
/// table, or the fixed library) is the name; otherwise every `/` becomes
/// `\` and, if the string has more than one character, the prefix is
/// prepended.
pub fn archive_name(path: &[u8]) -> Vec<u8> {
    if has_prefix(path) {
        return path.to_vec();
    }
    let s: Vec<u8> = path
        .iter()
        .map(|&b| if b == b'/' { b'\\' } else { b })
        .collect();
    if s.len() > 1 {
        [TILE_PREFIX, &s].concat()
    } else {
        s
    }
}

fn has_prefix(path: &[u8]) -> bool {
    path.len() >= TILE_PREFIX.len() && path[..TILE_PREFIX.len()].eq_ignore_ascii_case(TILE_PREFIX)
}

/// A table string names a file: more than one character (`fixups.md`
/// §12: the 0/1-character strings are the `0` placeholders, which the
/// path fix leaves alone).
pub fn names_file(s: &[u8]) -> bool {
    s.len() > 1
}

/// A parsed DS1 as the preset seam takes it (`Ds1Input`): the stored
/// width and height (`d2_formats` reports them + 1), the stored act, the
/// layers and records as stored.
pub fn ds1_input(path: &[u8], d: &Ds1) -> Result<Ds1Input, WorldDataError> {
    if d.version < 7 {
        return Err(WorldDataError::OldOrientations {
            path: String::from_utf8_lossy(path).into_owned(),
            version: d.version,
        });
    }
    Ok(Ds1Input {
        version: d.version,
        width: d.width.wrapping_sub(1),
        height: d.height.wrapping_sub(1),
        act: d.act as i32,
        tag_type: d.tag_type,
        walls: d.walls.clone(),
        orientations: d.orientations.clone(),
        floors: d.floors.clone(),
        shadow: d.shadow.clone(),
        objects: d
            .objects
            .iter()
            .map(|o| Ds1ObjectInput {
                kind: o.kind,
                id: o.id,
                x: o.x,
                y: o.y,
                flags: o.flags,
            })
            .collect(),
        paths: d
            .paths
            .iter()
            .map(|p| Ds1PathInput {
                x: p.x,
                y: p.y,
                points: p.points.iter().map(|q| (q.x, q.y, q.action)).collect(),
            })
            .collect(),
    })
}

/// A lvlsub DS1 as the outdoor seam takes it (`outdoor-tilesub.md` §1.3–
/// §1.4): the DS1 file record after the preset parser (`preset.md` §5.2,
/// [`Ds1File::from_input`]), its groups (variant count = the group's last
/// value, tilesub open question 1), and grids of (W + 1) × (H + 1) cells.
/// Wall layers are raw (the `k << 18` OR is the sim's, [`SubFile::wall_at`]).
pub fn sub_file(path: &[u8], d: &Ds1, pd: &PresetData) -> Result<SubFile, WorldDataError> {
    let input = ds1_input(path, d)?;
    let file = Ds1File::from_input(&input, pd).map_err(|source| WorldDataError::Preset {
        path: String::from_utf8_lossy(path).into_owned(),
        source,
    })?;
    let (w, h) = (file.stride(), file.height as usize + 1);
    let grid = |cells: &Vec<u32>| CellGrid {
        width: w,
        height: h,
        cells: cells.clone(),
    };
    Ok(SubFile {
        method: file.tag_type,
        groups: d
            .groups
            .iter()
            .map(|g| SubGroup {
                x: g.x as i32,
                y: g.y as i32,
                w: g.width as i32,
                h: g.height as i32,
                variants: g.unknown as i32,
            })
            .collect(),
        floor: file.floors.first().map(grid),
        walls: file.walls.iter().map(grid).collect(),
        tile_types: file.orientations.iter().map(grid).collect(),
        shadow: Some(grid(&file.shadow)),
        units: file.units.iter().map(|u| u.to_seam()).collect(),
    })
}

/// One DT1 tile header as the DRLG reads it (`rooms.md` §9.3).
pub fn tile_info(t: &Dt1Tile) -> TileInfo {
    TileInfo {
        orientation: t.orientation,
        main: t.main_index,
        sub: t.sub_index,
        rarity: t.rarity,
        material: t.material_flags,
        subtile_flags: t.subtile_flags,
    }
}

/// Parsed lvlprest DS1 files, keyed by the table string the preset code
/// asks with.
#[derive(Clone, Debug, Default)]
pub struct Ds1Files(pub BTreeMap<Vec<u8>, Ds1Input>);

impl Ds1Source for Ds1Files {
    fn ds1(&self, path: &[u8]) -> Option<&Ds1Input> {
        self.0.get(path)
    }
}

/// DT1 tile headers in file order, keyed by the table string (or fixed
/// library path) the room build asks with.
#[derive(Clone, Debug, Default)]
pub struct Dt1Files(pub BTreeMap<Vec<u8>, Vec<TileInfo>>);

impl TileSource for Dt1Files {
    fn dt1(&self, path: &[u8]) -> Option<&[TileInfo]> {
        self.0.get(path).map(Vec::as_slice)
    }
}

/// Every file the level types read, parsed.
#[derive(Clone, Debug, Default)]
pub struct WorldFiles {
    pub ds1: Ds1Files,
    pub subs: SubFileMap,
    pub dt1: Dt1Files,
}

impl WorldFiles {
    /// Loads every DS1 named by a lvlprest `File1`–`File6` or lvlsub
    /// `File` string and every DT1 named by a lvltypes `File` string or
    /// the fixed library. `read` gets the archive name ([`archive_name`])
    /// and returns the file's bytes.
    pub fn load(
        drlg: &DrlgData,
        pd: &PresetData,
        od: &OutdoorData,
        mut read: impl FnMut(&str) -> Result<Vec<u8>, String>,
    ) -> Result<Self, WorldDataError> {
        let mut out = Self::default();
        for path in pd.defs.iter().flat_map(|d| d.file.iter()) {
            if !names_file(path) || out.ds1.0.contains_key(path) {
                continue;
            }
            let d = parse_ds1(path, &mut read)?;
            out.ds1.0.insert(path.clone(), ds1_input(path, &d)?);
        }
        for path in od.subs.iter().map(|r| &r.file) {
            if !names_file(path) || out.subs.0.contains_key(path) {
                continue;
            }
            let d = parse_ds1(path, &mut read)?;
            out.subs.0.insert(path.clone(), sub_file(path, &d, pd)?);
        }
        let library = drlg
            .lvltypes
            .iter()
            .flatten()
            .map(Vec::as_slice)
            .chain(FIXED_LIBRARY);
        for path in library {
            if !names_file(path) || out.dt1.0.contains_key(path) {
                continue;
            }
            let bytes = read_file(path, &mut read)?;
            let dt1 = Dt1::parse(&bytes).map_err(|source| WorldDataError::Parse {
                path: lossy(path),
                source,
            })?;
            out.dt1
                .0
                .insert(path.to_vec(), dt1.tiles.iter().map(tile_info).collect());
        }
        Ok(out)
    }
}

impl SubFiles for WorldFiles {
    fn sub_file(&self, file: &[u8]) -> Option<&SubFile> {
        self.subs.sub_file(file)
    }
}

fn lossy(path: &[u8]) -> String {
    String::from_utf8_lossy(path).into_owned()
}

fn read_file(
    path: &[u8],
    read: &mut impl FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<Vec<u8>, WorldDataError> {
    let name = archive_name(path);
    if !name.is_ascii() {
        return Err(WorldDataError::BadPath(path.to_vec()));
    }
    let name = String::from_utf8(name).map_err(|e| WorldDataError::BadPath(e.into_bytes()))?;
    read(&name).map_err(|detail| WorldDataError::Read { path: name, detail })
}

fn parse_ds1(
    path: &[u8],
    read: &mut impl FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<Ds1, WorldDataError> {
    let bytes = read_file(path, read)?;
    Ds1::parse(&bytes).map_err(|source| WorldDataError::Parse {
        path: lossy(path),
        source,
    })
}
