// Spec: specs/monsters/ai-bodies-2.md §14, specs/skills/bodies-2.md (§3.1, §5.3, §5.4)
//! Duriel's Charge and Jab in the synthetic Lair, headless (task
//! `q-a2-charge-jab`, `docs/handoff/q-a2-charge-jab.md`): the synthetic
//! tables get Charge (srvst 31 / srvdo 67) and Jab (srvst 5 / srvdo 7)
//! rows as Duriel's `Skill1` / `Skill2`; Duriel (AI 44) charges the
//! distant player and jabs him once he is near.
//! PROVISIONAL (REC-274): d2rs-own, unverified.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::monster_ai::MonsterAi;
use d2_client::app::play::{
    add_client_data, add_game, add_walk, predict_link, send_create_game_for,
};
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
use d2_sim::stats::{stat, StatLists};
use d2_sim::units::hooks::{MonsterInfo, UnitData};
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::ActionTables;

mod app_support;
use app_support::SharedLink;

const DURIEL: u32 = a2::DURIEL_CLASS;
const TYRAEL: u32 = a2::TYRAEL_CLASS;
const DOOR: u32 = a2::TYRAEL_DOOR_CLASS;
const DURIEL_ANIMS: [&[u8; 8]; 4] = [b"DUA1HTH\0", b"DUA2HTH\0", b"DUS1HTH\0", b"DUS2HTH\0"];
const MINDAMAGE: u16 = 21;
const MAXDAMAGE: u16 = 22;
const TOHIT: u16 = 19;
const CHARGE: u32 = 1;
const JAB: u32 = 2;
/// Modes S1 / S2 (`Sk1mode` / `Sk2mode`).
const MODE_S1: u32 = 8;
const MODE_S2: u32 = 9;
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
        c.skill1 = 0xFFFF;
        c.skill2 = 0xFFFF;
        c.skill3 = 0xFFFF;
        c.skill4 = 0xFFFF;
        c.aip4 = 100;
    }
    // Duriel: Skill1 Charge, Skill2 Jab; both always drawn true.
    let d = &mut v[DURIEL as usize];
    (d.skill1, d.skill2, d.aip3, d.aip5) = (CHARGE as u16, JAB as u16, 100, 100);
    d.aip4 = 0;
    (d.velocity, d.run) = (6, 9);
    let mut m2: Monstats2 = blank();
    (m2.mnu, m2.mwl, m2.ma1, m2.mdt) = (true, true, true, true);
    (v, vec![m2])
}

fn anim_data() -> AnimData {
    let mut a = fx::anim_data();
    let mut events = [0u8; animdata::EVENTS];
    events[2] = 1;
    // The charge steps along its path at every frame (frame code 1).
    let mut charge_events = [0u8; animdata::EVENTS];
    charge_events[..6].fill(1);
    for name in DURIEL_ANIMS {
        let events = if name == b"DUS1HTH\0" {
            charge_events
        } else {
            events
        };
        let len = name.iter().position(|&b| b == 0).unwrap();
        a.buckets[animdata::hash(&name[..len])].push(AnimRecord {
            name: *name,
            frames: 6,
            speed: 256,
            events,
        });
    }
    a
}

