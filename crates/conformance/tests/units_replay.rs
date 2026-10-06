//! Mode schedules (`specs/sim/units.md` §4.2) through
//! `conformance::units::replay_anim`: the synthetic recording
//! `fixtures/units-anim.jsonl` (the hand-built recording of
//! `check_units.py --selftest` plus one schedule of each form; its sets
//! were computed by `check_units.py`'s own model, and `check_units.py`
//! passes it with 0 errors), every single-field perturbation of it
//! (METHODS M08), and the `tick-raw-1` recordings in `traces/raw/`
//! (ignored: local, needs `record_tick.py` 0.2.0 recordings).
//!
//! No `Covers:` claims: a claim here counts as verified against 1.14d
//! (`docs/COVERAGE.md` §2), which only a passing recording earns.

use std::path::PathBuf;

use conformance::raw::{raw_files, RawRecording, TICK_RAW};
use conformance::units::{replay_anim, UnitStats};
use serde_json::Value;

fn fixture() -> RawRecording {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/units-anim.jsonl");
    RawRecording::load(&path, TICK_RAW).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn synthetic_schedules_replay_exactly() {
    let stats = replay_anim(&fixture()).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        stats,
        UnitStats {
            anim_records: 6,
            // main (player), percent, start frame (byte −1), main
            // (monster), frames with p = 0 (no events).
            schedules: 5,
            events: 2 + 4 + 4 + 4,
            cancels: 1,
            sequences_skipped: 1,
            outside_record_skipped: 0,
        }
    );
}

/// The sets of computed schedules: (record index, set).
fn schedule_sets(rec: &RawRecording) -> Vec<usize> {
    let mut out = Vec::new();
    let mut in_group = false;
    for (i, r) in rec.records.iter().enumerate() {
        match r["k"].as_str() {
            Some("anim") => in_group = r["seq"] == false,
            Some("cancel") => {}
            Some("set") if in_group => {
                out.push(i);
                if r["ty"] == 1 {
                    in_group = false;
                }
            }
            _ => in_group = false,
        }
    }
    out
}

fn mismatch_of(rec: &RawRecording) -> conformance::raw::Mismatch {
    match replay_anim(rec) {
        Err(e) => e
            .mismatch()
            .cloned()
            .unwrap_or_else(|| panic!("not a mismatch: {e}")),
        Ok(s) => panic!("perturbed recording replayed: {s:?}"),
    }
}

#[test]
fn every_changed_field_is_reported_at_its_record() {
    let base = fixture();
    let sets = schedule_sets(&base);
    assert_eq!(sets.len(), 14);
    for &i in &sets {
        for (field, delta) in [("req", 1), ("a1", 1), ("a2", 1), ("ty", 1)] {
            let mut rec = base.clone();
            let r = &mut rec.records[i];
            let v = r[field].as_i64().unwrap();
            r[field] = Value::from(v + delta);
            let m = mismatch_of(&rec);
            let want = match field {
                "req" => "expire",
                // A type change ends the group early (1) or late (0, 2).
                "ty" => m.field.as_str(),
                _ => field,
            };
            assert_eq!(
                (m.at, m.field.as_str()),
                (i as u64, want),
                "{field} at {i}: {m}"
            );
        }
    }
}

#[test]
fn a_missing_or_extra_set_is_reported() {
    let base = fixture();
    // Drop the first action event of the percent schedule (record 44).
    let mut rec = base.clone();
    assert_eq!(rec.records[44]["tm"], "MP0");
    rec.records.remove(44);
    let m = mismatch_of(&rec);
    assert_eq!((m.at, m.field.as_str()), (44, "expire"), "{m}");
    // Drop the ENDANIM of the main-form monster schedule (record 57).
    let mut rec = base.clone();
    assert_eq!(rec.records[57]["tm"], "KM3");
    rec.records.remove(57);
    let m = mismatch_of(&rec);
    assert_eq!((m.at, m.field.as_str()), (57, "group"), "{m}");
    // An extra set after the p = 0 frames variant (record 58).
    let mut rec = base.clone();
    let mut extra = rec.records[57].clone();
    extra["tm"] = "X".into();
    rec.records.insert(59, extra);
    let m = mismatch_of(&rec);
    assert_eq!((m.at, m.field.as_str()), (59, "group"), "{m}");
}

#[test]
fn cancels_follow_the_form() {
    let base = fixture();
    // The percent variant cancels the live ENDANIM M5: without the
    // recorded cancel, d2-sim's cancel is unmatched (at the anim record).
    let mut rec = base.clone();
    assert_eq!(rec.records[43]["k"], "cancel");
    rec.records.remove(43);
    let m = mismatch_of(&rec);
    assert_eq!((m.at, m.field.as_str()), (42, "cancel"), "{m}");
    // The main form cancels nothing: a recorded cancel of a live mode
    // timer before its sets is reported.
    let mut rec = base.clone();
    assert_eq!(rec.records[53]["fn"], "0x5539b0");
    // A live ENDANIM of monster K before its main-form schedule...
    let mut live = rec.records[19].clone();
    live["g"] = 3.into();
    live["tm"] = "K0".into();
    rec.records.insert(53, live);
    // ...cancelled between the anim record (now 54) and its first set.
    rec.records.insert(
        55,
        serde_json::json!({"k": "cancel", "tm": "K0", "deferred": false}),
    );
    let m = mismatch_of(&rec);
    assert_eq!((m.at, m.field.as_str()), (55, "cancel"), "{m}");
}

#[test]
fn another_format_is_rejected() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stats-lists.jsonl");
    let e = RawRecording::load(&path, TICK_RAW).unwrap_err();
    assert!(e.to_string().contains("stats-raw-1"), "{e}");
}

/// Every `tick-raw-1` recording in `traces/raw/` (`docs/HANDOFF.md` §5
/// A1: `record_tick.py` 0.2.0, with `anim` records).
#[test]
#[ignore = "needs traces/raw/*-tick.jsonl (local)"]
fn recorded_mode_schedules_replay_exactly() {
    let files = raw_files("-tick.jsonl");
    assert!(!files.is_empty(), "no traces/raw/*-tick.jsonl");
    let mut total = UnitStats::default();
    for path in &files {
        let rec = RawRecording::load(path, TICK_RAW).unwrap_or_else(|e| panic!("{e}"));
        let s = replay_anim(&rec).unwrap_or_else(|e| panic!("{e}"));
        println!("{}: {s:?}", path.display());
        total.anim_records += s.anim_records;
        total.schedules += s.schedules;
        total.events += s.events;
    }
    assert!(
        total.schedules > 0,
        "no anim records: record with record_tick.py 0.2.0 (HANDOFF §5 A1)"
    );
}
