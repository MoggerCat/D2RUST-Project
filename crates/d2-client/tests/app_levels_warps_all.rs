// Spec: specs/sim/path-placement.md (§12.1, §12.2), specs/drlg/rooms.md (§3.3), specs/client/model.md (§8 rule 7)
//! Every level warp of the app's synthetic game, headless (task
//! `q-levels-warps-all`, `docs/handoff/q-levels-warps-all.md`): the
//! player is put in each level that has warps (the act-change queue, as
//! `app_act_travel`), then every warp tile of the level is clicked
//! (C→S 0x13): the server player must end in the level the DRLG's vis
//! slot names, the client's model must follow, and nothing is rejected.
//! PROVISIONAL (REC-230): the chain levels' places are made up.

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

// Covers: specs/sim/path-placement.md §12.2 r1; specs/sim/path-placement.md §12.2 r5
#[test]
fn every_warp_of_every_level_loads_its_destination() {
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
            assert!(n < 2000, "the player reaches level {to}");
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
    // Duriel's Lair is quest-gated (`0x00545B80`, `quests-act2.md` §8.2):
    // its warps are `app_act2_duriel`'s.
    let lair = d2_client::app::synthetic_act2::DURIELS_LAIR;
    let warps: Vec<_> = single_player::synthetic_level_warps()
        .into_iter()
        .filter(|w| w.0 != lair && w.1 != lair)
        .collect();
    assert!(warps.len() > 40, "{}", warps.len());
    let mut levels: Vec<u32> = warps.iter().map(|w| w.0).collect();
    levels.dedup();
    for level in levels {
        // Put the player in the level (the act-change queue).
        if server_level(&server) != Some(level) {
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
            settle(&mut app, level);
        }
        for &(_, to, class) in warps.iter().filter(|w| w.0 == level) {
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
            settle(&mut app, to);
            // And back to where the next tile is.
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
            settle(&mut app, level);
        }
    }
    let b = &app.world().resource::<BridgeResource>().0;
    assert!(b.log().rejected.is_empty(), "{:?}", b.log().rejected);
}