fn install_fixtures(sim: &mut single_player::Sim) {
    let s = &mut sim.events.action.sys;
    s.stats = StatLists::new(d2_sim::bench_fixtures::stat_data());
    let (monstats, monstats2) = rows();
    let mut skills = fx::skills();
    let skill_row = |st: u8, d: u8| {
        let mut r: Skills = fx::skill_rec();
        (r.srvstfunc, r.srvdofunc, r.srvmissile) = (u16::from(st), u16::from(d), 0xFFFF);
        (r.anim, r.range, r.intown, r.srcdam) = (1, 1, true, 128);
        r
    };
    skills.skills.resize(3, fx::skill_rec());
    let mut attack: Skills = fx::skill_rec();
    attack.srvdofunc = 1;
    skills.skills[0] = attack;
    skills.skills[CHARGE as usize] = skill_row(31, 67);
    skills.skills[JAB as usize] = skill_row(5, 7);
    let mut combat = fx::combat_tables();
    combat.monstats = monstats.clone();
    combat.monstats2 = monstats2.clone();
    s.hooks.x.monsters = MonsterAi::from_tables(&monstats, &monstats2);
    s.hooks.tables = Arc::new(ActionTables {
        missiles: vec![fx::arrow()],
        skills,
        combat,
        levels: vec![blank(); 150],
        skill_modes: {
            let mut m = vec![[0u8; 8]; CLASSES];
            m[DURIEL as usize][..2].copy_from_slice(&[MODE_S1 as u8, MODE_S2 as u8]);
            m
        },
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

fn life(server: &Server) -> i32 {
    app_support::with(server, |l| {
        let sim = &mut l.host_mut().game;
        let (p, _) = single_player::local_player(sim).expect("joined");
        sim.events
            .action
            .with(&mut sim.game, |_, v| v.stat(p, stat::HITPOINTS))
    })
}

/// The world-placed unit of `ty`/`class` (the Lair's population): (unit,
/// GUID, mode).
fn placed(server: &Server, ty: UnitType, class: u32) -> Option<(UnitId, u32, u32)> {
    app_support::with(server, move |l| {
        let sim = &mut l.host_mut().game;
        let (p, _) = single_player::local_player(sim)?;
        let room = sim.game.lists.unit(p)?.room()?;
        let found = sim.game.lists.room_units(room).into_iter().find(|&u| {
            sim.game.lists.unit(u).is_some_and(|e| e.ty == ty)
                && sim
                    .events
                    .action
                    .sys
                    .units
                    .get(u)
                    .is_some_and(|r| r.class == class)
        })?;
        let r = sim.events.action.sys.units.get(found)?;
        Some((found, r.guid, r.mode))
    })
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

// Covers: specs/monsters/ai-bodies-2.md §14; specs/skills/bodies-2.md §3.1; specs/skills/bodies-2.md §5.3; specs/skills/bodies-2.md §5.4
#[test]
fn duriel_charges_a_distant_player_and_jabs_a_near_one() {
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

    // The Lair's population: Duriel, Tyrael and his door are there with
    // no test spawn (REC-234).
    for _ in 0..30 {
        step(&mut app);
    }
    let (duriel, _, _) = placed(&server, UnitType::Monster, DURIEL).expect("Duriel in the Lair");
    placed(&server, UnitType::Monster, TYRAEL).expect("Tyrael in the Lair");
    let (_, _, door_mode) = placed(&server, UnitType::Object, DOOR).expect("Tyrael's door");
    assert_eq!(door_mode, 0, "the door is shut while Duriel lives");
    assert_eq!(
        objects_of(&app, DOOR),
        1,
        "the door is in the client's model"
    );

    // Duriel's modes over time, with his distance to the player.
    app_support::with(&server, move |l| {
        let sim = &mut l.host_mut().game;
        let (p, _) = single_player::local_player(sim).unwrap();
        let a = &mut sim.events.action;
        a.with(&mut sim.game, |_, v| {
            v.set_base(p, stat::MAXHP, 256_000);
            v.set_base(p, stat::HITPOINTS, 256_000);
            v.set_base(duriel, stat::LEVEL, 1);
            v.set_base(duriel, stat::MAXHP, 25600);
            v.set_base(duriel, stat::HITPOINTS, 25600);
            v.set_base(duriel, TOHIT, 1000);
            v.set_base(duriel, MINDAMAGE, 64);
            v.set_base(duriel, MAXDAMAGE, 64);
        });
    });
    let before = life(&server);
    let mut seen: Vec<(u32, i32)> = Vec::new();
    for _ in 0..800 {
        step(&mut app);
        let (mode, d) = app_support::with(&server, move |l| {
            let sim = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(sim).unwrap();
            let a = &mut sim.events.action;
            let (px, py) = a.sys.hooks.path_position(p);
            let (dx, dy) = a.sys.hooks.path_position(duriel);
            let mode = a.sys.units.get(duriel).map_or(0, |r| r.mode);
            (mode, (px - dx).abs().max((py - dy).abs()))
        });
        if seen.last().map(|s| s.0) != Some(mode) {
            seen.push((mode, d));
        }
    }
    let after = life(&server);
    assert!(after < before, "Duriel hit the player: {before} -> {after}");
    let charge = seen
        .iter()
        .find(|s| s.0 == MODE_S1)
        .expect("Duriel charged");
    assert!(charge.1 > 3, "the charge starts at a distance: {charge:?}");
    let jab = seen.iter().find(|s| s.0 == MODE_S2).expect("Duriel jabbed");
    assert!(jab.1 <= 3, "the jab is a close attack: {jab:?}");
    let b = &app.world().resource::<BridgeResource>().0;
    assert!(b.log().rejected.is_empty(), "{:?}", b.log().rejected);
}
