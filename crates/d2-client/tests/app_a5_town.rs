// Spec: specs/world/npc.md §3 and §8; specs/ui/menus.md §2; specs/world/waypoints.md §7
//! (q-a5-town) Harrogath in the app's synthetic game, headless: after the
//! act change the town's NPCs (Larzuk, Anya, Malah, Nihlathak,
//! Qual-Kehk, Cain) and the waypoint are in the client's model, and
//! interacting with each NPC (C→S 0x13) brings its dialog (S→C 0x28).
//! PROVISIONAL (REC-144): the NPC placement, the preview's town layout.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{
    add_client_data, add_game, add_walk, predict_link, send_create_game_for,
};
use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, GameData};
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::predict::Speeds;
use d2_client::bridge::world::UnitKey;
use d2_client::bridge::BridgeResource;
use d2_client::ui::panels::npc::msg_chat_end;
use d2_server::seams::Clock;
use d2_sim::missiles::seams::MissileBodies;

mod app_support;
use app_support::SharedLink;

/// The host clock, advanced by the test.
struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

/// Wraps the shared link and notes the id of every S→C message.
struct Tap {
    inner: SharedLink<ThreadLink<d2_client::app::single_player::Link<StepClock>>>,
    seen: Arc<Mutex<Vec<u8>>>,
}

impl ServerLink for Tap {
    fn protocol_version(&self) -> u32 {
        self.inner.protocol_version()
    }
    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        self.inner.send(queue, msg)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        self.inner.pump()
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        let v = self.inner.receive();
        self.seen
            .lock()
            .unwrap()
            .extend(v.iter().filter_map(|m| m.first().copied()));
        v
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

// Covers: specs/world/npc.md §3, §8.3; specs/world/waypoints.md §7 r5
#[test]
fn harrogaths_npcs_and_waypoint_are_there_and_talk() {
    for which in 0..single_player::ACT5_NPCS.len() {
        session(which);
    }
}

/// A fresh game, the act change to Harrogath, then NPC number `which`
/// (the second chat of one game does not start without the UI's 0x31
/// before the close, so each NPC gets its own game).
fn session(which: usize) {
    let data = GameData::Synthetic;
    let ms = Arc::new(AtomicU32::new(1000));
    let character = single_player::new_character("sorceress", "Test").unwrap();
    let (link, _) = single_player::start_with(
        data.clone(),
        single_player::DEFAULT_SEED,
        character.clone(),
        StepClock(ms.clone()),
    )
    .unwrap();
    let server = Arc::new(Mutex::new(link));
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_resource::<ButtonInput<MouseButton>>();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let (link, tap) = predict_link(Box::new(Tap {
        inner: SharedLink(server.clone()),
        seen: seen.clone(),
    }));
    add_game(&mut app, link, false).unwrap();
    add_walk(&mut app, tap, Some(Speeds { walk: 6, run: 9 }));
    send_create_game_for(&mut app, &character).unwrap();
    add_client_data(
        &mut app,
        single_player::client_drlg_source(&data),
        single_player::client_level_rows(&data),
    );
    app_support::synthetic_skill_rows(&mut app);
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .set_unit_rows(single_player::synthetic_unit_rows());
    app.update();
    let mut steps = 0;
    let mut step = |app: &mut App, n: usize| {
        for _ in 0..n {
            ms.fetch_add(40, Ordering::SeqCst);
            app.update();
            steps += 1;
            assert!(steps < 6000, "the test finishes");
        }
    };
    while app_support::local_player(&server).is_none() {
        step(&mut app, 1);
    }
    step(&mut app, 30);
    // Warriv's "Go East": the act change through the hooks queue.
    app_support::with(&server, |l| {
        let (p, _) = single_player::local_player(&l.host().game).unwrap();
        l.host_mut()
            .game
            .events
            .action
            .hooks()
            .act_changes
            .push((p, single_player::ACT5_TOWN, 0));
    });
    while server_level(&server) != Some(single_player::ACT5_TOWN) {
        step(&mut app, 1);
    }
    step(&mut app, 30);
    let units: Vec<(UnitKey, u32)> = app
        .world()
        .resource::<BridgeResource>()
        .0
        .world()
        .units
        .iter()
        .map(|(k, u)| (*k, u.class))
        .collect();
    assert!(
        units.iter().any(|(k, _)| k.unit_type == 2),
        "the Act V waypoint is in the model: {units:?}"
    );
    let mut failed = Vec::new();
    for (idx, class) in single_player::ACT5_NPCS
        .into_iter()
        .enumerate()
        .filter(|(i, _)| *i == which)
    {
        let Some(&(key, _)) = units
            .iter()
            .find(|(k, c)| k.unit_type == 1 && *c == u32::from(class))
        else {
            panic!("NPC {class} is in the model: {units:?}");
        };
        // Stand beside the NPC (the synthetic room has no walkable path
        // across it; the server starts a talk only within a few sub-tiles).
        app_support::with(&server, move |l| {
            let (p, _) = single_player::local_player(&l.host().game).unwrap();
            let g = &mut l.host_mut().game;
            let room = g.game.lists.unit(p).and_then(|u| u.room());
            let (x, y) = (
                single_player::ACT5_NPC_X0 + 4 * idx as i32,
                single_player::ACT5_NPC_Y + 3,
            );
            g.events
                .action
                .with(&mut g.game, |g, v| v.path_teleport(g, p, room, x, y));
        });
        step(&mut app, 3);
        app.world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .interact(key)
            .unwrap();
        let before = seen.lock().unwrap().iter().filter(|&&i| i == 0x28).count();
        let mut talked = false;
        for _ in 0..100 {
            step(&mut app, 1);
            talked = seen.lock().unwrap().iter().filter(|&&i| i == 0x28).count() > before;
            if talked {
                break;
            }
        }
        if !talked {
            failed.push(class);
        }
        // Leave the chat (C→S 0x30), as the UI does.
        app.world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .send_bytes(&msg_chat_end(key.guid))
            .unwrap();
        step(&mut app, 60);
    }
    assert!(failed.is_empty(), "no dialog from {failed:?}");
    let b = &app.world().resource::<BridgeResource>().0;
    assert!(b.log().rejected.is_empty(), "{:?}", b.log().rejected);
}
