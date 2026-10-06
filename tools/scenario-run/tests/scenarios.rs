// Spec: specs/tools/scenario.md §4, §5 (test vectors: the starter scenarios on the synthetic install)
//! Every script in `traces/scenarios/` runs on d2rs over the synthetic
//! install (CI, no game files): it parses and round-trips, two runs
//! give byte-identical traces (determinism), the trace reads back, and
//! the comparator finds every perturbation of a real run's records at
//! exactly that record (METHODS M08). The same on the user's install is
//! `#[ignore]` (`D2_GAME_DIR`).

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use conformance::scenario::trace::Record;
use conformance::scenario::{compare, Scenario, TraceFile, Verdict};
use scenario_run::{run, Data, RunError};

fn data() -> &'static Data {
    static D: OnceLock<Data> = OnceLock::new();
    D.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("scenario-run-{}", std::process::id()));
        Data::synthetic(&dir).unwrap_or_else(|e| panic!("{e}"))
    })
}

fn scenarios_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../traces/scenarios")
}

fn starters() -> Vec<(String, Scenario)> {
    let mut out: Vec<(String, Scenario)> = std::fs::read_dir(scenarios_dir())
        .expect("traces/scenarios")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "scenario"))
        .map(|p| {
            let text = std::fs::read_to_string(&p).expect("readable");
            let s = Scenario::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            let stem = p.file_stem().unwrap().to_string_lossy().into_owned();
            (stem, s)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn trace_of(s: &Scenario, d: &Data) -> TraceFile {
    run(s, d)
        .unwrap_or_else(|e| panic!("{}: {e}", s.name))
        .trace
}

// Covers: specs/tools/scenario.md §1 r1, §2 r6
#[test]
fn starters_parse_and_round_trip() {
    let all = starters();
    let names: Vec<&str> = all.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        [
            "cast-firebolt",
            "champion-pack",
            "kill-monster",
            "pickup-drop",
            "run-cold-plains",
            "vendor-buy-sell",
            "walk-town",
            "waypoint-travel"
        ]
    );
    for (stem, s) in &all {
        assert_eq!(&s.name, stem, "name line = file name");
        let text = s.to_text();
        let again = Scenario::parse(&text).unwrap();
        assert_eq!(&again, s, "{stem}");
        assert_eq!(again.to_text(), text, "{stem}");
    }
}

// Covers: specs/tools/scenario.md §4 r2, §4 r4, §4 r5, §4 r6, §4 r7, §4 r8, §4 r10
#[test]
fn every_starter_runs_twice_identically() {
    for (stem, s) in starters() {
        let a = trace_of(&s, data());
        let b = trace_of(&s, data());
        let text = a.to_text();
        assert_eq!(text, b.to_text(), "{stem}: two runs differ");
        assert_eq!(TraceFile::parse(&text).unwrap(), a, "{stem}: reads back");
        let h = &a.header;
        assert_eq!(
            (h.side.as_str(), h.data.as_str(), h.scenario_sha256.as_str()),
            ("d2rs", "synthetic", s.sha256().as_str())
        );
        // One c2s record per step; snapshots at the snapshot ticks.
        let c2s = a
            .records
            .iter()
            .filter(|r| matches!(r, Record::C2s { .. }))
            .count();
        assert_eq!(c2s, s.steps.len(), "{stem}");
        let snap: std::collections::BTreeSet<u32> = a
            .records
            .iter()
            .filter(|r| matches!(r, Record::Unit { .. }))
            .map(Record::tick)
            .collect();
        if s.records(conformance::scenario::script::Stream::Units) {
            assert_eq!(snap, s.snapshot_ticks(), "{stem}");
        }
        // Against itself: no divergence (partial while d2rs has gaps).
        let r = compare(&a, &b).unwrap();
        assert!(r.first.is_none(), "{stem}: {r}");
        assert_ne!(r.verdict, Verdict::Diverged);
    }
}

/// A run that sends S→C messages: waypoint travel within the synthetic
/// town (level 1, the one waypoint level of the synthetic set).
fn travel() -> Scenario {
    let text = std::fs::read_to_string(scenarios_dir().join("waypoint-travel.scenario")).unwrap();
    Scenario::parse(
        &text
            .replace("level=3", "level=1")
            .replace("name waypoint-travel", "name travel"),
    )
    .unwrap()
}

// Covers: specs/tools/scenario.md §4 r5
#[test]
fn server_messages_are_captured_at_their_tick() {
    let t = trace_of(&travel(), data());
    let ids: Vec<(u32, u8)> = t
        .records
        .iter()
        .filter_map(|r| match r {
            Record::S2c { t, bytes, client } => {
                assert_eq!(*client, 0);
                Some((*t, bytes[0]))
            }
            _ => None,
        })
        .collect();
    // The travel's reply (`waypoints.md` §6, the server tests' arrival
    // message 0x0D among them) at the tick of the 0x49 step.
    assert!(ids.contains(&(50, 0x0D)), "{ids:x?}");
}

/// Changes one record so that it differs in a compared, unmasked place.
fn perturb(r: &mut Record) -> bool {
    match r {
        Record::C2s { bytes: Ok(b), .. } | Record::S2c { bytes: b, .. } => {
            let k = b.len() - 1;
            b[k] ^= 0x01;
        }
        Record::C2s { bytes: Err(u), .. } => u.push('x'),
        Record::Rng { after, .. } => after[1] ^= 1,
        Record::Draw { before, .. } => before[0] ^= 1,
        Record::Unit { life, .. } => *life += 1,
        Record::Stats { base, .. } => match base.last_mut() {
            Some(e) => e.2 += 1,
            None => return false,
        },
        Record::End { .. } => return false,
    }
    true
}

// Covers: specs/tools/scenario.md §5 r3, §5 r4, §5 r5
#[test]
fn comparator_finds_every_perturbed_record_of_a_real_run() {
    for s in [travel(), starters().remove(6).1] {
        let original = trace_of(&s, data());
        let mut checked = 0;
        for k in 0..original.records.len() {
            let mut ours = original.clone();
            if !perturb(&mut ours.records[k]) {
                continue;
            }
            let rec = &original.records[k];
            let index = original.records[..k]
                .iter()
                .filter(|r| r.tick() == rec.tick() && r.kind() == rec.kind())
                .count();
            let r = compare(&original, &ours).unwrap();
            let d = r
                .first
                .as_ref()
                .unwrap_or_else(|| panic!("{}: record {k} not found", s.name));
            assert_eq!(
                (d.tick, d.stream.as_str(), d.index),
                (rec.tick(), rec.kind(), index),
                "{}: record {k}: {r}",
                s.name
            );
            checked += 1;
        }
        assert!(checked > 100, "{}: {checked}", s.name);
    }
}

/// `champion-pack` with the synthetic monster class 1 (ghoul1) for the
/// fallen of the script.
fn champion_pack() -> Scenario {
    let text = std::fs::read_to_string(scenarios_dir().join("champion-pack.scenario")).unwrap();
    let text = text
        .replace("spawn 19", "spawn 1")
        .replace("@1:19", "@1:1")
        .replace("name champion-pack", "name champion-pack-synthetic");
    Scenario::parse(&text).unwrap()
}

// Covers: specs/tools/scenario.md §3.1 r2, §3.1 r3
#[test]
fn a_spawned_champion_pack_is_recorded() {
    let t = trace_of(&champion_pack(), data());
    let spawned: Vec<&Record> = t.records.iter().filter(|r| matches!(r, Record::Spawn { .. })).collect();
    let [Record::Spawn { t: 5, i: 0, guid: Ok(Some(leader)) }] = spawned.as_slice() else {
        panic!("{spawned:?}")
    };
    // The leader and its 1–3 minions are monsters of class 1 at the next
    // snapshot.
    let pack: Vec<u32> = t
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Unit { t: 10, ty: 1, class: 1, guid, .. } => Some(*guid),
            _ => None,
        })
        .collect();
    assert!(pack.contains(leader), "{pack:?}");
    assert!((2..=4).contains(&pack.len()), "{pack:?}");
    // The spawn drew from the game seed before the drain of tick 5.
    let rng5 = t.records.iter().find_map(|r| match r {
        Record::Rng { t: 5, before, after } => Some((*before, *after)),
        _ => None,
    });
    let (before, after) = rng5.expect("rng at tick 5");
    assert_ne!(before, after);
    assert_eq!(t.to_text(), trace_of(&champion_pack(), data()).to_text());
}

