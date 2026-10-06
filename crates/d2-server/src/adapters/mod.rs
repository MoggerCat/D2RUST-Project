// Spec: specs/sim/intents-events.md
//! The seams of [`crate::seams`] on the real crates (`docs/HANDOFF.md` §2
//! step 4): [`ProtoSizes`] on `d2-proto`'s size lookup, [`SimGame`] on
//! `d2_sim::game::Game` and `d2_sim::tick`. `d2-sim` may not depend on
//! `d2-server` (depcheck), so the adapters live here.

mod sim;
mod sizes;

pub use sim::{AdapterError, PlayerData, PlayerFields, SimGame, UnitFacts, Unspecified};
pub use sizes::ProtoSizes;
