// Spec: specs/monsters/ai.md §2, §5.2, §7.1; specs/skills/use.md §5.2–§5.3; specs/combat/damage.md §5.2, §7; specs/combat/vitals.md §4.8
//! A monster attacks the player in the play preview, headless over the
//! synthetic single-player game: a class-0 monster (AI 3, Zombie) next to
//! the player notices it, attacks in A1, the hit runs the server's damage
//! path and the player's life reaches 0 (then q-death: DT, the screen).
//!
//! Synthetic fills: the stat table, a `monstats` class with the Zombie AI,
//! the Attack skill (`srvdofunc` 1), and the AnimData record of the
//! monster's A1 (no game files). Provisional parts: REC-109 in
//! `docs/HANDOFF.md` §7.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::death::{add_death, DeathScreen};
use d2_client::app::monster_ai::MonsterAi;
use d2_client::app::play::{add_client_data, add_game, add_preview, send_create_game_for};
use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, BuildError, GameData};
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::local::{LocalLink, PendingSession};
use d2_client::bridge::mirror::DynLink;
use d2_client::bridge::modes::player_mode;
use d2_client::bridge::world::TILE;
use d2_client::bridge::BridgeResource;
use d2_client::world_view::tile_assets::TileAssets;
use d2_data::tables::{Monstats, Monstats2, Record, Skills};
use d2_formats::animdata::{self, AnimData, AnimRecord};
use d2_server::adapters::ProtoSizes;
use d2_server::host::Host;
use d2_server::seams::Clock;
use d2_sim::bench_fixtures::combat as fx;
use d2_sim::missiles::unit_flag as flags;
use d2_sim::monsters::ai::{install, AiControl};
use d2_sim::stats::{stat, StatLists};
use d2_sim::tick::events::event;
use d2_sim::units::hooks::{MonsterInfo, UnitData};
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::UnitType;
use d2_sim::wiring::action::ActionTables;

const MONSTER_A1: &[u8; 8] = b"ZOA1HTH\0";
const MINDAMAGE: u16 = 21;
const MAXDAMAGE: u16 = 22;
const TOHIT: u16 = 19;

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

fn blank<T: Record>() -> T {
    T::decode(&vec![0u8; T::SIZE])
}

/// Monster class 0: the Zombie AI (3) that always picks A1 in combat, the
/// modes NU / WL / A1, no skills of its own.
fn zombie() -> (Vec<Monstats>, Vec<Monstats2>) {
    let mut m = fx::monster_class();
    m.ai = 3;
    m.aip4 = 100;
    m.code = *b"zo\0\0";
    m.monstatsex = 0;
    let mut m2: Monstats2 = blank();
    (m2.mnu, m2.mwl, m2.ma1, m2.mdt) = (true, true, true, true);
    (vec![m], vec![m2])
}

/// The A1 animation: 6 frames at speed 256 with the attack event (1) on
/// frame 2.
fn anim_data() -> AnimData {
    let mut a = fx::anim_data();
    let mut events = [0u8; animdata::EVENTS];
    events[2] = 1;
    let len = MONSTER_A1.iter().position(|&b| b == 0).unwrap();
    a.buckets[animdata::hash(&MONSTER_A1[..len])].push(AnimRecord {
        name: *MONSTER_A1,
        frames: 6,
        speed: 256,
        events,
    });
    a
}

/// The synthetic game with the combat tables of this test (set before the
/// player joins).
fn install_fixtures(sim: &mut single_player::Sim) {
    let s = &mut sim.events.action.sys;
    s.stats = StatLists::new(d2_sim::bench_fixtures::stat_data());
    let (monstats, monstats2) = zombie();
    let mut skills = fx::skills();
    let mut attack: Skills = fx::skill_rec();
    attack.srvdofunc = 1;
    skills.skills[0] = attack;
    let mut combat = fx::combat_tables();
    combat.monstats = monstats.clone();
    combat.monstats2 = monstats2.clone();
    s.hooks.x.monsters = MonsterAi::from_tables(&monstats, &monstats2);
    s.hooks.tables = Arc::new(ActionTables {
        missiles: vec![fx::arrow()],
        skills,
        combat,
        levels: vec![blank(); 150],
        skill_modes: vec![[0; 8]],
    });
    s.hooks.anim_data = Some(Arc::new(anim_data()));
    s.data = UnitData {
        monsters: vec![MonsterInfo {
            enabled: true,
            aidel: [15, 15, 15],
            moves: 0,
        }],
        ..UnitData::default()
    };
}

fn step(app: &mut App, ms: &AtomicU32, n: usize) {
    for _ in 0..n {
        app.update();
        ms.fetch_add(40, Ordering::SeqCst);
    }
}

