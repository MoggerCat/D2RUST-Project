// Spec: specs/drlg/levels.md §3–§5; specs/monsters/population.md §2.1, §3; specs/monsters/init.md; specs/sim/tick.md §4 (the seed finder on synthetic data)
//! The seed finder on the synthetic install (`test_fixtures`), in CI, on
//! the live host (`test_fixtures::game::Seams`): room population
//! (`population.md` §3) reads the act DRLG's coordinate lists and
//! population queries (`levels.md` §11.6), so packs, champions and
//! uniques appear and depend on the seed.

use std::path::PathBuf;
use std::sync::OnceLock;

use seed_finder::query::{parse_args, Kind, Query};
use seed_finder::world::Prepared;
use seed_finder::{check_seed, describe, search, Outcome};
use test_fixtures::game::{ActCreation, GameData, Seams};
use test_fixtures::{install, synth};

/// The synthetic field: a one-room preset level beside the town.
const FIELD: u32 = 2;

fn prepared() -> &'static Prepared {
    static P: OnceLock<Prepared> = OnceLock::new();
    P.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("seed-finder-{}", std::process::id()));
        let i = install::build(&dir, &synth::synthetic()).unwrap_or_else(|e| panic!("{e}"));
        let d = GameData::from_install(&i).unwrap_or_else(|e| panic!("{e}"));
        let mut p = Prepared::new(&d, ActCreation::TownOnly).unwrap_or_else(|e| panic!("{e}"));
        // The synthetic monstats rows have no `isSpawn`: a region list
        // (`population.md` §2.3 step 4) would stay empty. The two
        // non-NPC rows are made spawnable.
        p.tweak_world(|t| {
            for m in t.pop.monstats.iter_mut().take(2) {
                m.is_spawn = true;
            }
            // Up to two random bosses in the field (§5 step 2).
            t.pop.levels[FIELD as usize].mon_umax = [2; 3];
        });
        p
    })
}

fn query(args: &str) -> Query {
    let a: Vec<String> = args.split_whitespace().map(str::to_owned).collect();
    parse_args(&a).unwrap_or_else(|e| panic!("{e}")).query
}

/// Uniques in the field over seeds 1–100 (each with its minions).
const UNIQUE_SEEDS: [u32; 5] = [16, 29, 45, 77, 88];
/// The one seed of 1–100 whose unique stands within 2 tiles of the
/// field's entrance from the town (sub-tile (20, 20)): at (11, 22).
const NEAR_SEED: u32 = 88;
const NEAR: &str = "--level 2 --from 1 --seeds 1-100 --monster kind=unique --near entrance:2";

fn seeds(o: &Outcome) -> Vec<u32> {
    o.hits.iter().map(|h| h.seed).collect()
}

#[test]
fn known_seed_matches_and_others_do_not() {
    let q = query(&format!("{NEAR} --threads 4"));
    let o = search(prepared(), &q, Seams::default).unwrap();
    assert_eq!(o.unsupported, None);
    assert_eq!(o.checked, 100);
    assert_eq!(seeds(&o), [NEAR_SEED]);
    let h = &o.hits[0];
    assert_eq!((h.init_seed, h.game_seed), (NEAR_SEED, NEAR_SEED));
    assert_eq!(h.view.anchors.entrance, Some((20, 20)));
    let (m, d) = &h.found[0].monsters[0];
    assert_eq!((m.kind, m.x, m.y, *d), (Kind::Unique, 11, 22, Some(1)));
    let text = describe(h, &q, &[]);
    assert!(
        text.starts_with("seed 88 (init_seed 88, game seed 88): level 2, 1 rooms"),
        "{text}"
    );
    assert!(text.contains("unique #1 at (11, 22), 1 tiles"), "{text}");

    // Without the distance every unique seed matches.
    let q = query("--level 2 --seeds 1-100 --monster kind=unique --threads 3");
    let o = search(prepared(), &q, Seams::default).unwrap();
    assert_eq!(seeds(&o), UNIQUE_SEEDS);
}

// M08: the check fails when an input changes: a radius of 1 tile drops
// the unique at 1.8 tiles; fixing either seed loses seed 88's unique
// (the map seed gives the room seeds of the pick, kind and placement,
// `rng.md` §5.4; the game seed the density draws, `population.md` §3.2).
#[test]
fn perturbed_queries_lose_the_match() {
    let p = prepared();
    let q = query("--level 2 --from 1 --seeds 1-100 --monster kind=unique --near entrance:1");
    assert_eq!(
        seeds(&search(p, &q, Seams::default).unwrap()),
        [] as [u32; 0]
    );
    for fixed in ["--init-seed 7", "--game-seed 7"] {
        let q = query(&format!("{NEAR} {fixed}"));
        let o = search(p, &q, Seams::default).unwrap();
        assert!(!seeds(&o).contains(&NEAR_SEED), "{fixed}");
        assert_eq!(o.checked, 100);
    }
    let q = query("--level 2 --seeds 1-100 --monster kind=unique,count=2");
    assert_eq!(
        seeds(&search(p, &q, Seams::default).unwrap()),
        [] as [u32; 0]
    );
}

