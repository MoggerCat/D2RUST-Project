// Spec: specs/drlg/levels.md §3–§5; specs/drlg/maze.md; specs/drlg/outdoor.md; specs/monsters/population.md §11 (the seed finder on the user's tables)
//! The seed finder on the live 1.14d tables (`#[ignore]`, `D2_GAME_DIR`):
//! the Den of Evil (8) and the Blood Moor (2) build for seeds 1–8
//! without an unsupported step, twice with the same result; the views
//! are printed for the local run (`docs/handoff/seed-finder.md`).
//! **Unconfirmed** until the first local run: no `Covers:` claim.

use d2_data::bin;
use d2_formats::mpq::ArchiveSet;
use seed_finder::query::parse_args;
use seed_finder::world::Prepared;
use seed_finder::{describe, search};
use test_fixtures::game::{ActCreation, GameData, Seams};

#[test]
#[ignore = "needs D2_GAME_DIR"]
fn den_of_evil_and_blood_moor_build_on_live_tables() {
    let dir = std::env::var("D2_GAME_DIR").expect("D2_GAME_DIR must be set");
    let set = ArchiveSet::open_dir(dir).expect("archives open");
    let bins = bin::load(&set, bin::DEFAULT_LANGUAGE).expect("live set loads");
    let data = GameData::load(bins, &set).expect("game data");
    let p = Prepared::new(&data, ActCreation::Full).expect("prepared");
    for args in [
        "--level 8 --from 2 --seeds 1-8 --threads 4",
        "--level 2 --from 1 --seeds 1-8 --threads 4",
    ] {
        let a: Vec<String> = args.split_whitespace().map(str::to_owned).collect();
        let q = parse_args(&a).unwrap().query;
        let o = search(&p, &q, Seams::default).unwrap();
        assert_eq!(o.unsupported, None, "{args}");
        assert_eq!(o.hits.len(), 8, "{args}");
        for h in &o.hits {
            println!("{}", describe(h, &q, &[]));
        }
        assert_eq!(o, search(&p, &q, Seams::default).unwrap(), "{args}: twice");
    }
}
