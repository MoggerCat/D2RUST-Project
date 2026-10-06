//! Replays the committed tick traces `traces/sim/tick/*.json` through
//! `d2-sim::tick` (`specs/sim/tick.md`, `specs/sim/unit-order.md`, Test
//! vectors): every timer run in order and every list snapshot, exactly
//! (`conformance::tick`). The perturbation tests (METHODS M08) change one
//! expected record and require the replay to report exactly that record.

use std::path::PathBuf;

use conformance::tick::{replay, Mismatch, ReplayError, ReplayStats};
use conformance::Trace;
use serde_json::Value;

fn trace_path(id: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../traces/sim/tick/{id}.json"))
}

fn load(id: &str) -> Trace {
    Trace::load(&trace_path(id)).unwrap_or_else(|e| panic!("{e}"))
}

fn run(trace: &Trace) -> ReplayStats {
    replay(trace).unwrap_or_else(|e| panic!("{e}"))
}

fn mismatch(trace: &Trace) -> Mismatch {
    match replay(trace) {
        Err(ReplayError::Mismatch(m)) => m,
        Err(e) => panic!("expected a mismatch, got {e}"),
        Ok(stats) => panic!("perturbed trace replayed: {stats:?}"),
    }
}

fn seq(event: &Value) -> u64 {
    event["data"]["seq"].as_u64().unwrap()
}

fn counts(trace: &Trace) -> (usize, usize) {
    let runs = trace
        .expected
        .iter()
        .filter(|e| e["kind"] == "timer_run")
        .count();
    (runs, trace.expected.len() - runs)
}

/// Each trace replays with every run and snapshot compared.
fn replays_exactly(id: &str) {
    let trace = load(id);
    let stats = run(&trace);
    let (runs, snapshots) = counts(&trace);
    assert_eq!(stats.timer_runs, runs, "{id}: timer runs compared");
    assert_eq!(stats.snapshots, snapshots, "{id}: snapshots compared");
    assert_eq!(stats.inputs, trace.inputs.len(), "{id}: inputs applied");
    assert_eq!(
        i64::from(stats.ticks),
        trace.setup["frames"].as_i64().unwrap(),
        "{id}: ticks"
    );
}

#[test]
fn sim_0006_replays_exactly() {
    replays_exactly("sim-0006");
}

#[test]
fn sim_0007_replays_exactly() {
    replays_exactly("sim-0007");
}

#[test]
fn sim_0008_replays_exactly() {
    replays_exactly("sim-0008");
}

/// Swaps the timers of the `n`-th pair of adjacent runs of one tick (as
/// `convert_tick.py --perturb-run`); returns the first one's seq.
fn swap_runs(trace: &mut Trace, n: usize) -> u64 {
    let ex = &trace.expected;
    let pairs: Vec<usize> = (0..ex.len() - 1)
        .filter(|&j| {
            ex[j]["kind"] == "timer_run"
                && ex[j + 1]["kind"] == "timer_run"
                && ex[j]["tick"] == ex[j + 1]["tick"]
                && seq(&ex[j + 1]) == seq(&ex[j]) + 1
                && ex[j]["data"]["timer"] != ex[j + 1]["data"]["timer"]
        })
        .collect();
    let j = pairs[n];
    let a = trace.expected[j]["data"]["timer"].clone();
    let b = trace.expected[j + 1]["data"]["timer"].clone();
    trace.expected[j]["data"]["timer"] = b;
    trace.expected[j + 1]["data"]["timer"] = a;
    seq(&trace.expected[j])
}

#[test]
fn a_swapped_timer_run_is_reported_exactly() {
    for n in [0, 40, 400] {
        let mut trace = load("sim-0007");
        let at = swap_runs(&mut trace, n);
        let m = mismatch(&trace);
        assert_eq!((m.seq, m.field.as_str()), (at, "timer"), "pair {n}: {m}");
    }
}

#[test]
fn a_changed_run_is_reported_exactly() {
    // A run of a timer that d2-sim has not scheduled (yet).
    let mut trace = load("sim-0007");
    let j = trace
        .expected
        .iter()
        .position(|e| e["kind"] == "timer_run" && e["tick"] == 100)
        .unwrap();
    trace.expected[j]["data"]["timer"] = 999_999.into();
    let m = mismatch(&trace);
    assert_eq!(
        (m.seq, m.field.as_str()),
        (seq(&trace.expected[j]), "timer"),
        "{m}"
    );
}

#[test]
fn a_changed_snapshot_is_reported_exactly() {
    // Reverse one room's unit list in the first snapshot that has one
    // (as `convert_tick.py --perturb-lists`).
    let mut trace = load("sim-0007");
    let mut perturbed = None;
    'find: for (j, e) in trace.expected.iter_mut().enumerate() {
        if e["kind"] != "lists" {
            continue;
        }
        let Some(acts) = e["data"]["acts"].as_array_mut() else {
            continue;
        };
        for (a, rooms) in acts.iter_mut().enumerate() {
            let Some(rooms) = rooms.as_array_mut() else {
                continue;
            };
            for (r, room) in rooms.iter_mut().enumerate() {
                let units = room["units"].as_array_mut().unwrap();
                let reversed: Vec<Value> = units.iter().rev().cloned().collect();
                if units.len() >= 2 && *units != reversed {
                    *units = reversed;
                    perturbed = Some((j, format!("acts[{a}][{r}].units[0]")));
                    break 'find;
                }
            }
        }
    }
    let (j, field) = perturbed.unwrap();
    let m = mismatch(&trace);
    assert_eq!(m.seq, seq(&trace.expected[j]), "{m}");
    // The first differing value: the type or GUID of the room's first unit.
    assert!(m.field.starts_with(&field), "{m}");
}

#[test]
fn a_changed_schedule_is_reported_exactly() {
    // An input: the recorded expire of a timed schedule.
    let mut trace = load("sim-0007");
    let j = trace
        .inputs
        .iter()
        .position(|e| e["kind"] == "timer_set" && e["data"]["req"].as_i64() > Some(0))
        .unwrap();
    let expire = trace.inputs[j]["data"]["expire"].as_i64().unwrap();
    trace.inputs[j]["data"]["expire"] = (expire + 1).into();
    let m = mismatch(&trace);
    assert_eq!(
        (m.seq, m.field.as_str()),
        (seq(&trace.inputs[j]), "expire"),
        "{m}"
    );
}
