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
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
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
use d2_sim::items::inventory::tables::InvItemRec;
use d2_sim::items::inventory::InvItem;
use d2_sim::items::moves::ty;
use d2_sim::items::tables::ItemRec;
use d2_sim::items::{q, ItemRequest};
use d2_sim::missiles::seams::MissileBodies;
use d2_sim::monsters::ai::install;
use d2_sim::stats::{stat, StatLists};
use d2_sim::tick::events::event;
use d2_sim::units::hooks::MonsterInfo;
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::ActionTables;
use d2_sim::wiring::economy::ItemSpawn;

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
/// Stat 14, gold on the person.
const GOLD: u16 = 14;
/// The synthetic act 1 hire row's mercenary class (`single_player`).
const MERC_CLASS: u32 = 271;

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
        s.monanim = 4; // monster A1
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

/// The A1 records (attack event 1 on frame 2) and the DT records (6
/// frames, no event) of the monster and of every player class. A name
/// the file lacks gets the default record (`animdata.md` §3, 2048
/// frames), so without a DT record the player's death would not end.
fn anim_data() -> AnimData {
    let mut a = fx::anim_data();
    let mut put = |name: [u8; 8], event: bool| {
        let mut events = [0u8; animdata::EVENTS];
        if event {
            events[2] = 1;
        }
        let len = name.iter().position(|&b| b == 0).unwrap();
        a.buckets[animdata::hash(&name[..len])].push(AnimRecord {
            name,
            frames: 6,
            speed: 256,
            events,
        });
    };
    put(*b"ZOA1HTH\0", true);
    put(*b"ZODTHTH\0", false);
    put(*b"RGA1HTH\0", true);
    put(*b"RGDTHTH\0", false);
    for t in TOKENS {
        put([t[0], t[1], b'A', b'1', b'H', b'T', b'H', 0], true);
        put([t[0], t[1], b'D', b'T', b'H', b'T', b'H', 0], false);
    }
    a
}

/// The combat tables of this test, set on the synthetic game before the
/// player joins (so the join's native skills and start stats run).
fn install_fixtures(sim: &mut single_player::Sim) {
    let s = &mut sim.events.action.sys;
    s.stats = StatLists::new(d2_sim::bench_fixtures::stat_data());
    // Row 0 of the synthetic game's monster rows becomes the test's
    // monster; the other rows stay (the mercenary's class 271 is one).
    let (zombie, zombie2) = monster();
    let mut monstats = s.hooks.tables.combat.monstats.clone();
    let mut monstats2 = s.hooks.tables.combat.monstats2.clone();
    monstats.resize_with(monstats.len().max(1), blank);
    monstats2.resize_with(monstats.len(), blank);
    monstats[0] = zombie[0].clone();
    monstats2[0] = zombie2[0].clone();
    // The mercenary's row: the Hireable AI (61) and the zombie's modes.
    let m = MERC_CLASS as usize;
    monstats.resize_with(monstats.len().max(m + 1), blank);
    monstats2.resize_with(monstats.len(), blank);
    let mut merc = zombie[0].clone();
    merc.ai = 61;
    merc.code = *b"rg\0\0";
    // The rogue's default action is monster skill 1 (`ai-bodies-6.md` §7
    // step 7.6): Attack (skill 0) here, in mode A1.
    merc.skill1 = 0;
    merc.sk1lvl = 1;
    (merc.velocity, merc.run) = (6, 9);
    // AI param 1 ≠ 0: it closes in and attacks (§7 step 6) rather than
    // backing off like an archer (step 5), as its skill is melee here.
    merc.aip1 = 1;
    monstats[m] = merc;
    monstats2[m] = zombie2[0].clone();
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
        skill_modes: {
            let mut m = vec![[0u8; 8]; MERC_CLASS as usize + 1];
            m[MERC_CLASS as usize] = [4; 8];
            m
        },
    });
    s.hooks.vitals = Some(Arc::new(vitals()));
    s.hooks.anim_data = Some(Arc::new(anim_data()));
    let modes = [
        "dt", "nu", "wl", "gh", "a1", "a2", "bl", "sc", "s1", "s2", "s3", "s4", "dd", "kb", "sq",
        "rn",
    ];
    let player_modes = [
        "DT", "NU", "WL", "RN", "GH", "TN", "TW", "A1", "A2", "BL", "SC", "TH", "KK", "S1", "S2",
        "S3", "S4", "DD", "SQ",
    ]
    .iter()
    .map(|m| code(m.as_bytes()))
    .collect();
    s.hooks.x.looks = Some(Arc::new(UnitLooks {
        player_tokens: TOKENS.iter().map(|t| code(&t[..])).collect(),
        player_modes,
        monster_modes: modes.iter().map(|m| code(m.as_bytes())).collect(),
        monsters: [
            (
                0,
                MonsterRow {
                    token: code(b"zo"),
                    base_w: None,
                    composite_death: false,
                },
            ),
            (
                MERC_CLASS,
                MonsterRow {
                    token: code(b"rg"),
                    base_w: None,
                    composite_death: false,
                },
            ),
        ]
        .into(),
        ..UnitLooks::default()
    }));
    let zombie_info = MonsterInfo {
        enabled: true,
        aidel: [15, 15, 15],
        moves: 0,
    };
    s.data
        .monsters
        .resize_with(s.data.monsters.len().max(MERC_CLASS as usize + 1), || {
            zombie_info
        });
    s.data.monsters[0] = zombie_info;
    s.data.monsters[MERC_CLASS as usize] = zombie_info;
    add_potion_rows(sim);
}

