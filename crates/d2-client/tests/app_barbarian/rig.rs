// Spec: specs/skills/use.md §5; specs/skills/bodies.md §3.8, §4.2
//! The Barbarian test rig: the synthetic single-player game with a
//! barbarian, the skill rows of her level-1 skills, the AnimData of her
//! swing, and helpers to leave town, spawn a monster and send intents.
#![allow(dead_code)]

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{add_client_data, add_game, add_preview, send_create_game_for};
use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, BuildError, Link};
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::local::{LocalLink, PendingSession};
use d2_client::bridge::mirror::DynLink;
use d2_client::bridge::world::TILE;
use d2_client::bridge::BridgeResource;
use d2_client::rules::unit_composite::code;
use d2_client::world_view::tile_assets::TileAssets;
use d2_client::world_view::unit_assets::{MonsterRow, UnitLooks};
use d2_data::bin::BinTable;
use d2_data::fixup::maps::StateMaps;
use d2_data::tables::{Charstats, Monstats, Monstats2, Record, Skills, States};
use d2_formats::animdata::{self, AnimData, AnimRecord};
use d2_server::adapters::ProtoSizes;
use d2_server::host::Host;
use d2_server::seams::Clock;
use d2_sim::bench_fixtures::combat as fx;
use d2_sim::missiles::unit_flag as flags;
use d2_sim::skills::list::ListOwner;
use d2_sim::stats::{stat, StatLists, StateTable};
use d2_sim::units::hooks::{MonsterInfo, UnitData};
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::ActionTables;

pub const BASH: usize = 126;
pub const HOWL: usize = 130;
pub const LEAP: usize = 132;
pub const DOUBLE_SWING: usize = 133;
/// Made-up ids of the Assassin's kicks (real: 269 and 273); the rig's
/// player is a barbarian, but the kick body only reads stats.
pub const DRAGON_TALON: usize = 101;
pub const DRAGON_FLIGHT: usize = 102;
/// Made-up id: the real Leap Attack is a monster skill.
pub const LEAP_ATTACK: usize = 100;
pub const SHOUT: usize = 138;
pub const BATTLE_ORDERS: usize = 149;
pub const WHIRLWIND: usize = 151;
pub const NATURAL_RESISTANCE: usize = 153;
pub const BARBARIAN: u32 = 4;

/// Made-up state ids of the warcries (the real ones are in `states.txt`).
pub const SHOUT_STATE: i16 = 26;
pub const BATTLE_ORDERS_STATE: i16 = 51;
pub const HOWL_STATE: i16 = 28;

const TOHIT: u16 = 19;
const MINDAMAGE: u16 = 21;
const MAXDAMAGE: u16 = 22;

pub struct StepClock(Arc<AtomicU32>);

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

/// A made-up monster class 0 with no AI of its own (it stands still).
fn dummy() -> (Vec<Monstats>, Vec<Monstats2>) {
    let mut m = fx::monster_class();
    m.code = *b"zo\0\0";
    m.monstatsex = 0;
    let mut m2: Monstats2 = blank();
    (m2.mnu, m2.mwl, m2.ma1, m2.mdt) = (true, true, true, true);
    (vec![m], vec![m2])
}

/// The barbarian's bare-hand swing: 8 frames at speed 256, the attack
/// event (1) on frame 3 (`animdata.md` §2).
fn anim_data() -> AnimData {
    let mut a = fx::anim_data();
    for name in [b"BAA1HTH\0", b"BAA2HTH\0"] {
        let mut events = [0u8; animdata::EVENTS];
        // Two swing events, frames 3 and 6 (Double Swing alternates).
        events[3] = 1;
        events[6] = 1;
        let len = name.iter().position(|&b| b == 0).unwrap();
        a.buckets[animdata::hash(&name[..len])].push(AnimRecord {
            name: *name,
            frames: 8,
            speed: 256,
            events,
        });
    }
    // d2rs-own, unverified: a longer cast animation (24 frames) for the
    // leaps, which restart it from frame 10 (`bodies-2.md` §2.13) and step
    // the path on the frame events 12, 14, ... 22.
    let mut events = [0u8; animdata::EVENTS];
    events[3..24].fill(1);
    let name = *b"BASCHTH\0";
    a.buckets[animdata::hash(&name[..7])].push(AnimRecord {
        name,
        frames: 24,
        speed: 256,
        events,
    });
    a
}

