// Spec: specs/world/quests-act4.md §4 (A4Q3), §5 (A4Q2), §8; specs/world/quests.md §4.4–§4.6, §9
//! (q-a4-endgame) Act IV's endgame in the app's synthetic game, headless,
//! on the Chaos Sanctuary and the Fortress: the five seals open, the
//! seal bosses spawn from the dummy objects and die, the Sanctum clears
//! and Diablo spawns at the start point and dies (chain 23); the
//! Hellforge answers (chain 24) and Hephasto's death reaches it; the
//! portal to Harrogath changes the act. PROVISIONAL (REC-162): every
//! place, the plain-monster superunique spawn and the hand-spawned
//! bosses are `d2rs-own, unverified`.

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
use d2_client::bridge::world::{UnitKey, OBJECT, TILE};
use d2_client::bridge::BridgeResource;
use d2_server::seams::Clock;
use d2_sim::missiles::seams::MissileBodies;
use d2_sim::units::lifecycle::AllocRequest;
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
    #[allow(dead_code)]
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
}

impl Rig {
    fn assert_clean(&self) {
        let b = &self.app.world().resource::<BridgeResource>().0;
        assert!(b.log().rejected.is_empty(), "{:?}", b.log().rejected);
    }

    /// Walks the warp line from the Fortress to `to`.
    fn walk_to(&mut self, to: u32) {
        while self.level() != Some(to) {
            let i = a4::index(self.level().expect("a level")).expect("on the line");
            let t = self
                .find(TILE, a4::on(i))
                .unwrap_or_else(|| panic!("tile {} in the model", a4::on(i)));
            self.interact(t);
            self.wait_level(a4::LEVELS[i + 1]);
        }
    }

    /// An object of `class` at sub-tile offset (dx, dy) from the player's
    /// room origin (`origin`), allocated by hand.
    fn place(&mut self, class: u32, at: (i32, i32)) {
        app_support::with(&self.server, move |l| {
            let g = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(g).unwrap();
            let room = g.game.lists.unit(p).and_then(|u| u.room()).unwrap();
            let req = AllocRequest {
                ty: UnitType::Object,
                class,
                room: Some(room),
                add: true,
                fixed_guid: None,
                mode: 0,
                allied: false,
            };
            g.events
                .action
                .with(&mut g.game, |g, v| v.allocate(g, &req, at.0, at.1))
                .unwrap_or_else(|| panic!("object {class}"));
        });
    }

    /// Stands the player next to (x, y) of the current room.
    fn stand_at(&mut self, x: i32, y: i32) {
        app_support::with(&self.server, move |l| {
            let g = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(g).unwrap();
            let room = g.game.lists.unit(p).and_then(|u| u.room());
            g.events
                .action
                .with(&mut g.game, |g, v| v.path_teleport(g, p, room, x, y));
        });
        self.step(5);
    }

    /// Clicks the object of `class` after standing beside it.
    fn operate(&mut self, class: u32, at: (i32, i32)) {
        self.stand_at(at.0 + 2, at.1 + 3);
        let key = self
            .find(OBJECT, class)
            .unwrap_or_else(|| panic!("object {class} in the model: {:?}", self.units()));
        self.interact(key);
        self.step(10);
    }

    /// The quest-parse kill of every living monster of `class`.
    fn kill_class(&mut self, class: u32) -> usize {
        app_support::with(&self.server, move |l| {
            let g = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(g).unwrap();
            let victims: Vec<UnitId> = g
                .game
                .lists
                .units_of_type(UnitType::Monster)
                .into_iter()
                .filter(|&u| {
                    g.events
                        .action
                        .with(&mut g.game, |_, v| v.units.get(u).map(|r| r.class))
                        == Some(class)
                })
                .collect();
            for &u in &victims {
                g.events
                    .action
                    .hooks()
                    .x
                    .kill_step(&mut g.game, KillStep::QuestKill, u, p);
            }
            victims.len()
        })
    }

    fn q2(&self) -> d2_sim::world::quests::act4::q2::Extra {
        app_support::with(&self.server, |l| {
            l.host()
                .game
                .world
                .quests
                .record(23)
                .expect("chain 23")
                .extra
                .a4
                .q2
                .clone()
        })
    }

    fn monsters_of(&self, class: u32) -> usize {
        app_support::with(&self.server, move |l| {
            let g = &mut l.host_mut().game;
            g.game
                .lists
                .units_of_type(UnitType::Monster)
                .into_iter()
                .filter(|&u| {
                    g.events
                        .action
                        .with(&mut g.game, |_, v| v.units.get(u).map(|r| r.class))
                        == Some(class)
                })
                .count()
        })
    }
}

/// Sub-tile origin of the Chaos Sanctuary's room (`single_player.rs`
/// `synthetic_types`: tile (40, 0)).
const OX: i32 = 40 * 5;
const OY: i32 = 0;
/// The five seals and the start point (sub-tiles from the room origin).
const SEALS: [(u32, (i32, i32)); 5] = [
    (393, (150, 150)),
    (395, (150, 100)),
    (392, (100, 100)),
    (394, (100, 60)),
    (396, (60, 100)),
];
const START: (i32, i32) = (150, 40);

