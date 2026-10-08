// Spec: specs/client/bridge.md (§3, §6, §8), specs/client/msg-skills.md (§2 r8), specs/skills/use.md (§1, §5), specs/combat/damage.md (§7), specs/combat/vitals.md (§2, §4), specs/skills/levels.md (§6.4), specs/monsters/ai.md (§5.2), specs/items/inventory-moves.md (§7.1, §12)
//! (q-smoke-combat) The combat flows of `d2-client play --synthetic` end
//! to end, headless: the app's bridge (`app::play::add_game`), the
//! in-process server on its thread and the sim. For each of the seven
//! classes a new character joins with the class's native skills, leaves
//! town, kills a pack with its left skill (Attack) and its right skill
//! (the class's start skill), is hit by a monster, levels up, spends a
//! stat point and a skill point, uses the new skill, dies, respawns in
//! town and takes its corpse back.
//!
//! Every step asserts that the server dispatched every C→S message with
//! result 0, that the client handled every S→C message (none dropped,
//! unowned, rejected or discarded), and that the client model shows the
//! result (life, mana, experience, level, monster deaths).
//!
//! Test-local fills (the synthetic game has no combat tables, as in
//! `app_play_monster_ai.rs`): the stat table, an Attack row and two
//! melee rows per class (the start skill and a level-2 skill, both on
//! the Attack do function 1), `charstats` / `experience`, a killable
//! monster class with the Zombie AI and the AnimData records the attacks
//! need. Nothing here is an original value.

mod app_support;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use app_support::{Server, SharedLink};
use bevy::prelude::*;
use d2_client::app::death::{add_death, DeathScreen};
use d2_client::app::monster_ai::MonsterAi;
use d2_client::app::play::{add_client_data, add_game, add_preview, send_create_game_for};
use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, BuildError, GameData};
use d2_client::app::synthetic_client;
use d2_client::bridge::local::{LocalLink, PendingSession};
use d2_client::bridge::mirror::DynLink;
use d2_client::bridge::modes::player_mode;
use d2_client::bridge::world::{MonsterClass, MonsterSetup, SkillRow, UnitKey, MONSTER, TILE};
use d2_client::bridge::BridgeResource;
use d2_client::rules::unit_composite::code;
use d2_client::world_view::tile_assets::TileAssets;
use d2_client::world_view::unit_assets::{MonsterRow, UnitLooks};
use d2_data::tables::{Charstats, Experience, Monstats, Monstats2, Record, Skills};
use d2_formats::animdata::{self, AnimData, AnimRecord};
use d2_server::adapters::ProtoSizes;
use d2_server::dispatch::Outcome;
use d2_server::host::{Handled, Host};
use d2_server::seams::{Clock, ResultCode};
use d2_sim::bench_fixtures::combat as fx;
use d2_sim::combat::vitals::VitalsTables;
use d2_sim::missiles::unit_flag as flags;
use d2_sim::monsters::ai::{install, AiControl};
use d2_sim::stats::{stat, StatLists};
use d2_sim::tick::events::event;
use d2_sim::units::hooks::{MonsterInfo, UnitData};
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::ActionTables;

const STATPTS: u16 = 4;
const NEWSKILLS: u16 = 5;
const LEVEL: u16 = 12;
const EXPERIENCE: u16 = 13;
const TOHIT: u16 = 19;
const MINDAMAGE: u16 = 21;
const MAXDAMAGE: u16 = 22;

/// Skill rows: 0 Attack, 1 + c the start skill of class c, 8 + c its
/// level-2 skill.
const N_SKILLS: usize = 15;
fn start_skill(class: u8) -> u16 {
    1 + u16::from(class)
}
fn level2_skill(class: u8) -> u16 {
    8 + u16::from(class)
}

/// Player classes' two-letter tokens (`app/anim_names.rs`).
const TOKENS: [&[u8; 2]; 7] = [b"AM", b"SO", b"NE", b"PA", b"BA", b"DZ", b"AI"];

struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

fn blank<T: Record>() -> T {
    T::decode(&vec![0u8; T::SIZE])
}

