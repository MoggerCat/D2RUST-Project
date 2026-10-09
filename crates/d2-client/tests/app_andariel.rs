// Spec: specs/world/quests-act1.md §10.8; specs/world/quests.md §4.3, §4.4; specs/world/npc.md §8.3
//! Sisters to the Slaughter in the play preview, headless over the
//! synthetic single-player game (task `q-a1-andariel`,
//! `docs/handoff/q-a1-andariel.md`): the player enters Catacombs Level 4
//! (quest event 3: the quest goes to state 3), Andariel dies (the kill
//! parse links her to chain 6: credit, the portal timer), Warriv's scroll
//! message grants the reward and his travel row opens Act II.
//!
//! PROVISIONAL (REC in `docs/HANDOFF.md` §7): Andariel's link to chain 6;
//! the quest events run when the tick returns; the synthetic Catacombs 4
//! is a flat level.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{add_client_data, add_game, send_create_game_for};
use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, BuildError};
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::local::{LocalLink, PendingSession};
use d2_client::bridge::mirror::DynLink;
use d2_client::bridge::BridgeResource;
use d2_data::tables::{Monstats, Monstats2, Record};
use d2_server::adapters::ProtoSizes;
use d2_server::host::Host;
use d2_server::seams::Clock;
use d2_sim::bench_fixtures::combat as fx;
use d2_sim::stats::{stat, StatLists};
use d2_sim::units::hooks::{MonsterInfo, UnitData};
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::ActionTables;
use d2_sim::world::quests::{bit, npc};

mod app_support;

const ANDARIEL: u32 = 156;
const SLOT: usize = 6;

struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

struct Shared<L>(Arc<Mutex<L>>);

impl<L: ServerLink> ServerLink for Shared<L> {
    fn protocol_version(&self) -> u32 {
        self.0.lock().unwrap().protocol_version()
    }
    fn send(&mut self, q: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        self.0.lock().unwrap().send(q, msg)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        self.0.lock().unwrap().pump()
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        self.0.lock().unwrap().receive()
    }
}

/// Killable monster rows for every class up to Andariel's.
fn rows() -> (Vec<Monstats>, Vec<Monstats2>) {
    let mut m = fx::monster_class();
    m.code = *b"an\0\0";
    let m2: Monstats2 = Monstats2::decode(&vec![0u8; Monstats2::SIZE]);
    (
        vec![m; ANDARIEL as usize + 1],
        vec![m2; ANDARIEL as usize + 1],
    )
}

fn install_fixtures(sim: &mut single_player::Sim) {
    let s = &mut sim.events.action.sys;
    s.stats = StatLists::new(d2_sim::bench_fixtures::stat_data());
    let (monstats, monstats2) = rows();
    let mut combat = fx::combat_tables();
    combat.monstats = monstats;
    combat.monstats2 = monstats2;
    s.hooks.tables = Arc::new(ActionTables {
        missiles: vec![fx::arrow()],
        skills: fx::skills(),
        combat,
        levels: vec![Record::decode(&vec![0u8; d2_data::tables::Levels::SIZE]); 150],
        skill_modes: vec![[0; 8]],
        overlay_count: 0,
    });
    s.data = UnitData {
        monsters: vec![
            MonsterInfo {
                enabled: true,
                aidel: [15, 15, 15],
                moves: 0,
            };
            ANDARIEL as usize + 1
        ],
        ..UnitData::default()
    };
}

fn step(app: &mut App, ms: &AtomicU32, n: usize) {
    for _ in 0..n {
        app.update();
        ms.fetch_add(40, Ordering::SeqCst);
    }
}

type Link =
    Arc<Mutex<ThreadLink<LocalLink<single_player::Sim, ProtoSizes, PendingSession, StepClock>>>>;

/// The server player's level id.
fn server_level(link: &Link) -> Option<u32> {
    link.lock()
        .unwrap()
        .with(|l| {
            let (p, _) = single_player::local_player(&l.host().game)?;
            let g = &mut l.host_mut().game;
            let room = g.game.lists.unit(p)?.room()?;
            g.events.action.hooks().drlg.level_id(&g.game, room)
        })
        .unwrap()
}

/// A monster of `class` one sub-tile from the player; returns its GUID.
fn spawn(link: &Link, class: u32) -> (UnitId, u32) {
    link.lock()
        .unwrap()
        .with(move |l| {
            let sim = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(sim).expect("joined");
            let a = &mut sim.events.action;
            let (px, py) = a.sys.hooks.path_position(p);
            let room = sim.game.lists.unit(p).and_then(|e| e.room()).expect("room");
            let req = AllocRequest {
                ty: UnitType::Monster,
                class,
                room: Some(room),
                add: true,
                fixed_guid: None,
                mode: 1,
                allied: false,
            };
            let m = a
                .with(&mut sim.game, |g, v| v.allocate(g, &req, px + 1, py))
                .expect("monster");
            a.with(&mut sim.game, |_, v| {
                v.set_base(m, stat::LEVEL, 1);
                v.set_base(m, stat::MAXHP, 25600);
                v.set_base(m, stat::HITPOINTS, 25600);
            });
            (m, sim.game.lists.unit(m).expect("entry").guid)
        })
        .unwrap()
}

