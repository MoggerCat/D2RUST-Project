// Spec: specs/drlg/outdoor.md §8; specs/drlg/rooms.md §4; specs/monsters/population.md §2.3, §3
//! Act II in the play host (task `q-act-worlds`): a `Session` created in
//! act 1 (Lut Gholein), its player walking out to the Rocky Waste and the
//! Dry Hills, with the desert levels populated and the client told of the
//! monsters. Made-up data; no `Covers:` claim.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::OnceLock;

use d2_sim::units::UnitType;
use test_fixtures::act2::{self, DRY_HILLS, ROCKY_WASTE, TOWN};
use test_fixtures::game::{ActCreation, GameData};
use test_fixtures::host::{border_goals, Session, Setup};
use test_fixtures::install;
use test_fixtures::synth::Synthetic;

const ACT: u8 = 1;

/// The Act II set with `isSpawn` on every monster that has a rarity.
fn spawning_act2() -> Synthetic {
    let mut s = act2::act2();
    let f = s.tables.file("monstats.txt");
    let col = |name: &str| {
        f.columns
            .iter()
            .position(|c| c.eq_ignore_ascii_case(name))
            .unwrap()
    };
    let (rarity, id) = (col("Rarity"), col("Id"));
    let rows: Vec<usize> = f
        .rows
        .iter()
        .enumerate()
        .filter(|(_, r)| !r[rarity].is_empty() && !r[id].is_empty())
        .map(|(i, _)| i)
        .collect();
    assert!(!rows.is_empty(), "the set has spawnable monsters");
    for i in rows {
        s.tables.set("monstats", i, "isSpawn", "1");
    }
    s
}

fn data() -> &'static GameData {
    static D: OnceLock<GameData> = OnceLock::new();
    D.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("act2-play-{}", std::process::id()));
        let i = install::build(&dir, &spawning_act2()).unwrap_or_else(|e| panic!("{e}"));
        GameData::from_install(&i).unwrap_or_else(|e| panic!("{e}"))
    })
}

fn setup() -> Setup {
    Setup {
        creation: ActCreation::Full,
        init_seed: 644_409_375,
        town: TOWN,
        game_seed: 1234,
        class: 3,
        known_waypoints: Vec::new(),
    }
}

fn walk_to(fx: &mut Session, from: u32, to: u32, what: &str) {
    let a = fx.level_rect(from);
    let b = fx.level_rect(to);
    let p = fx.pos();
    let goals = border_goals(a, b, p);
    fx.walk(&goals, |f| f.unit_level(f.player) == Some(to), what);
    fx.assert_clean(what);
}

#[test]
fn the_player_walks_from_lut_gholein_into_the_desert_with_monsters() {
    let mut fx = Session::new_in_act(data(), &setup(), ACT);
    assert_eq!(fx.unit_level(fx.player), Some(TOWN));
    walk_to(&mut fx, TOWN, ROCKY_WASTE, "Rocky Waste");
    assert_eq!(fx.unit_level(fx.player), Some(ROCKY_WASTE));

    let mons = fx.sim().game.lists.units_of_type(UnitType::Monster);
    let mut in_waste = Vec::new();
    for u in mons {
        if fx.unit_level(u) == Some(ROCKY_WASTE) {
            in_waste.push(fx.sim().game.lists.unit(u).unwrap().guid);
        }
    }
    assert!(!in_waste.is_empty(), "the Rocky Waste populated");
    let added: BTreeSet<u32> = fx
        .transcript
        .iter()
        .filter(|m| m[0] == 0xAC && m.len() >= 5)
        .map(|m| u32::from_le_bytes([m[1], m[2], m[3], m[4]]))
        .collect();
    assert!(
        in_waste.iter().any(|g| added.contains(g)),
        "monsters reached the client"
    );

    // Onward, to the Dry Hills (the chain's next level).
    let next = [ROCKY_WASTE, DRY_HILLS];
    let (a, b) = (fx.level_rect(next[0]), fx.level_rect(next[1]));
    let p = fx.pos();
    let goals = border_goals(a, b, p);
    if !goals.is_empty() {
        fx.walk(
            &goals,
            |f| f.unit_level(f.player) == Some(DRY_HILLS),
            "Dry Hills",
        );
        fx.assert_clean("Dry Hills");
    }
}
