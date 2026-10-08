// Spec: specs/sim/path-placement.md (§12.1, §12.2), specs/drlg/rooms.md (§3.3), specs/client/model.md (§8 rule 7)
//! Act I's dungeons in the app's synthetic game, headless (task
//! `q-a1-dungeons`, `docs/handoff/q-a1-dungeons.md`): from the Rogue
//! Encampment the player clicks the warp tile of each level on the way to
//! Catacombs level 4 (town, Blood Moor, Black Marsh, Dark Wood, Tamoe
//! Highland, Monastery Gate, Outer Cloister, Barracks, Jail 1–3, Inner
//! Cloister, Cathedral, Catacombs 1–4); the server player and the client
//! model follow each step and nothing is rejected. The branches (caves,
//! holes, pits, passages, crypt, mausoleum, Tristram) are each entered and
//! left once. PROVISIONAL (REC-249): the tree's places are made up.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{add_client_data, add_game, add_walk, predict_link, send_create_game};
use d2_client::app::single_player::{self, GameData};
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

// Covers: specs/sim/path-placement.md §12.2 r1; specs/drlg/levels.md §5
#[test]
fn walk_from_the_encampment_to_catacombs_4() {
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
        assert!(steps.get() < 200_000, "the warps happen");
    };
    while app_support::local_player(&server).is_none() {
        step(&mut app);
    }
    for _ in 0..8 {
        step(&mut app);
    }
    let client_level = |app: &App| {
        app.world()
            .resource::<BridgeResource>()
            .0
            .world()
            .player_level()
    };
    let settle = |app: &mut App, to: u32| {
        let mut n = 0;
        while server_level(&server) != Some(to) {
            step(app);
            n += 1;
            assert!(
                n < 2000,
                "the player reaches level {to} (at {:?}, errors {}, rejected {:?})",
                server_level(&server),
                app_support::with(&server, |l| format!(
                    "{:?}",
                    l.host_mut().game.events.action.hooks().errors
                )),
                app.world().resource::<BridgeResource>().0.log().rejected
            );
        }
        for _ in 0..8 {
            step(app);
        }
        assert_eq!(
            client_level(app),
            Some(to as u16),
            "the client follows to {to}"
        );
    };
    use d2_client::app::synthetic_chains::slots;
    // The tile of `level` that leads to `to`, clicked.
    let go = |app: &mut App, level: u32, to: u32| {
        let class = slots(level)
            .into_iter()
            .chain(
                single_player::synthetic_level_warps()
                    .into_iter()
                    .filter(|w| w.0 == level)
                    .map(|w| (0, w.1, w.2)),
            )
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
    assert_eq!(server_level(&server), Some(single_player::ACT1_TOWN));
    // The town reaches the Blood Moor by a border, not a tile: put the
    // player there, as the walk does.
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
    put(&mut app, single_player::BLOOD_MOOR);
    let path = [6u32, 5, 7, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37];
    let mut at = single_player::BLOOD_MOOR;
    for to in path {
        go(&mut app, at, to);
        at = to;
    }
    // Back out of Catacombs 4 to Catacombs 3 by its way-back tile.
    go(&mut app, 37, 36);
    // Each branch once, from its parent.
    for (parent, child) in [
        (3u32, 13u32),
        (5, 38),
        (5, 10),
        (10, 14),
        (7, 12),
        (12, 16),
        (6, 11),
        (11, 15),
        (17, 18),
        (17, 19),
    ] {
        put(&mut app, parent);
        go(&mut app, parent, child);
        go(&mut app, child, parent);
    }
    let b = &app.world().resource::<BridgeResource>().0;
    assert!(b.log().rejected.is_empty(), "{:?}", b.log().rejected);
}
