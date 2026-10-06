// Spec: specs/data/loading.md (the loaded set); the search of `seed_finder` (lib)
//! `seed-finder`: searches seeds on the user's own tables
//! (`D2_GAME_DIR`) and prints the matching seeds. Exit 0 with matches
//! or none, 2 when the level is unsupported, 1 on errors.

use anyhow::{anyhow, Context, Result};
use d2_data::bin;
use d2_formats::mpq::ArchiveSet;
use seed_finder::query::parse_args;
use seed_finder::world::{monster_names, Prepared};
use seed_finder::{describe, search};
use test_fixtures::game::{ActCreation, GameData, Seams};

fn run() -> Result<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut a = parse_args(&args)?;
    let dir = std::env::var("D2_GAME_DIR").context("D2_GAME_DIR must name the game folder")?;
    let set = ArchiveSet::open_dir(&dir).map_err(|e| anyhow!("{dir}: {e}"))?;
    let bins = bin::load(&set, bin::DEFAULT_LANGUAGE).map_err(|e| anyhow!("load: {e}"))?;
    let data = GameData::load(bins, &set).map_err(|e| anyhow!("{e}"))?;
    let names = monster_names(&set, &data.fixed);
    a.query.resolve(&names)?;
    let creation = if a.town_only {
        ActCreation::TownOnly
    } else {
        ActCreation::Full
    };
    let p = Prepared::new(&data, creation)?;
    let q = &a.query;
    if p.act_of(q.level).is_none() {
        eprintln!("unsupported level {}: no levels.txt row", q.level);
        return Ok(2);
    }
    eprintln!(
        "note: rooms are streamed in level-list order and populated in one room \
         pass (tick 1), an order the original does not produce (levels.md §11.6 \
         rule 4): a hit is a candidate until a scenario reproduces it."
    );
    let out = search(&p, q, Seams::default)?;
    for h in &out.hits {
        println!("{}", describe(h, q, &names));
    }
    println!(
        "{} match(es) in {} seed(s) checked from {}",
        out.hits.len(),
        out.checked,
        q.first
    );
    if let Some((seed, e)) = out.unsupported {
        eprintln!("unsupported level {} (seed {seed}): {e}", q.level);
        return Ok(2);
    }
    Ok(0)
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("seed-finder: {e:#}");
            std::process::exit(1);
        }
    }
}
