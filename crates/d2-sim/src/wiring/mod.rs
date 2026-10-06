//! Wiring: the seam traits of the Phase 3 modules implemented on their
//! real providers. Each submodule wires one group of seams; the modules'
//! own code stays as written and keeps its fake-based tests.
//!
//! - [`economy`]: items ↔ stats / stat lists, treasure → item creation,
//!   cube and quests → items and unit fields.

pub mod economy;
// Spec: specs/sim/tick.md §3, §5.6 (wiring of the module seams)
//! Adapters that connect each module's seams (narrow traits) to their
//! real providers in `d2-sim`. Nothing here decides behaviour: rules stay
//! in the modules; an adapter maps a seam call to a provider call.
pub mod action;
