// Spec: specs/world/quests-act5-2.md §7.5, §7.8; specs/world/quests.md §8.2
//! (q-act3-act5-gaps) The Arreat Summit in the app's synthetic game,
//! headless: arriving there creates the Ancient statues and the altar
//! (preset objects), the quest control's inits register them, a click on the
//! altar runs its operate on the quest control, and the summit's exits to
//! the Ancients' Way and the Worldstone Keep stay closed until the Ancients
//! are defeated (`0x0058D090`). PROVISIONAL (REC-246): the objects' places
//! and the chain's exits are made up (`// d2rs-own, unverified`).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{
    add_client_data, add_game, add_walk, predict_link, send_create_game_for,
};
use d2_client::app::single_player::{self, GameData};
use d2_client::app::synthetic_act5 as a5;
use d2_client::app::synthetic_chains;
use d2_client::bridge::predict::Speeds;
use d2_client::bridge::world::{UnitKey, OBJECT, TILE};
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

struct Rig {
    app: App,
    server: Server,
    ms: Arc<AtomicU32>,
    steps: u32,
}

impl Rig {
    /// A fresh game with the player moved to the Arreat Summit.
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
        let (link, tap) = predict_link(Box::new(SharedLink(server.clone())));
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
            steps: 0,
        };
        while app_support::local_player(&rig.server).is_none() {
            rig.step(1);
        }
        rig.step(30);
        app_support::with(&rig.server, |l| {
            let (p, _) = single_player::local_player(&l.host().game).unwrap();
            l.host_mut()
                .game
                .events
                .action
                .hooks()
                .act_changes
                .push((p, a5::SUMMIT, 0));
        });
        rig.wait_level(a5::SUMMIT);
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

    fn find(&self, ty: u8, class: u32) -> Option<UnitKey> {
        self.app
            .world()
            .resource::<BridgeResource>()
            .0
            .world()
            .units
            .iter()
            .find(|(k, u)| k.unit_type == ty && u.class == class)
            .map(|(k, _)| *k)
    }

    fn interact(&mut self, key: UnitKey) {
        self.app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .interact(key)
            .unwrap();
    }

    /// Stands the player beside (x, y) of the current room.
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

    /// The summit room's sub-tile origin.
    fn origin(&self) -> (i32, i32) {
        let (tx, ty) = synthetic_chains::room_origin(a5::SUMMIT).unwrap();
        (tx * 5, ty * 5)
    }

    fn q5<T>(&self, f: impl FnOnce(&mut d2_sim::world::quests::QuestRecord) -> T) -> T {
        app_support::with(&self.server, |l| {
            f(l.host_mut()
                .game
                .world
                .quests
                .record_mut(35)
                .expect("chain 35"))
        })
    }
}

// Covers: specs/world/quests-act5-2.md §7.8
#[test]
fn the_statues_and_the_altar_register_with_the_quest_and_the_altar_answers() {
    let mut rig = Rig::new();
    for class in a5::STATUES {
        assert!(rig.find(OBJECT, class).is_some(), "statue {class}");
    }
    let altar = rig.find(OBJECT, a5::ALTAR).expect("the altar");
    let guids = rig.q5(|r| r.extra.a5.q5.statue_guids);
    assert!(guids.iter().all(|&g| g != 0), "statue GUIDs {guids:?}");
    assert_ne!(rig.q5(|r| r.extra.a5.q5.altar_guid), 0);
    // Qual-Kehk has started the quest (stands in for talking to him): the
    // altar's operate sets status 3 and its own mode.
    rig.q5(|r| r.not_intro = true);
    let (ox, oy) = rig.origin();
    let o = a5::PRESET_OBJECTS[3].1;
    rig.stand_at(ox + o.0 + 2, oy + o.1 + 3);
    rig.interact(altar);
    rig.step(20);
    assert_eq!(rig.q5(|r| r.extra.a5.q5.altar_mode), 2);
    assert_eq!(rig.q5(|r| r.status), 3);
    let b = &rig.app.world().resource::<BridgeResource>().0;
    assert!(b.log().rejected.is_empty(), "{:?}", b.log().rejected);
}

// Covers: specs/world/quests.md §8.2
#[test]
fn the_summit_exits_wait_for_the_ancients() {
    let mut rig = Rig::new();
    let [back, on] = synthetic_chains::slots(a5::SUMMIT);
    let (_, back_class) = back.expect("a way back");
    let (keep, on_class) = on.expect("a way on");
    assert_eq!(keep, 128);
    rig.q5(|r| r.not_intro = true);
    rig.step(30);
    for class in [on_class, back_class] {
        let t = rig.find(TILE, class).expect("the exit tile");
        rig.interact(t);
        rig.step(80);
        assert_eq!(rig.level(), Some(a5::SUMMIT), "exit {class} stays closed");
    }
    // The Ancients are defeated: the way on opens.
    rig.q5(|r| r.extra.a5.q5.defeated = true);
    rig.step(30);
    let t = rig.find(TILE, on_class).expect("the exit tile");
    rig.interact(t);
    rig.wait_level(128);
    let b = &rig.app.world().resource::<BridgeResource>().0;
    assert!(b.log().rejected.is_empty(), "{:?}", b.log().rejected);
}
