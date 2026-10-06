//! Tests added from mutation testing of the plain-Rust parts of
//! `d2-client` (METHODS M08): each kills mutants the existing tests let
//! survive. One test binary, so a mutation run links it once per mutant.
//! Survivors, classifications and counts: `docs/handoff/mutants-client.md`.

mod assets_cache;
mod assets_size;
mod audio;
mod bridge;
mod controls;
mod frames;
mod scene;
mod ui;
mod verify;
