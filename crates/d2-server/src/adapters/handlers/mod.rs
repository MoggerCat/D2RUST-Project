// Spec: specs/sim/intents-events.md
//! Intent handlers per system (`docs/HANDOFF.md` §2 step 3): each module
//! routes its C→S ids from [`super::SimGame`]'s `handle` into the `d2-sim`
//! module that owns their behaviour.

pub mod world;
