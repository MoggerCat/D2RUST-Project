// Spec: specs/tools/perf.md
//! Server tick timing for `tools/perf` (off unless [`enable`] is called).
//!
//! [`crate::host::Host::frame`] measures the drain and, when a tick runs,
//! the tick plus its flush with the wall clock and appends one
//! [`TickTime`] here. The times are only read by the perf report; they
//! never reach the game (the sim stays clock-free, CLAUDE.md rule 6).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// One server frame that ran a tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TickTime {
    /// The game frame after the tick.
    pub frame: u32,
    /// Drain (C→S messages) in microseconds.
    pub drain_us: u32,
    /// The tick (`Tick::tick`) in microseconds.
    pub tick_us: u32,
    /// The flush after the tick in microseconds.
    pub flush_us: u32,
}

static ON: AtomicBool = AtomicBool::new(false);
static TIMES: Mutex<Vec<TickTime>> = Mutex::new(Vec::new());

/// Starts recording tick times (process-wide).
pub fn enable() {
    ON.store(true, Ordering::Relaxed);
}

/// Whether tick times are recorded.
pub fn enabled() -> bool {
    ON.load(Ordering::Relaxed)
}

pub(crate) fn record(t: TickTime) {
    if let Ok(mut v) = TIMES.lock() {
        v.push(t);
    }
}

/// Takes the recorded tick times.
pub fn take() -> Vec<TickTime> {
    TIMES
        .lock()
        .map(|mut v| std::mem::take(&mut *v))
        .unwrap_or_default()
}

/// Microseconds since `t`, saturated to u32.
pub(crate) fn us_since(t: std::time::Instant) -> u32 {
    u32::try_from(t.elapsed().as_micros()).unwrap_or(u32::MAX)
}
