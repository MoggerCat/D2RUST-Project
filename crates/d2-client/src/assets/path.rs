// Spec: specs/client/assets.md
//! Canonical asset paths (§A1) and the plain-Rust read behind the `mpq://`
//! source.
//!
//! MPQ names are case-insensitive (`formats/mpq.md` §3: `a`–`z` fold to
//! `A`–`Z`, `/` to `\`); Bevy asset paths are not. Every path is therefore
//! canonicalized (ASCII lowercase, `/` separators, no leading `/`) before
//! it reaches the `AssetServer`, so two spellings of one file give one
//! handle. The reader accepts canonical paths only (M07).

use std::fmt;

use d2_formats::mpq::ArchiveSet;

/// An archive path in canonical form, without the `mpq://` prefix:
/// `data/global/palette/act1/pal.dat`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalPath(String);

/// Why a path cannot be canonical. Every variant names the path.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    #[error("asset path is empty")]
    Empty,
    #[error("asset path {0:?} contains a non-ASCII character")]
    NonAscii(String),
    #[error("asset path {0:?} starts with a separator")]
    Leading(String),
    #[error("asset path {0:?} has an empty, `.` or `..` component")]
    BadComponent(String),
    /// `#` starts an asset label in Bevy paths; `:` would read as a source.
    #[error("asset path {0:?} contains a reserved character (`#` or `:`)")]
    Reserved(String),
    #[error("asset path {0:?} is not canonical (expected {1:?})")]
    NotCanonical(String, String),
}

/// Folds an archive or asset path to canonical spelling: `\` → `/`,
/// `A`–`Z` → `a`–`z`. Every other byte is kept (`mpq.md` §3 folds no
/// others). Total; [`CanonicalPath::new`] adds the validity checks.
pub fn fold(path: &str) -> String {
    path.chars()
        .map(|c| match c {
            '\\' => '/',
            c => c.to_ascii_lowercase(),
        })
        .collect()
}

impl CanonicalPath {
    /// Canonicalizes an archive path (`\` or `/` separators, any case).
    pub fn new(path: &str) -> Result<CanonicalPath, PathError> {
        let folded = fold(path);
        check(&folded)?;
        Ok(CanonicalPath(folded))
    }

    /// Accepts `path` only if it is already canonical (the reader's check).
    pub fn parse_canonical(path: &str) -> Result<CanonicalPath, PathError> {
        let c = CanonicalPath::new(path)?;
        if c.0 != path {
            return Err(PathError::NotCanonical(path.to_string(), c.0));
        }
        Ok(c)
    }

    /// The canonical path, `/` separators.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The name to look up in the archive set (`\` separators).
    pub fn archive_name(&self) -> String {
        self.0.replace('/', "\\")
    }

    /// Bevy asset path with the source prefix: `mpq://data/...`.
    pub fn asset_path(&self) -> String {
        format!("{}://{}", super::SOURCE, self.0)
    }
}

impl fmt::Display for CanonicalPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn check(folded: &str) -> Result<(), PathError> {
    let owned = || folded.to_string();
    if folded.is_empty() {
        return Err(PathError::Empty);
    }
    if !folded.is_ascii() {
        return Err(PathError::NonAscii(owned()));
    }
    if folded.starts_with('/') {
        return Err(PathError::Leading(owned()));
    }
    if folded.contains(['#', ':']) {
        return Err(PathError::Reserved(owned()));
    }
    if folded
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(PathError::BadComponent(owned()));
    }
    Ok(())
}

/// Where asset bytes come from: the user's archive set, or a test fake.
pub trait FileSource: Send + Sync + 'static {
    /// Reads `archive_name` (`\` separators). `None` if no archive holds
    /// it; `Some(Err(message))` if one does but reading failed.
    fn read_file(&self, archive_name: &str) -> Option<Result<Vec<u8>, String>>;
}

impl FileSource for ArchiveSet {
    fn read_file(&self, archive_name: &str) -> Option<Result<Vec<u8>, String>> {
        self.find(archive_name)
            .map(|archive| archive.read(archive_name).map_err(|e| e.to_string()))
    }
}

/// A failed asset read. Every variant names the path (§A1: no fallback).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReadError {
    #[error(transparent)]
    Path(#[from] PathError),
    #[error("asset {0} not found in any archive")]
    NotFound(CanonicalPath),
    #[error("asset {0}: {1}")]
    Archive(CanonicalPath, String),
}

/// Reads the asset at `path` (relative to `mpq://`, canonical) from
/// `source`. A non-canonical path is an error, not folded silently: it
/// would load a second handle for the same file.
pub fn read_asset<S: FileSource + ?Sized>(source: &S, path: &str) -> Result<Vec<u8>, ReadError> {
    let path = CanonicalPath::parse_canonical(path)?;
    match source.read_file(&path.archive_name()) {
        None => Err(ReadError::NotFound(path)),
        Some(Err(msg)) => Err(ReadError::Archive(path, msg)),
        Some(Ok(bytes)) => Ok(bytes),
    }
}

