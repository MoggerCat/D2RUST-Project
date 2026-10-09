// Spec: specs/tools/state-snapshot.md
//! Debug read-outs of a game: nothing here decides behaviour or writes to
//! the game (CLAUDE.md rule 6: no I/O, no RNG step, no list change).
//!
//! - [`state`]: the `state-1` game-state snapshot (one record per server
//!   tick) the 1.14d recorder and `d2-client state-dump` both write.

pub mod state;
