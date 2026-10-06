//! Stat lists (`specs/sim/stat-lists.md`, `specs/sim/stats.md`) through
//! `conformance::stats::replay_stats`: the synthetic recording
//! `fixtures/stats-lists.jsonl` (the hand-built recording of
//! `check_stats.py --selftest`, values from the specs' test vectors;
//! `check_stats.py` passes it with 0 errors), perturbations of it
//! reported at exactly the changed record (METHODS M08), and the
//! `stats-raw-1` recordings in `traces/raw/` (ignored: local, needs
//! `record_stats.py`, `docs/HANDOFF.md` §5 A2).
//!
//! No `Covers:` claims: a claim here counts as verified against 1.14d
//! (`docs/COVERAGE.md` §2), which only a passing recording earns.

use std::path::PathBuf;

use conformance::raw::{raw_files, Mismatch, RawRecording, STATS_RAW};
use conformance::stats::{replay_stats, StatsStats};

fn fixture() -> RawRecording {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stats-lists.jsonl");
    RawRecording::load(&path, STATS_RAW).unwrap_or_else(|e| panic!("{e}"))
}

fn mismatch_of(rec: &RawRecording) -> Mismatch {
    match replay_stats(rec) {
        Err(e) => e
            .mismatch()
            .cloned()
            .unwrap_or_else(|| panic!("not a mismatch: {e}")),
        Ok(s) => panic!("perturbed recording replayed: {s:?}"),
    }
}

fn find(rec: &RawRecording, kind: &str, n: usize) -> usize {
    rec.records
        .iter()
        .enumerate()
        .filter(|(_, r)| r["k"] == kind)
        .nth(n)
        .unwrap_or_else(|| panic!("no {kind} #{n}"))
        .0
}

#[test]
fn synthetic_recording_replays_exactly() {
    let stats = replay_stats(&fixture()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        stats,
        StatsStats {
            ticks: 2,
            // 11 ss at top level (2 more inside callbacks are d2-sim's
            // own), 2 sat, 1 sdt, 1 stg, and the expiry's state toggle
            // (host work).
            operations: 11 + 2 + 1 + 1 + 1,
            host_operations: 1,
            callbacks: 5,
            expiries: 1,
            expiry_frees: 1,
            seeded_lists: 1,
            snapshots: 3,
            lists_compared: 5,
            regen_skipped: 1,
        }
    );
}

#[test]
fn a_changed_snapshot_value_is_reported_at_the_snapshot() {
    // As `check_stats.py --perturb-snap N`: the first value of the first
    // list of snapshot N with a value.
    for n in 0..3 {
        let mut rec = fixture();
        let i = find(&rec, "ssn", n);
        let d = &mut rec.records[i]["lists"][0];
        let key = if d["F"].as_array().is_some_and(|a| !a.is_empty()) {
            "F"
        } else {
            "b"
        };
        let v = d[key][0][2].as_i64().unwrap();
        d[key][0][2] = (v + 1).into();
        let m = mismatch_of(&rec);
        assert_eq!(m.at, i as u64, "snapshot {n}: {m}");
        let want = if key == "F" { "full[0]" } else { "base[0]" };
        assert!(m.field.ends_with(want), "snapshot {n}: {m}");
    }
}

#[test]
fn a_changed_callback_is_reported_at_the_callback() {
    // As `check_stats.py --perturb-cb N`: the new value + 1.
    for n in 0..5 {
        let mut rec = fixture();
        let i = find(&rec, "scb", n);
        let v = rec.records[i]["new"].as_i64().unwrap();
        rec.records[i]["new"] = (v + 1).into();
        let m = mismatch_of(&rec);
        assert_eq!(
            (m.at, m.field.as_str()),
            (i as u64, "callback.new"),
            "callback {n}: {m}"
        );
    }
}

#[test]
fn a_missing_or_extra_callback_is_reported() {
    // Remove the second callback of the detach (stat 11, 2560 → 0).
    let base = fixture();
    let i = find(&base, "scb", 4);
    let mut rec = base.clone();
    rec.records.drain(i..=i + 1);
    let m = mismatch_of(&rec);
    assert_eq!((m.at, m.field.as_str()), (i as u64, "callback"), "{m}");
    // A callback d2-sim does not make: after the strength write of the
    // first operation (stat 0 has no fCallback).
    let mut rec = base.clone();
    let j = find(&base, "ss", 0);
    let extra = rec.records[find(&base, "scb", 0)].clone();
    rec.records.insert(j + 1, extra);
    rec.records.insert(j + 2, serde_json::json!({"k": "scx"}));
    let m = mismatch_of(&rec);
    assert_eq!((m.at, m.field.as_str()), (j as u64 + 1, "callback"), "{m}");
}

