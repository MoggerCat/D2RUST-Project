// Spec: specs/formats/native-assets.md
//! The common API of every native kind (§4.3, §5.1): a decoded
//! `d2-formats` value is written to native files, read back from them, and
//! compared. N3's converter calls [`round_trip`] per file; N4's native
//! source calls [`NativeKind::read`].
//!
//! Files are in memory: a writer returns [`NativeFile`]s (path relative to
//! the native `base/` folder), a reader takes a [`FileStore`]. The caller
//! decides how they reach the disk (`.work/` then an atomic rename, §4.6).

use std::collections::BTreeMap;
use std::fmt;

use d2_formats::palette::Rgb;

/// A native file produced by a writer: `path` is relative to `base/` and
/// canonical (`P` plus the kind's suffix, §3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFile {
    pub path: String,
    pub bytes: Vec<u8>,
}

/// A load or write error naming the native file (M07).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{file}: {message}")]
pub struct NativeError {
    pub file: String,
    pub message: String,
}

impl NativeError {
    pub fn new(file: impl Into<String>, message: impl Into<String>) -> Self {
        NativeError {
            file: file.into(),
            message: message.into(),
        }
    }
}

/// Where a reader finds native files, by path relative to `base/`.
pub trait FileStore {
    /// The file's bytes, or `None` when the path does not exist.
    fn read(&self, path: &str) -> Option<Vec<u8>>;
}

/// An in-memory store (tests, and the converter's read-back of `.work/`).
#[derive(Debug, Clone, Default)]
pub struct MemStore(pub BTreeMap<String, Vec<u8>>);

impl MemStore {
    pub fn from_files(files: &[NativeFile]) -> Self {
        MemStore(
            files
                .iter()
                .map(|f| (f.path.clone(), f.bytes.clone()))
                .collect(),
        )
    }
}

impl FileStore for MemStore {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        self.0.get(path).cloned()
    }
}

/// The colours written to the `PLTE` chunk of every PNG: a viewing aid
/// only (§2.1 r2). The converter passes the act 1 `pal.dat`.
pub type ViewPalette = [Rgb; 256];

/// A grey ramp, for callers with no `pal.dat` at hand (tests).
pub fn grey_palette() -> ViewPalette {
    let mut p = [Rgb::default(); 256];
    for (i, c) in p.iter_mut().enumerate() {
        let v = i as u8;
        *c = Rgb { r: v, g: v, b: v };
    }
    p
}

/// The first difference between a decoded original and its native read-back:
/// the native file that holds it and a description naming direction /
/// frame / pixel (or field) (§7.1 r3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    pub file: String,
    pub detail: String,
}

impl fmt::Display for Difference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file, self.detail)
    }
}

/// Why a round-trip check failed (§4.5: `failed:<check>`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CheckError {
    #[error("write failed: {0}")]
    Write(NativeError),
    #[error("read-back failed: {0}")]
    Read(NativeError),
    #[error("mismatch: {0}")]
    Mismatch(Difference),
}

/// A decoded value with a native form.
pub trait NativeKind: Sized + PartialEq + fmt::Debug {
    /// The `native = "<kind>"` tag of the sidecar (§2.1 r7).
    const KIND: &'static str;
    /// The per-kind `native_version` this build writes and reads.
    const NATIVE_VERSION: u32 = 1;

    /// The native files of the value whose canonical source path is `p`
    /// (e.g. `data/global/ui/panel/x.dc6`), sorted by path. Deterministic:
    /// the same value gives the same bytes (§4.2).
    fn write(&self, p: &str, view: &ViewPalette) -> Result<Vec<NativeFile>, NativeError>;

    /// The value back from the native files of `p`. Strict (M07): any
    /// deviation from the spelling of §2 is an error naming the file.
    fn read(p: &str, store: &dyn FileStore) -> Result<Self, NativeError>;

    /// The first difference between `self` (the decoded original) and
    /// `native` (read back), or `None` when equal.
    fn first_difference(&self, native: &Self, p: &str) -> Option<Difference>;
}

/// The check of §4.3 / §7.1 r1: write, read back, compare. Returns the
/// native files on success.
pub fn round_trip<K: NativeKind>(
    original: &K,
    p: &str,
    view: &ViewPalette,
) -> Result<Vec<NativeFile>, CheckError> {
    let files = original.write(p, view).map_err(CheckError::Write)?;
    check_files(original, p, &files)?;
    Ok(files)
}

/// Reads `files` back and compares with `original` (also the entry for a
/// perturbed file set, §7.1 r3).
pub fn check_files<K: NativeKind>(
    original: &K,
    p: &str,
    files: &[NativeFile],
) -> Result<(), CheckError> {
    let store = MemStore::from_files(files);
    let back = K::read(p, &store).map_err(CheckError::Read)?;
    if &back == original {
        return Ok(());
    }
    Err(CheckError::Mismatch(
        original.first_difference(&back, p).unwrap_or(Difference {
            file: p.to_string(),
            detail: "decoded values differ".into(),
        }),
    ))
}

/// Reads a store file or fails naming it.
pub(crate) fn need(store: &dyn FileStore, path: &str) -> Result<Vec<u8>, NativeError> {
    store
        .read(path)
        .ok_or_else(|| NativeError::new(path, "missing native file"))
}

/// Early-returns the first differing field of two structs as a
/// [`Difference`] in `$file`, naming `$what`.
macro_rules! diff_fields {
    ($file:expr, $what:expr, $a:expr, $b:expr, [$($f:ident),+ $(,)?]) => {
        $(
            if $a.$f != $b.$f {
                return Some($crate::kind::Difference {
                    file: $file.to_string(),
                    detail: format!(
                        "{}: {} {:?} != {:?}",
                        $what,
                        stringify!($f),
                        $a.$f,
                        $b.$f
                    ),
                });
            }
        )+
    };
}
pub(crate) use diff_fields;

/// The first differing pixel of two index buffers laid out `width` wide.
pub(crate) fn pixel_diff(a: &[u8], b: &[u8], width: usize) -> Option<String> {
    if a.len() != b.len() {
        return Some(format!("pixel count {} != {}", a.len(), b.len()));
    }
    let i = a.iter().zip(b).position(|(x, y)| x != y)?;
    let w = width.max(1);
    Some(format!(
        "pixel ({}, {}): index {} != {}",
        i % w,
        i / w,
        a[i],
        b[i]
    ))
}
