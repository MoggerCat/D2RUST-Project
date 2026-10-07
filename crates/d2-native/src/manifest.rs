// Spec: specs/formats/native-assets.md §3.3, §3.4
//! `manifest.toml` and `files.tsv` of a native root: the versioned identity
//! of a conversion and one row per converted source file.
//!
//! Output is deterministic (§4.2): keys in a fixed order, sorted maps, rows
//! sorted by path in byte order, `\n` line ends, no timestamps. Readers are
//! strict: an unknown `format_version` or a key of the wrong type is an
//! error naming it.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// `format` key of every manifest.
pub const FORMAT: &str = "d2rs-native";
/// Layout version of this spec (§3.3).
pub const FORMAT_VERSION: u32 = 1;
/// The oldest converter version whose `base/` this build accepts (§3.4 r2);
/// raised when a native format changes.
pub const MIN_CONVERTER_VERSION: &str = "0.0.0";
/// `files.tsv` header row (§3.3).
pub const FILES_HEADER: &str = "path\tkind\tarchive\tsource_sha256\tnative\tstatus";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    #[error("manifest.toml: {0}")]
    Manifest(String),
    #[error("files.tsv line {line}: {msg}")]
    Files { line: usize, msg: String },
    #[error("native root not usable: {0}")]
    Unusable(String),
}

/// One archive the converter opened: identity only (§4.7 r2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveId {
    pub name: String,
    pub size: u64,
    pub sha256: String,
}

/// Per-kind counts (§3.3 `counts`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KindCounts {
    pub converted: u64,
    pub failed: u64,
    pub skipped: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub converter_version: String,
    pub converter_commit: String,
    /// `native_version` of each kind used.
    pub kinds: BTreeMap<String, u32>,
    pub language: String,
    pub lod: bool,
    pub archives: Vec<ArchiveId>,
    pub counts: BTreeMap<String, KindCounts>,
    /// Hash-table entries no name reached, per archive name.
    pub unnamed_blocks: BTreeMap<String, u64>,
    pub complete: bool,
    /// Lowercase hex SHA-256 of `files.tsv`.
    pub files_sha256: String,
}

fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn bare_or_quoted(key: &str) -> String {
    if !key.is_empty()
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        key.to_owned()
    } else {
        quote(key)
    }
}