#[test]
fn a_changed_flag_bit_and_expiry_are_reported() {
    // The attach of the item list with reset 0 (an input): d2-sim keeps
    // the damage-related tohit (19) out of the player's full array, so
    // the first snapshot after it differs there.
    let mut rec = fixture();
    let i = find(&rec, "sat", 0);
    rec.records[i]["r"] = 0.into();
    let m = mismatch_of(&rec);
    let snap = find(&rec, "ssn", 0);
    assert_eq!(
        (m.at, m.field.as_str()),
        (snap as u64, "snapshot 0xP full[6]"),
        "{m}"
    );
    // Expiry: a list the recording frees that d2-sim keeps (expire frame
    // after the expiry's frame).
    let mut rec = fixture();
    let i = find(&rec, "sxp", 0);
    rec.records[i]["lists"][0][2] = 21.into();
    let m = mismatch_of(&rec);
    assert_eq!((m.at, m.field.as_str()), (i as u64 + 1, "expiry"), "{m}");
}

#[test]
fn a_changed_table_derivation_is_reported() {
    // The recorded dependants of level (12: item_hp_perlevel 216) and
    // op table of maxhp (7) each lose an entry: the d2-data fix-up
    // derives them, and the two must agree.
    for (stat, column, field) in [(12, 12, "deps"), (7, 11, "optable")] {
        let mut rec = fixture();
        let i = find(&rec, "stab", 0);
        let list = rec.records[i]["isc"][stat][column].as_array_mut().unwrap();
        assert!(!list.is_empty());
        list.pop();
        let m = mismatch_of(&rec);
        assert_eq!(
            (m.at, m.field.as_str()),
            (i as u64, format!("isc[{stat}].{field}").as_str()),
            "{m}"
        );
    }
}

/// Every `stats-raw-1` recording in `traces/raw/`.
#[test]
#[ignore = "needs traces/raw/*-stats.jsonl (local, HANDOFF §5 A2)"]
fn recorded_stat_lists_replay_exactly() {
    let files = raw_files("-stats.jsonl");
    assert!(!files.is_empty(), "no traces/raw/*-stats.jsonl");
    for path in &files {
        let rec = RawRecording::load(path, STATS_RAW).unwrap_or_else(|e| panic!("{e}"));
        let s = replay_stats(&rec).unwrap_or_else(|e| panic!("{e}"));
        println!("{}: {s:?}", path.display());
        assert!(
            s.operations > 0 && s.snapshots > 0,
            "{}: nothing replayed",
            path.display()
        );
    }
}

/// The first snapshot's tree (a player list with an item list attached)
/// as a seed (`ssd`), then the same snapshot: d2-sim rebuilds it from
/// base values and the chain alone (full arrays §6, mod array §11).
fn seeded() -> RawRecording {
    let base = fixture();
    let snap = base.records[find(&base, "ssn", 0)].clone();
    let units = serde_json::json!({
        "0:1": {"t": 0, "c": 4, "act": "A1"},
        "4:7": {"t": 4, "c": 100, "act": null},
    });
    let records = vec![
        base.records[0].clone(),
        base.records[find(&base, "stab", 0)].clone(),
        serde_json::json!({"k": "tick", "f": 1}),
        serde_json::json!({"k": "ssd", "lists": snap["lists"], "units": units}),
        snap,
    ];
    RawRecording::from_records("seeded", records, STATS_RAW).unwrap()
}

#[test]
fn a_seeded_tree_rebuilds_its_dump() {
    let s = replay_stats(&seeded()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((s.seeded_lists, s.lists_compared), (2, 2), "{s:?}");
}

#[test]
fn a_seed_d2_sim_cannot_rebuild_is_reported_at_the_seed() {
    // The dump's full maxstamina (11) one higher than vitality gives.
    let mut rec = seeded();
    let full = rec.records[3]["lists"][0]["F"].as_array_mut().unwrap();
    assert_eq!(full[4][0], 11);
    full[4][2] = 2561.into();
    let m = mismatch_of(&rec);
    assert_eq!((m.at, m.field.as_str()), (3, "seed 0xP full[4]"), "{m}");
    // A mod key d2-sim does not keep (hitpoints 6, §11.1).
    let mut rec = seeded();
    let mods = rec.records[3]["lists"][0]["m"].as_array_mut().unwrap();
    mods.push((6 << 16).into());
    let m = mismatch_of(&rec);
    assert_eq!((m.at, m.field.as_str()), (3, "seed 0xP mod"), "{m}");
}