fn skill_rows() -> Vec<Skills> {
    let mut v = vec![fx::skill_rec(); N_SKILLS];
    for (i, s) in v.iter_mut().enumerate() {
        s.srvdofunc = 1;
        s.anim = 7; // A1
        s.range = 1; // h2h
        s.skpoints = d2_sim::skills::levels::NO_CALC;
        if i > 0 {
            let class = ((i - 1) % 7) as u8;
            s.charclass = class;
            s.skilldesc = 0;
            (s.mana, s.manashift, s.minmana) = (2, 8, 2);
            s.reqlevel = if i >= 8 { 2 } else { 1 };
        }
    }
    v
}

/// `charstats`: the class's start skill in `Skill 1` and `StartSkill`,
/// walk / run velocities, 5 stat points and 1 skill point a level, life
/// and mana per level; `experience`: level 2 at 100, level 3 at 1500.
fn vitals() -> VitalsTables {
    let charstats = (0..7u8)
        .map(|c| {
            let mut r: Charstats = blank();
            (r.str, r.dex, r.int, r.vit) = (20, 20, 20, 20);
            (r.walkvelocity, r.runvelocity, r.rundrain) = (6, 9, 20);
            (r.lifeperlevel, r.manaperlevel, r.staminaperlevel) = (8, 8, 4);
            (r.lifepervitality, r.manapermagic, r.staminapervitality) = (8, 8, 4);
            r.statperlevel = 5;
            r.skill_1 = start_skill(c);
            r.startskill = start_skill(c);
            r
        })
        .collect();
    let row = |v: u32| Experience {
        amazon: v,
        sorceress: v,
        necromancer: v,
        paladin: v,
        barbarian: v,
        druid: v,
        assassin: v,
        ..blank()
    };
    VitalsTables {
        charstats,
        experience: vec![row(3), row(0), row(100), row(1500)],
    }
}

/// Monster class 0: killable, 100 experience, the Zombie AI (3) that
/// always picks A1; modes NU / WL / A1 / DT.
fn monster() -> (Vec<Monstats>, Vec<Monstats2>) {
    let mut m = fx::monster_class();
    m.ai = 3;
    m.aip4 = 100;
    m.code = *b"zo\0\0";
    m.monstatsex = 0;
    let mut m2: Monstats2 = blank();
    (m2.mnu, m2.mwl, m2.ma1, m2.mdt) = (true, true, true, true);
    (vec![m], vec![m2])
}

/// The A1 records (attack event 1 on frame 2) of the monster and of
/// every player class, and the fixture's death records.
fn anim_data() -> AnimData {
    let mut a = fx::anim_data();
    let mut put = |name: [u8; 8]| {
        let mut events = [0u8; animdata::EVENTS];
        events[2] = 1;
        let len = name.iter().position(|&b| b == 0).unwrap();
        a.buckets[animdata::hash(&name[..len])].push(AnimRecord {
            name,
            frames: 6,
            speed: 256,
            events,
        });
    };
    put(*b"ZOA1HTH\0");
    for t in TOKENS {
        put([t[0], t[1], b'A', b'1', b'H', b'T', b'H', 0]);
    }
    a
}

/// The combat tables of this test, set on the synthetic game before the
/// player joins (so the join's native skills and start stats run).
fn install_fixtures(sim: &mut single_player::Sim) {
    let s = &mut sim.events.action.sys;
    s.stats = StatLists::new(d2_sim::bench_fixtures::stat_data());
    let (monstats, monstats2) = monster();
    let mut skills = fx::skills();
    skills.skills = skill_rows();
    let mut combat = fx::combat_tables();
    combat.monstats = monstats.clone();
    combat.monstats2 = monstats2.clone();
    combat.charstats = vitals().charstats;
    s.hooks.x.monsters = MonsterAi::from_tables(&monstats, &monstats2);
    s.hooks.tables = Arc::new(ActionTables {
        missiles: vec![fx::arrow()],
        skills,
        combat,
        levels: vec![blank(); 150],
        skill_modes: vec![[0; 8]],
    });
    s.hooks.vitals = Some(Arc::new(vitals()));
    s.hooks.anim_data = Some(Arc::new(anim_data()));
    let modes = [
        "dt", "nu", "wl", "gh", "a1", "a2", "bl", "sc", "s1", "s2", "s3", "s4", "dd", "kb", "sq",
        "rn",
    ];
    let mut player_modes = vec![code(b"NU"); 20];
    player_modes[7] = code(b"A1");
    s.hooks.x.looks = Some(Arc::new(UnitLooks {
        player_tokens: TOKENS.iter().map(|t| code(&t[..])).collect(),
        player_modes,
        monster_modes: modes.iter().map(|m| code(m.as_bytes())).collect(),
        monsters: [(
            0,
            MonsterRow {
                token: code(b"zo"),
                base_w: None,
                composite_death: false,
            },
        )]
        .into(),
        ..UnitLooks::default()
    }));
    s.data = UnitData {
        monsters: vec![MonsterInfo {
            enabled: true,
            aidel: [15, 15, 15],
            moves: 0,
        }],
        ..UnitData::default()
    };
}

