// Spec: specs/tools/perf.md
//! Frame and tick timing for `tools/perf` (§2), on only when
//! `D2_PERF_OUT` names a file.
//!
//! Per frame of the Bevy app: `First` start → after `PreUpdate` (the
//! bridge pump: the server's drain, tick and flush run inside it) → after
//! `Update` (client update: model, UI, world view packing) → after `Last`
//! (post-update and last). Per render-world frame: the `Render` schedule
//! from `ExtractCommands` to `PostCleanup` (prepare, the compositor node,
//! submit, present). Per server tick: [`d2_server::perf`]. At exit
//! [`write_report`] writes everything plus the peak RSS as one JSON file
//! (`perf 1`). Wall-clock time only reaches this report, never the game.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use bevy::app::MainScheduleOrder;
use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;
use bevy::render::{Render, RenderApp, RenderSystems};

/// The report format line.
pub const FORMAT: &str = "perf 1";

static ON: AtomicBool = AtomicBool::new(false);

/// One main-world frame, microseconds.
#[derive(Clone, Copy, Debug, Default)]
struct FrameTime {
    /// `First` start to the next frame's `First` start (0 for the last).
    interval: u32,
    /// `First` + `PreUpdate` (the bridge pump with the server frame).
    pre: u32,
    /// `StateTransition` .. `Update` (client update).
    update: u32,
    /// `PostUpdate` + `Last`.
    post: u32,
}

#[derive(Default)]
struct Log {
    frames: Vec<FrameTime>,
    render: Vec<u32>,
    /// Headless (`state-dump`) bridge frames that ran a tick: the whole
    /// `Bridge::frame` (server frame, receive, client model), µs.
    bridge: Vec<u32>,
    start: Option<Instant>,
    marks: [Option<Instant>; 3],
    render_start: Option<Instant>,
}

static LOG: Mutex<Log> = Mutex::new(Log {
    frames: Vec::new(),
    render: Vec::new(),
    bridge: Vec::new(),
    start: None,
    marks: [None; 3],
    render_start: None,
});

/// Turns timing on when `D2_PERF_OUT` is set (call first in `main`).
pub fn enable_from_env() {
    if std::env::var_os("D2_PERF_OUT").is_some_and(|v| !v.is_empty()) {
        ON.store(true, Ordering::Relaxed);
        d2_server::perf::enable();
    }
}

/// Whether timing is on.
pub fn enabled() -> bool {
    ON.load(Ordering::Relaxed)
}

#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
struct AfterPreUpdate;
#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
struct AfterUpdate;
#[derive(ScheduleLabel, Debug, Hash, PartialEq, Eq, Clone)]
struct AfterLast;

fn us(d: std::time::Duration) -> u32 {
    u32::try_from(d.as_micros()).unwrap_or(u32::MAX)
}

fn frame_start() {
    let now = Instant::now();
    let Ok(mut log) = LOG.lock() else { return };
    if let (Some(s), [Some(a), Some(b), Some(c)]) = (log.start, log.marks) {
        let t = FrameTime {
            interval: us(now - s),
            pre: us(a - s),
            update: us(b - a),
            post: us(c - b),
        };
        log.frames.push(t);
    }
    log.start = Some(now);
    log.marks = [None; 3];
}

fn mark<const I: usize>() {
    if let Ok(mut log) = LOG.lock() {
        log.marks[I] = Some(Instant::now());
    }
}

fn render_start() {
    if let Ok(mut log) = LOG.lock() {
        log.render_start = Some(Instant::now());
    }
}

fn render_end() {
    if let Ok(mut log) = LOG.lock() {
        if let Some(s) = log.render_start.take() {
            log.render.push(us(s.elapsed()));
        }
    }
}

/// Headless: one `Bridge::frame` that started at `t0` (kept when it ticked).
pub fn record_bridge_frame(t0: Instant, ticked: bool) {
    if !ticked {
        return;
    }
    if let Ok(mut log) = LOG.lock() {
        log.bridge.push(us(t0.elapsed()));
    }
}

/// Adds the timing systems to `app` when timing is on.
pub fn add(app: &mut App) {
    if !enabled() {
        return;
    }
    app.init_schedule(AfterPreUpdate)
        .init_schedule(AfterUpdate)
        .init_schedule(AfterLast);
    {
        let mut order = app.world_mut().resource_mut::<MainScheduleOrder>();
        order.insert_after(PreUpdate, AfterPreUpdate);
        order.insert_after(Update, AfterUpdate);
        order.insert_after(Last, AfterLast);
    }
    app.add_systems(First, frame_start)
        .add_systems(AfterPreUpdate, mark::<0>)
        .add_systems(AfterUpdate, mark::<1>)
        .add_systems(AfterLast, mark::<2>);
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render
            .add_systems(Render, render_start.in_set(RenderSystems::ExtractCommands))
            .add_systems(Render, render_end.in_set(RenderSystems::PostCleanup));
    }
}

/// Peak resident set size in KiB (`VmHWM`), Linux only.
fn peak_rss_kib() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = s.lines().find(|l| l.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

/// Writes the report to `D2_PERF_OUT` (when timing is on).
pub fn write_report() {
    if !enabled() {
        return;
    }
    let Some(path) = std::env::var_os("D2_PERF_OUT") else {
        return;
    };
    let ticks = d2_server::perf::take();
    let log = LOG
        .lock()
        .map(|mut l| std::mem::take(&mut *l))
        .unwrap_or_default();
    let list = |v: Vec<String>| v.join(",");
    let text = format!(
        "{{\"format\":\"{FORMAT}\",\"peak_rss_kib\":{},\n\"ticks\":[{}],\n\"frames\":[{}],\n\"render\":[{}],\n\"bridge\":[{}]}}\n",
        peak_rss_kib().unwrap_or(0),
        list(
            ticks
                .iter()
                .map(|t| format!("[{},{},{},{}]", t.frame, t.drain_us, t.tick_us, t.flush_us))
                .collect()
        ),
        list(
            log.frames
                .iter()
                .map(|f| format!("[{},{},{},{}]", f.interval, f.pre, f.update, f.post))
                .collect()
        ),
        list(log.render.iter().map(u32::to_string).collect()),
        list(log.bridge.iter().map(u32::to_string).collect()),
    );
    if let Err(e) = std::fs::write(&path, text) {
        eprintln!("perf: {}: {e}", std::path::Path::new(&path).display());
    }
}
