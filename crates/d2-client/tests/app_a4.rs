// Spec: specs/world/npc.md §3, §8; specs/world/quests-act4.md §3 (A4Q1); specs/world/quests.md §4.4–§4.6
//! (q-a4) Act IV in the app's synthetic game, headless: after the act
//! change the Pandemonium Fortress's NPCs (Tyrael, Jamella, Halbu, Cain)
//! and waypoint are in the client's model and each NPC answers a click
//! with a dialog (S→C 0x28); the warp line leads to the Plains of
//! Despair, where Izual (linked to chain 22 by the host) dies and moves
//! A4Q1 to state 4. PROVISIONAL (REC-143): made-up levels and places.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{
    add_client_data, add_game, add_walk, predict_link, send_create_game_for,
};
use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, GameData};
use d2_client::app::synthetic_act4 as a4;
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::predict::Speeds;
use d2_client::bridge::world::{UnitKey, TILE};
use d2_client::bridge::BridgeResource;
use d2_client::ui::panels::npc::msg_chat_end;
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

struct Rig {
    app: App,
    server: Server,
    ms: Arc<AtomicU32>,
    seen: Arc<Mutex<Vec<u8>>>,
    steps: u32,
}

impl Rig {
    /// A fresh game with the player moved to the Fortress by the act change.
    fn new() -> Rig {
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
        let server: Server = Arc::new(Mutex::new(link));
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
        let mut rig = Rig {
            app,
            server,
            ms,
            seen,
            steps: 0,
        };
        while app_support::local_player(&rig.server).is_none() {
            rig.step(1);
        }
        rig.step(30);
        // Tyrael's act change (the hooks queue, as Warriv's in q-a2-town).
        app_support::with(&rig.server, |l| {
            let (p, _) = single_player::local_player(&l.host().game).unwrap();
            l.host_mut()
                .game
                .events
                .action
                .hooks()
                .act_changes
                .push((p, a4::FORTRESS, 0));
        });
        rig.wait_level(a4::FORTRESS);
        rig
    }

    fn step(&mut self, n: usize) {
        for _ in 0..n {
            self.ms.fetch_add(40, Ordering::SeqCst);
            self.app.update();
            self.steps += 1;
            assert!(self.steps < 20000, "the test finishes");
        }
    }

    fn level(&self) -> Option<u32> {
        app_support::with(&self.server, |l| {
            let (p, _) = single_player::local_player(&l.host().game)?;
            let g = &mut l.host_mut().game;
            let room = g.game.lists.unit(p)?.room()?;
            g.events.action.hooks().drlg.level_id(&g.game, room)
        })
    }

    fn wait_level(&mut self, to: u32) {
        while self.level() != Some(to) {
            self.step(1);
        }
        self.step(30);
    }

    fn units(&self) -> Vec<(UnitKey, u32)> {
        self.app
            .world()
            .resource::<BridgeResource>()
            .0
            .world()
            .units
            .iter()
            .map(|(k, u)| (*k, u.class))
            .collect()
    }

    fn find(&self, ty: u8, class: u32) -> Option<UnitKey> {
        self.units()
            .into_iter()
            .find(|(k, c)| k.unit_type == ty && *c == class)
            .map(|(k, _)| k)
    }

    fn interact(&mut self, key: UnitKey) {
        self.app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .interact(key)
            .unwrap();
    }

    fn chain22(&self) -> u8 {
        app_support::with(&self.server, |l| {
            l.host()
                .game
                .world
                .quests
                .record(22)
                .expect("chain 22")
                .state
        })
    }

    fn assert_clean(&self) {
        let b = &self.app.world().resource::<BridgeResource>().0;
        assert!(b.log().rejected.is_empty(), "{:?}", b.log().rejected);
    }
}

// Covers: specs/world/npc.md §3, §8.3; specs/world/waypoints.md §7 r5
#[test]
fn the_fortress_npcs_and_waypoint_are_there_and_talk() {
    for which in 0..a4::NPCS.len() {
        let mut rig = Rig::new();
        assert!(
            rig.units().iter().any(|(k, _)| k.unit_type == 2),
            "the Act IV waypoint is in the model: {:?}",
            rig.units()
        );
        let class = a4::NPCS[which];
        let key = rig
            .find(1, u32::from(class))
            .unwrap_or_else(|| panic!("NPC {class} is in the model: {:?}", rig.units()));
        // Stand beside the NPC (the server starts a talk only within a few
        // sub-tiles).
        app_support::with(&rig.server, move |l| {
            let (p, _) = single_player::local_player(&l.host().game).unwrap();
            let g = &mut l.host_mut().game;
            let room = g.game.lists.unit(p).and_then(|u| u.room());
            let x = a4::NPC_X0 + a4::NPC_STEP * which as i32;
            let y = a4::NPC_Y + 3;
            g.events
                .action
                .with(&mut g.game, |g, v| v.path_teleport(g, p, room, x, y));
        });
        rig.step(3);
        rig.interact(key);
        let before = rig
            .seen
            .lock()
            .unwrap()
            .iter()
            .filter(|&&i| i == 0x28)
            .count();
        let mut talked = false;
        for _ in 0..100 {
            rig.step(1);
            talked = rig
                .seen
                .lock()
                .unwrap()
                .iter()
                .filter(|&&i| i == 0x28)
                .count()
                > before;
            if talked {
                break;
            }
        }
        assert!(talked, "no dialog from NPC {class}");
        rig.app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .send_bytes(&msg_chat_end(key.guid))
            .unwrap();
        rig.step(60);
        rig.assert_clean();
    }
}

// Covers: specs/world/quests-act4.md §3.5; specs/world/quests.md §4.4; specs/world/quests.md §4.5
#[test]
fn the_warp_line_leads_to_izual_whose_death_moves_chain_22() {
    let mut rig = Rig::new();
    assert_eq!(rig.chain22(), 1, "A4Q1 starts at state 1");
    for i in 0..2 {
        let t = rig
            .find(TILE, a4::on(i))
            .unwrap_or_else(|| panic!("tile {} in the model", a4::on(i)));
        rig.interact(t);
        rig.wait_level(a4::LEVELS[i + 1]);
    }
    assert_eq!(rig.level(), Some(a4::PLAINS_OF_DESPAIR));
    let izual = rig
        .find(1, a4::IZUAL)
        .expect("Izual in the Plains of Despair");
    let guid = izual.guid;
    app_support::with(&rig.server, move |l| {
        let g = &mut l.host_mut().game;
        let (p, _) = single_player::local_player(g).unwrap();
        let unit: UnitId = g
            .game
            .lists
            .units_of_type(UnitType::Monster)
            .into_iter()
            .find(|&u| g.game.lists.unit(u).is_some_and(|e| e.guid == guid))
            .expect("Izual on the server");
        g.events
            .action
            .hooks()
            .x
            .kill_step(&mut g.game, KillStep::QuestKill, unit, p);
    });
    rig.step(30);
    assert_eq!(rig.chain22(), 4, "Izual's death: state 4");
    rig.assert_clean();
}
