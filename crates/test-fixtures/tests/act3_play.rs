// Spec: specs/drlg/levels.md §3 (act creation), §4 (get-or-allocate); specs/drlg/maze.md §6
//! Act III in the play host (task `q-a3-dungeons`): a `Session` created in
//! act 2 (Kurast Docks), frames run, and a dungeon level allocated and
//! built inside the live game (the sim's own DRLG, not a side world).
//! Made-up data; no `Covers:` claim.

use std::path::PathBuf;
use std::sync::OnceLock;

use test_fixtures::act3::{self, TOWN};
use test_fixtures::game::{ActCreation, GameData};
use test_fixtures::host::{Session, Setup};
use test_fixtures::install;

const ACT: u8 = 2;

fn data() -> &'static GameData {
    static D: OnceLock<GameData> = OnceLock::new();
    D.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("act3-play-{}", std::process::id()));
        let i = install::build(&dir, &act3::act3()).unwrap_or_else(|e| panic!("{e}"));
        GameData::from_install(&i).unwrap_or_else(|e| panic!("{e}"))
    })
}

fn setup() -> Setup {
    Setup {
        creation: ActCreation::TownOnly,
        init_seed: 644_409_375,
        town: TOWN,
        game_seed: 1234,
        class: 3,
        known_waypoints: Vec::new(),
    }
}

#[test]
fn a_session_starts_in_kurast_docks_and_runs_frames() {
    let mut fx = Session::new_in_act(data(), &setup(), ACT);
    assert_eq!(fx.unit_level(fx.player), Some(TOWN));
    for _ in 0..10 {
        fx.frame();
    }
    fx.assert_clean("Kurast Docks");
    assert_eq!(fx.unit_level(fx.player), Some(TOWN));
}

#[test]
fn the_live_game_builds_every_dungeon_level_of_the_act() {
    let mut fx = Session::new_in_act(data(), &setup(), ACT);
    for &(id, _) in &act3::MAZE_LEVELS {
        let sim = fx.sim();
        let rooms = sim
            .events
            .action
            .hooks()
            .drlg
            .with_act(ACT, &mut sim.game.lists, |dr, svc| {
                let l = dr.get_or_alloc_level(svc.data, svc.types, id)?;
                // A neighbouring level's stamps may have built it already.
                if dr.level_rooms(l).is_empty() {
                    dr.generate_level(svc.data, svc.types, l)?;
                }
                let rooms = dr.level_rooms(l);
                for &r in &rooms {
                    dr.stream_room(svc, r)?;
                }
                Ok::<_, d2_sim::drlg::DrlgError>(rooms.len())
            })
            .expect("act 2 has a DRLG")
            .unwrap_or_else(|e| panic!("level {id}: {e:?}"));
        assert!(rooms >= 36, "level {id}: {rooms} rooms");
    }
    // The built levels sit in the same act as the town.
    for _ in 0..5 {
        fx.frame();
    }
    fx.assert_clean("dungeon levels built");
    for &(id, _) in &act3::MAZE_LEVELS {
        let r = fx.level_rect(id);
        assert!(r.w > 0 && r.h > 0, "level {id} has a rect");
    }
}