/// The `hp1` row appended to the synthetic item tables (which hold gems
/// and runes only): a healing potion, auto-belted, usable, of a beltable
/// type (as `d2-server`'s item-move tests). Returns nothing; the record
/// is the last of the combined array.
fn add_potion_rows(sim: &mut single_player::Sim) {
    let w = &mut sim.world;
    w.tables.items.push(ItemRec {
        code: *b"hp1 ",
        type_: ty::HPOT as i16,
        level: 1,
        invwidth: 1,
        invheight: 1,
        spawnable: 1,
        ..ItemRec::default()
    });
    if let Some((start, n)) = w.tables.parts[2] {
        w.tables.parts[2] = Some((start, n + 1));
    }
    let inv = w
        .inventory
        .as_mut()
        .expect("the synthetic inventory tables");
    inv.tables.items.push(InvItemRec {
        code: *b"hp1 ",
        type_: ty::HPOT as i16,
        invwidth: 1,
        invheight: 1,
        autobelt: 1,
        useable: 1,
        ..InvItemRec::default()
    });
    inv.tables.itemtypes[usize::from(ty::HPOT)].beltable = 1;
}

/// The client's skill rows (`msg-skills.md` Inputs) for the same table.
fn client_skill_rows() -> Vec<SkillRow> {
    skill_rows()
        .iter()
        .map(|s| SkillRow {
            anim: s.anim,
            monanim: s.monanim,
            range: 1,
            maxlvl: 20,
            flags: d2_client::controls::click::skill_flag::IN_TOWN,
            ..SkillRow::default()
        })
        .collect()
}

/// Wraps the shared link and keeps every S→C message (as
/// `app_mercs_acts.rs`).
struct Tap {
    inner: SharedLink<ThreadLink<single_player::Link<StepClock>>>,
    seen: Arc<Mutex<Vec<Vec<u8>>>>,
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
        self.seen.lock().unwrap().extend(v.iter().cloned());
        v
    }
}

