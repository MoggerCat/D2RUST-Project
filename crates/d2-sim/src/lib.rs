//! Deterministic D2 game logic, run at a fixed 25 Hz tick.
//!
//! Input: the previous state plus this tick's player intents.
//! Output: the new state plus events. Phase 3 so far: the D2 seeded RNG
//! ([`rng`]), unit lists ([`units`]), the tick core ([`tick`], [`game`]),
//! combat ([`combat`]) and skill levels ([`skills`]).
//!
//! Rules (CLAUDE.md rule 6): no I/O, no wall-clock time, no global or thread
//! RNG, no unordered iteration affecting outcomes, no floating point. Some
//! of these are enforced by `clippy.toml` and `[lints]` in this crate.

pub mod combat;
pub mod game;
pub mod rng;
pub mod skills;
pub mod tick;
pub mod units;

/// Simulation ticks per second, matching the original game.
pub const TICKS_PER_SECOND: u32 = 25;