/// A skill row shaped like the real `skills.txt` row of the named skill,
/// numbers made up (`// d2rs-own, unverified`).
/// The offset in `skills_code` of the formula "500" (`0x08` push i16,
/// `0x00` end); offset 0 is never a formula.
pub const LEN_CALC: u32 = 4;
/// The formula "25" (the passive's fire resistance).
pub const RESIST_CALC: u32 = 8;
pub const PASSIVE_STATE: i16 = 70;
pub const FIRE_RESIST: i16 = 39;

pub fn skill_row(skill: usize) -> Skills {
    let mut s: Skills = fx::skill_rec();
    s.charclass = 4;
    s.skilldesc = 0;
    s.intown = true;
    s.range = 1;
    s.mana = 2;
    s.manashift = 8;
    s.anim = 7;
    match skill {
        BASH => {
            (s.srvstfunc, s.srvdofunc) = (32, 2);
            s.hitshift = 8;
        }
        // The warcries: a missile ring (`srvdo 68`) and a state on the
        // caster; Howl's own state sits on the monsters it scares.
        // A passive (the masteries, Natural Resistance, Iron Skin share the
        // shape): a state whose list holds `passivestat1` = `passivecalc1`.
        NATURAL_RESISTANCE => {
            s.passive = true;
            s.passivestate = PASSIVE_STATE as u16;
            s.passivestat1 = FIRE_RESIST as u16;
            s.passivecalc1 = RESIST_CALC;
            for f in [
                &mut s.passivestat2,
                &mut s.passivestat3,
                &mut s.passivestat4,
                &mut s.passivestat5,
            ] {
                *f = 0xFFFF;
            }
            s.srvstfunc = 0;
            s.srvdofunc = 0;
        }
        LEAP => {
            (s.srvstfunc, s.srvdofunc) = (40, 77);
            s.anim = 10;
            // The leap's reach (`aurarangecalc`, `bodies-2.md` §2.13).
            s.aurarangecalc = RESIST_CALC;
            // d2rs-own, unverified: the landing's area damage (`calc1`).
            s.calc1 = LEN_CALC;
        }
        // The kicks: the `Kick` flag makes the physical part
        // `(dex + str - 20) / 4..3` (`levels.md` §3.3); `Param1` / `Param2`
        // are the enhanced damage per level (`bodies-2.md` §2.6).
        DRAGON_TALON | DRAGON_FLIGHT => {
            (s.srvstfunc, s.srvdofunc) = if skill == DRAGON_TALON {
                (24, 42)
            } else {
                (0, 52)
            };
            s.kick = true;
            s.param1 = 5;
            s.param2 = 7;
            s.hitshift = 8;
            s.range = 1;
        }
        LEAP_ATTACK => {
            (s.srvstfunc, s.srvdofunc) = (41, 78);
            s.aurarangecalc = RESIST_CALC;
            // d2rs-own, unverified: the strike's enhanced damage (`calc1`).
            s.calc1 = LEN_CALC;
            s.srvoverlay = 0xFFFF;
            s.hitshift = 8;
            // Cast from a distance (range 2), then leap.
            s.range = 2;
            s.anim = 10;
        }
        WHIRLWIND => {
            (s.srvstfunc, s.srvdofunc) = (38, 76);
        }
        DOUBLE_SWING => {
            (s.srvstfunc, s.srvdofunc) = (32, 70);
            s.hitshift = 8;
        }
        SHOUT | BATTLE_ORDERS | HOWL => {
            (s.srvstfunc, s.srvdofunc) = (0, 68);
            s.auralencalc = LEN_CALC;
            s.srvmissile = 0xFFFF;
            s.srvmissilea = 0;
            s.aurastate = match skill {
                SHOUT => SHOUT_STATE,
                BATTLE_ORDERS => BATTLE_ORDERS_STATE,
                _ => HOWL_STATE,
            } as u16;
            for f in [
                &mut s.aurastat1,
                &mut s.aurastat2,
                &mut s.aurastat3,
                &mut s.aurastat4,
                &mut s.aurastat5,
                &mut s.aurastat6,
            ] {
                *f = 0xFFFF;
            }
        }
        _ => {}
    }
    s
}

