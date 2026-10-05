//! Replays traces from `traces/` (format: `traces/FORMAT.md`) against
//! `d2-sim` and reports coverage. Phase 4 fills this in.

/// Trace format version this harness reads. Must match `traces/FORMAT.md`.
pub const TRACE_FORMAT_VERSION: u32 = 1;
