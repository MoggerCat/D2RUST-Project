//! Wiring: the seam traits of the Phase 3 modules implemented on their
//! real providers. Each submodule wires one group of seams; the modules'
//! own code stays as written and keeps its fake-based tests.
//!
//! - [`economy`]: items ↔ stats / stat lists, treasure → item creation,
//!   cube and quests → items and unit fields.

pub mod economy;