fn local_mode(app: &App) -> u32 {
    app.world()
        .resource::<BridgeResource>()
        .0
        .world()
        .local()
        .expect("local")
        .mode
}

// Covers: specs/monsters/ai.md §5.2
// Covers: specs/skills/use.md §5.3
// Covers: specs/combat/vitals.md §4.8
#[test]
fn a_monster_next_to_the_player_attacks_until_the_player_dies() {
    let character = single_player::new_character("sorceress", "Test").unwrap();
    let ms = Arc::new(AtomicU32::new(1000));
    let clock = StepClock(ms.clone());
    let spawn_character = character.clone();
    let link = ThreadLink::spawn(move || {
        let mut g = single_player::build_with(
            &GameData::Synthetic,
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
    let link = Arc::new(Mutex::new(link));
    let dyn_link: DynLink = Box::new(Shared(link.clone()));
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<ButtonInput<KeyCode>>();
    add_game(&mut app, dyn_link, false).unwrap();
    send_create_game_for(&mut app, &character).unwrap();
    let data = GameData::Synthetic;
    let levels = single_player::client_level_rows(&data);
    add_client_data(
        &mut app,
        single_player::client_drlg_source(&data),
        levels.clone(),
    );
    add_preview(&mut app, levels, TileAssets::default());
    add_death(&mut app);
    step(&mut app, &ms, 10);
    assert_eq!(local_mode(&app), player_mode::TOWN_NEUTRAL, "joined");

    // Out of town (the AI skips players in a town room, `ai.md` §5.2 step
    // 5.1): through the Blood Moor's cave entrance to the Den of Evil.
    let entrance = app
        .world()
        .resource::<BridgeResource>()
        .0
        .world()
        .units
        .iter()
        .find(|(k, u)| k.unit_type == TILE && u.class == single_player::BLOOD_MOOR_TO_DEN)
        .map(|(k, _)| *k)
        .expect("the cave entrance reached the client");
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .interact(entrance)
        .unwrap();
    for _ in 0..200 {
        let level = link
            .lock()
            .unwrap()
            .with(|l| {
                let (p, _) = single_player::local_player(&l.host().game)?;
                let g = &mut l.host_mut().game;
                let room = g.game.lists.unit(p)?.room()?;
                g.events.action.hooks().drlg.level_id(&g.game, room)
            })
            .unwrap();
        if level == Some(single_player::DEN_OF_EVIL) {
            break;
        }
        step(&mut app, &ms, 1);
    }
    step(&mut app, &ms, 20);

    // A monster one sub-tile from the player, with the Zombie AI, and a
    // player who one hit kills.
    link.lock()
        .unwrap()
        .with(|l| {
            let sim = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(sim).expect("joined");
            let a = &mut sim.events.action;
            let (px, py) = a.sys.hooks.path_position(p);
            let room = sim.game.lists.unit(p).and_then(|e| e.room()).expect("room");
            let req = AllocRequest {
                ty: UnitType::Monster,
                class: 0,
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
                v.set_base(p, stat::HITPOINTS, 2560);
                v.set_base(p, stat::MAXHP, 2560);
                v.set_base(m, stat::LEVEL, 1);
                v.set_base(m, stat::MAXHP, 25600);
                v.set_base(m, stat::HITPOINTS, 25600);
                v.set_base(m, TOHIT, 1000);
                v.set_base(m, MINDAMAGE, 5120);
                v.set_base(m, MAXDAMAGE, 5120);
            });
            a.sys.units.get_mut(m).unwrap().flags |=
                flags::IS_VALID_TARGET | flags::CAN_BE_ATTACKED;
            a.ai(&mut sim.game, |g, cx| {
                cx.store.entry(m).control = Some(AiControl::default());
                install(g, cx, m, 0);
            })
            .expect("ai");
            let at = sim.game.frame + 1;
            sim.game
                .schedule_event(m, u32::from(event::AI_THINK), at, None, 0, 0)
                .unwrap();
        })
        .unwrap();

    // The AI thinks, the monster attacks in A1, the Attack do hits through
    // the server's damage path, the player's life reaches 0.
    step(&mut app, &ms, 120);
    let (life, errors) = link
        .lock()
        .unwrap()
        .with(|l| {
            let sim = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(sim).expect("joined");
            let a = &mut sim.events.action;
            let errors = format!("{:?}", a.sys.hooks.errors);
            (
                a.with(&mut sim.game, |_, v| v.stat(p, stat::HITPOINTS)),
                errors,
            )
        })
        .unwrap();
    assert!(life <= 0, "the player's life is {life} ({errors})");
    assert_eq!(local_mode(&app), player_mode::DEATH, "the player died");
    assert!(app.world().resource::<DeathScreen>().active);
}
