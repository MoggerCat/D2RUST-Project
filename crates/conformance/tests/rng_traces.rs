//! Every recorded RNG trace in `traces/sim/rng/` replays exactly
//! (`specs/sim/rng.md`, "Test vectors").

use std::path::PathBuf;

use conformance::{rng, Trace};

// Covers: specs/sim/rng.md §2, §3 text, §3 r2, §3 r4
#[test]
fn recorded_rng_traces_replay_exactly() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../traces/sim/rng");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    assert!(
        paths.len() >= 5,
        "expected the five recorded traces, found {}",
        paths.len()
    );

    let mut draws = 0;
    for path in &paths {
        let trace = Trace::load(path).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            (trace.area.as_str(), trace.behavior.as_str()),
            ("sim", "rng")
        );
        draws += rng::replay(&trace).unwrap_or_else(|e| panic!("{e}"));
    }
    // sim-0001..0005: 64 + 57 + 7 + 64 + 64 draws.
    assert!(draws >= 256, "only {draws} draws replayed");
}

#[test]
fn a_wrong_draw_is_reported() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../traces/sim/rng/sim-0004.json");
    let mut trace = Trace::load(&path).unwrap();
    trace.expected[3]["data"]["value"] = 1.into();
    let err = rng::replay(&trace).unwrap_err().to_string();
    assert!(err.contains("expected[3]"), "{err}");
}
