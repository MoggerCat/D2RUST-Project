//! Deterministic D2 game logic, run at a fixed 25 Hz tick.
//!
//! Input: the previous state plus this tick's player intents.
//! Output: the new state plus events. Phase 3 so far: the D2 seeded RNG
//! ([`rng`]), unit lists ([`units`]) and the tick core ([`tick`], [`game`]).
//!
//! Rules (CLAUDE.md rule 6): no I/O, no wall-clock time, no global or thread
//! RNG, no unordered iteration affecting outcomes, no floating point. Some
//! of these are enforced by `clippy.toml` and `[lints]` in this crate.

pub mod game;
pub mod rng;
pub mod tick;
pub mod units;
pub mod world;

/// Simulation ticks per second, matching the original game.
pub const TICKS_PER_SECOND: u32 = 25;
