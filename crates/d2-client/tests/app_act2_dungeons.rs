// Spec: specs/drlg/maze.md (§4, §5.2, §5.4, §5.5, §6.2, §6.3, §9), specs/drlg/levels.md (§3, §5), specs/sim/path-placement.md (§12.2), specs/world/quests-act2.md (§8.1)
//! The Act 2 dungeons in the app's own synthetic game, headless (task
//! `q-a2-dungeons`, `docs/handoff/q-a2-dungeons.md`): from Lut Gholein
//! every line of dungeons (sewers, Halls of the Dead, Claw Viper Temple,
//! Maggot Lair, Stony Tomb, Arcane Sanctuary, and through the Canyon
//! stand-in the seven Tal Rasha tombs) is entered by its warp tile, built
//! by the real maze generator, and left again by its way back. The
//! staff-tomb (true tomb) level the Act II DRLG chose is a tomb, is the
//! largest of them, and is the one the object seam reports.
//! PROVISIONAL: where the lines start (d2rs-own, unverified).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{add_client_data, add_game, add_walk, predict_link, send_create_game};
use d2_client::app::single_player::{self, GameData, ACT2_TOWN};
use d2_client::app::synthetic_act2 as a2;
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

/// (staff tomb, boss tomb, seam value, rooms of each tomb level).
fn tombs(server: &Server) -> (u32, u32, u32, Vec<usize>) {
    app_support::with(server, |l| {
        let g = &mut l.host_mut().game;
        let hooks = g.events.action.hooks();
        let seam = hooks.x.staff_tomb;
        let d = hooks.drlg.dungeon.acts[1].as_ref().expect("act 1");
        let rooms = (a2::FIRST_TOMB..a2::FIRST_TOMB + 7)
            .map(|id| d.find_level(id).map_or(0, |i| d.level_rooms(i).len()))
            .collect();
        (d.staff_tomb, d.boss_tomb, seam, rooms)
    })
}

// Covers: specs/drlg/maze.md §9 r1; specs/drlg/maze.md §6.3; specs/drlg/maze.md §5.2; specs/drlg/levels.md §3; specs/world/quests-act2.md §8.1
#[test]
fn every_act2_dungeon_is_entered_built_and_left() {
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
    let take = |app: &mut App, class: u32, to: u32| {
        let t = app
            .world()
            .resource::<BridgeResource>()
            .0
            .world()
            .units
            .iter()
            .find(|(k, u)| k.unit_type == TILE && u.class == class)
            .map(|(k, _)| *k)
            .unwrap_or_else(|| panic!("tile {class} (to level {to}) in the model"));
        let sent = app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .interact(t)
            .unwrap();
        assert!(sent.is_empty());
        while server_level(&server) != Some(to) {
            step(app);
        }
        for _ in 0..30 {
            step(app);
        }
        let level = app
            .world()
            .resource::<BridgeResource>()
            .0
            .world()
            .player_level();
        assert_eq!(level, Some(to as u16), "the client follows to {to}");
    };
    // The true tomb the Act II DRLG chose: a tomb, not the boss tomb, and
    // the seam the objects read has it.
    let (staff, boss, seam, _) = tombs(&server);
    assert!(a2::is_tomb(staff) && a2::is_tomb(boss) && staff != boss);
    assert_eq!(seam, staff, "Pending::object_staff_tomb");
    // Down every line and back: each entrance from its parent, each way
    // on, then every way back.
    let edges = a2::edges();
    let mut visited = Vec::new();
    for e in edges.iter().filter(|e| e.from == ACT2_TOWN || e.from == a2::CANYON) {
        // The Canyon is entered first so the tombs' parent exists.
        if e.from == a2::CANYON && server_level(&server) != Some(a2::CANYON) {
            take(&mut app, edges_to(&edges, a2::CANYON).0, a2::CANYON);
            if e.from == a2::CANYON {
                // Fall through: we are in the Canyon now.
            }
        }
        if e.from == ACT2_TOWN && server_level(&server) != Some(ACT2_TOWN) {
            take(&mut app, edges_to(&edges, a2::CANYON).1, ACT2_TOWN);
        }
        take(&mut app, e.to_class, e.to);
        let mut here = *e;
        loop {
            visited.push(here.to);
            let Some(next) = edges.iter().find(|n| n.from == here.to) else {
                break;
            };
            take(&mut app, next.to_class, next.to);
            here = *next;
        }
        // Back up the line to its parent.
        let mut up = here;
        loop {
            take(&mut app, up.back_class, up.from);
            if up.from == e.from {
                break;
            }
            up = *edges.iter().find(|n| n.to == up.from).unwrap();
        }
    }
    for id in a2::dungeon_levels() {
        assert!(visited.contains(&id), "level {id} was visited");
    }
    // The special generation: the staff tomb is the largest, the boss tomb
    // next, the rest the base size (`maze.md` §5.2).
    let (staff, boss, _, rooms) = tombs(&server);
    let at = |id: u32| rooms[(id - a2::FIRST_TOMB) as usize];
    for id in a2::FIRST_TOMB..a2::FIRST_TOMB + 7 {
        if id != staff && id != boss {
            assert!(at(staff) > at(id) && at(boss) > at(id), "{rooms:?}");
        }
    }
    assert!(at(staff) > at(boss), "{rooms:?}");
    let b = &app.world().resource::<BridgeResource>().0;
    assert!(b.log().rejected.is_empty(), "{:?}", b.log().rejected);
}

/// (to class, back class) of the pair into `level`.
fn edges_to(edges: &[a2::Edge], level: u32) -> (u32, u32) {
    let e = edges.iter().find(|e| e.to == level).unwrap();
    (e.to_class, e.back_class)
}