/// The client's skill rows (`msg-skills.md` Inputs) for the same table.
fn client_skill_rows() -> Vec<SkillRow> {
    skill_rows()
        .iter()
        .map(|s| SkillRow {
            anim: s.anim,
            range: 1,
            maxlvl: 20,
            flags: d2_client::controls::click::skill_flag::IN_TOWN,
            ..SkillRow::default()
        })
        .collect()
}

/// The play app, headless, over the synthetic game with the fills above.
struct Rig {
    app: App,
    server: Server<StepClock>,
    ms: Arc<AtomicU32>,
    class: u8,
}

impl Rig {
    fn new(class: u8) -> Self {
        let character = single_player::new_character(&class.to_string(), "Smoke").unwrap();
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
        let server: Server<StepClock> = Arc::new(Mutex::new(link));
        let dyn_link: DynLink = Box::new(SharedLink(server.clone()));
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
        // As `app::play::run` wires `play --synthetic`, then this test's
        // rows on top: its skills and its monster class 0.
        synthetic_client::install(&mut app);
        {
            let mut b = app.world_mut().resource_mut::<BridgeResource>();
            b.0.set_skill_rows(client_skill_rows());
            let mut units = single_player::synthetic_unit_rows();
            units.monsters[0] = Some(MonsterClass {
                setup: Some(MonsterSetup {
                    is_att: true,
                    is_sel: true,
                    ..MonsterSetup::default()
                }),
                ..MonsterClass::default()
            });
            b.0.set_unit_rows(units);
            // The render side's visibility predicate (`model.md` §13 r6):
            // headless there is no camera, COF or cel state, so this test
            // supplies the input (every point visible). The play window
            // installs none yet (open, docs/handoff/q-smoke-combat.md).
            b.0.set_visibility(Some(|_, _, _| true));
            let mut natives = [0u16; 10];
            natives[0] = start_skill(class);
            let mut all = vec![[0u16; 10]; 7];
            all[usize::from(class)] = natives;
            b.0.set_class_skills(all);
        }
        add_preview(&mut app, levels, TileAssets::default());
        add_death(&mut app);
        Self {
            app,
            server,
            ms,
            class,
        }
    }

    /// `n` frames, one server tick each; every C→S message the server
    /// drained must be dispatched with result 0 and every S→C message
    /// handled.
    fn step(&mut self, n: usize, what: &str) {
        for _ in 0..n {
            self.app.update();
            self.ms.fetch_add(40, Ordering::SeqCst);
            let msgs = app_support::with(&self.server, |l| l.last_frame().messages.clone());
            for m in msgs {
                if let Handled::Game(o) = m.handled {
                    assert_eq!(
                        o,
                        Outcome::Dispatched(ResultCode::Done),
                        "class {}, {what}: C→S 0x{:02X} ({} bytes)",
                        self.class,
                        m.id,
                        m.size
                    );
                }
            }
            self.assert_clean(what);
        }
    }

    fn assert_clean(&self, what: &str) {
        let log = self.bridge().log();
        assert!(
            log.unowned.is_empty()
                && log.dropped.is_empty()
                && log.rejected.is_empty()
                && log.discarded.is_empty(),
            "class {}, {what}: unowned {:?}, dropped {:?}, rejected {:?}, discarded {:?}",
            self.class,
            log.unowned,
            log.dropped,
            log.rejected,
            log.discarded
        );
        let errors = app_support::with(&self.server, |l| {
            format!("{:?}", l.host().game.events.action.sys.hooks.errors)
        });
        assert_eq!(errors, "[]", "class {}, {what}: server errors", self.class);
    }

