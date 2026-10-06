//! Authoritative server. Runs `d2-sim`, validates every intent, owns
//! accounts and character storage. Runs in-process for single player.
//!
//! Phase 3: the local transport and host loop (`specs/sim/intents-events.md`
//! §1–§4, `specs/sim/tick.md` §1, §8). What other crates provide comes in
//! through the traits in [`seams`]. Accounts and storage: Phases 5 and 7.

pub mod adapters;
pub mod buffers;
pub mod dispatch;
pub mod host;
pub mod seams;
pub mod transport;
pub mod world_data;

#[cfg(test)]
mod tests;
