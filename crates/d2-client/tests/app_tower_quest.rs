// Spec: specs/world/quests-act1.md (§10.7), specs/world/quests.md (§4.4–§4.6), specs/sim/path-placement.md (§12.2), specs/drlg/maze.md (§9)
//! The Forgotten Tower quest in the app's own synthetic game, headless
//! (task `q-a1-tower`, `docs/handoff/q-a1-tower.md`): the Black Marsh's
//! Moldy Tome opens the quest (A1Q5, state 2), the Tower and its five
//! cellars are built by the maze generator behind the warp tiles, the
//! level changes reach the quest control (Tower Cellar 5: state 3), and
//! the Countess's death (a monster linked to chain 5 by monster init)
//! completes it (state 5). PROVISIONAL: the synthetic Black Marsh, its
//! warp pair, the tome's place, and the Countess's spawn by hand (the
//! synthetic game has no population; d2rs-own, unverified).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{add_client_data, add_game, add_walk, predict_link, send_create_game};
use d2_client::app::single_player::{self, GameData};
use d2_client::app::synthetic_tower as tower;
use d2_client::bridge::predict::Speeds;
use d2_client::bridge::world::{UnitKey, OBJECT, TILE};
use d2_client::bridge::BridgeResource;
use d2_server::seams::Clock;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::{KillStep, Pending};

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

/// (state, status word) of quest chain 5 on the server.
fn chain5(server: &Server) -> (u8, u8) {
    app_support::with(server, |l| {
        let r = l.host().game.world.quests.record(5).expect("chain 5");
        (r.state, r.status)
    })
}

// Covers: specs/world/quests-act1.md §10.7 r2; specs/world/quests-act1.md §10.7 r3; specs/world/quests-act1.md §10.7 r4; specs/world/quests.md §4.4; specs/world/quests.md §4.5
#[test]
fn the_tome_the_cellars_and_the_countess_run_chain_5() {
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
        assert!(steps.get() < 20000, "the warps happen");
    };
    while app_support::local_player(&server).is_none() {
        step(&mut app);
    }
    for _ in 0..30 {
        step(&mut app);
    }
    let find = |app: &App, ty: u8, class: u32| {
        app.world()
            .resource::<BridgeResource>()
            .0
            .world()
            .units
            .iter()
            .find(|(k, u)| k.unit_type == ty && u.class == class)
            .map(|(k, _)| *k)
    };
    let interact = |app: &mut App, key: UnitKey| {
        let sent = app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .interact(key)
            .unwrap();
        assert!(sent.is_empty());
    };
    let take = |app: &mut App, class: u32, to: u32| {
        let t = find(app, TILE, class).unwrap_or_else(|| panic!("tile {class} in the model"));
        interact(app, t);
        while server_level(&server) != Some(to) {
            step(app);
        }
        for _ in 0..30 {
            step(app);
        }
    };
    assert_eq!(chain5(&server).0, 0);
    // The Black Marsh hangs off the Dark Wood (q-a1-vis-links): put the
    // player there.
    app_support::with(&server, |l| {
        let (p, _) = single_player::local_player(&l.host().game).unwrap();
        l.host_mut()
            .game
            .events
            .action
            .hooks()
            .act_changes
            .push((p, tower::BLACK_MARSH, 0));
    });
    while server_level(&server) != Some(tower::BLACK_MARSH) {
        step(&mut app);
    }
    for _ in 0..30 {
        step(&mut app);
    }
    // The tome (a quest object, operate function 6) opens the quest.
    let tome = find(&app, OBJECT, tower::TOME_CLASS).expect("the Moldy Tome in the model");
    interact(&mut app, tome);
    for _ in 0..60 {
        step(&mut app);
    }
    assert_eq!(chain5(&server).0, 2, "the tome sets state 2");
    // The Tower, then the cellars down to Cellar 5.
    take(&mut app, tower::MARSH_TO_TOWER, tower::FORGOTTEN_TOWER);
    for i in 0..tower::TOWER_LEVELS.len() - 1 {
        take(&mut app, tower::down(i), tower::TOWER_LEVELS[i + 1]);
    }
    assert_eq!(chain5(&server).0, 3, "Tower Cellar 5: state 3");
    // The Countess: monster init links her to chain 5; her death reaches
    // the quest control.
    app_support::with(&server, |l| {
        let g = &mut l.host_mut().game;
        let (p, _) = single_player::local_player(g).unwrap();
        let (room, pos) = {
            let e = g.game.lists.unit(p).unwrap();
            (e.room().unwrap(), g.events.action.hooks().path_position(p))
        };
        let req = d2_sim::units::lifecycle::AllocRequest {
            ty: UnitType::Monster,
            class: tower::COUNTESS_CLASS,
            room: Some(room),
            add: true,
            fixed_guid: None,
            mode: 1,
            allied: false,
        };
        let countess: UnitId = g
            .events
            .action
            .with(&mut g.game, |gm, v| v.allocate(gm, &req, pos.0 + 4, pos.1))
            .expect("the Countess");
        let x = &mut g.events.action.hooks().x;
        x.monster_quest_chain(countess, tower::TOWER_CHAIN);
        x.kill_step(&mut g.game, KillStep::QuestKill, countess, p);
    });
    for _ in 0..30 {
        step(&mut app);
    }
    assert_eq!(chain5(&server).0, 5, "the Countess's death: state 5");
    let b = &app.world().resource::<BridgeResource>().0;
    assert!(b.log().rejected.is_empty(), "{:?}", b.log().rejected);
}
