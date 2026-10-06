//! Replays traces from `traces/` (format: `traces/FORMAT.md`) against
//! `d2-sim` and reports mismatches. Reading trace files is this crate's job;
//! `d2-sim` itself does no I/O.
//!
//! | Module | Input | Replayed through |
//! |---|---|---|
//! | [`rng`] | `traces/sim/rng/*.json` | `d2_sim::rng` |
//! | [`tick`] | `traces/sim/tick/*.json` | `d2_sim::tick`, `UnitLists` |
//! | [`rooms`] | `traces/sim/tick/*.json` | a [`rooms::RoomModel`] |
//! | [`units`] | `traces/raw/*-tick.jsonl` (`tick-raw-1`) | `d2_sim::units::anim` |
//! | [`stats`] | `traces/raw/*-stats.jsonl` (`stats-raw-1`) | `d2_sim::stats::StatLists` |
//! | [`packets`] | `traces/raw/*-packets.jsonl` (`packets-raw-1`) | a [`packets::PacketServer`] (`d2-server`) |

pub mod packets;
pub mod raw;
pub mod rng;
pub mod rooms;
pub mod stats;
pub mod tick;
mod trace;
pub mod units;

pub use trace::{Trace, TraceError};

/// Trace format version this harness reads. Must match `traces/FORMAT.md`.
pub const TRACE_FORMAT_VERSION: u32 = 1;

/// Game version every trace must be recorded from.
pub const GAME_VERSION: &str = "1.14d";