// Covers: specs/tools/scenario.md §4 r10
#[test]
fn what_the_runner_cannot_run_is_an_error() {
    let base =
        "scenario 1\nname x\ngame 1.14d\nseed 1\nmap 2\ndifficulty normal\nexpansion yes\nend 1\n";
    let s = Scenario::parse(&format!("{base}char save some.d2s\n")).unwrap();
    assert!(matches!(run(&s, data()), Err(RunError::Unsupported(m)) if m.contains("d2s")));
    let s = Scenario::parse(&format!("{base}char class 1\nchar area 0 3\n")).unwrap();
    assert!(matches!(run(&s, data()), Err(RunError::Unsupported(_))));
    let s = Scenario::parse(&format!(
        "{base}char class 1\nchar area 0 1\nchar at 100000 5\n"
    ))
    .unwrap();
    assert!(matches!(run(&s, data()), Err(RunError::Unsupported(m)) if m.contains("in no room")));
}

/// The starters on the user's install: they build and run
/// deterministically (`D2_GAME_DIR`).
#[test]
#[ignore = "needs the game files (D2_GAME_DIR)"]
fn starters_run_on_the_live_install() {
    let dir = std::env::var_os("D2_GAME_DIR").expect("D2_GAME_DIR");
    let live = Data::live(Path::new(&dir)).unwrap_or_else(|e| panic!("{e}"));
    for (stem, s) in starters() {
        let a = trace_of(&s, &live);
        assert_eq!(a.to_text(), trace_of(&s, &live).to_text(), "{stem}");
        println!(
            "{stem}: {} records, gaps {:?}",
            a.records.len(),
            a.header.gaps
        );
    }
}
