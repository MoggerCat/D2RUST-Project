//! Readers for D2 file formats: MPQ, DC6, DCC, DT1, DS1, COF, AnimData, palettes and
//! string tables. Phase 1 fills this in; each reader names its spec.
//!
//! Binary parsing uses explicit little-endian reads, never transmutes.

pub mod animdata;
pub mod cof;
mod cursor;
pub mod dc6;
pub mod dcc;
pub mod ds1;
pub mod dt1;
pub mod font;
pub mod mpq;
pub mod palette;
pub mod tbl;

pub use cursor::FormatError;

/// The data archives every supported install must contain (see
/// `docs/REQUIRED_FILES.md`). Video archives are optional for engine work.
pub const REQUIRED_MPQS: &[&str] = &[
    "d2data.mpq",
    "d2exp.mpq",
    "d2char.mpq",
    "d2sfx.mpq",
    "d2speech.mpq",
    "d2xtalk.mpq",
    "d2music.mpq",
    "d2xmusic.mpq",
    "patch_d2.mpq",
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn game_dir() -> PathBuf {
        PathBuf::from(std::env::var("D2_GAME_DIR").expect("D2_GAME_DIR must be set"))
    }

    /// Finds `name` in `dir`, ignoring case (installs vary: `d2data.mpq`, `D2Data.mpq`).
    fn find_ci(dir: &std::path::Path, name: &str) -> Option<PathBuf> {
        std::fs::read_dir(dir)
            .ok()?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.eq_ignore_ascii_case(name))
            })
    }

    #[test]
    #[ignore = "needs original game files in D2_GAME_DIR"]
    fn game_dir_contains_required_mpqs() {
        let dir = game_dir();
        let missing: Vec<_> = REQUIRED_MPQS
            .iter()
            .filter(|name| find_ci(&dir, name).is_none())
            .collect();
        assert!(
            missing.is_empty(),
            "missing in {}: {missing:?}",
            dir.display()
        );
    }
}
