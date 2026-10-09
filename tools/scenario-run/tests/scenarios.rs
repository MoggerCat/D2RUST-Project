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
            "poke-firebolt",
            "poke-spawn-town",
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
            .filter(|r| {
                matches!(
                    r,
                    Record::C2s { .. } | Record::Spawn { .. } | Record::Poke { .. }
                )
            })
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

/// A run that sends S→C messages: waypoint travel from the synthetic
/// town (level 1) to the keep (level 4, the second act 0 waypoint level
/// of the synthetic set).
fn travel() -> Scenario {
    let text = std::fs::read_to_string(scenarios_dir().join("waypoint-travel.scenario")).unwrap();
    Scenario::parse(
        &text
            .replace("level=3", "level=4")
            .replace("char waypoint 3", "char waypoint 4")
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
        Record::C2s { bytes: Err(u), .. } | Record::Spawn { guid: Err(u), .. } => u.push('x'),
        Record::Spawn { guid: Ok(g), .. } => *g = g.map_or(Some(0), |g| Some(g ^ 1)),
        Record::Poke { guid, r, .. } => match guid {
            Some(g) => *g ^= 1,
            None => *r = if r == "ok" { "failed" } else { "ok" }.into(),
        },
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
    let walk = starters()
        .into_iter()
        .find(|(n, _)| n == "walk-town")
        .expect("walk-town")
        .1;
    for s in [travel(), walk, synthetic("poke-spawn-town")] {
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

/// A starter with the synthetic set's rows for the 1.14d ones it names:
/// monster class 1 (ghoul1) for the fallen (19), skill 2 (Firebolt) for
/// Fire Bolt (36), objects row 1 (Chest) for the brazier (39).
fn synthetic(name: &str) -> Scenario {
    let text = std::fs::read_to_string(scenarios_dir().join(format!("{name}.scenario"))).unwrap();
    let text = text
        .replace("spawn 19", "spawn 1")
        .replace("@1:19", "@1:1")
        .replace("skill 36", "skill 2")
        .replace("skill=36", "skill=2")
        .replace("object 39", "object 1")
        .replace(&format!("name {name}"), &format!("name {name}-synthetic"));
    Scenario::parse(&text).unwrap()
}

fn pokes(t: &conformance::scenario::TraceFile) -> Vec<(u32, u32, String, String, Option<u32>)> {
    t.records
        .iter()
        .filter_map(|r| match r {
            Record::Poke { t, i, d, r, guid } => Some((*t, *i, d.clone(), r.clone(), *guid)),
            _ => None,
        })
        .collect()
}

// Covers: specs/tools/poke.md §3 r1, §3 r3, §5 r1, §5 r2
#[test]
fn poke_spawn_town_spawns_pokes_and_records_the_monster_every_tick() {
    let s = synthetic("poke-spawn-town");
    let t = trace_of(&s, data());
    let leader = t
        .records
        .iter()
        .find_map(|r| match r {
            Record::Spawn {
                t: 1,
                i: 0,
                guid: Ok(Some(g)),
            } => Some(*g),
            _ => None,
        })
        .expect("the fallen is spawned at tick 1");
    let p = pokes(&t);
    assert_eq!(p.len(), 3, "{p:?}");
    assert_eq!(p[0], (2, 0, "seed-unit".into(), "ok".into(), None));
    assert_eq!(
        (p[1].0, p[1].2.as_str(), p[1].3.as_str()),
        (3, "object", "ok")
    );
    let object = p[1].4.expect("the object's GUID");
    assert_eq!(p[2], (4, 0, "time".into(), "ok".into(), None));
    // The monster's unit record at every tick from 1 to the end; the
    // object's from tick 3.
    let ticks = |ty: u32, guid: u32| -> Vec<u32> {
        t.records
            .iter()
            .filter_map(|r| match r {
                Record::Unit {
                    t, ty: y, guid: g, ..
                } if *y == ty && *g == guid => Some(*t),
                _ => None,
            })
            .collect()
    };
    assert_eq!(ticks(1, leader), (1..=s.end).collect::<Vec<_>>());
    assert_eq!(ticks(2, object), (3..=s.end).collect::<Vec<_>>());
    assert!(
        t.header.gaps.iter().all(|g| !g.starts_with("poke")),
        "{:?}",
        t.header.gaps
    );
    assert_eq!(t.to_text(), trace_of(&s, data()).to_text());
}

// Covers: specs/tools/poke.md §1 r2, §3 r4
#[test]
fn every_directive_runs_on_d2rs_or_is_a_listed_gap() {
    let base = "scenario 1\nname p\ngame 1.14d\nseed 0x1234\ninit 644409375\ndifficulty normal\nexpansion yes\nend 6\nchar class 1\nchar area 0 1\nrecord units stats\nsnapshot every 1\n";
    let steps = concat!(
        "at 1 spawn 1 @x+4 @y normal\n",
        "at 2 poke object 1 @x-6 @y mode 0\n",
        "at 2 poke missile 1 @x @y @x+10 @y skill 2 1\n",
        "at 2 poke missile 1 @x @y @x+10 @y owner @1:1\n",
        "at 2 poke seed-game 1 2\n",
        "at 3 poke seed-unit @player 3 4\n",
        "at 3 poke time 5 100\n",
        "at 3 poke pos @1:1 @x+6 @y+4\n",
        "at 3 poke item pt1 @x+2 @y ilvl 3\n",
        "at 4 poke stat @player 14 0 777\n",
        "at 4 poke state @player 1 on\n",
        "at 4 poke freeze 2\n",
        "at 4 poke superunique 0 @x+3 @y\n",
        "at 5 poke warp 4\n",
        "at 5 poke seed-unit @1:99 1 1\n",
        "at 5 poke item zzz @x @y\n",
        "at 6 poke warp 40\n",
    );
    let s = Scenario::parse(&format!("{base}{steps}")).unwrap();
    let t = trace_of(&s, data());
    let p: Vec<(String, String)> = pokes(&t)
        .into_iter()
        .map(|(_, _, d, r, _)| (d, r))
        .collect();
    let want: Vec<(&str, &str)> = vec![
        ("object", "ok"),
        ("missile", "ok"),
        ("missile", "ok"),
        ("seed-game", "ok"),
        ("seed-unit", "ok"),
        ("time", "ok"),
        ("pos", "ok"),
        ("item", "ok"),
        ("stat", "ok"),
        ("state", "ok"),
        ("freeze", "ok"),
        ("superunique", "ok"),
        ("warp", "ok"),
        ("seed-unit", "unresolved"),
        ("item", "failed"),
        // Another act: the act change runs (§1 `warp` row); the base
        // synthetic set has no Act II levels, so its spawn search finds
        // no room (`waypoints.md` §11 steps 8–9).
        ("warp", "failed"),
    ];
    let got: Vec<(&str, &str)> = p.iter().map(|(d, r)| (d.as_str(), r.as_str())).collect();
    assert_eq!(got, want);
    // No poke is a gap now (§3 rule 4: gaps are in the header).
    assert!(
        t.header.gaps.iter().all(|g| !g.starts_with("poke")),
        "{:?}",
        t.header.gaps
    );
    // The seed write and the stat write show in the records.
    let stat = t.records.iter().find_map(|r| match r {
        Record::Stats {
            t: 4, ty: 0, base, ..
        } => base.iter().find(|e| e.0 == 14).copied(),
        _ => None,
    });
    assert_eq!(stat, Some((14, 0, 777)));
    let moved = t.records.iter().find_map(|r| match r {
        Record::Unit { t: 3, ty: 1, x, .. } => Some(*x),
        _ => None,
    });
    let me = t.records.iter().find_map(|r| match r {
        Record::Unit { t: 3, ty: 0, x, .. } => Some(*x),
        _ => None,
    });
    assert_eq!(moved.zip(me).map(|(m, p)| m - p), Some(6));
    assert_eq!(t.to_text(), trace_of(&s, data()).to_text());
}

/// The synthetic install with all five acts (`test_fixtures::acts`).
fn all_acts_data() -> &'static Data {
    static D: OnceLock<Data> = OnceLock::new();
    D.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("scenario-run-acts-{}", std::process::id()));
        let i = test_fixtures::install::build(&dir, &test_fixtures::acts::all_acts())
            .unwrap_or_else(|e| panic!("{e}"));
        Data {
            game: test_fixtures::game::GameData::from_install(&i).unwrap_or_else(|e| panic!("{e}")),
            kind: scenario_run::DataKind::Synthetic,
            save_dir: None,
        }
    })
}

