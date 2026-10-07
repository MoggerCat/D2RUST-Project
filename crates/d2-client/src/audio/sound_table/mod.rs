// Spec: specs/audio/sound-table.md
//! The sound table and request layer (`sound-table.md`): sound id →
//! file path, groups and variants ([`table`]), volume, pan and settings
//! ([`volume`]), requests, sound ticks and the 16 channels ([`system`]).
//!
//! Hook mapping (§13): [`SoundPaths`] is the play mode's `SoundTable` /
//! `SoundBank::file` (§1, §3, §4 r1–r2); the variant choice runs on the
//! local player's client unit seed through [`SoundWorld`] (§4 r3–r5);
//! admission, stealing and the scheduler rules (§5–§7) are
//! [`SoundSystem`], which hands the core every start, stop and volume/pan
//! send as cues (so the core runs with [`crate::audio::Unlimited`]);
//! [`DeviceGain`] is the `GainCurve` of §8.3.

pub mod system;
pub mod table;
pub mod volume;

#[cfg(test)]
mod tests;

pub use system::{
    ChannelKind, Fade, Request, RequestState, SoundCtx, SoundError, SoundSystem, SoundWorld,
    CHANNELS, REQUEST_SLOTS,
};
pub use table::{
    is_music_path, is_speech, LoadState, SoundEntry, SoundPaths, SoundTableData, SoundTableError,
};
pub use volume::{DeviceGain, SoundSettings};