#[test]
fn same_seed_same_result_twice() {
    let p = prepared();
    let q = query("--level 2 --from 1");
    for seed in 1..=40 {
        let a = check_seed(p, &q, seed, Seams::default()).unwrap();
        let b = check_seed(p, &q, seed, Seams::default()).unwrap();
        assert_eq!(a, b, "seed {seed}");
    }
    // Thread count does not change the outcome.
    let q1 = query("--level 2 --seeds 1-100 --monster kind=minion,count=4 --threads 1");
    let q8 = Query {
        threads: 8,
        ..q1.clone()
    };
    let a = search(p, &q1, Seams::default).unwrap();
    assert_eq!(a, search(p, &q8, Seams::default).unwrap());
    assert!(!a.hits.is_empty());
}

#[test]
fn limit_keeps_the_lowest_seeds() {
    let q = query("--level 2 --seeds 1-100 --monster kind=unique --limit 2 --threads 4");
    let o = search(prepared(), &q, Seams::default).unwrap();
    assert_eq!(seeds(&o), UNIQUE_SEEDS[..2]);
    assert_eq!(o.checked, UNIQUE_SEEDS[1]);
}

#[test]
fn a_level_d2rs_cannot_build_is_unsupported() {
    // The synthetic cave (level 3) is a maze whose level type has no
    // lvlmaze-backed generator on this data: the first seed stops the
    // search.
    let q = query("--level 3 --seeds 5-50 --threads 2");
    let o = search(prepared(), &q, Seams::default).unwrap();
    assert!(o.hits.is_empty());
    let (seed, e) = o.unsupported.expect("unsupported");
    assert_eq!(seed, 5);
    assert!(e.contains("LevelType"), "{e}");
    // A level without a levels.txt row.
    let e = check_seed(prepared(), &query("--level 99"), 1, Seams::default()).unwrap_err();
    assert!(e.to_string().contains("no levels.txt row"), "{e}");
}

// M08 for the wiring: room population runs on the live host
// (`levels.md` §11.6), so random-boss queries are answered: the unique
// seeds of the earlier fake (one record per room, index 1) are the live
// host's too, because a one-record room's list is exactly that record
// (`levels.md` §11.2 step 2).
#[test]
fn the_live_host_answers_random_boss_queries() {
    let p = prepared();
    let q = query("--level 2 --seeds 1-100 --monster kind=unique --threads 2");
    let o = search(p, &q, Seams::default).unwrap();
    assert_eq!(o.unsupported, None);
    assert_eq!(seeds(&o), UNIQUE_SEEDS);
    let q = query("--level 2 --seeds 1-20");
    let o = search(p, &q, Seams::default).unwrap();
    assert_eq!(o.hits.len(), 20);
    assert!(o.hits.iter().any(|h| !h.view.monsters.is_empty()));
}

/// The binary on a synthetic install as `D2_GAME_DIR` (`--town-only`):
/// matches printed, exit 0; a unique query accepted; an unsupported
/// level, exit 2.
#[test]
fn the_binary_runs_on_an_install() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("seed-finder-bin-{}", std::process::id()));
    install::build(&dir, &synth::synthetic()).unwrap_or_else(|e| panic!("{e}"));
    let run = |args: &str| {
        std::process::Command::new(env!("CARGO_BIN_EXE_seed-finder"))
            .args(args.split_whitespace())
            .env("D2_GAME_DIR", &dir)
            .output()
            .unwrap()
    };
    let o = run("--level 2 --from 1 --seeds 1-3 --threads 2 --town-only");
    let out = String::from_utf8_lossy(&o.stdout);
    assert_eq!(o.status.code(), Some(0), "{out}");
    assert!(out.starts_with("seed 1 (init_seed 1, game seed 1): level 2, 1 rooms, 0 monsters, entrance (20, 20)\nseed 2 "), "{out}");
    assert!(
        out.ends_with("3 match(es) in 3 seed(s) checked from 1\n"),
        "{out}"
    );
    let o = run("--level 2 --seeds 1-100 --monster kind=unique --threads 2 --town-only");
    assert_eq!(o.status.code(), Some(0));
    let out = String::from_utf8_lossy(&o.stdout);
    // Accepted (no longer refused); the binary's tables have no
    // spawnable rows (no `tweak_world`), so the count is not pinned.
    assert!(
        out.ends_with(" match(es) in 100 seed(s) checked from 1\n"),
        "{out}"
    );
    let o = run("--level 3 --seeds 1-3 --town-only");
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("unsupported level 3 (seed 1)"));
    assert_eq!(run("--level 2 --bogus 1").status.code(), Some(1));
}