// Covers: specs/tools/poke.md §1; specs/world/waypoints.md §11
#[test]
fn poke_warp_to_act_two_runs_the_act_change() {
    let s = Scenario::parse(concat!(
        "scenario 1\nname warp-act2\ngame 1.14d\nseed 0x1234\ninit 644409375\n",
        "difficulty normal\nexpansion yes\nend 6\nchar class 1\nchar area 0 1\n",
        "record s2c units\nsnapshot every 1\n",
        "at 3 poke warp 40\n",
    ))
    .unwrap();
    let t = trace_of(&s, all_acts_data());
    let p: Vec<(u32, String, String)> = pokes(&t)
        .into_iter()
        .map(|(t, _, d, r, _)| (t, d, r))
        .collect();
    assert_eq!(p, vec![(3, "warp".to_owned(), "ok".to_owned())]);
    // §11 steps 13 and 16: 0x05, then 0x03 LoadAct with act 1 (Act II)
    // and its town level 40, then 0x53; 0x04 after them, from the tick's
    // client pass (`flows/act-change.md` §1 r4; the poke runs before
    // tick 3, so that pass is tick 3's).
    let s2c: Vec<(u32, Vec<u8>)> = t
        .records
        .iter()
        .filter_map(|r| match r {
            Record::S2c { t, bytes, .. } => Some((*t, bytes.clone())),
            _ => None,
        })
        .collect();
    let at = |id: u8| s2c.iter().position(|(_, b)| b[0] == id);
    let (u, l, e) = (at(0x05), at(0x03), at(0x53));
    assert!(u.is_some() && u < l && l < e, "{s2c:x?}");
    let load = &s2c[l.unwrap()];
    assert_eq!(load.0, 3);
    assert_eq!(load.1[1], 1, "LoadAct act");
    assert_eq!(u16::from_le_bytes([load.1[6], load.1[7]]), 40, "town level");
    assert!(at(0x04) > e, "{s2c:x?}");
    // Step 17: the new act's rooms are added (0x07 of level 40).
    assert!(
        s2c[e.unwrap()..]
            .iter()
            .any(|(_, b)| b[0] == 0x07 && b[5] == 40),
        "{s2c:x?}"
    );
    assert_eq!(t.to_text(), trace_of(&s, all_acts_data()).to_text());
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
    let spawned: Vec<&Record> = t
        .records
        .iter()
        .filter(|r| matches!(r, Record::Spawn { .. }))
        .collect();
    let [Record::Spawn {
        t: 5,
        i: 0,
        guid: Ok(Some(leader)),
    }] = spawned.as_slice()
    else {
        panic!("{spawned:?}")
    };
    // The leader and its 1–3 minions are monsters of class 1 at the next
    // snapshot.
    let pack: Vec<u32> = t
        .records
        .iter()
        .filter_map(|r| match r {
            Record::Unit {
                t: 10,
                ty: 1,
                class: 1,
                guid,
                ..
            } => Some(*guid),
            _ => None,
        })
        .collect();
    assert!(pack.contains(leader), "{pack:?}");
    assert!((2..=4).contains(&pack.len()), "{pack:?}");
    // The spawn drew from the game seed before the drain of tick 5.
    let rng5 = t.records.iter().find_map(|r| match r {
        Record::Rng {
            t: 5,
            before,
            after,
        } => Some((*before, *after)),
        _ => None,
    });
    let (before, after) = rng5.expect("rng at tick 5");
    assert_ne!(before, after);
    assert_eq!(t.to_text(), trace_of(&champion_pack(), data()).to_text());
}

