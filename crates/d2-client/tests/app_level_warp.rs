// Spec: specs/sim/path-placement.md (§12.1, §12.2), specs/drlg/rooms.md (§3.3), specs/client/model.md (§8 rule 7, §11, §12)
//! A level warp in the app's own synthetic game, headless (task
//! `q-warps`, `docs/handoff/q-warps.md`): the Blood Moor's cave entrance
//! is a tile unit (S→C 0x09); the client's interact sender puts C→S 0x13
//! (unit type 5) on the wire; the server runs the walk into the warp
//! (`0x005550B0`), places the player in the Den of Evil and sends the new
//! level's 0x07 / 0x15 / 0x0D; the client's model follows to the new
//! level. PROVISIONAL (REC-99): the tile creation, the 0x13 tile case.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{add_client_data, add_game, add_walk, predict_link, send_create_game};
use d2_client::app::single_player::{self};
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

// Covers: specs/sim/path-placement.md §12.2 r1; specs/sim/path-placement.md §12.2 r5; specs/sim/path-placement.md §12.2 r6; specs/client/model.md §8 r7
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn clicking_the_cave_entrance_takes_the_player_to_the_den_of_evil() {
    let data = app_support::game_data();
    let ms = Arc::new(AtomicU32::new(1000));
    let (link, _) = single_player::start(
        data.clone(),
        single_player::DEFAULT_SEED,
        StepClock(ms.clone()),
    )
    .unwrap();
    let server = Arc::new(Mutex::new(link));
    let mut app = App::new();
    app.insert_resource(d2_client::bridge::mirror::ScriptedClock(ms.clone()));
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
    app_support::live_tables(&mut app);
    app.update();
    let mut steps = 0;
    let mut step = |app: &mut App| {
        ms.fetch_add(40, Ordering::SeqCst);
        app.update();
        steps += 1;
        assert!(steps < 3000, "the warp happens");
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
    assert_eq!(level(&app), Some(single_player::ACT1_TOWN as u16));
    // The Den's cave entrance lies in the Blood Moor: the player walks
    // out of the town by the route over the server's collision.
    app_support::walk_into(
        &mut app,
        &server,
        &ms,
        single_player::ACT1_TOWN,
        single_player::BLOOD_MOOR,
    );
    // The server places the entrance's tile when the player brings its
    // room into play: walk the Blood Moor's rooms nearest first until the
    // tile stands within reach and the client model holds it.
    app_support::approach(
        &mut app,
        &server,
        &ms,
        TILE,
        &[app_support::warp_id(
            single_player::BLOOD_MOOR,
            single_player::DEN_OF_EVIL,
        )],
    );
    // The cave entrance is a tile unit of the client's model.
    let tile = |app: &App| {
        app.world()
            .resource::<BridgeResource>()
            .0
            .world()
            .units
            .iter()
            .find(|(k, u)| {
                k.unit_type == TILE
                    && u.class
                        == app_support::warp_id(
                            single_player::BLOOD_MOOR,
                            single_player::DEN_OF_EVIL,
                        )
            })
            .map(|(k, _)| *k)
    };
    let entrance: UnitKey = tile(&app).expect("the Blood Moor's cave entrance reached the client");
    // The click's interact sender (`model.md` §8 rule 7) sends 0x13 for
    // the tile; the server warps the player.
    let sent = app
        .world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .interact(entrance)
        .unwrap();
    assert!(sent.is_empty());
    while server_level(&server) != Some(single_player::DEN_OF_EVIL) {
        step(&mut app);
    }
    for _ in 0..30 {
        step(&mut app);
    }
    // The client's model follows: the new level, the player placed in
    // it, the arrival tile of the Den among its units.
    assert_eq!(level(&app), Some(single_player::DEN_OF_EVIL as u16));
    let b = &app.world().resource::<BridgeResource>().0;
    assert!(b.log().rejected.is_empty(), "{:?}", b.log().rejected);
    assert!(
        b.world().units.iter().any(|(k, u)| k.unit_type == TILE
            && u.class
                == app_support::warp_id(single_player::DEN_OF_EVIL, single_player::BLOOD_MOOR)),
        "the Den's way back is in the client's model"
    );
}
