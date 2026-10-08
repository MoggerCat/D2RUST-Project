// Spec: specs/world/quests-act2.md (§8.4, §8.8, §8.11), specs/world/quests.md (§8.2), specs/sim/path-placement.md (§12.2)
//! Duriel's Lair in the app's own synthetic game, headless (task
//! `q-a2-duriel`, `docs/handoff/q-a2-duriel.md`): the tomb holding the
//! orifice has the orifice and a way into the Lair that stays closed
//! until the lair is open (the quest gate, `Pending::warp_quest_gate`);
//! then the warp leads into level 73, and Duriel's death moves chain 13
//! (state 3, killed). The staff hand-in itself is the `d2-server`
//! `quests_act2` test (the synthetic game has no item tables for `hst `).
//! PROVISIONAL (REC-167): d2rs-own, unverified.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{add_client_data, add_game, add_walk, predict_link, send_create_game};
use d2_client::app::single_player::{self, GameData, ACT2_TOWN};
use d2_client::app::synthetic_act2 as a2;
use d2_client::bridge::predict::Speeds;
use d2_client::bridge::world::{OBJECT, TILE};
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

/// Duriel's `monstats` class.
const DURIEL: u32 = 211;

fn server_level(server: &Server) -> Option<u32> {
    app_support::with(server, |l| {
        let (p, _) = single_player::local_player(&l.host().game)?;
        let g = &mut l.host_mut().game;
        let room = g.game.lists.unit(p)?.room()?;
        g.events.action.hooks().drlg.level_id(&g.game, room)
    })
}

/// (state, Duriel killed) of chain 13.
fn chain13(server: &Server) -> (u8, bool) {
    app_support::with(server, |l| {
        let r = l.host().game.world.quests.record(13).expect("chain 13");
        (r.state, r.extra.a2.q6.duriel_killed)
    })
}

// Covers: specs/world/quests-act2.md §8.4; specs/world/quests-act2.md §8.8; specs/world/quests.md §8.2; specs/world/quests-act2.md §8.11
#[test]
fn the_lair_opens_from_the_staff_tomb_and_duriels_death_moves_chain_13() {
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
        assert!(steps.get() < 60000, "the warps happen");
    };
    while app_support::local_player(&server).is_none() {
        step(&mut app);
    }
    for _ in 0..30 {
        step(&mut app);
    }
    // Act II by the act-change queue (as `app_act_travel`).
    app_support::with(&server, |l| {
        let (p, _) = single_player::local_player(&l.host().game).unwrap();
        let g = &mut l.host_mut().game;
        g.events.action.hooks().act_changes.push((p, ACT2_TOWN, 0));
    });
    let wp = app
        .world()
        .resource::<BridgeResource>()
        .0
        .world()
        .units
        .iter()
        .find(|(k, _)| k.unit_type == 2)
        .map(|(k, _)| *k)
        .expect("the town waypoint");
    app_support::with(&server, |l| {
        let (p, _) = single_player::local_player(&l.host().game).unwrap();
        let g = &mut l.host_mut().game;
        let wp = g.events.action.hooks().waypoints.entry(p).or_default();
        wp.get_mut(0).set(9).unwrap();
    });
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .interact(wp)
        .unwrap();
    for _ in 0..10 {
        step(&mut app);
    }
    while server_level(&server) != Some(ACT2_TOWN) {
        step(&mut app);
    }
    for _ in 0..30 {
        step(&mut app);
    }
    let take_no_wait = |app: &mut App, class: u32| {
        let t = app
            .world()
            .resource::<BridgeResource>()
            .0
            .world()
            .units
            .iter()
            .find(|(k, u)| k.unit_type == TILE && u.class == class)
            .map(|(k, _)| *k)
            .unwrap_or_else(|| panic!("tile {class} in the model"));
        app.world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .interact(t)
            .unwrap();
    };
    let take = |app: &mut App, class: u32, to: u32| {
        take_no_wait(app, class);
        while server_level(&server) != Some(to) {
            step(app);
        }
        for _ in 0..30 {
            step(app);
        }
    };
    // Lut Gholein -> the Canyon -> the tomb holding the orifice.
    let edges = a2::edges();
    let staff = app_support::with(&server, |l| {
        let g = &mut l.host_mut().game;
        g.events.action.hooks().x.staff_tomb
    });
    assert!(a2::is_tomb(staff));
    let into = |to: u32| edges.iter().find(|e| e.to == to).unwrap().to_class;
    take(&mut app, into(a2::CANYON), a2::CANYON);
    take(&mut app, into(staff), staff);
    // The orifice stands there (a quest object, operate 25).
    let orifice = app
        .world()
        .resource::<BridgeResource>()
        .0
        .world()
        .units
        .iter()
        .any(|(k, u)| k.unit_type == OBJECT && u.class == a2::ORIFICE_CLASS);
    assert!(orifice, "the orifice in the staff tomb");
    // The lair is closed (a not-intro game, lair not open): the tile
    // leaves the player where he is.
    let lair_tile = edges
        .iter()
        .find(|e| e.from == staff && e.to == a2::DURIELS_LAIR)
        .unwrap()
        .to_class;
    app_support::with(&server, |l| {
        l.host_mut()
            .game
            .world
            .quests
            .record_mut(13)
            .unwrap()
            .not_intro = true;
    });
    for _ in 0..30 {
        step(&mut app);
    }
    take_no_wait(&mut app, lair_tile);
    for _ in 0..200 {
        step(&mut app);
    }
    assert_eq!(server_level(&server), Some(staff), "the gate is closed");
    // The staff is handed in (the d2-server test): the lair is open.
    app_support::with(&server, |l| {
        l.host_mut()
            .game
            .world
            .quests
            .record_mut(13)
            .unwrap()
            .extra
            .a2
            .q6
            .lair_open = true;
    });
    for _ in 0..30 {
        step(&mut app);
    }
    take_no_wait(&mut app, lair_tile);
    for _ in 0..300 {
        step(&mut app);
    }
    assert_eq!(server_level(&server), Some(a2::DURIELS_LAIR));
    // Duriel: monster init links him to chain 13; his death reaches the
    // quest control.
    assert!(!chain13(&server).1);
    app_support::with(&server, |l| {
        let g = &mut l.host_mut().game;
        let (p, _) = single_player::local_player(g).unwrap();
        let (room, pos) = {
            let e = g.game.lists.unit(p).unwrap();
            (e.room().unwrap(), g.events.action.hooks().path_position(p))
        };
        let req = d2_sim::units::lifecycle::AllocRequest {
            ty: UnitType::Monster,
            class: DURIEL,
            room: Some(room),
            add: true,
            fixed_guid: None,
            mode: 1,
            allied: false,
        };
        let duriel: UnitId = g
            .events
            .action
            .with(&mut g.game, |gm, v| v.allocate(gm, &req, pos.0 + 4, pos.1))
            .expect("Duriel");
        g.events
            .action
            .hooks()
            .x
            .kill_step(&mut g.game, KillStep::QuestKill, duriel, p);
    });
    for _ in 0..30 {
        step(&mut app);
    }
    assert_eq!(chain13(&server), (3, true), "Duriel's death: state 3");
    let b = &app.world().resource::<BridgeResource>().0;
    assert!(b.log().rejected.is_empty(), "{:?}", b.log().rejected);
}