// Covers: specs/tools/poke.md §3 r1
#[test]
fn poke_firebolt_casts_at_the_spawned_monster_by_reference() {
    let s = synthetic("poke-firebolt");
    let t = trace_of(&s, data());
    // The cast steps resolve against the spawned monster.
    let casts: Vec<&Record> = t
        .records
        .iter()
        .filter(|r| matches!(r, Record::C2s { bytes: Ok(b), .. } if b[0] == 0x0D))
        .collect();
    assert_eq!(casts.len(), 2, "{casts:?}");
    assert_eq!(t.to_text(), trace_of(&s, data()).to_text());
}

// Covers: specs/tools/poke.md §2 r1, §2 r2, §2 r3
#[test]
fn committed_poke_files_parse_and_round_trip() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../traces/pokes");
    let mut n = 0;
    for e in std::fs::read_dir(&dir).expect("traces/pokes") {
        let path = e.expect("entry").path();
        if path.extension().is_none_or(|x| x != "poke") {
            continue;
        }
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        assert!(
            stem.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
            "{stem}"
        );
        let text = std::fs::read_to_string(&path).unwrap();
        let f = d2_sim::poke::PokeFile::parse(&text).unwrap_or_else(|e| panic!("{stem}: {e}"));
        assert_eq!(
            d2_sim::poke::PokeFile::parse(&f.to_text()).unwrap(),
            f,
            "{stem}"
        );
        n += 1;
    }
    assert!(n >= 1);
}

// Covers: specs/tools/scenario.md §4 r10
#[test]
fn what_the_runner_cannot_run_is_an_error() {
    let base =
        "scenario 1\nname x\ngame 1.14d\nseed 1\ninit 2\ndifficulty normal\nexpansion yes\nend 1\n";
    let s = Scenario::parse(&format!("{base}char save Some_char\n")).unwrap();
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
