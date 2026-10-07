// Spec: specs/formats/native-assets.md §1 r3
//! The name set: what the converter looks up in the archives.

use std::collections::{BTreeMap, BTreeSet};

use d2_formats::mpq::names::known_names;
use d2_formats::mpq::{ArchiveSet, MpqError};
use d2_native::manifest::check_native_name;

use crate::kind::Kind;

/// Folds a path to canonical spelling (`client/assets.md` §A1): `\` → `/`,
/// `A`–`Z` → `a`–`z`. `None` when it cannot be a native path (non-ASCII,
/// `#`, a character that breaks `files.tsv`, dot parts).
pub fn canonical(name: &str) -> Option<String> {
    let c: String = name
        .chars()
        .map(|c| {
            if c == '\\' {
                '/'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect();
    if !c.is_ascii() || c.contains('#') || check_native_name(&c).is_err() {
        return None;
    }
    Some(c)
}

/// The archive spelling of a canonical path.
pub fn archive_name(canon: &str) -> String {
    canon.replace('/', "\\")
}

/// Canonical path → the kind index that claims it (`None`: unconverted).
#[derive(Debug, Default)]
pub struct NameSet {
    pub names: BTreeMap<String, Option<usize>>,
    /// Listfile names that cannot be native paths.
    pub rejected: BTreeSet<String>,
}

fn claim(kinds: &[Box<dyn Kind>], canon: &str) -> Option<usize> {
    kinds.iter().position(|k| k.claims(canon))
}

/// Builds the name set (§1 r3): every listfile and the extra names, then
/// the closure over the names that converted files reference.
pub fn build(set: &ArchiveSet, kinds: &[Box<dyn Kind>]) -> Result<NameSet, MpqError> {
    let mut out = NameSet::default();
    let mut work: Vec<String> = Vec::new();
    let add = |out: &mut NameSet, work: &mut Vec<String>, raw: &str| match canonical(raw) {
        None => {
            out.rejected.insert(raw.to_owned());
        }
        Some(c) => {
            if !out.names.contains_key(&c) {
                let k = claim(kinds, &c);
                out.names.insert(c.clone(), k);
                work.push(c);
            }
        }
    };
    for raw in known_names(set.archives())?.into_values() {
        add(&mut out, &mut work, &raw);
    }
    while let Some(c) = work.pop() {
        let Some(k) = out.names[&c] else { continue };
        if !kinds[k].has_references() {
            continue;
        }
        let Ok(Some(bytes)) = set
            .read_with_source(&archive_name(&c))
            .map(|o| o.map(|x| x.1))
        else {
            continue;
        };
        for r in kinds[k].references(&c, &bytes) {
            add(&mut out, &mut work, &r);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Covers: specs/formats/native-assets.md §1 r3
    #[test]
    fn two_spellings_fold_to_one_canonical_path() {
        assert_eq!(
            canonical(r"DATA\Global\UI\Panel.DC6").as_deref(),
            Some("data/global/ui/panel.dc6")
        );
        assert_eq!(
            canonical("data/global/ui/panel.dc6"),
            canonical(r"data\global\ui\PANEL.dc6")
        );
        assert_eq!(canonical("caf\u{e9}.txt"), None);
        assert_eq!(canonical("a#b"), None);
        assert_eq!(canonical("a;b"), None);
    }
}
