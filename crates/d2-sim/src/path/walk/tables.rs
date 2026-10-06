// Spec: specs/sim/pathing.md (Constants & data dependencies; machine table specs/sim/path-tables.tsv)
//! The constant tables of `sim/path-tables.tsv`, parsed from the embedded
//! TSV. The parser is strict (METHODS M07): an unknown table, a missing or
//! repeated row, or a bad number is an error, never a default.

use thiserror::Error;

/// The spec table, embedded (pathing.md "Code embeds the TSV").
pub const PATH_TABLES_TSV: &str = include_str!("../../../../../specs/sim/path-tables.tsv");

/// Rows in `path-tables.tsv` (pathing.md: 582 rows).
pub const PATH_TABLE_ROWS: usize = 582;

/// A parse failure of the table text.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TableError {
    #[error("line {0}: bad header")]
    Header(usize),
    #[error("line {0}: {1}")]
    Row(usize, &'static str),
    #[error("table {0}: rows missing")]
    Missing(&'static str),
    #[error("{0} rows, expected {PATH_TABLE_ROWS}")]
    Count(usize),
}

/// One row of `velmod_*` (a = by passive skill, b = velocity modifier).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VelMod {
    pub a: i32,
    pub b: i32,
}

/// One row of `tan` (x, y, angle).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TanRow {
    pub x: i32,
    pub y: i32,
    pub angle: i32,
}

/// One row of `animstat` (a has-base, b base, c stat).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnimStat {
    pub has_base: i32,
    pub base: i32,
    pub stat: i32,
}

/// Every table of `path-tables.tsv`, by its spec name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathTables {
    pub pathtype_flags: [u32; 18],
    pub pathtype_diroff: [i32; 18],
    pub pattern_of_size: [u8; 4],
    /// (dx, dy) of the eight directions (`dir8_toward`).
    pub dir8_toward: [(i32, i32); 8],
    pub dir8_target: [(i32, i32); 8],
    pub testdir: [[u8; 3]; 25],
    /// 255 = none.
    pub altdir: [[u8; 3]; 25],
    pub dist8_path: [i32; 64],
    pub dist8_unit: [i32; 64],
    pub snap9: [i32; 81],
    pub tan: [TanRow; 128],
    pub dirdiff: [i32; 64],
    pub field_dx: [i32; 9],
    pub field_dy: [i32; 9],
    pub velmod_player: [VelMod; 20],
    pub velmod_monster: [VelMod; 16],
    pub velmod_monster_x: [VelMod; 16],
    pub animstat: [AnimStat; 5],
}

impl PathTables {
    /// The embedded spec tables. Panics only if the committed TSV is
    /// malformed, which `tables_parse` (CI) rules out.
    pub fn embedded() -> PathTables {
        match PathTables::parse(PATH_TABLES_TSV) {
            Ok(t) => t,
            Err(e) => panic!("specs/sim/path-tables.tsv: {e}"),
        }
    }

