// Spec: specs/drlg/maze.md (§4, §9), specs/drlg/rooms.md (§3.3), specs/sim/path-placement.md (§12.2)
//! Act I's tree dungeons are built by the maze generator, headless (task
//! `q-dungeon-builds`, `docs/handoff/q-dungeon-builds.md`): Cave Level 2,
//! Underground Passage 1–2, Holes 1–2 and Pits 1–2 each get several
//! preset rooms (not one flat room), and the player walks into each by
//! the warp tile of its parent and out by its own way back.
//! PROVISIONAL (REC-261): rows, room counts and tile places are made up.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{add_client_data, add_game, add_walk, predict_link, send_create_game};
use d2_client::app::single_player::{self, GameData};
use d2_client::app::synthetic_a1_maze::LEVELS;
use d2_client::app::synthetic_chains::slots;
use d2_client::bridge::predict::Speeds;
use d2_client::bridge::world::TILE;
use d2_client::bridge::BridgeResource;
use d2_server::seams::Clock;

mod app_support;
use app_support::SharedLink;

struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

type Server = app_support::Server<StepClock>;

fn server_level(server: &Server) -> Option<u32> {
    app_support::with(server, |l| {
        let (p, _) = single_player::local_player(&l.host().game)?;
        let g = &mut l.host_mut().game;
        let room = g.game.lists.unit(p)?.room()?;
        g.events.action.hooks().drlg.level_id(&g.game, room)
    })
}

fn room_count(server: &Server, id: u32) -> usize {
    app_support::with(server, move |l| {
        let g = &mut l.host_mut().game;
        let d = g.events.action.hooks().drlg.dungeon.acts[0]
            .as_ref()
            .expect("act 0");
        d.level_rooms(d.find_level(id).expect("level")).len()
    })
}

// Covers: specs/drlg/maze.md §9 r1; specs/drlg/levels.md §5; specs/sim/path-placement.md §12.2 r1
#[test]
fn the_tree_dungeons_are_maze_builds_and_can_be_walked() {
    let data = GameData::Synthetic;
    let ms = Arc::new(AtomicU32::new(1000));
    let (link, _) = single_player::start(
        data.clone(),
        single_player::DEFAULT_SEED,
        StepClock(ms.clone()),
    )
    .unwrap();
    let server: Server = Arc::new(Mutex::new(link));
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
    let steps = std::cell::Cell::new(0u32);
    let step = |app: &mut App| {
        ms.fetch_add(40, Ordering::SeqCst);
        app.update();
        steps.set(steps.get() + 1);
        assert!(steps.get() < 100_000, "the warps happen");
    };
    while app_support::local_player(&server).is_none() {
        step(&mut app);
    }
    for _ in 0..8 {
        step(&mut app);
    }
    let settle = |app: &mut App, to: u32| {
        let mut n = 0;
        while server_level(&server) != Some(to) {
            step(app);
            n += 1;
            assert!(n < 2000, "the player reaches level {to}");
        }
        for _ in 0..8 {
            step(app);
        }
        let l = app
            .world()
            .resource::<BridgeResource>()
            .0
            .world()
            .player_level();
        assert_eq!(l, Some(to as u16), "the client follows to {to}");
    };
    let go = |app: &mut App, level: u32, to: u32| {
        let class = slots(level)
            .into_iter()
            .find(|s| s.1 == to)
            .unwrap_or_else(|| panic!("level {level} has a way to {to}"))
            .2;
        let tile = app
            .world()
            .resource::<BridgeResource>()
            .0
            .world()
            .units
            .iter()
            .find(|(k, u)| k.unit_type == TILE && u.class == class)
            .map(|(k, _)| *k)
            .unwrap_or_else(|| panic!("level {level}: tile {class} (to {to}) in the model"));
        let sent = app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .interact(tile)
            .unwrap();
        assert!(sent.is_empty());
        settle(app, to);
    };
    let put = |app: &mut App, level: u32| {
        app_support::with(&server, move |l| {
            let (p, _) = single_player::local_player(&l.host().game).unwrap();
            l.host_mut()
                .game
                .events
                .action
                .hooks()
                .act_changes
                .push((p, level, 0));
        });
        settle(app, level);
    };
    // Each maze level once, from its parent, and back.
    for child in LEVELS {
        let parent = slots(child)[0].1;
        put(&mut app, parent);
        go(&mut app, parent, child);
        let rooms = room_count(&server, child);
        assert!(rooms >= 4, "level {child}: the maze built {rooms} rooms");
        go(&mut app, child, parent);
    }
    // The passage and the hole go one level deeper and back.
    for (a, b) in [(10u32, 14u32), (11, 15), (12, 16)] {
        put(&mut app, a);
        go(&mut app, a, b);
        go(&mut app, b, a);
    }
    let b = &app.world().resource::<BridgeResource>().0;
    assert!(b.log().rejected.is_empty(), "{:?}", b.log().rejected);
}
