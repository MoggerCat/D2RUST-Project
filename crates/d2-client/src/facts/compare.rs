// Spec: specs/tools/facts-render.md (§6)
//! `facts-compare`: the first difference between two fact sets, cause
//! before effect (frame inputs, sprites, draws, frame outputs).

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use super::{
    check_frame_keys, read, FactsError, Role, Table, DRAWS_FILE, DRAW_COLUMNS, FRAME_COLUMNS,
    FRAME_FILE, FRAME_KEYS, SPRITES_FILE, SPRITE_COLUMNS, UNKNOWN,
};

/// Columns and keys never compared (§2 r7, §3 table).
const INFO: [&str; 3] = ["i", "at", "seq"];

/// The three files of one side.
#[derive(Debug, Clone)]
pub struct FactSet {
    pub draws: Table,
    pub frame: Table,
    /// `None`: no `sprites.tsv` found (§6 r3: unmeasured).
    pub sprites: Option<Table>,
}

impl FactSet {
    /// Reads a scene or dump directory. `sprites_fallback` is tried when
    /// the directory has no `sprites.tsv` (§6: `../../sprites.tsv`).
    pub fn read(dir: &Path, sprites_fallback: Option<&Path>) -> Result<FactSet, FactsError> {
        if !dir.is_dir() {
            return Err(FactsError::Io {
                path: dir.to_path_buf(),
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "no such directory"),
            });
        }
        let draws = read(dir, DRAWS_FILE, &DRAW_COLUMNS)?;
        let frame = read(dir, FRAME_FILE, &FRAME_COLUMNS)?;
        check_frame_keys(&dir.join(FRAME_FILE).display().to_string(), &frame)?;
        let sprites = [Some(dir), sprites_fallback]
            .into_iter()
            .flatten()
            .find(|d| d.join(SPRITES_FILE).is_file())
            .map(|d| read(d, SPRITES_FILE, &SPRITE_COLUMNS))
            .transpose()?;
        Ok(FactSet {
            draws,
            frame,
            sprites,
        })
    }
}

/// Where the first difference is (§6 r1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    FrameInput,
    Sprites,
    Draws,
    FrameOutput,
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Stage::FrameInput => "frame.tsv (inputs)",
            Stage::Sprites => "sprites.tsv",
            Stage::Draws => "draws.tsv",
            Stage::FrameOutput => "frame.tsv (outputs)",
        })
    }
}

/// The first difference: stage, row (0-based data row of that file),
/// column, and both full rows (`None`: the row is missing on that side).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    pub stage: Stage,
    pub row: usize,
    pub column: String,
    pub original: Option<Vec<String>>,
    pub d2rs: Option<Vec<String>>,
}

impl fmt::Display for Difference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let row = |r: &Option<Vec<String>>| r.as_ref().map_or("<none>".into(), |r| r.join("\t"));
        writeln!(
            f,
            "first difference: {}, row {}, column {}",
            self.stage, self.row, self.column
        )?;
        writeln!(f, "  original: {}", row(&self.original))?;
        write!(f, "  d2rs:     {}", row(&self.d2rs))
    }
}

/// The compare's result (§6 r4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Match,
    Diverged(Difference),
    /// No difference; unmeasured cells per column (or `sprites.tsv`).
    Partial(BTreeMap<String, usize>),
}

impl Outcome {
    /// Exit code of §6 r4 (3, error, is the caller's).
    pub fn exit_code(&self) -> i32 {
        match self {
            Outcome::Match => 0,
            Outcome::Diverged(_) => 1,
            Outcome::Partial(_) => 2,
        }
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Outcome::Match => f.write_str("MATCH"),
            Outcome::Diverged(d) => write!(f, "DIVERGED\n{d}"),
            Outcome::Partial(u) => {
                write!(f, "PARTIAL: no difference; unmeasured:")?;
                for (k, n) in u {
                    write!(f, " {k} {n};")?;
                }
                Ok(())
            }
        }
    }
}

struct Walk<'a> {
    ignore: &'a [String],
    unmeasured: BTreeMap<String, usize>,
}