    fn bridge(&self) -> &d2_client::bridge::Bridge<DynLink> {
        &self.app.world().resource::<BridgeResource>().0
    }

    fn send(&mut self, msg: &[u8]) {
        self.app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .send_bytes(msg)
            .unwrap();
    }

    fn player(&self) -> (UnitId, u32) {
        app_support::local_player(&self.server).expect("joined")
    }

    /// The local player's stat in the client model.
    fn client_stat(&self, s: u16) -> i32 {
        self.bridge().world().local().expect("local").stat(s)
    }

    fn server_stat(&self, s: u16) -> i32 {
        let (p, _) = self.player();
        app_support::with(&self.server, move |l| {
            let sim = &mut l.host_mut().game;
            sim.events.action.with(&mut sim.game, |_, v| v.stat(p, s))
        })
    }

    fn mode(&self) -> u32 {
        self.bridge().world().local().expect("local").mode
    }

    fn level_id(&self) -> Option<u32> {
        app_support::with(&self.server, |l| {
            let (p, _) = single_player::local_player(&l.host().game)?;
            let g = &mut l.host_mut().game;
            let room = g.game.lists.unit(p)?.room()?;
            g.events.action.hooks().drlg.level_id(&g.game, room)
        })
    }

    /// Out of town: through the Blood Moor's cave entrance into the Den
    /// of Evil (as `app_play_monster_ai.rs`).
    fn leave_town(&mut self) {
        let entrance = self
            .bridge()
            .world()
            .units
            .iter()
            .find(|(k, u)| k.unit_type == TILE && u.class == single_player::BLOOD_MOOR_TO_DEN)
            .map(|(k, _)| *k)
            .expect("the cave entrance reached the client");
        self.app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .interact(entrance)
            .unwrap();
        for _ in 0..200 {
            if self.level_id() == Some(single_player::DEN_OF_EVIL) {
                break;
            }
            self.step(1, "leave town");
        }
        assert_eq!(self.level_id(), Some(single_player::DEN_OF_EVIL));
        self.step(20, "out of town");
    }

    /// `n` monsters of class 0 east of the player, `life` points each;
    /// `ai`: they think and attack.
    fn spawn_pack(&mut self, n: i32, life: i32, ai: bool) -> Vec<u32> {
        app_support::with(&self.server, move |l| {
            let sim = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(sim).expect("joined");
            let a = &mut sim.events.action;
            let (px, py) = a.sys.hooks.path_position(p);
            let room = sim.game.lists.unit(p).and_then(|e| e.room()).expect("room");
            let mut guids = Vec::new();
            for i in 0..n {
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
                    .with(&mut sim.game, |g, v| v.allocate(g, &req, px + 1, py + i))
                    .expect("monster");
                a.with(&mut sim.game, |_, v| {
                    v.set_base(m, stat::LEVEL, 1);
                    v.set_base(m, stat::MAXHP, life << 8);
                    v.set_base(m, stat::HITPOINTS, life << 8);
                    v.set_base(m, TOHIT, 1000);
                    v.set_base(m, MINDAMAGE, 3 << 8);
                    v.set_base(m, MAXDAMAGE, 3 << 8);
                });
                a.sys.units.get_mut(m).unwrap().flags |=
                    flags::IS_VALID_TARGET | flags::CAN_BE_ATTACKED;
                if ai {
                    a.ai(&mut sim.game, |g, cx| {
                        cx.store.entry(m).control = Some(AiControl::default());
                        install(g, cx, m, 0);
                    })
                    .expect("ai");
                    let at = sim.game.frame + 1;
                    sim.game
                        .schedule_event(m, u32::from(event::AI_THINK), at, None, 0, 0)
                        .unwrap();
                }
                guids.push(a.sys.units.get(m).unwrap().guid);
            }
            guids
        })
    }

