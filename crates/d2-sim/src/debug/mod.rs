// Spec: specs/tools/state-snapshot.md
//! Debug read-outs of a game: nothing here decides behaviour or writes to
//! the game (CLAUDE.md rule 6: no I/O, no RNG step, no list change).
//!
//! - [`state`]: the `state-1` game-state snapshot (one record per server
//!   tick) the 1.14d recorder and `d2-client state-dump` both write.
//! - `rng_trace` (`rng-trace` feature, off by default): the log of every
//!   seeded draw (`specs/tools/rng-trace.md`).
//! - [`coverage`] (`coverage-map` feature, off by default): behaviour
//!   coverage counters (`specs/tools/coverage-map.md`).

pub mod coverage;
#[cfg(feature = "rng-trace")]
pub mod rng_trace;
pub mod state;