    /// Parses the TSV text (columns `table index a b c d e va`).
    pub fn parse(text: &str) -> Result<PathTables, TableError> {
        let mut t = PathTables {
            pathtype_flags: [0; 18],
            pathtype_diroff: [0; 18],
            pattern_of_size: [0; 4],
            dir8_toward: [(0, 0); 8],
            dir8_target: [(0, 0); 8],
            testdir: [[0; 3]; 25],
            altdir: [[0; 3]; 25],
            dist8_path: [0; 64],
            dist8_unit: [0; 64],
            snap9: [0; 81],
            tan: [TanRow::default(); 128],
            dirdiff: [0; 64],
            field_dx: [0; 9],
            field_dy: [0; 9],
            velmod_player: [VelMod::default(); 20],
            velmod_monster: [VelMod::default(); 16],
            velmod_monster_x: [VelMod::default(); 16],
            animstat: [AnimStat::default(); 5],
        };
        // (name, rows) in the order the seen-bitmaps below are kept.
        const NAMES: [(&str, usize); 18] = [
            ("pathtype_flags", 18),
            ("pathtype_diroff", 18),
            ("pattern_of_size", 4),
            ("dir8_toward", 8),
            ("dir8_target", 8),
            ("testdir", 25),
            ("altdir", 25),
            ("dist8_path", 64),
            ("dist8_unit", 64),
            ("snap9", 81),
            ("tan", 128),
            ("dirdiff", 64),
            ("field_dx", 9),
            ("field_dy", 9),
            ("velmod_player", 20),
            ("velmod_monster", 16),
            ("velmod_monster_x", 16),
            ("animstat", 5),
        ];
        let mut seen: Vec<Vec<bool>> = NAMES.iter().map(|(_, n)| vec![false; *n]).collect();
        let mut rows = 0usize;
        let mut lines = text.lines().enumerate();
        match lines.next() {
            Some((_, "table\tindex\ta\tb\tc\td\te\tva")) => {}
            _ => return Err(TableError::Header(1)),
        }
        for (ln, line) in lines {
            let ln = ln + 1;
            if line.is_empty() {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() != 8 {
                return Err(TableError::Row(ln, "expected 8 columns"));
            }
            let num = |s: &str| -> Result<Option<i64>, TableError> {
                if s.is_empty() {
                    Ok(None)
                } else {
                    s.parse::<i64>()
                        .map(Some)
                        .map_err(|_| TableError::Row(ln, "bad number"))
                }
            };
            let req = |s: &str| -> Result<i32, TableError> {
                match num(s)? {
                    Some(v) => i32::try_from(v).map_err(|_| TableError::Row(ln, "out of range")),
                    None => Err(TableError::Row(ln, "missing value")),
                }
            };
            let Some(ti) = NAMES.iter().position(|(n, _)| *n == cols[0]) else {
                return Err(TableError::Row(ln, "unknown table"));
            };
            let i: usize = cols[1]
                .parse()
                .map_err(|_| TableError::Row(ln, "bad index"))?;
            if i >= NAMES[ti].1 {
                return Err(TableError::Row(ln, "index out of range"));
            }
            if seen[ti][i] {
                return Err(TableError::Row(ln, "repeated row"));
            }
            seen[ti][i] = true;
            rows += 1;
            let a = req(cols[2])?;
            match cols[0] {
                "pathtype_flags" => t.pathtype_flags[i] = a as u32,
                "pathtype_diroff" => t.pathtype_diroff[i] = a,
                "pattern_of_size" => t.pattern_of_size[i] = a as u8,
                "dir8_toward" => t.dir8_toward[i] = (a, req(cols[3])?),
                "dir8_target" => t.dir8_target[i] = (a, req(cols[3])?),
                "testdir" => {
                    t.testdir[i] = [a as u8, req(cols[3])? as u8, req(cols[4])? as u8];
                }
                "altdir" => {
                    t.altdir[i] = [a as u8, req(cols[3])? as u8, req(cols[4])? as u8];
                }
                "dist8_path" => t.dist8_path[i] = a,
                "dist8_unit" => t.dist8_unit[i] = a,
                "snap9" => t.snap9[i] = a,
                "tan" => {
                    t.tan[i] = TanRow {
                        x: a,
                        y: req(cols[3])?,
                        angle: req(cols[4])?,
                    }
                }
                "dirdiff" => t.dirdiff[i] = a,
                "field_dx" => t.field_dx[i] = a,
                "field_dy" => t.field_dy[i] = a,
                "velmod_player" | "velmod_monster" | "velmod_monster_x" => {
                    let v = VelMod {
                        a,
                        b: req(cols[3])?,
                    };
                    match cols[0] {
                        "velmod_player" => t.velmod_player[i] = v,
                        "velmod_monster" => t.velmod_monster[i] = v,
                        _ => t.velmod_monster_x[i] = v,
                    }
                }
                "animstat" => {
                    t.animstat[i] = AnimStat {
                        has_base: a,
                        base: req(cols[3])?,
                        stat: req(cols[4])?,
                    }
                }
                _ => return Err(TableError::Row(ln, "unknown table")),
            }
        }
        for (k, (name, _)) in NAMES.iter().enumerate() {
            if seen[k].iter().any(|s| !s) {
                return Err(TableError::Missing(name));
            }
        }
        if rows != PATH_TABLE_ROWS {
            return Err(TableError::Count(rows));
        }
        Ok(t)
    }
}