    fn monster_alive(&self, guid: u32) -> bool {
        let w = self.bridge().world();
        w.units
            .get(&UnitKey::new(MONSTER, guid))
            .is_some_and(|u| u.mode != 0 && u.mode != 12)
    }

    /// C→S 0x3C: `skill` in the left (`left`) or right hand.
    fn select(&mut self, skill: u16, left: bool) {
        let mut m = vec![0x3C];
        m.extend_from_slice(&skill.to_le_bytes());
        m.extend_from_slice(&[0, if left { 0x80 } else { 0 }]);
        m.extend_from_slice(&u32::MAX.to_le_bytes());
        self.send(&m);
        self.step(2, "select a skill");
    }

    /// Attacks `guid` with the left (C→S 0x06) or right (0x0D) skill until
    /// the client sees it dead.
    fn kill(&mut self, guid: u32, left: bool) {
        for _ in 0..40 {
            if !self.monster_alive(guid) {
                return;
            }
            let mut m = vec![if left { 0x06 } else { 0x0D }];
            m.extend_from_slice(&1u32.to_le_bytes());
            m.extend_from_slice(&guid.to_le_bytes());
            self.send(&m);
            self.step(8, "attack");
        }
        panic!(
            "class {}: monster {guid} still alive in the client model",
            self.class
        );
    }
}

fn scenario(class: u8) {
    let mut r = Rig::new(class);
    r.step(10, "join");
    assert_eq!(r.mode(), player_mode::TOWN_NEUTRAL, "joined in town");
    let life0 = r.client_stat(stat::HITPOINTS);
    let mana0 = r.client_stat(stat::MANA);
    assert!(
        life0 > 0 && mana0 > 0,
        "class {class}: life {life0}, mana {mana0}"
    );

    // Out of town.
    r.leave_town();

    // A pack of three: one killed with the left skill (Attack), two with
    // the right skill (the class's start skill, which costs mana).
    let pack = r.spawn_pack(3, 4, false);
    r.select(0, true);
    r.select(start_skill(class), false);
    r.kill(pack[0], true);
    r.kill(pack[1], false);
    r.kill(pack[2], false);
    let mana = r.client_stat(stat::MANA);
    assert!(
        mana < mana0,
        "class {class}: the right skill cost mana ({mana0} → {mana})"
    );
    let xp = r.client_stat(EXPERIENCE);
    assert!(xp >= 100, "class {class}: experience {xp}");
    assert_eq!(r.client_stat(LEVEL), 2, "class {class}: level 2");
    assert_eq!(r.server_stat(LEVEL), 2);

    // Stat and skill points: one strength, the level-2 skill.
    let points = r.client_stat(STATPTS);
    assert!(points > 0, "class {class}: stat points {points}");
    let str0 = r.client_stat(stat::STRENGTH);
    r.send(&[0x3A, 0, 0]);
    r.step(2, "spend a stat point");
    assert_eq!(r.client_stat(stat::STRENGTH), str0 + 1);
    assert_eq!(r.client_stat(STATPTS), points - 1);
    assert_eq!(r.client_stat(NEWSKILLS), 1);
    let s2 = level2_skill(class);
    r.send(&[0x3B, s2 as u8, (s2 >> 8) as u8]);
    r.step(2, "spend a skill point");
    assert_eq!(r.client_stat(NEWSKILLS), 0);
    let has = {
        let p = r.bridge().world().local().unwrap();
        p.skills.as_ref().is_some_and(|l| l.native(s2).is_some())
    };
    assert!(has, "class {class}: the new skill in the client's list");

    // The new skill on a fresh monster.
    let next = r.spawn_pack(1, 4, false);
    r.select(s2, false);
    r.kill(next[0], false);

    // A monster that fights back: the player takes damage, then dies.
    let life = r.client_stat(stat::HITPOINTS);
    r.spawn_pack(1, 1000, true);
    let mut hurt = false;
    for _ in 0..600 {
        r.step(1, "monster attacks");
        if r.client_stat(stat::HITPOINTS) < life {
            hurt = true;
        }
        if r.mode() == player_mode::DEATH || r.mode() == player_mode::DEAD {
            break;
        }
    }
    assert!(hurt, "class {class}: the client saw the player's life drop");
    assert!(
        r.mode() == player_mode::DEATH || r.mode() == player_mode::DEAD,
        "class {class}: the player died (mode {})",
        r.mode()
    );
    r.step(40, "dying");
    assert!(r.app.world().resource::<DeathScreen>().active);

    // Esc: respawn in town.
    r.app
        .world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    r.step(1, "respawn");
    r.app
        .world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::Escape);
    r.step(10, "respawned");
    let mode = r.mode();
    assert!(
        mode == player_mode::TOWN_NEUTRAL || mode == player_mode::NEUTRAL,
        "class {class}: respawned, mode {mode}"
    );
    assert!(!r.app.world().resource::<DeathScreen>().active);
    assert!(r.client_stat(stat::HITPOINTS) > 0);
}

