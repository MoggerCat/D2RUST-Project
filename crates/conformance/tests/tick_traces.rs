//! The committed tick traces in `traces/sim/tick/` load and are well formed
//! (`specs/sim/tick.md`, Test vectors). `tick_replay.rs` replays them
//! against `d2-sim::tick`; `tools/trace-recorder/convert_tick.py --check`
//! replays them through the spec model (CI).

use std::path::PathBuf;

use conformance::Trace;

const INPUT_KINDS: &[&str] = &[
    "timer_set",
    "timer_cancel",
    "hash_add",
    "hash_remove",
    "room_add",
    "room_remove",
    "queue_add",
    "queue_remove",
    "queue_clear",
    "room_activate",
    "room_deactivate",
];
const EXPECTED_KINDS: &[&str] = &["timer_run", "lists"];

/// Checks one trace's shape; the error names the first problem.
fn check(trace: &Trace) -> Result<(), String> {
    if (trace.area.as_str(), trace.behavior.as_str()) != ("sim", "tick") {
        return Err(format!("{}: not a sim/tick trace", trace.id));
    }
    let frames = trace.setup["frames"].as_i64().ok_or("setup.frames")?;
    // seq numbers 1..N over both arrays; ticks never go back within one.
    let mut seqs = Vec::new();
    for (events, kinds) in [
        (&trace.inputs, INPUT_KINDS),
        (&trace.expected, EXPECTED_KINDS),
    ] {
        let mut last_tick = 1;
        for event in events {
            let kind = event["kind"].as_str().ok_or("kind")?;
            if !kinds.contains(&kind) {
                return Err(format!("{}: unknown kind {kind}", trace.id));
            }
            let tick = event["tick"].as_i64().ok_or("tick")?;
            if !(last_tick..=frames).contains(&tick) {
                return Err(format!("{}: tick {tick} after {last_tick}", trace.id));
            }
            last_tick = tick;
            seqs.push(event["data"]["seq"].as_u64().ok_or("seq")?);
        }
    }
    seqs.sort_unstable();
    let n = seqs.len() as u64;
    if !seqs.iter().copied().eq(1..=n) {
        return Err(format!("{}: seq is not 1..{n}", trace.id));
    }
    Ok(())
}

fn trace_paths() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../traces/sim/tick");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    paths
}

#[test]
fn tick_traces_are_well_formed() {
    let paths = trace_paths();
    assert!(
        paths.len() >= 3,
        "expected sim-0006..0008, found {}",
        paths.len()
    );
    for path in &paths {
        let trace = Trace::load(path).unwrap_or_else(|e| panic!("{e}"));
        check(&trace).unwrap_or_else(|e| panic!("{e}"));
    }
}

#[test]
fn a_broken_trace_is_reported() {
    let path = &trace_paths()[0];
    let mut trace = Trace::load(path).unwrap();
    trace.expected[5]["data"]["seq"] = trace.expected[4]["data"]["seq"].clone();
    assert!(check(&trace).unwrap_err().contains("seq is not"));
    let mut trace = Trace::load(path).unwrap();
    trace.inputs[3]["tick"] = 0.into();
    assert!(check(&trace).unwrap_err().contains("tick 0"));
}