/// An in-memory [`FileSource`] keyed case-insensitively like an archive
/// (for tests and tools; holds no game data of its own).
#[derive(Debug, Default, Clone)]
pub struct MemorySource {
    files: std::collections::BTreeMap<String, Vec<u8>>,
}

impl MemorySource {
    pub fn insert(&mut self, archive_name: &str, bytes: Vec<u8>) {
        self.files.insert(mpq_fold(archive_name), bytes);
    }
}

/// `mpq.md` §3 name normalization (`a`–`z` → `A`–`Z`, `/` → `\`).
fn mpq_fold(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' => '\\',
            c => c.to_ascii_uppercase(),
        })
        .collect()
}

impl FileSource for MemorySource {
    fn read_file(&self, archive_name: &str) -> Option<Result<Vec<u8>, String>> {
        self.files.get(&mpq_fold(archive_name)).cloned().map(Ok)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: specs/client/assets.md §a1-paths-and-identity
    #[test]
    fn two_spellings_one_path() {
        // Test vector §A1.
        let a = CanonicalPath::new(r"DATA\Global\X.DC6").unwrap();
        let b = CanonicalPath::new(r"data\global\x.dc6").unwrap();
        assert_eq!(a, b);
        assert_eq!(a.as_str(), "data/global/x.dc6");
        assert_eq!(a.asset_path(), "mpq://data/global/x.dc6");
        assert_eq!(a.archive_name(), r"data\global\x.dc6");
        assert_eq!(
            super::super::asset_path(r"DATA\Global\X.DC6"),
            super::super::asset_path(r"data\global\x.dc6")
        );
    }

    // Covers: specs/client/assets.md §a1-paths-and-identity
    #[test]
    fn invalid_paths_are_errors() {
        assert_eq!(CanonicalPath::new(""), Err(PathError::Empty));
        assert!(matches!(
            CanonicalPath::new(r"\data\x.dc6"),
            Err(PathError::Leading(_))
        ));
        assert!(matches!(
            CanonicalPath::new("data//x.dc6"),
            Err(PathError::BadComponent(_))
        ));
        assert!(matches!(
            CanonicalPath::new("data/../x.dc6"),
            Err(PathError::BadComponent(_))
        ));
        assert!(matches!(
            CanonicalPath::new("data/x.dc6/"),
            Err(PathError::BadComponent(_))
        ));
        assert!(matches!(
            CanonicalPath::new("data/x.dc6#0"),
            Err(PathError::Reserved(_))
        ));
        assert!(matches!(
            CanonicalPath::new("data/é.dc6"),
            Err(PathError::NonAscii(_))
        ));
    }

    fn source() -> MemorySource {
        let mut s = MemorySource::default();
        s.insert(r"data\global\X.dc6", vec![1, 2, 3]);
        s
    }

    // Covers: specs/client/assets.md §a1-paths-and-identity
    #[test]
    fn reader_reads_canonical_paths() {
        assert_eq!(
            read_asset(&source(), "data/global/x.dc6"),
            Ok(vec![1, 2, 3])
        );
    }

    // Covers: specs/client/assets.md §a1-paths-and-identity
    #[test]
    fn missing_path_is_an_error_naming_it() {
        // Test vector §A1.
        let err = read_asset(&source(), "data/global/missing.dc6").unwrap_err();
        assert!(matches!(err, ReadError::NotFound(_)));
        assert!(err.to_string().contains("data/global/missing.dc6"), "{err}");
    }

    // Covers: specs/client/assets.md §a1-paths-and-identity
    #[test]
    fn reader_rejects_non_canonical_paths() {
        // Perturbation (M08): the same file, spelled with capitals, is
        // refused rather than loaded under a second handle.
        let err = read_asset(&source(), "data/global/X.dc6").unwrap_err();
        assert_eq!(
            err,
            ReadError::Path(PathError::NotCanonical(
                "data/global/X.dc6".into(),
                "data/global/x.dc6".into()
            ))
        );
        assert!(err.to_string().contains("data/global/X.dc6"));
    }

    // Covers: specs/client/assets.md §a1-paths-and-identity
    #[test]
    fn archive_errors_name_the_path() {
        struct Broken;
        impl FileSource for Broken {
            fn read_file(&self, _: &str) -> Option<Result<Vec<u8>, String>> {
                Some(Err("bad sector".into()))
            }
        }
        let err = read_asset(&Broken, "data/x.dc6").unwrap_err();
        assert_eq!(err.to_string(), "asset data/x.dc6: bad sector");
    }
}