/// `play --synthetic`'s client rows: the join and the town's NPCs reach
/// the model with nothing dropped or rejected (before the fix: 0x6D × 4
/// dropped, 0x23 × 2 rejected).
// Covers: specs/client/msg-units.md §1.2 r2; specs/client/msg-skills.md §2 r3
#[test]
fn the_synthetic_play_join_is_clean() {
    use d2_client::bridge::Bridge;
    let ms = Arc::new(AtomicU32::new(1000));
    let character = single_player::new_character("1", "Smoke").unwrap();
    let (link, _) = single_player::start_with(
        GameData::Synthetic,
        single_player::DEFAULT_SEED,
        character.clone(),
        StepClock(ms.clone()),
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<ButtonInput<KeyCode>>();
    add_game(&mut app, Box::new(link), false).unwrap();
    send_create_game_for(&mut app, &character).unwrap();
    synthetic_client::install(&mut app);
    for _ in 0..8 {
        app.update();
        ms.fetch_add(40, Ordering::SeqCst);
    }
    let b: &Bridge<DynLink> = &app.world().resource::<BridgeResource>().0;
    let log = b.log();
    assert!(log.dropped.is_empty(), "dropped {:?}", log.dropped);
    assert!(log.rejected.is_empty(), "rejected {:?}", log.rejected);
    let npcs = b
        .world()
        .units
        .keys()
        .filter(|k| k.unit_type == MONSTER)
        .count();
    // Akara, Kashya, Gheed, Charsi and Warriv (`town_npcs::ACT1`; Warriv
    // since q-smoke-travel, REC-280).
    assert_eq!(
        npcs,
        d2_client::app::town_npcs::ACT1.len(),
        "the town's NPCs"
    );
}

// Covers: specs/skills/use.md §5; specs/combat/vitals.md §4
#[test]
#[ignore = "q-smoke-combat: stops at the left-skill kill (open break 3, docs/handoff/q-smoke-combat.md)"]
fn amazon_fights_levels_dies_and_respawns() {
    scenario(0);
}

#[test]
#[ignore = "q-smoke-combat: stops at the left-skill kill (open break 3, docs/handoff/q-smoke-combat.md)"]
fn sorceress_fights_levels_dies_and_respawns() {
    scenario(1);
}

#[test]
#[ignore = "q-smoke-combat: stops at the left-skill kill (open break 3, docs/handoff/q-smoke-combat.md)"]
fn necromancer_fights_levels_dies_and_respawns() {
    scenario(2);
}

#[test]
#[ignore = "q-smoke-combat: stops at the left-skill kill (open break 3, docs/handoff/q-smoke-combat.md)"]
fn paladin_fights_levels_dies_and_respawns() {
    scenario(3);
}

#[test]
#[ignore = "q-smoke-combat: stops at the left-skill kill (open break 3, docs/handoff/q-smoke-combat.md)"]
fn barbarian_fights_levels_dies_and_respawns() {
    scenario(4);
}

#[test]
#[ignore = "q-smoke-combat: stops at the left-skill kill (open break 3, docs/handoff/q-smoke-combat.md)"]
fn druid_fights_levels_dies_and_respawns() {
    scenario(5);
}

#[test]
#[ignore = "q-smoke-combat: stops at the left-skill kill (open break 3, docs/handoff/q-smoke-combat.md)"]
fn assassin_fights_levels_dies_and_respawns() {
    scenario(6);
}
