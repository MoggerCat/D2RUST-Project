// Spec: specs/drlg/maze.md (§4, §9), specs/drlg/preset.md (§4, §6, §8, §9), specs/drlg/rooms.md (§3.3), specs/sim/path-placement.md (§12.2)
//! Entering a maze level in the app's own synthetic game, headless (task
//! `q-act1-dungeons`, `docs/handoff/q-act1-dungeons.md`): the Den of
//! Evil's stairs lead to Cave Level 1, a maze level the real maze
//! generator builds into preset rooms when the warp is taken; the player
//! is placed in it and the client follows. PROVISIONAL: the stairs tile
//! and the room carrying the way back (d2rs-own, unverified).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{add_client_data, add_game, add_walk, predict_link, send_create_game};
use d2_client::app::single_player::{self, GameData};
use d2_client::app::synthetic_maze;
use d2_client::bridge::predict::Speeds;
use d2_client::bridge::world::{UnitKey, TILE};
use d2_client::bridge::BridgeResource;
use d2_server::seams::Clock;

mod app_support;
use app_support::SharedLink;

/// The host clock, advanced by the test.
struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

/// The server player's level id.
fn server_level(server: &app_support::Server<StepClock>) -> Option<u32> {
    app_support::with(server, |l| {
        let (p, _) = single_player::local_player(&l.host().game)?;
        let g = &mut l.host_mut().game;
        let room = g.game.lists.unit(p)?.room()?;
        g.events.action.hooks().drlg.level_id(&g.game, room)
    })
}

// Covers: specs/drlg/maze.md §9 r1; specs/drlg/levels.md §5; specs/sim/path-placement.md §12.2 r1; specs/sim/path-placement.md §12.2 r5
#[test]
fn the_dens_stairs_build_the_maze_level_and_place_the_player() {
    let data = GameData::Synthetic;
    let ms = Arc::new(AtomicU32::new(1000));
    let (link, _) = single_player::start(
        data.clone(),
        single_player::DEFAULT_SEED,
        StepClock(ms.clone()),
    )
    .unwrap();
    let server = Arc::new(Mutex::new(link));
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_resource::<ButtonInput<MouseButton>>();
    let (link, tap) = predict_link(Box::new(SharedLink(server.clone())));
    add_game(&mut app, link, false).unwrap();
    add_walk(&mut app, tap, Some(Speeds { walk: 6, run: 9 }));
    send_create_game(&mut app).unwrap();
    add_client_data(
        &mut app,
        single_player::client_drlg_source(&data),
        single_player::client_level_rows(&data),
    );
    app_support::synthetic_skill_rows(&mut app);
    app.update();
    let mut steps = 0;
    let mut step = |app: &mut App| {
        ms.fetch_add(40, Ordering::SeqCst);
        app.update();
        steps += 1;
        assert!(steps < 6000, "the warps happen");
    };
    while app_support::local_player(&server).is_none() {
        step(&mut app);
    }
    for _ in 0..30 {
        step(&mut app);
    }
    let level = |app: &App| {
        app.world()
            .resource::<BridgeResource>()
            .0
            .world()
            .player_level()
    };
    let tile = |app: &App, class: u32| {
        app.world()
            .resource::<BridgeResource>()
            .0
            .world()
            .units
            .iter()
            .find(|(k, u)| k.unit_type == TILE && u.class == class)
            .map(|(k, _)| *k)
    };
    let take = |app: &mut App, key: UnitKey| {
        let sent = app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .interact(key)
            .unwrap();
        assert!(sent.is_empty());
    };
    // Blood Moor -> Den of Evil.
    let entrance = tile(&app, single_player::BLOOD_MOOR_TO_DEN).expect("cave entrance");
    take(&mut app, entrance);
    while server_level(&server) != Some(single_player::DEN_OF_EVIL) {
        step(&mut app);
    }
    for _ in 0..30 {
        step(&mut app);
    }
    assert_eq!(level(&app), Some(single_player::DEN_OF_EVIL as u16));
    // The Den's stairs -> Cave Level 1 (a maze level).
    let stairs = tile(&app, synthetic_maze::DEN_TO_CAVE).expect("the Den's stairs");
    take(&mut app, stairs);
    while server_level(&server) != Some(synthetic_maze::CAVE_LEVEL_1) {
        step(&mut app);
    }
    for _ in 0..30 {
        step(&mut app);
    }
    // The maze level was built: preset rooms, the player in one of them.
    let rooms = app_support::with(&server, |l| {
        let g = &mut l.host_mut().game;
        let hooks = g.events.action.hooks();
        let d = hooks.drlg.dungeon.acts[0].as_ref().expect("act 0");
        let idx = d.find_level(synthetic_maze::CAVE_LEVEL_1).expect("level");
        d.level_rooms(idx).len()
    });
    assert!(rooms >= 9, "the maze built {rooms} preset rooms");
    assert_eq!(level(&app), Some(synthetic_maze::CAVE_LEVEL_1 as u16));
    let b = &app.world().resource::<BridgeResource>().0;
    assert!(b.log().rejected.is_empty(), "{:?}", b.log().rejected);
    assert!(
        tile(&app, synthetic_maze::CAVE_TO_DEN).is_some(),
        "the maze level's way back is in the client's model"
    );
}