/// The play app, headless, over the synthetic game with the fills above.
struct Rig {
    app: App,
    server: Server<StepClock>,
    ms: Arc<AtomicU32>,
    class: u8,
    /// Every S→C message the client received, in order.
    seen: Arc<Mutex<Vec<Vec<u8>>>>,
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
        let seen = Arc::new(Mutex::new(Vec::new()));
        let dyn_link: DynLink = Box::new(Tap {
            inner: SharedLink(server.clone()),
            seen: seen.clone(),
        });
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
            b.0.set_visibility(Some(d2_client::bridge::world::VisibleFn::new(|_, _, _| {
                true
            })));
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
            seen,
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
            let (px, py) = sim.events.action.sys.hooks.path_position(p);
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
                // Through the world's allocation: the monster type init
                // runs (`monsters/init.md` §5: unit flags, AI control).
                let m = sim
                    .events
                    .with(&mut sim.game, |g, v| v.allocate(g, &req, px + 1, py + i))
                    .expect("monster");
                let a = &mut sim.events.action;
                a.with(&mut sim.game, |_, v| {
                    v.set_base(m, stat::LEVEL, 1);
                    // The kill's experience, as monster init sets it.
                    v.set_base(m, EXPERIENCE, 100);
                    v.set_base(m, stat::MAXHP, life << 8);
                    v.set_base(m, stat::HITPOINTS, life << 8);
                    v.set_base(m, TOHIT, 1000);
                    v.set_base(m, MINDAMAGE, 3);
                    v.set_base(m, MAXDAMAGE, 3);
                });
                if ai {
                    // The spawner's first think (as `start_host_ai`,
                    // REC-254): install state 0, a think next frame.
                    a.ai(&mut sim.game, |g, cx| install(g, cx, m, 0))
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

    /// The GUID of the dead player unit that is not the local one: the
    /// corpse (single player). A corpse coming back into view gets 0x59
    /// (mode 5), then the corpse 0x74 (PROVISIONAL REC-279), which sets
    /// it to mode 0.
    fn corpse(&self) -> Option<u32> {
        let w = self.bridge().world();
        w.units
            .values()
            .find(|u| u.key.unit_type == 0 && Some(u.key) != w.local_player && u.is_dead())
            .map(|u| u.key.guid)
    }

    /// A healing potion (`hp1`) on the ground one sub-tile east of the
    /// player, as a drop leaves it (mode 3, unit flag 0x10); its GUID.
    fn drop_potion(&mut self) -> u32 {
        app_support::with(&self.server, |l| {
            let sim = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(sim).expect("joined");
            let (px, py) = sim.events.action.sys.hooks.path_position(p);
            let room = sim.game.lists.unit(p).and_then(|e| e.room());
            let record = sim.world.tables.items.len() - 1;
            let mut rq = ItemRequest {
                item: record as i32,
                format: 101,
                ilvl: 1,
                quality: q::NORMAL,
                flags2: 0x2,
                ..ItemRequest::default()
            };
            let (game, events, world) = (&mut sim.game, &mut sim.events, &mut sim.world);
            let u = world
                .with_economy(game, events, |econ, _| {
                    econ.create_item(
                        &mut rq,
                        false,
                        ItemSpawn {
                            room,
                            mode: 3,
                            init_flags: 1,
                        },
                    )
                })
                .expect("the potion");
            let a = &mut events.action.sys;
            a.hooks.items.get_mut(u).unwrap().flags |= 0x10;
            let guid = a.units.get(u).unwrap().guid;
            world.inventory.as_mut().unwrap().state.items.insert(
                u,
                InvItem {
                    x: px + 1,
                    y: py,
                    ..InvItem::new(guid, record)
                },
            );
            guid
        })
    }

    /// The client model's unit of `ty` and `class`.
    fn unit_of_class(&self, ty: u8, class: u32) -> UnitKey {
        let w = self.bridge().world();
        *w.units
            .iter()
            .find(|(k, u)| k.unit_type == ty && u.class == class)
            .unwrap_or_else(|| panic!("class {class} in the model"))
            .0
    }

    /// Teleports the player (server side) three sub-tiles south of `key`.
    fn stand_beside(&mut self, key: UnitKey) {
        let (x, y) = self.bridge().world().units[&key]
            .position
            .expect("the unit's position");
        let (p, _) = self.player();
        app_support::with(&self.server, move |l| {
            let g = &mut l.host_mut().game;
            let room = g.game.lists.unit(p).and_then(|u| u.room());
            g.events.action.with(&mut g.game, |g, v| {
                v.path_teleport(g, p, room, i32::from(x), i32::from(y) + 3)
            });
        });
        self.step(3, "stand beside");
    }

    /// C→S 0x36 hire from `seller`: its GUID and the row's first name id.
    fn hire(&mut self, seller: UnitKey) {
        let mut m = vec![0x36];
        m.extend_from_slice(&seller.guid.to_le_bytes());
        m.extend_from_slice(&100u32.to_le_bytes());
        self.send(&m);
        self.step(20, "hire");
    }

    /// The mercenary warped with the player (`hirelings.md` §6 rules 1,
    /// 5): on the server it is in the player's level within 10
    /// sub-tiles, and the client has it.
    fn assert_merc_near(&self, merc: UnitKey, what: &str) {
        let (p, _) = self.player();
        let near = app_support::with(&self.server, move |l| {
            let h = &l.host().game;
            let m = h.game.lists.find_unit(UnitType::Monster, merc.guid)?;
            let a = &h.events.action.sys.hooks;
            let level = |u: UnitId| {
                let room = h.game.lists.unit(u)?.room()?;
                a.drlg.level_id(&h.game, room)
            };
            let ((mx, my), (px, py)) = (a.path_position(m), a.path_position(p));
            Some(level(m)? == level(p)? && (mx - px).abs().max((my - py).abs()) <= 10)
        });
        assert_eq!(
            near,
            Some(true),
            "class {}: the mercenary {what}",
            self.class
        );
        assert!(
            self.bridge().world().units.contains_key(&merc),
            "class {}: the client has the mercenary ({what})",
            self.class
        );
    }

    /// The hired mercenary (class 271) in the client model.
    fn merc(&self) -> Option<UnitKey> {
        let w = self.bridge().world();
        w.units
            .iter()
            .find(|(k, u)| k.unit_type == MONSTER && u.class == MERC_CLASS)
            .map(|(k, _)| *k)
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

    // A mercenary from Kashya (C→S 0x36, `npc.md` §7.3; the synthetic
    // hire row is made up, REC-157). Below level 8 she refuses until
    // the Sisters' Burial Grounds (quest 2) is done.
    let kashya = r.unit_of_class(MONSTER, u32::from(d2_sim::world::npc::class::KASHYA));
    r.stand_beside(kashya);
    r.app
        .world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .interact(kashya)
        .unwrap();
    r.step(20, "talk to Kashya");
    let gold0 = 100_000;
    let (p, _) = r.player();
    app_support::with(&r.server, move |l| {
        let g = &mut l.host_mut().game;
        g.events
            .action
            .with(&mut g.game, |_, v| v.set_base(p, GOLD, gold0));
    });
    r.step(2, "gold");
    let n = r.seen.lock().unwrap().len();
    r.hire(kashya);
    assert_eq!(r.merc(), None, "class {class}: the hire is gated");
    let answer: Vec<u8> = r.seen.lock().unwrap()[n..]
        .iter()
        .filter(|m| m[0] == 0x2A)
        .map(|m| m[2])
        .collect();
    assert_eq!(answer, [11], "class {class}: the gate's 0x2A code");
    app_support::with(&r.server, move |l| {
        let g = &mut l.host_mut().game;
        g.world.rest.quests.get_mut(&p).unwrap().flags[0].set(2, 0);
    });
    r.hire(kashya);
    let merc = r.merc().expect("the mercenary reached the client's model");
    // The synthetic hireling rows give no to-hit or damage: the test's
    // (as the Zombie's).
    app_support::with(&r.server, move |l| {
        let h = &mut l.host_mut().game;
        let m = h
            .game
            .lists
            .find_unit(UnitType::Monster, merc.guid)
            .unwrap();
        h.events.action.with(&mut h.game, |_, v| {
            v.set_base(m, TOHIT, 1000);
            v.set_base(m, MINDAMAGE, 3);
            v.set_base(m, MAXDAMAGE, 3);
        });
    });
    // The hire's creation ran the monster type init (`monsters/init.md`
    // §5 step 1: unit flags |= 0x0A; the hire handler lends the world).
    let merc_flags = app_support::with(&r.server, move |l| {
        let g = &l.host().game;
        let u = g.game.lists.find_unit(UnitType::Monster, merc.guid);
        u.and_then(|u| g.events.action.sys.units.get(u))
            .map(|r| r.flags)
    });
    // The owner link (`hirelings.md` §3.2 rule 8, `0x0058F030`) is in the
    // AI control, where the Hireable think reads it.
    let (_, player_guid) = r.player();
    let owner = app_support::with(&r.server, move |l| {
        let g = &l.host().game;
        let u = g.game.lists.find_unit(UnitType::Monster, merc.guid)?;
        let c = g.events.action.sys.hooks.ai.as_ref()?.control(u)?;
        c.minion_owner.map(|o| (o.ty, o.guid))
    });
    assert_eq!(
        owner,
        Some((UnitType::Player, player_guid)),
        "class {class}: the mercenary's owner link"
    );
    assert_eq!(
        merc_flags.map(|f| f & d2_sim::monsters::init::unit_flag::AT_INIT),
        Some(d2_sim::monsters::init::unit_flag::AT_INIT),
        "class {class}: the mercenary's type init"
    );
    // Closing the dialog ends the chat (C→S 0x30, `npc.md`): until then
    // the player is busy and picks nothing up (`inventory-moves.md` §8.1).
    let mut m = vec![0x30];
    m.extend_from_slice(&1u32.to_le_bytes());
    m.extend_from_slice(&kashya.guid.to_le_bytes());
    r.send(&m);
    r.step(2, "end the chat");
    assert!(
        r.server_stat(GOLD) < gold0,
        "class {class}: the hire cost gold"
    );

    // Out of town; the mercenary follows (`hirelings.md` §6).
    r.leave_town();
    r.step(10, "the mercenary follows");
    r.assert_merc_near(merc, "followed into the Den");

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
    // Only the mercenary attacks it: its life shows the mercenary's hits.
    let zombie = r.spawn_pack(1, 1000, true)[0];
    let mut hurt = false;
    let mut merc_modes = std::collections::BTreeSet::new();
    for _ in 0..600 {
        r.step(1, "monster attacks");
        if let Some(u) = r.bridge().world().units.get(&merc) {
            merc_modes.insert(u.mode);
        }
        if r.client_stat(stat::HITPOINTS) < life {
            hurt = true;
        }
        if r.mode() == player_mode::DEATH || r.mode() == player_mode::DEAD {
            break;
        }
    }
    assert!(hurt, "class {class}: the client saw the player's life drop");
    assert!(
        merc_modes.contains(&4),
        "class {class}: the client saw the mercenary attack (A1), modes {merc_modes:?}"
    );
    let zombie_life = app_support::with(&r.server, move |l| {
        let h = &mut l.host_mut().game;
        let z = h.game.lists.find_unit(UnitType::Monster, zombie);
        z.map(|z| {
            h.events
                .action
                .with(&mut h.game, |_, v| v.stat(z, stat::HITPOINTS) >> 8)
        })
    });
    assert!(
        zombie_life.is_some_and(|l| l < 1000),
        "class {class}: the mercenary hurt the zombie ({zombie_life:?})"
    );
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

    // Back to the corpse: C→S 0x16 type 0 on it takes it back
    // (`inventory-moves.md` §7.1, §12): S→C 0x8E, the corpse leaves.
    r.leave_town();
    r.step(10, "the corpse comes into view");
    r.assert_merc_near(merc, "followed into the Den again");
    let corpse = r.corpse().expect("the corpse lies where the player died");
    let mut taken = false;
    for _ in 0..40 {
        let mut m = vec![0x16];
        m.extend_from_slice(&0u32.to_le_bytes());
        m.extend_from_slice(&corpse.to_le_bytes());
        m.extend_from_slice(&0u32.to_le_bytes());
        r.send(&m);
        r.step(8, "take the corpse back");
        if r.corpse().is_none() {
            taken = true;
            break;
        }
    }
    assert!(
        taken,
        "class {class}: the corpse {corpse} was not taken back"
    );
    let left = app_support::with(&r.server, move |l| {
        let a = &l.host().game.events.action;
        a.sys
            .hooks
            .death
            .owners
            .keys()
            .filter(|&&c| a.sys.units.get(c).is_some_and(|u| u.guid == corpse))
            .count()
    });
    assert_eq!(left, 0, "class {class}: the server freed the corpse");

    // A healing potion: picked up into the belt (C→S 0x16 type 4,
    // `inventory-moves.md` §7.1, §8.1), then drunk from it (0x26, §7.17;
    // the effect is PROVISIONAL REC-102: life over time).
    // A unit next to it (the mercenary) turns the click into a walk to
    // it (§7.1 type 4 step 2): click again until it is in the belt.
    let potion = r.drop_potion();
    let in_belt = |r: &Rig| {
        let w = r.bridge().world();
        d2_client::bridge::items::belt(w)
            .values()
            .any(|i| i.key.guid == potion)
    };
    for _ in 0..10 {
        let mut m = vec![0x16];
        m.extend_from_slice(&4u32.to_le_bytes());
        m.extend_from_slice(&potion.to_le_bytes());
        m.extend_from_slice(&0u32.to_le_bytes());
        r.send(&m);
        r.step(8, "pick up the potion");
        if in_belt(&r) {
            break;
        }
    }
    assert!(
        in_belt(&r),
        "class {class}: the potion reached the client's belt"
    );
    let (p, _) = r.player();
    let max = r.server_stat(stat::MAXHP);
    app_support::with(&r.server, move |l| {
        let sim = &mut l.host_mut().game;
        sim.events.action.with(&mut sim.game, |_, v| {
            v.set_base(p, stat::HITPOINTS, max / 4)
        });
    });
    r.step(2, "wounded");
    let low = r.client_stat(stat::HITPOINTS);
    assert_eq!(
        r.server_stat(74),
        0,
        "class {class}: no life regeneration yet"
    );
    let mut m = vec![0x26];
    m.extend_from_slice(&potion.to_le_bytes());
    m.extend_from_slice(&[0; 8]);
    r.send(&m);
    r.step(2, "drink");
    // Stat 74 (life regeneration per tick) is the potion list's.
    let regen = r.server_stat(74);
    assert!(
        regen > 0,
        "class {class}: the potion's life list is on ({regen})"
    );
    r.step(58, "the potion works");
    let healed = r.client_stat(stat::HITPOINTS);
    assert!(
        healed > low,
        "class {class}: the potion healed ({low} → {healed})"
    );
    let gone = {
        let w = r.bridge().world();
        !d2_client::bridge::items::belt(w)
            .values()
            .any(|i| i.key.guid == potion)
    };
    assert!(gone, "class {class}: the potion left the belt");
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
    // Akara, Kashya, Gheed, Charsi and Warriv (new content: q-smoke-travel,
    // REC-280, his travel row to Act II).
    assert_eq!(npcs, 5, "the town's five NPCs");
}

// Covers: specs/skills/use.md §5; specs/combat/vitals.md §4
#[test]
fn amazon_fights_levels_dies_and_respawns() {
    scenario(0);
}

#[test]
fn sorceress_fights_levels_dies_and_respawns() {
    scenario(1);
}

#[test]
fn necromancer_fights_levels_dies_and_respawns() {
    scenario(2);
}

#[test]
fn paladin_fights_levels_dies_and_respawns() {
    scenario(3);
}

#[test]
fn barbarian_fights_levels_dies_and_respawns() {
    scenario(4);
}

#[test]
fn druid_fights_levels_dies_and_respawns() {
    scenario(5);
}

#[test]
fn assassin_fights_levels_dies_and_respawns() {
    scenario(6);
}