impl Manifest {
    /// The manifest as TOML text, keys in the order of §3.3.
    pub fn to_toml(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "format = {}", quote(FORMAT));
        let _ = writeln!(s, "format_version = {FORMAT_VERSION}");
        let _ = writeln!(s, "complete = {}", self.complete);
        let _ = writeln!(s, "files_sha256 = {}", quote(&self.files_sha256));
        let _ = writeln!(s, "\n[converter]");
        let _ = writeln!(s, "version = {}", quote(&self.converter_version));
        let _ = writeln!(s, "commit = {}", quote(&self.converter_commit));
        let _ = writeln!(s, "\n[kinds]");
        for (k, v) in &self.kinds {
            let _ = writeln!(s, "{} = {v}", bare_or_quoted(k));
        }
        let _ = writeln!(s, "\n[source]");
        let _ = writeln!(s, "language = {}", quote(&self.language));
        let _ = writeln!(s, "lod = {}", self.lod);
        for a in &self.archives {
            let _ = writeln!(s, "\n[[source.archive]]");
            let _ = writeln!(s, "name = {}", quote(&a.name));
            let _ = writeln!(s, "size = {}", a.size);
            let _ = writeln!(s, "sha256 = {}", quote(&a.sha256));
        }
        for (k, c) in &self.counts {
            let _ = writeln!(s, "\n[counts.{}]", bare_or_quoted(k));
            let _ = writeln!(s, "converted = {}", c.converted);
            let _ = writeln!(s, "failed = {}", c.failed);
            let _ = writeln!(s, "skipped = {}", c.skipped);
        }
        let _ = writeln!(s, "\n[unnamed_blocks]");
        for (k, v) in &self.unnamed_blocks {
            let _ = writeln!(s, "{} = {v}", bare_or_quoted(k));
        }
        s
    }

    /// Parses a manifest. Rejects an unknown `format` or `format_version`.
    pub fn from_toml(text: &str) -> Result<Manifest, ManifestError> {
        let err = |m: String| ManifestError::Manifest(m);
        let t: toml::Table = text.parse().map_err(|e| err(format!("{e}")))?;
        let str_of = |t: &toml::Table, k: &str| -> Result<String, ManifestError> {
            t.get(k)
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .ok_or_else(|| err(format!("`{k}` missing or not a string")))
        };
        let int_of = |t: &toml::Table, k: &str| -> Result<u64, ManifestError> {
            t.get(k)
                .and_then(|v| v.as_integer())
                .and_then(|v| u64::try_from(v).ok())
                .ok_or_else(|| err(format!("`{k}` missing or not a non-negative integer")))
        };
        let bool_of = |t: &toml::Table, k: &str| -> Result<bool, ManifestError> {
            t.get(k)
                .and_then(|v| v.as_bool())
                .ok_or_else(|| err(format!("`{k}` missing or not a boolean")))
        };
        let table_of = |t: &toml::Table, k: &str| -> Result<toml::Table, ManifestError> {
            t.get(k)
                .and_then(|v| v.as_table())
                .cloned()
                .ok_or_else(|| err(format!("`[{k}]` missing")))
        };

        let format = str_of(&t, "format")?;
        if format != FORMAT {
            return Err(err(format!("format `{format}`, expected `{FORMAT}`")));
        }
        let version = int_of(&t, "format_version")?;
        if version != u64::from(FORMAT_VERSION) {
            return Err(err(format!(
                "format_version {version} is not known (this build reads {FORMAT_VERSION})"
            )));
        }
        let conv = table_of(&t, "converter")?;
        let mut kinds = BTreeMap::new();
        for (k, v) in table_of(&t, "kinds")? {
            let n = v
                .as_integer()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| err(format!("kinds.{k} is not a version number")))?;
            kinds.insert(k, n);
        }
        let source = table_of(&t, "source")?;
        let mut archives = Vec::new();
        if let Some(list) = source.get("archive") {
            let list = list
                .as_array()
                .ok_or_else(|| err("source.archive is not an array".into()))?;
            for a in list {
                let a = a
                    .as_table()
                    .ok_or_else(|| err("source.archive entry is not a table".into()))?;
                archives.push(ArchiveId {
                    name: str_of(a, "name")?,
                    size: int_of(a, "size")?,
                    sha256: str_of(a, "sha256")?,
                });
            }
        }
        let mut counts = BTreeMap::new();
        for (k, v) in table_of(&t, "counts")? {
            let c = v
                .as_table()
                .ok_or_else(|| err(format!("counts.{k} is not a table")))?;
            counts.insert(
                k,
                KindCounts {
                    converted: int_of(c, "converted")?,
                    failed: int_of(c, "failed")?,
                    skipped: int_of(c, "skipped")?,
                },
            );
        }
        let mut unnamed_blocks = BTreeMap::new();
        for (k, v) in table_of(&t, "unnamed_blocks")? {
            let n = v
                .as_integer()
                .and_then(|n| u64::try_from(n).ok())
                .ok_or_else(|| err(format!("unnamed_blocks.{k} is not a count")))?;
            unnamed_blocks.insert(k, n);
        }
        Ok(Manifest {
            converter_version: str_of(&conv, "version")?,
            converter_commit: str_of(&conv, "commit")?,
            kinds,
            language: str_of(&source, "language")?,
            lod: bool_of(&source, "lod")?,
            archives,
            counts,
            unnamed_blocks,
            complete: bool_of(&t, "complete")?,
            files_sha256: str_of(&t, "files_sha256")?,
        })
    }

    /// The §3.4 r2 start conditions: known `kinds` versions (`known` maps
    /// each kind this build reads to its version), `complete = true`, and a
    /// converter version at least [`MIN_CONVERTER_VERSION`]. The error names
    /// the first reason.
    pub fn check_loadable(&self, known: &BTreeMap<String, u32>) -> Result<(), ManifestError> {
        let bad = |m: String| Err(ManifestError::Unusable(m));
        if !self.complete {
            return bad("the conversion is not complete; run the converter".into());
        }
        for (k, v) in &self.kinds {
            match known.get(k) {
                Some(n) if n == v => {}
                Some(n) => {
                    return bad(format!(
                        "kind `{k}` is native_version {v}, this build reads {n}"
                    ))
                }
                None => return bad(format!("kind `{k}` is not known to this build")),
            }
        }
        if version_tuple(&self.converter_version) < version_tuple(MIN_CONVERTER_VERSION) {
            return bad(format!(
                "converter {} is older than the minimum {MIN_CONVERTER_VERSION}",
                self.converter_version
            ));
        }
        Ok(())
    }
}

fn version_tuple(v: &str) -> (u64, u64, u64) {
    let mut it = v
        .split(['.', '-', '+'])
        .map(|p| p.parse::<u64>().unwrap_or(0));
    (
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
    )
}

/// One native file of a source file: `name:size:sha256` (§3.3). `name` is
/// relative to `base/`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NativeFile {
    pub name: String,
    pub size: u64,
    pub sha256: String,
}

/// `status` column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Ok,
    /// `failed:<check>`.
    Failed(String),
}

/// One `files.tsv` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRow {
    pub path: String,
    pub kind: String,
    pub archive: String,
    pub source_sha256: String,
    pub native: Vec<NativeFile>,
    pub status: Status,
}

