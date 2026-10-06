//! Replays traces from `traces/` (format: `traces/FORMAT.md`) against
//! `d2-sim` and reports mismatches. Reading trace files is this crate's job;
//! `d2-sim` itself does no I/O.

pub mod rng;
pub mod tick;
mod trace;

pub use trace::{Trace, TraceError};

/// Trace format version this harness reads. Must match `traces/FORMAT.md`.
pub const TRACE_FORMAT_VERSION: u32 = 1;

/// Game version every trace must be recorded from.
pub const GAME_VERSION: &str = "1.14d";
