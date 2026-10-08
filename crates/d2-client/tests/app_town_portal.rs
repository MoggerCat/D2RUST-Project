// Spec: specs/world/objects.md (§5.5, §12), specs/skills/bodies-3.md (§4.4), specs/client/model.md (§8 rule 7)
//! Town Portal in the app's own synthetic game, headless (task
//! `q-town-portal`, `docs/handoff/q-town-portal.md`). The player stands in
//! the Den of Evil (reached through its warp tile, as `app_level_warp.rs`);
//! the Town Portal use (`ActionSim::open_town_portal`, what the scroll's
//! C→S 0x20 ends in) makes the portal pair: the field portal reaches the
//! client (S→C 0x51), a click on it (C→S 0x13) moves the player to the
//! town, where the town portal's click brings the player back and removes
//! the pair. PROVISIONAL (REC-117): the creation, the links, the
//! removal on the way back from town.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{add_client_data, add_game, add_walk, predict_link, send_create_game};
use d2_client::app::single_player::{self, GameData};
use d2_client::bridge::predict::Speeds;
use d2_client::bridge::world::{UnitKey, OBJECT, TILE};
use d2_client::bridge::BridgeResource;
use d2_server::seams::Clock;
use d2_sim::units::UnitType;

mod app_support;
use app_support::SharedLink;

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

/// The class-59 objects of the server's game.
fn server_portals(server: &app_support::Server<StepClock>) -> usize {
    app_support::with(server, |l| {
        let g = &l.host().game.game;
        g.lists
            .units_of_type(UnitType::Object)
            .into_iter()
            .filter(|&u| {
                l.host()
                    .game
                    .events
                    .action
                    .sys
                    .units
                    .get(u)
                    .is_some_and(|r| r.class == 59)
            })
            .count()
    })
}

// Covers: specs/world/objects.md §12 r6; specs/world/objects.md §12 r8; specs/world/objects.md §12 r11; specs/world/objects.md §12 r12; specs/world/objects.md §5.5
#[test]
fn a_town_portal_takes_the_player_to_town_and_back_and_goes() {
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
        assert!(steps < 6000, "the test finishes");
    };
    while app_support::local_player(&server).is_none() {
        step(&mut app);
    }
    for _ in 0..30 {
        step(&mut app);
    }
    let find = |app: &App, ty: u8, class: u32| -> Option<UnitKey> {
        app.world()
            .resource::<BridgeResource>()
            .0
            .world()
            .units
            .iter()
            .find(|(k, u)| k.unit_type == ty && u.class == class)
            .map(|(k, _)| *k)
    };

    // Into the Den through its warp tile.
    let entrance = find(&app, TILE, single_player::BLOOD_MOOR_TO_DEN).expect("the cave entrance");
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .interact(entrance)
        .unwrap();
    while server_level(&server) != Some(single_player::DEN_OF_EVIL) {
        step(&mut app);
    }
    for _ in 0..30 {
        step(&mut app);
    }
    assert_eq!(server_portals(&server), 0);

    // The Town Portal use: the pair exists, the field portal reaches the
    // client.
    let made = app_support::with(&server, |l| {
        let (p, _) = single_player::local_player(&l.host().game).unwrap();
        let g = &mut l.host_mut().game;
        g.events.action.open_town_portal(&mut g.game, p).is_some()
    });
    assert!(made, "the pair was created");
    assert_eq!(server_portals(&server), 2);
    for _ in 0..10 {
        step(&mut app);
    }
    let field = find(&app, OBJECT, 59).expect("the field portal reached the client");

    // Click it (the portal's hostile delay of 5000 ms from the host
    // clock's zero, `objects.md` §12 rule 2, has passed): the player is
    // in the town.
    ms.fetch_add(10_000, Ordering::SeqCst);
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .interact(field)
        .unwrap();
    while server_level(&server) != Some(single_player::ACT1_TOWN) {
        step(&mut app);
    }
    for _ in 0..30 {
        step(&mut app);
    }
    assert_eq!(server_portals(&server), 2, "the pair stays on the way in");
    let town_portal = find(&app, OBJECT, 59).expect("the town portal reached the client");

    // Click the town portal: back in the Den, the pair is gone.
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .interact(town_portal)
        .unwrap();
    while server_level(&server) != Some(single_player::DEN_OF_EVIL) {
        step(&mut app);
    }
    for _ in 0..30 {
        step(&mut app);
    }
    assert_eq!(server_portals(&server), 0, "used from town, both go");
    assert!(
        find(&app, OBJECT, 59).is_none(),
        "the client's portals went too"
    );
    let b = &app.world().resource::<BridgeResource>().0;
    assert!(b.log().rejected.is_empty(), "{:?}", b.log().rejected);
}
