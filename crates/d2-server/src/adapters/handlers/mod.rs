// Spec: specs/sim/intents-events.md
//! Intent handlers per system (`docs/HANDOFF.md` §2 step 3): each module
//! replaces the stubs of [`super::SimGame`]'s `handle` for the C→S ids
//! its specs own.

pub mod skills;
