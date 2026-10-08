// Spec: specs/world/quests-act2.md (§8.3, §8.8, §8.11), specs/monsters/ai-bodies-2.md §14, specs/world/npc.md §8.3
//! Act II's close in the app's own synthetic game, headless (task
//! `q-a2-tyrael`, `docs/handoff/q-a2-tyrael.md`): the player enters
//! Duriel's Lair, Duriel (Duriel AI 44) attacks him, Duriel dies (chain 13
//! state 3), Tyrael's message 302 opens the portal and gives the credit,
//! Jerhyn's 442 and Meshif's 450 move the quest on, and Meshif's travel
//! row leads to Act III.
//! PROVISIONAL (REC-174): d2rs-own, unverified.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::monster_ai::MonsterAi;
use d2_client::app::play::{add_client_data, add_game, add_walk, predict_link, send_create_game_for};
use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, BuildError, GameData, ACT2_TOWN};
use d2_client::app::synthetic_act2 as a2;
use d2_client::bridge::local::{LocalLink, PendingSession};
use d2_client::bridge::predict::Speeds;
use d2_client::bridge::world::{OBJECT, TILE};
use d2_client::bridge::BridgeResource;
use d2_client::rules::unit_composite::code;
use d2_client::world_view::unit_assets::{MonsterRow, UnitLooks};
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
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::ActionTables;

mod app_support;
use app_support::SharedLink;

const DURIEL: u32 = 211;
const JERHYN: u32 = 201;
const MESHIF: u32 = 210;
const TYRAEL: u32 = 251;
const ACT3_TOWN: u32 = 75;
const PORTAL: u32 = 59;
const DURIEL_A1: &[u8; 8] = b"DUA1HTH\0";
const MINDAMAGE: u16 = 21;
const MAXDAMAGE: u16 = 22;
const TOHIT: u16 = 19;
const CLASSES: usize = TYRAEL as usize + 1;

struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

type Server = app_support::Server<StepClock>;

fn blank<T: Record>() -> T {
    T::decode(&vec![0u8; T::SIZE])
}

/// Every class a killable monster; Duriel (211) has the Duriel AI (44) and
/// plain attacks (no skills, aip4 = 0 picks A1).
fn rows() -> (Vec<Monstats>, Vec<Monstats2>) {
    let mut m = fx::monster_class();
    m.code = *b"du\0\0";
    m.monstatsex = 0;
    let mut v = vec![m.clone(); CLASSES];
    v[DURIEL as usize].ai = 44;
    for c in &mut v {
        c.skill4 = 0xFFFF;
    }
    let mut m2: Monstats2 = blank();
    (m2.mnu, m2.mwl, m2.ma1, m2.mdt) = (true, true, true, true);
    (v, vec![m2])
}

fn anim_data() -> AnimData {
    let mut a = fx::anim_data();
    let mut events = [0u8; animdata::EVENTS];
    events[2] = 1;
    let len = DURIEL_A1.iter().position(|&b| b == 0).unwrap();
    a.buckets[animdata::hash(&DURIEL_A1[..len])].push(AnimRecord {
        name: *DURIEL_A1,
        frames: 6,
        speed: 256,
        events,
    });
    a
}

fn install_fixtures(sim: &mut single_player::Sim) {
    let s = &mut sim.events.action.sys;
    s.stats = StatLists::new(d2_sim::bench_fixtures::stat_data());
    let (monstats, monstats2) = rows();
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
    let modes = [
        "dt", "nu", "wl", "gh", "a1", "a2", "bl", "sc", "s1", "s2", "s3", "s4", "dd", "kb", "sq",
        "rn",
    ];
    s.hooks.x.looks = Some(Arc::new(UnitLooks {
        monster_modes: modes.iter().map(|m| code(m.as_bytes())).collect(),
        monsters: [(
            DURIEL,
            MonsterRow {
                token: code(b"du"),
                base_w: None,
                composite_death: false,
            },
        )]
        .into(),
        ..UnitLooks::default()
    }));
    s.data = UnitData {
        monsters: vec![
            MonsterInfo {
                enabled: true,
                aidel: [15, 15, 15],
                moves: 0,
            };
            CLASSES
        ],
        ..UnitData::default()
    };
}

fn server_level(server: &Server) -> Option<u32> {
    app_support::with(server, |l| {
        let (p, _) = single_player::local_player(&l.host().game)?;
        let g = &mut l.host_mut().game;
        let room = g.game.lists.unit(p)?.room()?;
        g.events.action.hooks().drlg.level_id(&g.game, room)
    })
}

fn chain13(server: &Server) -> (u8, bool) {
    app_support::with(server, |l| {
        let r = l.host().game.world.quests.record(13).expect("chain 13");
        (r.state, r.extra.a2.q6.duriel_killed)
    })
}

fn life(server: &Server) -> i32 {
    app_support::with(server, |l| {
        let sim = &mut l.host_mut().game;
        let (p, _) = single_player::local_player(sim).expect("joined");
        sim.events
            .action
            .with(&mut sim.game, |_, v| v.stat(p, stat::HITPOINTS))
    })
}