impl Walk<'_> {
    fn skipped(&self, column: &str) -> bool {
        INFO.contains(&column) || self.ignore.iter().any(|c| c == column)
    }

    /// Whether `a` and `b` differ under §6 r3; counts unmeasured cells.
    fn differ(&mut self, column: &str, a: &str, b: &str) -> bool {
        if self.skipped(column) {
            return false;
        }
        if a == UNKNOWN || b == UNKNOWN {
            *self.unmeasured.entry(column.to_owned()).or_default() += 1;
            return false;
        }
        a != b
    }

    fn frame_stage(&mut self, o: &Table, d: &Table, role: Role) -> Option<Difference> {
        let stage = match role {
            Role::Output => Stage::FrameOutput,
            _ => Stage::FrameInput,
        };
        for (i, (key, r)) in FRAME_KEYS.iter().enumerate() {
            if *r == role && self.differ(key, &o.rows[i][1], &d.rows[i][1]) {
                return Some(Difference {
                    stage,
                    row: i,
                    column: (*key).to_owned(),
                    original: Some(o.rows[i].clone()),
                    d2rs: Some(d.rows[i].clone()),
                });
            }
        }
        None
    }

    fn sprites(&mut self, o: Option<&Table>, d: Option<&Table>) -> Option<Difference> {
        let (Some(o), Some(d)) = (o, d) else {
            *self
                .unmeasured
                .entry(super::SPRITES_FILE.into())
                .or_default() += 1;
            return None;
        };
        let key = |r: &Vec<String>| (r[0].clone(), r[1].clone(), r[2].clone());
        let by_key: BTreeMap<_, _> = o.rows.iter().map(|r| (key(r), r)).collect();
        for (n, row) in d.rows.iter().enumerate() {
            let Some(orig) = by_key.get(&key(row)) else {
                *self.unmeasured.entry("sprite rows".into()).or_default() += 1;
                continue;
            };
            for (c, name) in SPRITE_COLUMNS.iter().enumerate().skip(3) {
                if self.differ(name, &orig[c], &row[c]) {
                    return Some(Difference {
                        stage: Stage::Sprites,
                        row: n,
                        column: (*name).to_owned(),
                        original: Some((*orig).clone()),
                        d2rs: Some(row.clone()),
                    });
                }
            }
        }
        None
    }

    fn draws(&mut self, o: &Table, d: &Table) -> Option<Difference> {
        let n = o.rows.len().max(d.rows.len());
        for i in 0..n {
            let (a, b) = (o.rows.get(i), d.rows.get(i));
            let (Some(a), Some(b)) = (a, b) else {
                return Some(Difference {
                    stage: Stage::Draws,
                    row: i,
                    column: "<row>".into(),
                    original: a.cloned(),
                    d2rs: b.cloned(),
                });
            };
            for (c, name) in DRAW_COLUMNS.iter().enumerate() {
                if self.differ(name, &a[c], &b[c]) {
                    return Some(Difference {
                        stage: Stage::Draws,
                        row: i,
                        column: (*name).to_owned(),
                        original: Some(a.clone()),
                        d2rs: Some(b.clone()),
                    });
                }
            }
        }
        None
    }
}

/// Compares two parsed fact sets (§6 r1–r4). `ignore` names columns or
/// `frame.tsv` keys left out.
pub fn compare(original: &FactSet, d2rs: &FactSet, ignore: &[String]) -> Outcome {
    let mut w = Walk {
        ignore,
        unmeasured: BTreeMap::new(),
    };
    let first = w
        .frame_stage(&original.frame, &d2rs.frame, Role::Input)
        .or_else(|| w.sprites(original.sprites.as_ref(), d2rs.sprites.as_ref()))
        .or_else(|| w.draws(&original.draws, &d2rs.draws))
        .or_else(|| w.frame_stage(&original.frame, &d2rs.frame, Role::Output));
    match first {
        Some(d) => Outcome::Diverged(d),
        None if w.unmeasured.is_empty() => Outcome::Match,
        None => Outcome::Partial(w.unmeasured),
    }
}

/// Reads both directories and compares them (§6). The original's
/// `sprites.tsv` falls back to `<original>/../../sprites.tsv`.
pub fn compare_dirs(
    original: &Path,
    d2rs: &Path,
    ignore: &[String],
) -> Result<Outcome, FactsError> {
    let fallback = original.join("..").join("..");
    let o = FactSet::read(original, Some(&fallback))?;
    let d = FactSet::read(d2rs, None)?;
    Ok(compare(&o, &d, ignore))
}