pub struct Rig {
    pub app: App,
    pub ms: Arc<AtomicU32>,
    pub link: Arc<Mutex<ThreadLink<Link<StepClock>>>>,
}

impl Rig {
    /// The joined game with `skills` learned (level 1) by the barbarian.
    pub fn new(skills: &[usize]) -> Rig {
        Rig::with_rows(skills, skills)
    }

    /// Skill rows installed for `rows`; the barbarian has learned only
    /// `skills` (level 1) at the start.
    pub fn with_rows(rows: &[usize], skills: &[usize]) -> Rig {
        let character = single_player::new_character("barbarian", "Test").unwrap();
        let ms = Arc::new(AtomicU32::new(1000));
        let clock = StepClock(ms.clone());
        let spawn_character = character.clone();
        let rows: Vec<usize> = rows.to_vec();
        let link = ThreadLink::spawn(move || {
            let mut g = single_player::build_with(
                &crate::app_support::game_data(),
                single_player::DEFAULT_SEED,
                spawn_character,
            )?;
            install_fixtures(&mut g.sim, &rows);
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
        app.insert_resource(d2_client::bridge::mirror::ScriptedClock(ms.clone()));
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>();
        add_game(&mut app, dyn_link, false).unwrap();
        send_create_game_for(&mut app, &character).unwrap();
        let data = crate::app_support::game_data();
        let levels = single_player::client_level_rows(&data);
        add_client_data(
            &mut app,
            single_player::client_drlg_source(&data),
            levels.clone(),
        );
        add_preview(&mut app, levels, TileAssets::default());
        let mut r = Rig { app, ms, link };
        r.step(10);
        let learned: Vec<usize> = skills.to_vec();
        r.with(move |sim, p| {
            let h = &mut sim.events.action.sys.hooks;
            let rows = h.tables.skills.skills.clone();
            let list = h.skill_lists.entry(p).or_default();
            // The join ran the native skills on the synthetic `charstats`
            // (`client/msg-skills.md` §2 rule 8); this game's class skills
            // are the fixture's.
            *list = Default::default();
            let mut ids = [0xFFFFu16; 10];
            for (slot, s) in ids.iter_mut().zip(&learned) {
                *slot = *s as u16;
            }
            list.init_player(&rows, ListOwner::player(BARBARIAN as i32), Some(&ids))
                .unwrap();
        });
        r
    }

    pub fn step(&mut self, n: usize) {
        for _ in 0..n {
            self.app.update();
            self.ms.fetch_add(40, Ordering::SeqCst);
        }
    }

    /// Runs `f` on the server's sim with the local player.
    pub fn with<R: Send + 'static>(
        &mut self,
        f: impl FnOnce(&mut single_player::Sim, UnitId) -> R + Send + 'static,
    ) -> R {
        self.link
            .lock()
            .unwrap()
            .with(|l| {
                let sim = &mut l.host_mut().game;
                let (p, _) = single_player::local_player(sim).expect("joined");
                f(sim, p)
            })
            .unwrap()
    }
}

/// The synthetic game with the combat tables of this rig (set before the
/// player joins).
fn install_fixtures(sim: &mut single_player::Sim, learned: &[usize]) {
    let s = &mut sim.events.action.sys;
    let mut data = (*d2_sim::bench_fixtures::stat_data()).clone();
    // 200 states, none with a group flag: the warcries' states are plain.
    let states = BinTable {
        name: "states".into(),
        source: "synthetic".into(),
        count: 200,
        record_size: States::SIZE,
        records: vec![0u8; 200 * States::SIZE],
    };
    data.states = StateTable::new(
        &states,
        &StateMaps {
            words: 7,
            bitsets: vec![0; 40 * 7],
            ..StateMaps::default()
        },
    )
    .expect("states");
    s.stats = StatLists::new(Arc::new(data));
    let (monstats, monstats2) = dummy();
    let mut skills = fx::skills();
    let mut attack: Skills = fx::skill_rec();
    attack.srvdofunc = 1;
    attack.srvstfunc = 1;
    attack.anim = 7;
    attack.range = 1;
    attack.intown = true;
    skills.skills_code = vec![0, 0, 0, 0, 0x08, 0xF4, 0x01, 0x00, 0x08, 0x19, 0x00, 0x00];
    skills.skills = vec![fx::skill_rec(); 160];
    skills.skills[0] = attack;
    for &k in learned {
        skills.skills[k] = skill_row(k);
    }
    let mut combat = fx::combat_tables();
    combat.monstats = monstats.clone();
    combat.monstats2 = monstats2.clone();
    // d2rs-own, unverified: made-up speeds so the leap and the whirl
    // have a velocity (`charstats.txt` RunVelocity / WalkVelocity).
    let mut cs = Charstats::decode(&[0u8; Charstats::SIZE]);
    cs.walkvelocity = 6;
    cs.runvelocity = 9;
    combat.charstats = (0..7).map(|_| cs.clone()).collect();
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
    let player_modes = [
        "dt", "nu", "wl", "rn", "gh", "tn", "tw", "a1", "a2", "bl", "sc", "th", "kk", "s1", "s2",
        "s3", "s4", "dd", "sq",
    ];
    s.hooks.x.looks = Some(Arc::new(UnitLooks {
        player_tokens: vec![code(b"BA"); 7],
        player_modes: player_modes
            .iter()
            .map(|m| code(m.to_uppercase().as_bytes()))
            .collect(),
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

impl Rig {
    /// Out of the Rogue Encampment through the Blood Moor's cave entrance
    /// to the Den of Evil (a monster in a town room is no target, and the
    /// player is safe there).
    pub fn leave_town(&mut self) {
        let entrance = self
            .app
            .world()
            .resource::<BridgeResource>()
            .0
            .world()
            .units
            .iter()
            .find(|(k, u)| {
                k.unit_type == TILE
                    && u.class
                        == crate::app_support::warp_id(
                            single_player::BLOOD_MOOR,
                            single_player::DEN_OF_EVIL,
                        )
            })
            .map(|(k, _)| *k)
            .expect("the cave entrance reached the client");
        self.app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .interact(entrance)
            .unwrap();
        for _ in 0..200 {
            let level = self.with(move |sim, p| {
                let room = sim.game.lists.unit(p)?.room()?;
                sim.events.action.hooks().drlg.level_id(&sim.game, room)
            });
            if level == Some(single_player::DEN_OF_EVIL) {
                break;
            }
            self.step(1);
        }
        self.step(20);
        // Stand at one free spot of the Den whatever the walk's speed (the
        // tests aim their points from it; d2rs-own fixture position).
        self.with(move |sim, p| {
            use d2_sim::missiles::seams::MissileBodies;
            let room = sim.game.lists.unit(p).and_then(|u| u.room());
            sim.events
                .action
                .with(&mut sim.game, |g, v| v.path_teleport(g, p, room, 21, 61));
        });
        self.step(4);
        // A strong player: hits land, mana and life are plentiful.
        self.with(move |sim, p| {
            sim.events.action.with(&mut sim.game, |_, v| {
                for st in [stat::MANA, stat::MAXMANA] {
                    v.set_base(p, st, 100 << 8);
                }
                for st in [stat::HITPOINTS, stat::MAXHP] {
                    v.set_base(p, st, 100 << 8);
                }
                // The synthetic player is level 0: a level-0 attacker
                // hits 5% of the time (`hit.md` §3).
                v.set_base(p, stat::LEVEL, 20);
                v.set_base(p, TOHIT, 1000);
                // The kick's physical part reads strength and dexterity.
                v.set_base(p, 0, 100);
                v.set_base(p, 2, 100);
                v.set_base(p, MINDAMAGE, 5 << 8);
                v.set_base(p, MAXDAMAGE, 5 << 8);
            });
        });
    }

    /// A still monster of class 0, `dx` sub-tiles east of the player, with
    /// 100 life.
    pub fn spawn_monster(&mut self, dx: i32) -> UnitId {
        self.with(move |sim, p| {
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
                .with(&mut sim.game, |g, v| v.allocate(g, &req, px + dx, py))
                .expect("monster");
            a.with(&mut sim.game, |_, v| {
                v.set_base(m, stat::LEVEL, 1);
                v.set_base(m, stat::MAXHP, 100 << 8);
                v.set_base(m, stat::HITPOINTS, 100 << 8);
            });
            a.sys.units.get_mut(m).unwrap().flags |=
                flags::BIT1 | flags::IS_VALID_TARGET | flags::CAN_BE_ATTACKED;
            m
        })
    }

    /// Makes `skill` the player's right skill.
    pub fn select_right(&mut self, skill: usize) {
        self.with(move |sim, p| {
            let h = &mut sim.events.action.sys.hooks;
            let list = h.skill_lists.get_mut(&p).expect("list");
            let i = list
                .view()
                .iter()
                .position(|e| e.skill == skill as i32)
                .expect("learned");
            list.right = Some(i);
        });
    }

    fn send(&mut self, msg: &[u8]) {
        self.link
            .lock()
            .unwrap()
            .send(SendQueue::Game, msg)
            .unwrap();
    }

    /// C→S 0x0D: the right skill on a monster.
    pub fn right_click_unit(&mut self, m: UnitId) {
        let guid = self.with(move |sim, _| sim.game.lists.unit(m).expect("unit").guid);
        let mut msg = vec![0x0D];
        msg.extend(1u32.to_le_bytes());
        msg.extend(guid.to_le_bytes());
        self.send(&msg);
    }

    /// C→S 0x0C: the right skill at a point `dx`, `dy` sub-tiles from the
    /// player.
    pub fn right_click_point(&mut self, dx: i32, dy: i32) {
        let (px, py) = self.with(move |sim, p| sim.events.action.sys.hooks.path_position(p));
        let mut msg = vec![0x0C];
        msg.extend(((px + dx) as u16).to_le_bytes());
        msg.extend(((py + dy) as u16).to_le_bytes());
        self.send(&msg);
    }

    pub fn stat_of(&mut self, u: UnitId, st: u16) -> i32 {
        self.with(move |sim, _| sim.events.action.with(&mut sim.game, |_, v| v.stat(u, st)))
    }

    pub fn mana(&mut self) -> i32 {
        self.with(move |sim, p| {
            sim.events
                .action
                .with(&mut sim.game, |_, v| v.stat(p, stat::MANA))
        })
    }

    pub fn life(&mut self, u: UnitId) -> i32 {
        self.stat_of(u, stat::HITPOINTS)
    }

    /// The unit has a state list for `state` (the warcries put one).
    pub fn has_state(&mut self, u: UnitId, state: i16) -> bool {
        self.with(move |sim, _| {
            sim.events.action.with(&mut sim.game, |_, v| {
                v.state_list(u, state as u16).is_some()
            })
        })
    }

    /// Gives the player `n` unspent skill points (stat 5).
    pub fn give_skill_points(&mut self, n: i32) {
        self.with(move |sim, p| {
            sim.events
                .action
                .with(&mut sim.game, |_, v| v.set_base(p, 5, n));
        });
    }

    /// C→S 0x3B: spend a skill point on `skill`.
    pub fn add_skill_point(&mut self, skill: usize) {
        self.send(&[0x3B, skill as u8, (skill >> 8) as u8]);
    }

    /// `stat` of the player's list of `state`.
    pub fn state_stat(&mut self, state: i16, st: i16) -> Option<i32> {
        self.with(move |sim, p| {
            sim.events.action.with(&mut sim.game, |_, v| {
                v.state_stat(p, state as u16, st as u16)
            })
        })
    }

    pub fn player(&mut self) -> UnitId {
        self.with(|_, p| p)
    }

    pub fn errors(&mut self) -> String {
        self.with(move |sim, _| format!("{:?}", sim.events.action.sys.hooks.errors))
    }
}
