// Spec: specs/formats/mpq.md (Archive set)
//! The game's archives, searched in priority order.

use std::path::Path;

use super::{Archive, MpqError};

/// Archive names in lookup priority order (first match wins). Provisional,
/// see the spec's open questions.
pub const PRIORITY: [&str; 11] = [
    "patch_d2.mpq",
    "d2exp.mpq",
    "d2xmusic.mpq",
    "d2xtalk.mpq",
    "d2xvideo.mpq",
    "d2data.mpq",
    "d2char.mpq",
    "d2sfx.mpq",
    "d2music.mpq",
    "d2speech.mpq",
    "d2video.mpq",
];

/// Lookup rank of an archive file name (case-insensitive); unknown names
/// sort last.
pub fn priority(file_name: &str) -> usize {
    PRIORITY
        .iter()
        .position(|p| p.eq_ignore_ascii_case(file_name))
        .unwrap_or(PRIORITY.len())
}

/// The open archives of one install, in priority order.
#[derive(Debug)]
pub struct ArchiveSet {
    archives: Vec<Archive>,
}

impl ArchiveSet {
    /// Opens every known archive present in `dir` (names matched
    /// case-insensitively). Archives that are absent are skipped.
    pub fn open_dir(dir: impl AsRef<Path>) -> Result<ArchiveSet, MpqError> {
        let mut found: Vec<(usize, std::path::PathBuf)> = std::fs::read_dir(dir.as_ref())?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter_map(|p| {
                let name = p.file_name()?.to_str()?.to_owned();
                let rank = priority(&name);
                (rank < PRIORITY.len()).then_some((rank, p))
            })
            .collect();
        found.sort();
        let archives = found
            .into_iter()
            .map(|(_, p)| Archive::open(p))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ArchiveSet { archives })
    }

    pub fn archives(&self) -> &[Archive] {
        &self.archives
    }

    /// The highest-priority archive containing `name`.
    pub fn find(&self, name: &str) -> Option<&Archive> {
        self.archives.iter().find(|a| a.contains(name))
    }

    pub fn contains(&self, name: &str) -> bool {
        self.find(name).is_some()
    }

    /// Reads `name` from the highest-priority archive that has it.
    pub fn read(&self, name: &str) -> Result<Vec<u8>, MpqError> {
        self.find(name)
            .ok_or_else(|| MpqError::NotFound(name.to_owned()))?
            .read(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_order() {
        assert_eq!(priority("Patch_D2.mpq"), 0);
        assert!(priority("d2exp.mpq") < priority("d2data.mpq"));
        assert!(priority("D2XVIDEO.MPQ") < priority("d2data.mpq"));
        assert_eq!(priority("other.mpq"), PRIORITY.len());
    }
}
