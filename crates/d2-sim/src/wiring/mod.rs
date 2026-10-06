// Spec: specs/sim/tick.md §3, §5.6 (wiring of the module seams)
//! Wiring: the seam traits of the Phase 3 modules implemented on their
//! real providers. Each submodule wires one group of seams; the modules'
//! own code stays as written and keeps its fake-based tests. Nothing here
//! decides behaviour: rules stay in the modules; an adapter maps a seam
//! call to a provider call.
//!
//! - [`economy`]: items ↔ stats / stat lists, treasure → item creation,
//!   cube and quests → items and unit fields.
//! - [`action`]: combat, missiles, monster AI, DRLG and waypoints ↔ units,
//!   stats and rooms; the combined timer-event dispatcher the tick runs.

pub mod action;
pub mod economy;
pub mod interaction;
