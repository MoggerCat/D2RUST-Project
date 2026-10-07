// Spec: specs/formats/native-assets.md §1.1, §4.3, §8
//! One enum over every converted kind (§1.1) with a uniform
//! decode → write → read back → compare API, for the converter (N3) and
//! the runtime source (N4). N1's kinds go through [`NativeKind`], N2's
//! plain `write_*` / `read_*` functions are called directly. The enum
//! only dispatches: it holds no format logic.

use d2_formats::animdata::AnimData;
use d2_formats::cof::Cof;
use d2_formats::dc6::Dc6;
use d2_formats::dcc::Dcc;
use d2_formats::ds1::Ds1;
use d2_formats::dt1::Dt1;
use d2_formats::font::FontTable;
use d2_formats::palette::{Palette, Pl2};
use d2_formats::tbl::StringTable;

use crate::expfield::ExpFieldData;
use crate::kind::{self, grey_palette, CheckError, NativeFile, NativeKind};
use crate::toml_kinds::{self, TextError};
use crate::{animdata, tbl};

/// Canonical path of the animation table and the expansion field.
pub const ANIMDATA_PATH: &str = "data/global/animdata.d2";
pub const EXPFIELD_PATH: &str = "data/global/expfield.d2";

/// A failed step: `check` is the `files.tsv` check name (`decode`,
/// `C-DC6`, …), `detail` the first difference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetError {
    pub check: String,
    pub detail: String,
}

impl AssetError {
    fn new(check: &str, detail: impl Into<String>) -> AssetError {
        AssetError {
            check: check.to_owned(),
            detail: detail.into(),
        }
    }
}

/// What [`AssetKind::write`] produces.
#[derive(Debug, Clone, Default)]
pub struct AssetWritten {
    pub files: Vec<NativeFile>,
    /// Lines for the converter's report.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetKind {
    Dc6,
    Dcc,
    Dt1,
    Pal,
    Pl2,
    Cof,
    Ds1,
    Tbl,
    Font,
    Animdata,
    ExpField,
}

impl AssetKind {
    pub const ALL: [AssetKind; 11] = [
        AssetKind::Dc6,
        AssetKind::Dcc,
        AssetKind::Dt1,
        AssetKind::Pal,
        AssetKind::Pl2,
        AssetKind::Cof,
        AssetKind::Ds1,
        AssetKind::Tbl,
        AssetKind::Font,
        AssetKind::Animdata,
        AssetKind::ExpField,
    ];

