// Spec: specs/formats/native-assets.md §1.1, §2.8
//! The kinds this binary converts.
//!
//! Hook-up for N1 / N2: implement [`Kind`](crate::Kind) for each kind in
//! §1.1 as an adapter over the `d2-native` reader and writer of that kind
//! and add it to [`builtin`]. Until then only the verbatim `excel` copy
//! (§2.8 r1) is wired here.

use crate::kind::{Failure, Kind, NativeData, Written};

/// All kinds this build converts, in registry order.
pub fn builtin() -> Vec<Box<dyn Kind>> {
    vec![Box::new(Excel)]
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
