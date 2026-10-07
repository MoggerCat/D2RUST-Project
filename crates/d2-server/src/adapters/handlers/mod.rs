// Spec: specs/sim/intents-events.md
//! Intent handlers per system (`docs/HANDOFF.md` §2 step 3): each
//! submodule owns the C→S ids its system specs own, is called from
//! [`super::SimGame`]'s `handle` before the stub, and routes them into the
//! `d2-sim` module that owns their behaviour.

pub mod items;
pub mod player;
pub mod skills;
pub mod walk;
pub mod world;