// Covers: specs/world/quests-act4.md §5.4; specs/world/quests-act4.md §5.5; specs/world/quests-act4.md §5.6; specs/world/quests-act4.md §5.7
#[test]
fn the_seals_the_seal_bosses_and_diablo_run_chain_23() {
    let mut rig = Rig::new();
    rig.walk_to(a4::CHAOS_SANCTUARY);
    rig.place(a4::START_POINT, (OX + START.0, OY + START.1));
    rig.step(5);
    assert!(
        rig.q2().start_known,
        "the start point initialised (init 55)"
    );
    for (class, (x, y)) in SEALS {
        rig.place(class, (OX + x, OY + y));
    }
    rig.step(5);
    for (class, (x, y)) in SEALS {
        rig.operate(class, (OX + x, OY + y));
    }
    assert_eq!(rig.q2().seals, [true; 5], "all five seals open");
    // The dummies spawn the three bosses 27 frames on (rule 7).
    rig.step(60);
    for boss in [36, 37, 38] {
        assert_eq!(rig.monsters_of(boss), 1, "seal boss {boss} stands");
    }
    for boss in [36, 37, 38] {
        assert_eq!(rig.kill_class(boss), 1);
    }
    rig.step(10);
    assert_eq!(rig.q2().kills, 3, "three seal-boss kills");
    assert!(rig.q2().cleared, "the Sanctum cleared");
    // The spawn timer: Diablo within 400 frames at the start point.
    for _ in 0..40 {
        if rig.monsters_of(a4::DIABLO) > 0 {
            break;
        }
        rig.step(20);
    }
    assert_eq!(rig.monsters_of(a4::DIABLO), 1, "Diablo spawned");
    assert!(rig.q2().spawned);
    assert_eq!(rig.kill_class(a4::DIABLO), 1);
    rig.step(30);
    assert!(rig.q2().killed, "Diablo's death reached chain 23");
    rig.assert_clean();
}

/// Sub-tile origin of the River of Flame's room (tile (32, 0)).
const RIVER_OX: i32 = 32 * 5;

impl Rig {
    /// A monster of `class` at (x, y) of the player's room, by hand.
    fn place_monster(&mut self, class: u32, at: (i32, i32)) {
        app_support::with(&self.server, move |l| {
            let g = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(g).unwrap();
            let room = g.game.lists.unit(p).and_then(|u| u.room()).unwrap();
            let req = AllocRequest {
                ty: UnitType::Monster,
                class,
                room: Some(room),
                add: true,
                fixed_guid: None,
                mode: 1,
                allied: false,
            };
            g.events
                .action
                .with(&mut g.game, |g, v| v.allocate(g, &req, at.0, at.1))
                .unwrap_or_else(|| panic!("monster {class}"));
        });
    }

    fn chain24_state(&self) -> u8 {
        app_support::with(&self.server, |l| {
            l.host()
                .game
                .world
                .quests
                .record(24)
                .expect("chain 24")
                .state
        })
    }

    fn rest_log(&self) -> Vec<String> {
        app_support::with(&self.server, |l| l.host().game.world.rest.log.clone())
    }
}

// Covers: specs/world/quests-act4.md §4.6; specs/world/quests-act4.md §4.8; specs/world/quests-act4.md §8
#[test]
fn the_hellforge_answers_and_hephastos_death_reaches_chain_24() {
    let mut rig = Rig::new();
    rig.walk_to(a4::RIVER_OF_FLAME);
    let at = (RIVER_OX + 20, 20);
    rig.place(a4::HELLFORGE, at);
    rig.step(5);
    let before = rig.chain24_state();
    // No soulstone: the forge refuses and the quest moves to state 1 (§4.6).
    rig.operate(a4::HELLFORGE, at);
    assert_eq!(rig.chain24_state(), 1, "operate 49 ran (was {before})");
    // Hephasto is linked to chain 24 by class; his kill drops the hammer.
    rig.place_monster(a4::HEPHASTO, (RIVER_OX + 30, 30));
    rig.step(5);
    assert_eq!(rig.kill_class(a4::HEPHASTO), 1);
    rig.step(10);
    assert!(
        rig.rest_log()
            .iter()
            .any(|l| l.starts_with("drop item") && l.contains("[104, 102, 104, 32]")),
        "the hammer drop was asked for: {:?}",
        rig.rest_log()
    );
    rig.assert_clean();
}

// Covers: specs/world/quests-act4.md §5.9
#[test]
fn the_portal_to_harrogath_refuses_then_asks_for_the_act_change() {
    let mut rig = Rig::new();
    let at = (30, 20);
    rig.place(a4::HARROGATH_PORTAL, at);
    rig.step(5);
    // Without the quest done the portal refuses with sound 19.
    rig.operate(a4::HARROGATH_PORTAL, at);
    let log = rig.rest_log();
    assert!(
        log.iter()
            .any(|l| l.starts_with("sound") && l.ends_with(" 19")),
        "{log:?}"
    );
    assert!(!log.iter().any(|l| l.starts_with("byte 4c")), "{log:?}");
    // With 26.13 (edge case 14) it takes the player's interaction away
    // and asks for the act change to level 109 (the host runs it when
    // the call returns), then the waypoint (`0x005B4FF0`, reported).
    app_support::with(&rig.server, |l| {
        let g = &mut l.host_mut().game;
        let (p, _) = single_player::local_player(g).unwrap();
        g.world.rest.quests.get_mut(&p).unwrap().flags[0].set(26, 13);
    });
    rig.operate(a4::HARROGATH_PORTAL, at);
    let log = rig.rest_log();
    assert!(log.iter().any(|l| l.starts_with("byte 4c")), "{log:?}");
    assert!(log.iter().any(|l| l.contains("0x5b4ff0")), "{log:?}");
    let queued = app_support::with(&rig.server, |l| {
        l.host_mut().game.events.action.hooks().act_changes.len()
    });
    assert_eq!(queued, 0, "the host ran the act change");
    rig.assert_clean();
}