/// Whether `name` can sit in a `files.tsv` cell and in a `native` list: no
/// tab, line end, `;` or `:` (§3.3), no empty part, no `.` / `..` part, and
/// not absolute.
pub fn check_native_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("empty name".into());
    }
    if let Some(c) = name
        .chars()
        .find(|c| matches!(c, '\t' | '\n' | '\r' | ';' | ':' | '\\') || c.is_control())
    {
        return Err(format!(
            "`{name}`: character {c:?} cannot be in a native name"
        ));
    }
    if name
        .split('/')
        .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err(format!("`{name}`: empty or dot path part"));
    }
    Ok(())
}

impl FileRow {
    fn to_line(&self) -> String {
        let mut native = self.native.clone();
        native.sort();
        let native = native
            .iter()
            .map(|n| format!("{}:{}:{}", n.name, n.size, n.sha256))
            .collect::<Vec<_>>()
            .join(";");
        let status = match &self.status {
            Status::Ok => "ok".to_owned(),
            Status::Failed(c) => format!("failed:{}", c.replace(['\t', '\n', '\r'], " ")),
        };
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\n",
            self.path, self.kind, self.archive, self.source_sha256, native, status
        )
    }

    fn from_line(line: &str, no: usize) -> Result<FileRow, ManifestError> {
        let err = |msg: String| ManifestError::Files { line: no, msg };
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 6 {
            return Err(err(format!("{} columns, expected 6", cols.len())));
        }
        let mut native = Vec::new();
        if !cols[4].is_empty() {
            for item in cols[4].split(';') {
                let mut p = item.rsplitn(3, ':');
                let (sha, size, name) = match (p.next(), p.next(), p.next()) {
                    (Some(h), Some(s), Some(n)) => (h, s, n),
                    _ => {
                        return Err(err(format!(
                            "native entry `{item}` is not name:size:sha256"
                        )))
                    }
                };
                native.push(NativeFile {
                    name: name.to_owned(),
                    size: size
                        .parse()
                        .map_err(|_| err(format!("native size `{size}` is not a number")))?,
                    sha256: sha.to_owned(),
                });
            }
        }
        let status = match cols[5] {
            "ok" => Status::Ok,
            s => match s.strip_prefix("failed:") {
                Some(c) => Status::Failed(c.to_owned()),
                None => return Err(err(format!("status `{s}` is not ok or failed:<check>"))),
            },
        };
        Ok(FileRow {
            path: cols[0].to_owned(),
            kind: cols[1].to_owned(),
            archive: cols[2].to_owned(),
            source_sha256: cols[3].to_owned(),
            native,
            status,
        })
    }
}

/// `files.tsv` text: header, rows sorted by `path` in byte order.
pub fn write_files_tsv(rows: &[FileRow]) -> String {
    let mut sorted: Vec<&FileRow> = rows.iter().collect();
    sorted.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    let mut s = String::from(FILES_HEADER);
    s.push('\n');
    for r in sorted {
        s.push_str(&r.to_line());
    }
    s
}

