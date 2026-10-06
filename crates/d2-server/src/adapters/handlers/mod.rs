// Spec: specs/sim/intents-events.md
//! Intent handlers per system (`docs/HANDOFF.md` §2 step 3): each
//! submodule owns the C→S ids its system specs own and is called from
//! [`super::SimGame`]'s `handle` before the stub.

pub mod items;
