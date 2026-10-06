// Spec: specs/tools/scenario.md
//! Differential scenarios (`specs/tools/scenario.md`): one script runs on
//! the original 1.14d and on d2rs; each side writes a scenario trace
//! (`traces/FORMAT.md` §Scenario traces); [`compare`] reports the first
//! divergence.
//!
//! | Module | Does |
//! |---|---|
//! | [`script`] | the script parser (strict), the canonical writer, the typed-message encoder and its references |
//! | [`trace`] | the trace header and records, strict reader, canonical writer |
//! | [`compare`](mod@compare) | first divergence, masks (`specs/tools/scenario-masks.tsv`), summary |
//!
//! The d2rs runner is `tools/scenario-run`; the original-side runner's
//! contract is `docs/handoff/scenario-harness.md`.

pub mod compare;
pub mod script;
pub mod trace;

pub use compare::{compare, Divergence, Report, Verdict};
pub use script::Scenario;
pub use trace::{Header, Record, TraceFile};
