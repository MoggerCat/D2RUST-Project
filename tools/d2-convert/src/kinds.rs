// Spec: specs/formats/native-assets.md §1.1, §2.8
//! The kinds this binary converts.
//!
//! Hook-up for N1 / N2: implement [`Kind`](crate::Kind) for each kind in
//! §1.1 as an adapter over the `d2-native` reader and writer of that kind
//! and add it to [`builtin`]. Until then only the verbatim `excel` copy
//! (§2.8 r1) is wired here.

use d2_native::asset::{AssetError, AssetKind};
use d2_native::kind::NativeFile;

use crate::kind::{Failure, Kind, NativeData, Written};

/// All kinds this build converts, in registry order.
pub fn builtin() -> Vec<Box<dyn Kind>> {
    let mut v: Vec<Box<dyn Kind>> = vec![Box::new(Excel)];
    v.extend(
        AssetKind::ALL
            .into_iter()
            .map(|k| Box::new(Native(k)) as Box<dyn Kind>),
    );
    v
}

/// Audio is not converted for now (native-assets.md OQ3: "no sound for
/// now"): no kind claims `.wav`, so the report lists these files with this
/// reason.
pub const SKIPPED_EXTENSIONS: &[(&str, &str)] =
    &[("wav", "audio not converted for now (native-assets.md OQ3)")];

/// Adapter over `d2_native::asset::AssetKind` (N1's image kinds, N2's
/// text kinds).
pub struct Native(pub AssetKind);

fn fail(e: AssetError) -> Failure {
    Failure::new(e.check, e.detail)
}

impl Kind for Native {
    fn name(&self) -> &'static str {
        self.0.name()
    }
    fn native_version(&self) -> u32 {
        self.0.native_version()
    }
    fn phase(&self) -> u8 {
        self.0.phase()
    }
    fn claims(&self, canon: &str) -> bool {
        self.0.claims(canon)
    }
    fn has_references(&self) -> bool {
        self.0.has_references()
    }
    fn references(&self, canon: &str, src: &[u8]) -> Vec<String> {
        self.0.references(canon, src)
    }
    fn write(&self, canon: &str, src: &[u8]) -> Result<Written, Failure> {
        let w = self.0.write(canon, src).map_err(fail)?;
        Ok(Written {
            files: w
                .files
                .into_iter()
                .map(|f| NativeData {
                    name: f.path,
                    bytes: f.bytes,
                })
                .collect(),
            notes: w.notes,
        })
    }
    fn check(&self, canon: &str, src: &[u8], native: &[NativeData]) -> Result<(), Failure> {
        let files: Vec<NativeFile> = native
            .iter()
            .map(|n| NativeFile {
                path: n.name.clone(),
                bytes: n.bytes.clone(),
            })
            .collect();
        self.0.check(canon, src, &files).map_err(fail)
    }
}

/// `data/global/excel/*.txt`: a byte-for-byte copy (§2.8 r1); the check is
/// byte equality (C-TABLE step 1). The step that compiles the native set
/// against each live `.bin` (C-TABLE step 2, `_bin-overrides.toml`) comes
/// with N2's `tables.rs`.
pub struct Excel;

impl Kind for Excel {
    fn name(&self) -> &'static str {
        "excel"
    }
    fn native_version(&self) -> u32 {
        1
    }
    fn phase(&self) -> u8 {
        0
    }
    fn claims(&self, canon: &str) -> bool {
        canon
            .strip_prefix("data/global/excel/")
            .is_some_and(|rest| !rest.contains('/') && rest.ends_with(".txt"))
    }
    fn write(&self, canon: &str, src: &[u8]) -> Result<Written, Failure> {
        Ok(Written {
            files: vec![NativeData {
                name: canon.to_owned(),
                bytes: src.to_vec(),
            }],
            notes: Vec::new(),
        })
    }
    fn check(&self, canon: &str, src: &[u8], native: &[NativeData]) -> Result<(), Failure> {
        let [one] = native else {
            return Err(Failure::new(
                "C-TABLE",
                format!("{} native files", native.len()),
            ));
        };
        if one.name != canon {
            return Err(Failure::new("C-TABLE", format!("native name {}", one.name)));
        }
        match src.iter().zip(&one.bytes).position(|(a, b)| a != b) {
            Some(at) => Err(Failure::new(
                "C-TABLE",
                format!("{canon}: first difference at byte {at}"),
            )),
            None if src.len() != one.bytes.len() => Err(Failure::new(
                "C-TABLE",
                format!("{canon}: length {} vs {}", one.bytes.len(), src.len()),
            )),
            None => Ok(()),
        }
    }
}