    /// The kind name of `manifest.toml` / `files.tsv` (§1.1).
    pub fn name(self) -> &'static str {
        match self {
            AssetKind::Dc6 => "dc6",
            AssetKind::Dcc => "dcc",
            AssetKind::Dt1 => "dt1",
            AssetKind::Pal => "pal",
            AssetKind::Pl2 => "pl2",
            AssetKind::Cof => "cof",
            AssetKind::Ds1 => "ds1",
            AssetKind::Tbl => "tbl",
            AssetKind::Font => "font",
            AssetKind::Animdata => "animdata",
            AssetKind::ExpField => "expfield",
        }
    }

    pub fn native_version(self) -> u32 {
        1
    }

    /// Tables load first (§4.1 r3).
    pub fn phase(self) -> u8 {
        if self == AssetKind::Tbl {
            0
        } else {
            1
        }
    }

    /// The kind that converts the file at canonical path `canon`, if any.
    pub fn for_path(canon: &str) -> Option<AssetKind> {
        AssetKind::ALL.into_iter().find(|k| k.claims(canon))
    }

    pub fn claims(self, canon: &str) -> bool {
        let in_dir = |dir: &str, ext: &str| {
            canon.strip_prefix(dir).is_some_and(|r| {
                r.split_once('/')
                    .is_some_and(|(_, f)| !f.contains('/') && f.ends_with(ext))
            })
        };
        match self {
            AssetKind::Dc6 => canon.ends_with(".dc6"),
            AssetKind::Dcc => canon.ends_with(".dcc"),
            AssetKind::Dt1 => canon.ends_with(".dt1"),
            AssetKind::Pal => canon.ends_with("/pal.dat"),
            AssetKind::Pl2 => canon.ends_with(".pl2"),
            AssetKind::Cof => canon.ends_with(".cof"),
            AssetKind::Ds1 => canon.ends_with(".ds1"),
            AssetKind::Tbl => in_dir("data/local/lng/", ".tbl"),
            AssetKind::Font => in_dir("data/local/font/", ".tbl"),
            AssetKind::Animdata => canon == ANIMDATA_PATH,
            AssetKind::ExpField => canon == EXPFIELD_PATH,
        }
    }

    /// Whether [`AssetKind::references`] can name other files.
    pub fn has_references(self) -> bool {
        self == AssetKind::Ds1
    }

    /// Paths a source file names (§1 r3): a DS1's DT1 list. The embedded
    /// names may be developer paths or `.tg1`; only the part from
    /// `data\global` on is kept, with `.dt1`.
    pub fn references(self, _canon: &str, src: &[u8]) -> Vec<String> {
        if self != AssetKind::Ds1 {
            return Vec::new();
        }
        let Ok(ds1) = Ds1::parse(src) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for f in &ds1.files {
            let mut s = String::from_utf8_lossy(f)
                .to_ascii_lowercase()
                .replace('\\', "/");
            if let Some(i) = s.find("data/global/") {
                s = s[i..].to_owned();
            } else if s.starts_with("tiles/") {
                s = format!("data/global/{s}");
            }
            if let Some(stem) = s.strip_suffix(".tg1") {
                s = format!("{stem}.dt1");
            }
            if s.ends_with(".dt1") {
                out.push(s.replace('/', "\\"));
            }
        }
        out
    }

    /// Decodes `src` (path `canon`) and writes its native files, sorted by
    /// path.
    pub fn write(self, canon: &str, src: &[u8]) -> Result<AssetWritten, AssetError> {
        let dec = |e: d2_formats::FormatError| AssetError::new("decode", format!("{canon}: {e}"));
        let enc = |e: &dyn std::fmt::Display| AssetError::new("write", e.to_string());
        let view = grey_palette();
        let mut notes = Vec::new();
        let mut files = match self {
            AssetKind::Dc6 => Dc6::parse(src)
                .map_err(dec)?
                .write(canon, &view)
                .map_err(|e| enc(&e))?,
            AssetKind::Dcc => Dcc::parse(src)
                .map_err(dec)?
                .write(canon, &view)
                .map_err(|e| enc(&e))?,
            AssetKind::Dt1 => {
                let fs = Dt1::parse(src)
                    .map_err(dec)?
                    .write(canon, &view)
                    .map_err(|e| enc(&e))?;
                let side = fs.iter().find(|f| f.path == format!("{canon}.toml"));
                let n = side.map_or(0, |f| {
                    String::from_utf8_lossy(&f.bytes)
                        .matches("layout = \"blocks\"")
                        .count()
                });
                if n > 0 {
                    notes.push(format!("{n} tile(s) use the blocks layout (§2.3 r3)"));
                }
                fs
            }
            AssetKind::Pal => Palette::parse(src)
                .map_err(dec)?
                .write(canon, &view)
                .map_err(|e| enc(&e))?,
            AssetKind::Pl2 => Pl2::parse(src)
                .map_err(dec)?
                .write(canon, &view)
                .map_err(|e| enc(&e))?,
            AssetKind::ExpField => ExpFieldData::from_bytes(src)
                .map_err(|e| AssetError::new("decode", e.to_string()))?
                .write(canon, &view)
                .map_err(|e| enc(&e))?,
            AssetKind::Cof => text(
                canon,
                ".toml",
                toml_kinds::write_cof(canon, &Cof::parse(src).map_err(dec)?),
            )?,
            AssetKind::Ds1 => text(
                canon,
                ".toml",
                toml_kinds::write_ds1(canon, &Ds1::parse(src).map_err(dec)?),
            )?,
            AssetKind::Font => text(
                canon,
                ".toml",
                toml_kinds::write_font(canon, &FontTable::parse(src).map_err(dec)?),
            )?,
            AssetKind::Animdata => text(
                canon,
                ".tsv",
                animdata::write_animdata(canon, &AnimData::parse(src).map_err(dec)?),
            )?,
            AssetKind::Tbl => {
                let t = StringTable::parse(src).map_err(dec)?;
                if tbl::rebuild_differs(&t) {
                    notes.push("hash table differs from a rebuild (§2.6 r3)".into());
                }
                let f = tbl::write_tbl(canon, &t, true).map_err(|e| enc(&e))?;
                vec![
                    NativeFile {
                        path: format!("{canon}.toml"),
                        bytes: f.toml.into_bytes(),
                    },
                    NativeFile {
                        path: format!("{canon}.tsv"),
                        bytes: f.tsv.into_bytes(),
                    },
                ]
            }
        };
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(AssetWritten { files, notes })
    }

    /// §4.3: `native` is read back from disk; decode it with the native
    /// reader, decode `src` again, compare.
    pub fn check(self, canon: &str, src: &[u8], native: &[NativeFile]) -> Result<(), AssetError> {
        let name = format!("C-{}", self.name().to_ascii_uppercase());
        let check = match self {
            AssetKind::Dc6 => name.clone(),
            AssetKind::Dcc | AssetKind::Dt1 => name.clone(),
            AssetKind::Tbl => "C-TBL".into(),
            _ => "C-STRUCT".into(),
        };
        let dec = |e: d2_formats::FormatError| AssetError::new("decode", format!("{canon}: {e}"));
        let bad = |e: TextError| AssetError::new(&check, e.to_string());
        let img = |r: Result<(), CheckError>| {
            r.map_err(|e| match e {
                CheckError::Mismatch(d) => AssetError::new(&check, d.to_string()),
                other => AssetError::new(&check, other.to_string()),
            })
        };
        let file = |ext: &str| -> Result<String, AssetError> {
            let p = format!("{canon}{ext}");
            let b = native
                .iter()
                .find(|f| f.path == p)
                .ok_or_else(|| AssetError::new(&check, format!("{p}: missing native file")))?;
            String::from_utf8(b.bytes.clone())
                .map_err(|_| AssetError::new(&check, format!("{p}: not UTF-8")))
        };
        match self {
            AssetKind::Dc6 => img(kind::check_files(
                &Dc6::parse(src).map_err(dec)?,
                canon,
                native,
            )),
            AssetKind::Dcc => img(kind::check_files(
                &Dcc::parse(src).map_err(dec)?,
                canon,
                native,
            )),
            AssetKind::Dt1 => img(kind::check_files(
                &Dt1::parse(src).map_err(dec)?,
                canon,
                native,
            )),
            AssetKind::Pal => img(kind::check_files(
                &Palette::parse(src).map_err(dec)?,
                canon,
                native,
            )),
            AssetKind::Pl2 => img(kind::check_files(
                &Pl2::parse(src).map_err(dec)?,
                canon,
                native,
            )),
            AssetKind::ExpField => {
                let o = ExpFieldData::from_bytes(src)
                    .map_err(|e| AssetError::new("decode", e.to_string()))?;
                img(kind::check_files(&o, canon, native))
            }
            AssetKind::Cof => {
                let o = Cof::parse(src).map_err(dec)?;
                same(
                    canon,
                    &o,
                    &toml_kinds::read_cof(canon, &file(".toml")?).map_err(bad)?,
                    &check,
                )
            }
            AssetKind::Ds1 => {
                let o = Ds1::parse(src).map_err(dec)?;
                same(
                    canon,
                    &o,
                    &toml_kinds::read_ds1(canon, &file(".toml")?).map_err(bad)?,
                    &check,
                )
            }
            AssetKind::Font => {
                let o = FontTable::parse(src).map_err(dec)?;
                same(
                    canon,
                    &o,
                    &toml_kinds::read_font(canon, &file(".toml")?).map_err(bad)?,
                    &check,
                )
            }
            AssetKind::Animdata => {
                let o = AnimData::parse(src).map_err(dec)?;
                same(
                    canon,
                    &o,
                    &animdata::read_animdata(canon, &file(".tsv")?).map_err(bad)?,
                    &check,
                )
            }
            AssetKind::Tbl => {
                let o = StringTable::parse(src).map_err(dec)?;
                let back = tbl::read_tbl(canon, &file(".tsv")?, &file(".toml")?).map_err(bad)?;
                let norm = |t: &StringTable| {
                    let mut n = t.clone();
                    n.header.data_start = 0;
                    n.header.file_size = 0;
                    n
                };
                same(canon, &norm(&o), &norm(&back), &check)?;
                for i in 0..o.indices.len() {
                    if let Some(e) = o.element(i) {
                        let (x, y) = (o.find_slot(&e.key), back.find_slot(&e.key));
                        if x != y {
                            return Err(AssetError::new(
                                &check,
                                format!("{canon}: lookup of element {i}: slot {x:?} vs {y:?}"),
                            ));
                        }
                    }
                }
                Ok(())
            }
        }
    }
}

fn text(
    canon: &str,
    ext: &str,
    r: Result<String, TextError>,
) -> Result<Vec<NativeFile>, AssetError> {
    let s = r.map_err(|e| AssetError::new("write", e.to_string()))?;
    Ok(vec![NativeFile {
        path: format!("{canon}{ext}"),
        bytes: s.into_bytes(),
    }])
}

fn same<T: PartialEq + std::fmt::Debug>(
    canon: &str,
    a: &T,
    b: &T,
    check: &str,
) -> Result<(), AssetError> {
    if a == b {
        Ok(())
    } else {
        Err(AssetError::new(
            check,
            format!("{canon}: {}", toml_kinds::first_difference(a, b)),
        ))
    }
}