/// A monster of `class` one sub-tile from the player: (unit, GUID).
fn spawn(server: &Server, class: u32) -> (UnitId, u32) {
    app_support::with(server, move |l| {
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
}

/// Gives the monster its AI (`ai.md` §3.3) and schedules its first think.
fn start_ai(server: &Server, m: UnitId) {
    app_support::with(server, move |l| {
        let sim = &mut l.host_mut().game;
        let a = &mut sim.events.action;
        a.sys.units.get_mut(m).unwrap().flags |= flags::IS_VALID_TARGET | flags::CAN_BE_ATTACKED;
        a.ai(&mut sim.game, |g, cx| {
            cx.store.entry(m).control = Some(AiControl::default());
            install(g, cx, m, 0);
        })
        .expect("ai");
        let at = sim.game.frame + 1;
        sim.game
            .schedule_event(m, u32::from(event::AI_THINK), at, None, 0, 0)
            .unwrap();
    });
}

fn send(app: &mut App, m: &[u8]) {
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .send_bytes(m)
        .unwrap();
}

/// C→S 0x31: the message `msg` to the NPC `guid`.
fn say(app: &mut App, guid: u32, msg: u32) {
    let mut m = vec![0x31];
    m.extend_from_slice(&guid.to_le_bytes());
    m.extend_from_slice(&msg.to_le_bytes());
    send(app, &m);
}

fn objects_of(app: &App, class: u32) -> usize {
    app.world()
        .resource::<BridgeResource>()
        .0
        .world()
        .units
        .iter()
        .filter(|(k, u)| k.unit_type == OBJECT && u.class == class)
        .count()
}

// Covers: specs/world/quests-act2.md §8.11; specs/world/quests-act2.md §8.3; specs/monsters/ai-bodies-2.md §14; specs/world/npc.md §8.3
#[test]
fn duriel_fights_tyrael_opens_the_portal_and_meshif_travels_east() {
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
    let server: Server = Arc::new(Mutex::new(link));
    let data = GameData::Synthetic;
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<ButtonInput<KeyCode>>();
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
    // Act II (the act-change queue), then the Lair as `app_a2_duriel`.
    app_support::with(&server, |l| {
        let (p, _) = single_player::local_player(&l.host().game).unwrap();
        let g = &mut l.host_mut().game;
        g.events.action.hooks().act_changes.push((p, ACT2_TOWN, 0));
    });
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
            .unwrap_or_else(|| panic!("tile {class} in the model"));
        app.world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .interact(t)
            .unwrap();
        while server_level(&server) != Some(to) {
            step(app);
        }
        for _ in 0..30 {
            step(app);
        }
    };
    let edges = a2::edges();
    let staff = app_support::with(&server, |l| {
        l.host_mut().game.events.action.hooks().x.staff_tomb
    });
    let into = |to: u32| edges.iter().find(|e| e.to == to).unwrap().to_class;
    take(&mut app, into(a2::CANYON), a2::CANYON);
    take(&mut app, into(staff), staff);
    app_support::with(&server, |l| {
        let q = l.host_mut().game.world.quests.record_mut(13).unwrap();
        q.not_intro = true;
        q.extra.a2.q6.lair_open = true;
    });
    for _ in 0..30 {
        step(&mut app);
    }
    take(&mut app, into(a2::DURIELS_LAIR), a2::DURIELS_LAIR);

    // Duriel's AI: he attacks the player.
    let (duriel, _) = spawn(&server, DURIEL);
    app_support::with(&server, move |l| {
        let sim = &mut l.host_mut().game;
        let (p, _) = single_player::local_player(sim).unwrap();
        let a = &mut sim.events.action;
        a.with(&mut sim.game, |_, v| {
            v.set_base(p, stat::MAXHP, 256_000);
            v.set_base(p, stat::HITPOINTS, 256_000);
            v.set_base(duriel, TOHIT, 1000);
            v.set_base(duriel, MINDAMAGE, 1280);
            v.set_base(duriel, MAXDAMAGE, 1280);
        });
    });
    let before = life(&server);
    start_ai(&server, duriel);
    for _ in 0..120 {
        step(&mut app);
    }
    let after = life(&server);
    assert!(after < before, "Duriel hit the player: {before} -> {after}");

    // Duriel dies by the player's hand (the kill parse).
    assert!(!chain13(&server).1);
    app_support::with(&server, move |l| {
        let sim = &mut l.host_mut().game;
        let (p, _) = single_player::local_player(sim).unwrap();
        sim.events.action.combat(&mut sim.game, |w, _| {
            d2_sim::wiring::action::reaction::kill(w, duriel, p);
        });
    });
    for _ in 0..60 {
        step(&mut app);
    }
    assert_eq!(chain13(&server), (3, true), "Duriel's death: state 3");

    // Tyrael: message 302 opens the portal to Lut Gholein.
    let (_, tyrael) = spawn(&server, TYRAEL);
    assert_eq!(objects_of(&app, PORTAL), 0);
    say(&mut app, tyrael, 302);
    for _ in 0..60 {
        step(&mut app);
    }
    assert_eq!(chain13(&server).0, 4, "Tyrael's talk: state 4");
    assert!(objects_of(&app, PORTAL) > 0, "Tyrael's portal is there");

    // Jerhyn (442) and Meshif (450): the quest moves on, Meshif travels.
    let (_, jerhyn) = spawn(&server, JERHYN);
    say(&mut app, jerhyn, 442);
    for _ in 0..30 {
        step(&mut app);
    }
    let (_, meshif) = spawn(&server, MESHIF);
    say(&mut app, meshif, 450);
    for _ in 0..30 {
        step(&mut app);
    }
    assert_eq!(chain13(&server).0, 5, "Meshif's talk: state 5");
    let mut m = vec![0x38];
    m.extend_from_slice(&0u32.to_le_bytes());
    m.extend_from_slice(&meshif.to_le_bytes());
    m.extend_from_slice(&1u32.to_le_bytes());
    send(&mut app, &m);
    for _ in 0..300 {
        if server_level(&server) == Some(ACT3_TOWN) {
            break;
        }
        step(&mut app);
    }
    assert_eq!(server_level(&server), Some(ACT3_TOWN), "Act III");
    for _ in 0..40 {
        step(&mut app);
    }
    let b = &app.world().resource::<BridgeResource>().0;
    assert!(b.log().rejected.is_empty(), "{:?}", b.log().rejected);
}
