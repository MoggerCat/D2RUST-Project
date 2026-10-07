// Spec: specs/sim/intents-events.md §7.8; specs/drlg/rooms.md §3.3, §4; specs/monsters/population.md §2.3, §3
//! What the client receives while its player walks from the town into
//! the Blood Moor on the Act I-shaped synthetic set (first playable
//! scope G17 / G18, server half): the room switch's S→C 0x07 / 0x08 for
//! rooms of both levels, and the Blood Moor's monsters added to the
//! client (S→C 0xAC) once they spawn.
//!
//! The town gets one NPC preset here (DS1 type 1, a `keeper` row added
//! to `monpreset` act 1), as the Rogue Encampment's NPCs are (G17).
//!
//! The set's `monstats` rows get `isSpawn` here (the shared set leaves it
//! empty, so `population.md` §2.3 draws no region entry and no monster
//! ever spawns). No `Covers:` claim on the data: it is made up.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::OnceLock;

use d2_formats::ds1::Ds1Object;
use d2_sim::units::UnitType;
use test_fixtures::act1::{self, BLOOD_MOOR, TOWN};
use test_fixtures::drlg::archive_name;
use test_fixtures::game::{ActCreation, GameData};
use test_fixtures::host::{border_goals, Session, Setup};
use test_fixtures::install;
use test_fixtures::synth::Synthetic;

/// The Act I set with `isSpawn` on every monster that has a rarity (the
/// non-NPC rows).
fn spawning_act1() -> Synthetic {
    let mut s = act1::act1();
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
    // The town NPC: act 1's `monpreset` rows are beast1, Warden 0, then
    // this one (DS1 id 2).
    s.tables
        .row("monpreset", &[("Act", "1"), ("Place", "keeper")]);
    let mut town = act1::town();
    let (_, _, x, y) = act1::TOWN_WAYPOINT;
    town.objects.push(Ds1Object {
        kind: 1,
        id: NPC_DS1_ID,
        x: x + 5,
        y: y + 5,
        flags: 0,
    });
    let name = archive_name(act1::TOWN_DS1);
    let f = s
        .files
        .iter_mut()
        .find(|f| f.0 == name)
        .expect("the town file");
    f.1 = test_fixtures::ds1::write(&town);
    s
}

/// The NPC's DS1 id (its `monpreset` act 1 row).
const NPC_DS1_ID: u32 = 2;

fn data() -> &'static GameData {
    static D: OnceLock<GameData> = OnceLock::new();
    D.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("act1-stream-{}", std::process::id()));
        let i = install::build(&dir, &spawning_act1()).unwrap_or_else(|e| panic!("{e}"));
        GameData::from_install(&i).unwrap_or_else(|e| panic!("{e}"))
    })
}

fn setup() -> Setup {
    // `act1_game.rs`'s recorded Act I creation seeds and fixture choices.
    Setup {
        creation: ActCreation::Full,
        init_seed: 644_409_375,
        town: TOWN,
        game_seed: 1234,
        class: 3,
        known_waypoints: Vec::new(),
    }
}

/// The session walked out of the town until its player stands in a
/// Blood Moor room.
fn walked() -> Session {
    let mut fx = Session::new(data(), &setup());
    let town = fx.level_rect(TOWN);
    let moor = fx.level_rect(BLOOD_MOOR);
    let p = fx.pos();
    let goals = border_goals(town, moor, p);
    fx.walk(
        &goals,
        |f| f.unit_level(f.player) == Some(BLOOD_MOOR),
        "Blood Moor",
    );
    fx.assert_clean("Blood Moor");
    fx
}

#[test]
fn the_walk_into_the_blood_moor_streams_its_rooms_and_monsters() {
    let mut fx = walked();
    let ids: Vec<u8> = fx.transcript.iter().map(|m| m[0]).collect();
    assert!(ids.contains(&0x07), "room adds: {ids:02X?}");
    assert!(ids.contains(&0x08), "room removes: {ids:02X?}");

    // Every monster the server holds was spawned by the Blood Moor's
    // population (the town has none); each one the client was told of
    // came as an S→C 0xAC with the monster's GUID (u32 at 1).
    let mons = fx.sim().game.lists.units_of_type(UnitType::Monster);
    assert!(!mons.is_empty(), "the Blood Moor populated");
    let guids: BTreeSet<u32> = mons
        .iter()
        .map(|&u| fx.sim().game.lists.unit(u).unwrap().guid)
        .collect();
    let added: BTreeSet<u32> = fx
        .transcript
        .iter()
        .filter(|m| m[0] == 0xAC && m.len() >= 5)
        .map(|m| u32::from_le_bytes([m[1], m[2], m[3], m[4]]))
        .collect();
    assert!(!added.is_empty(), "monsters reached the client: {ids:02X?}");
    assert!(
        added.is_subset(&guids),
        "0xAC for unknown monsters: {added:?} vs {guids:?}"
    );
}

#[test]
fn the_town_npc_reaches_the_client() {
    let mut fx = Session::new(data(), &setup());
    for _ in 0..10 {
        fx.frame();
    }
    fx.assert_clean("town");
    // The class is the `monstats` row (the compiled `.bin` keeps the
    // `.txt` order).
    let set = spawning_act1();
    let f = set.tables.file("monstats.txt");
    let id = f.columns.iter().position(|c| c == "Id").unwrap();
    let keeper = f
        .rows
        .iter()
        .position(|r| r[id] == "keeper")
        .expect("keeper row") as u32;
    let sim = &fx.host.game;
    let npcs: Vec<u32> = sim
        .game
        .lists
        .units_of_type(UnitType::Monster)
        .into_iter()
        .filter(|&u| sim.events.action.sys.units.get(u).unwrap().class == keeper)
        .map(|u| sim.game.lists.unit(u).unwrap().guid)
        .collect();
    assert_eq!(npcs.len(), 1, "the preset NPC spawned in the town");
    let added: Vec<u32> = fx
        .transcript
        .iter()
        .filter(|m| m[0] == 0xAC && m.len() >= 5)
        .map(|m| u32::from_le_bytes([m[1], m[2], m[3], m[4]]))
        .collect();
    assert!(added.contains(&npcs[0]), "0xAC of the NPC: {added:?}");
}

/// Determinism (CLAUDE.md rule 6): two walks give the same transcript.
#[test]
fn the_stream_is_deterministic() {
    assert_eq!(walked().transcript, walked().transcript);
}
