// Spec: specs/formats/native-assets.md
//! d2rs-native asset formats: readers, writers and the exact round-trip
//! check of every kind (§2, §7.1). No Bevy (rule 5).
//!
//! Module lists are split by owning session (§8) so parallel sessions
//! never edit the same lines: each adds its `pub mod` lines only below
//! its own marker.

// ---- N1: images (png, sheet, tileset, expfield, pal)

// ---- N2: text and audio (toml_kinds, tbl, wav, animdata, tables)
pub mod animdata;
pub mod tables;
pub mod tbl;
pub mod toml_kinds;
pub mod wav;

// ---- N3: converter support (manifest)

// ---- N4: runtime source and mod layers (source, layers)
