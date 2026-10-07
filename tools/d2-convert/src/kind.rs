// Spec: specs/formats/native-assets.md §4.1, §4.3
//! The seam between the converter and the per-kind readers and writers
//! (N1: images, N2: text and audio). A kind decodes the original, writes
//! the native files, and checks bytes read back from disk against a fresh
//! decode of the source (§4.3).

/// A failed step: `check` goes in `files.tsv` as `failed:<check>` (e.g.
/// `decode`, `C-DC6`), `detail` is the first difference, for the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub check: String,
    pub detail: String,
}

impl Failure {
    pub fn new(check: impl Into<String>, detail: impl Into<String>) -> Failure {
        Failure {
            check: check.into(),
            detail: detail.into(),
        }
    }
}

/// One native file: `name` is relative to `base/` (canonical path plus the
/// kind's suffix, §3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeData {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// What [`Kind::write`] produces.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Written {
    pub files: Vec<NativeData>,
    /// Lines for the report (DT1 fallback tiles, `tbl` rebuild differences).
    pub notes: Vec<String>,
}

pub trait Kind: Sync {
    /// Name used in `manifest.toml` `kinds` / `counts` and `files.tsv`.
    fn name(&self) -> &'static str;
    /// `native_version` the writer produces (§2.1 r7).
    fn native_version(&self) -> u32;
    /// Conversion order: lower first (§4.1 r3: tables load before the kinds
    /// whose names come from them).
    fn phase(&self) -> u8 {
        1
    }
    /// Whether this kind converts the file at canonical path `canon`.
    fn claims(&self, canon: &str) -> bool;
    /// Whether [`Kind::references`] can return names (so the name-set
    /// closure of §1 r3 reads the file).
    fn has_references(&self) -> bool {
        false
    }
    /// Paths a file names (DS1 → DT1, COF components, sound files, …), in
    /// any spelling. Called only when [`Kind::has_references`]; a file that
    /// does not decode returns none (its conversion fails separately).
    fn references(&self, _canon: &str, _src: &[u8]) -> Vec<String> {
        Vec::new()
    }
    /// Decode `src` with the existing reader and write the native files.
    fn write(&self, canon: &str, src: &[u8]) -> Result<Written, Failure>;
    /// `native` holds the files exactly as read back from disk. Decode them
    /// with the native reader and compare with a decode of `src`.
    fn check(&self, canon: &str, src: &[u8], native: &[NativeData]) -> Result<(), Failure>;
}