fn quest_bits(link: &Link) -> Vec<u8> {
    link.lock()
        .unwrap()
        .with(|l| {
            let sim = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(sim).expect("joined");
            let f = sim.world.rest.quests.get_mut(&p).expect("quest record");
            (0..16).filter(|&b| f.flags[0].get(SLOT as u8, b)).collect()
        })
        .unwrap()
}

fn record(link: &Link) -> (u8, u8) {
    link.lock()
        .unwrap()
        .with(|l| {
            let r = l.host().game.world.quests.record(6).expect("chain 6");
            (r.state, r.status)
        })
        .unwrap()
}

fn send(app: &mut App, m: &[u8]) {
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .send_bytes(m)
        .unwrap();
}

fn app_with_game() -> (App, Arc<AtomicU32>, Link) {
    let character = single_player::new_character("sorceress", "Test").unwrap();
    let ms = Arc::new(AtomicU32::new(1000));
    let clock = StepClock(ms.clone());
    let spawn_character = character.clone();
    let link = ThreadLink::spawn(move || {
        let mut g = single_player::build_with(
            &app_support::game_data(),
            single_player::DEFAULT_SEED,
            spawn_character,
        )?;
        install_fixtures(&mut g.sim);
        Ok::<_, BuildError>(LocalLink::new(Host::new(
            g.sim,
            ProtoSizes,
            PendingSession::default(),
            clock,
        )))
    })
    .unwrap();
    let link: Link = Arc::new(Mutex::new(link));
    let dyn_link: DynLink = Box::new(Shared(link.clone()));
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<ButtonInput<KeyCode>>();
    add_game(&mut app, dyn_link, false).unwrap();
    send_create_game_for(&mut app, &character).unwrap();
    let data = app_support::game_data();
    add_client_data(
        &mut app,
        single_player::client_drlg_source(&data),
        single_player::client_level_rows(&data),
    );
    app_support::live_tables(&mut app);
    step(&mut app, &ms, 10);
    (app, ms, link)
}

// Covers: specs/world/quests.md §4.3; specs/world/quests.md §4.4; specs/world/quests-act1.md §10.8
// Covers: specs/world/npc.md §8.3
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn killing_andariel_completes_the_act_and_warriv_travels_east() {
    let (mut app, ms, link) = app_with_game();

    // Catacombs Level 4: quest event 3 sets the quest to state 3.
    link.lock()
        .unwrap()
        .with(|l| {
            let g = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(g).expect("joined");
            g.events
                .action
                .hooks()
                .act_changes
                .push((p, single_player::CATACOMBS_4, 0));
        })
        .unwrap();
    for _ in 0..200 {
        if server_level(&link) == Some(single_player::CATACOMBS_4) {
            break;
        }
        step(&mut app, &ms, 1);
    }
    assert_eq!(server_level(&link), Some(single_player::CATACOMBS_4));
    step(&mut app, &ms, 60);
    assert_eq!(record(&link).0, 3, "A1Q6 went to state 3 on the level");

    // Andariel dies by the player's hand: the kill parse credits the quest.
    let (andariel, _) = spawn(&link, ANDARIEL);
    link.lock()
        .unwrap()
        .with(move |l| {
            let sim = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(sim).expect("joined");
            sim.events.action.combat(&mut sim.game, |w, _| {
                d2_sim::wiring::action::reaction::kill(w, andariel, p);
            });
        })
        .unwrap();
    step(&mut app, &ms, 60);
    let bits = quest_bits(&link);
    assert!(
        bits.contains(&bit::PRIMARY_GOAL_DONE) && bits.contains(&bit::REWARD_PENDING),
        "the kill credited the player: {bits:?}"
    );

    // Warriv: his scroll message 183 grants the reward, his travel row
    // (C→S 0x38 action 0) changes the act.
    let (_, warriv) = spawn(&link, u32::from(npc::WARRIV1));
    let mut m = vec![0x31];
    m.extend_from_slice(&warriv.to_le_bytes());
    m.extend_from_slice(&183u32.to_le_bytes());
    send(&mut app, &m);
    step(&mut app, &ms, 20);
    let bits = quest_bits(&link);
    assert!(bits.contains(&bit::REWARD_GRANTED), "granted: {bits:?}");
    let mut m = vec![0x38];
    m.extend_from_slice(&0u32.to_le_bytes());
    m.extend_from_slice(&warriv.to_le_bytes());
    m.extend_from_slice(&1u32.to_le_bytes());
    send(&mut app, &m);
    for _ in 0..200 {
        if server_level(&link) == Some(single_player::ACT2_TOWN) {
            break;
        }
        step(&mut app, &ms, 1);
    }
    assert_eq!(server_level(&link), Some(single_player::ACT2_TOWN));
    step(&mut app, &ms, 40);
    let b = &app.world().resource::<BridgeResource>().0;
    assert_eq!(b.world().act.as_ref().map(|a| a.act), Some(1));
    assert!(b.log().rejected.is_empty(), "{:?}", b.log().rejected);
}
