// Spec: specs/formats/native-assets.md
//! d2rs-native asset formats: readers, writers and the exact round-trip
//! check of every kind (§2, §7.1). No Bevy (rule 5).
//!
//! Module lists are split by owning session (§8) so parallel sessions
//! never edit the same lines: each adds its `pub mod` lines only below
//! its own marker.

// ---- N1: images (png, sheet, tileset, expfield, pal)
pub mod expfield;
pub mod kind;
pub mod native_toml;
pub mod pal;
pub mod png;
pub mod sheet;
pub mod tileset;

// ---- N2: text and audio (toml_kinds, tbl, wav, animdata, tables)

// ---- N3: converter support (manifest)

// ---- N4: runtime source and mod layers (source, layers)