/// Parses `files.tsv`. With `lenient_tail`, a last line without its `\n`
/// (a write cut short by a crash) is dropped instead of being an error
/// (§4.6 r1: the row is appended after the rename, so a torn row means
/// the file is simply redone).
pub fn parse_files_tsv(text: &str, lenient_tail: bool) -> Result<Vec<FileRow>, ManifestError> {
    let mut lines: Vec<&str> = text.split('\n').collect();
    // `split` leaves an empty last element when the text ends in `\n`.
    let tail = lines.pop().unwrap_or("");
    if !tail.is_empty() && !lenient_tail {
        return Err(ManifestError::Files {
            line: lines.len() + 1,
            msg: "last line has no line end".into(),
        });
    }
    let mut it = lines.into_iter().enumerate();
    match it.next() {
        Some((_, h)) if h == FILES_HEADER => {}
        Some(_) => {
            return Err(ManifestError::Files {
                line: 1,
                msg: "header row is not the files.tsv header".into(),
            })
        }
        None if lenient_tail => return Ok(Vec::new()),
        None => {
            return Err(ManifestError::Files {
                line: 1,
                msg: "empty file".into(),
            })
        }
    }
    it.map(|(i, l)| FileRow::from_line(l, i + 1)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Manifest {
        let mut kinds = BTreeMap::new();
        kinds.insert("dc6".to_owned(), 1);
        kinds.insert("excel".to_owned(), 1);
        let mut counts = BTreeMap::new();
        counts.insert(
            "dc6".to_owned(),
            KindCounts {
                converted: 3,
                failed: 1,
                skipped: 2,
            },
        );
        let mut unnamed = BTreeMap::new();
        unnamed.insert("d2data.mpq".to_owned(), 7);
        Manifest {
            converter_version: "0.1.2".into(),
            converter_commit: "abc123".into(),
            kinds,
            language: "ENG".into(),
            lod: true,
            archives: vec![ArchiveId {
                name: "d2data.mpq".into(),
                size: 99,
                sha256: "ab".repeat(32),
            }],
            counts,
            unnamed_blocks: unnamed,
            complete: false,
            files_sha256: "cd".repeat(32),
        }
    }

    // Covers: specs/formats/native-assets.md §3.3
    #[test]
    fn manifest_round_trips_and_is_deterministic() {
        let m = sample();
        let text = m.to_toml();
        assert_eq!(text, m.to_toml());
        assert_eq!(Manifest::from_toml(&text).unwrap(), m);
        assert!(text.starts_with("format = \"d2rs-native\"\nformat_version = 1\n"));
        assert!(!text.contains("time") && !text.contains("date"));
    }

    // Covers: specs/formats/native-assets.md §3.3
    #[test]
    fn unknown_format_version_is_rejected() {
        let text = sample()
            .to_toml()
            .replace("format_version = 1", "format_version = 2");
        let e = Manifest::from_toml(&text).unwrap_err();
        assert!(e.to_string().contains("format_version 2"), "{e}");
        let text = sample().to_toml().replace("d2rs-native", "other");
        assert!(Manifest::from_toml(&text).is_err());
    }

    // Covers: specs/formats/native-assets.md §3.4
    #[test]
    fn loadable_needs_complete_and_known_kinds() {
        let mut m = sample();
        let mut known: BTreeMap<String, u32> = m.kinds.clone();
        assert!(m
            .check_loadable(&known)
            .unwrap_err()
            .to_string()
            .contains("not complete"));
        m.complete = true;
        m.check_loadable(&known).unwrap();
        known.insert("dc6".into(), 2);
        assert!(m
            .check_loadable(&known)
            .unwrap_err()
            .to_string()
            .contains("dc6"));
        known.remove("dc6");
        assert!(m
            .check_loadable(&known)
            .unwrap_err()
            .to_string()
            .contains("not known"));
    }

    fn row(path: &str, status: Status) -> FileRow {
        FileRow {
            path: path.into(),
            kind: "dc6".into(),
            archive: "d2data.mpq".into(),
            source_sha256: "11".repeat(32),
            native: vec![
                NativeFile {
                    name: format!("{path}.toml"),
                    size: 5,
                    sha256: "22".repeat(32),
                },
                NativeFile {
                    name: format!("{path}.png"),
                    size: 9,
                    sha256: "33".repeat(32),
                },
            ],
            status,
        }
    }

    // Covers: specs/formats/native-assets.md §3.3
    #[test]
    fn files_tsv_sorts_rows_and_native_lists() {
        let rows = vec![
            row("data/b.dc6", Status::Ok),
            row("data/a.dc6", Status::Failed("C-DC6".into())),
        ];
        let text = write_files_tsv(&rows);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], FILES_HEADER);
        assert!(lines[1].starts_with("data/a.dc6\t"));
        assert!(lines[1].ends_with("\tfailed:C-DC6"));
        // native entries sorted by name: `.png` before `.toml`
        assert!(lines[2].contains("data/b.dc6.png:9:"));
        assert!(lines[2].find(".png:").unwrap() < lines[2].find(".toml:").unwrap());
        let back = parse_files_tsv(&text, false).unwrap();
        assert_eq!(write_files_tsv(&back), text);
    }

    // Covers: specs/formats/native-assets.md §4.6 r1
    #[test]
    fn torn_last_row_is_dropped_only_when_lenient() {
        let text = write_files_tsv(&[row("a", Status::Ok), row("b", Status::Ok)]);
        let torn = &text[..text.len() - 10];
        assert!(parse_files_tsv(torn, false).is_err());
        let rows = parse_files_tsv(torn, true).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].path, "a");
    }

    // Covers: specs/formats/native-assets.md §3.3
    #[test]
    fn bad_rows_name_their_line() {
        let text = format!("{FILES_HEADER}\nonly\ttwo\n");
        let e = parse_files_tsv(&text, false).unwrap_err();
        assert!(e.to_string().contains("line 2"), "{e}");
        let text = format!("{FILES_HEADER}\na\tk\tz\th\t\tmaybe\n");
        assert!(parse_files_tsv(&text, false).is_err());
    }

    // Covers: specs/formats/native-assets.md §3.3
    #[test]
    fn native_names_cannot_break_the_row_grammar() {
        check_native_name("data/global/a.dc6.png").unwrap();
        for bad in ["a;b", "a:b", "a\tb", "a//b", "../x", "", "a\\b"] {
            assert!(check_native_name(bad).is_err(), "{bad}");
        }
    }
}
