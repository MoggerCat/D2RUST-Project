// Spec: specs/skills/use.md §5; specs/sim/pathing.md §10 (C→S 0x03); specs/client/bridge.md §3
//! The skill test rig on the user's install (q-fixture-migrate; it
//! replaces the barbarian and skill-gaps rigs, which laid made-up skill,
//! state, monster and AnimData tables over the game): the play app's own
//! game and client on `$D2_GAME_DIR` with a new character of the class,
//! the named skills of the install's `skills` table learned at level 1,
//! and helpers to walk out of town, spawn a monster and send intents.
//! Every table is the install's; the tests read their expected numbers
//! from the same rows.
//!
//! Staging (test inputs, not game rules): the player's skill list is set
//! to the learned skills, and [`Rig::strengthen`] gives the player mana,
//! life, level and to-hit so a single swing lands; a monster is spawned
//! next to the player with [`Rig::spawn_monster`].
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
use d2_client::world_view::tile_assets::TileAssets;
use d2_data::tables::Skills;
use d2_server::adapters::ProtoSizes;
use d2_server::host::Host;
use d2_server::seams::Clock;
use d2_sim::missiles::unit_flag as flags;
use d2_sim::skills::list::ListOwner;
use d2_sim::stats::stat;
use d2_sim::units::lifecycle::AllocRequest;
use d2_sim::units::{UnitId, UnitType};

/// `monstats` row 5, `zombie1`: a Blood Moor native.
pub const ZOMBIE: u32 = 5;

const TOHIT: u16 = 19;

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

/// The install's `skills` row `id`.
pub fn skill_row(id: usize) -> Skills {
    let d = crate::app_support::live();
    let rows: Vec<Skills> = d.tables.rows().expect("skills");
    rows.into_iter()
        .find(|r| usize::from(r.skill) == id)
        .unwrap_or_else(|| panic!("no skills row {id}"))
}

pub struct Rig {
    pub app: App,
    pub ms: Arc<AtomicU32>,
    pub link: Arc<Mutex<ThreadLink<Link<StepClock>>>>,
}

impl Rig {
    /// The joined game of a new `class` character (a
    /// `single_player::new_character` class name) with `learned` skills
    /// (install `skills` ids) at level 1.
    pub fn new(class: &str, learned: &[usize]) -> Rig {
        Rig::with_seed(class, learned, single_player::DEFAULT_SEED)
    }

    /// [`Rig::new`] on the game seed `seed`.
    pub fn with_seed(class: &str, learned: &[usize], seed: u32) -> Rig {
        let character = single_player::new_character(class, "Test").unwrap();
        let class_id = i32::from(single_player::parse_class(class).unwrap());
        let ms = Arc::new(AtomicU32::new(1000));
        let clock = StepClock(ms.clone());
        let spawn_character = character.clone();
        let link = ThreadLink::spawn(move || {
            let g =
                single_player::build_with(&crate::app_support::game_data(), seed, spawn_character)?;
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
        let data = crate::app_support::game_data();
        let levels = single_player::client_level_rows(&data);
        add_client_data(
            &mut app,
            single_player::client_drlg_source(&data),
            levels.clone(),
        );
        add_preview(&mut app, levels, TileAssets::default());
        crate::app_support::live_tables(&mut app);
        let mut r = Rig { app, ms, link };
        while r.joined().is_none() {
            r.step(1);
        }
        r.step(10);
        let learned: Vec<u16> = learned.iter().map(|&s| s as u16).collect();
        r.with(move |sim, p| {
            let h = &mut sim.events.action.sys.hooks;
            let rows = h.tables.skills.skills.clone();
            let list = h.skill_lists.entry(p).or_default();
            *list = Default::default();
            let mut ids = [0xFFFFu16; 10];
            for (slot, s) in ids.iter_mut().zip(&learned) {
                *slot = *s;
            }
            list.init_player(&rows, ListOwner::player(class_id), Some(&ids))
                .unwrap();
        });
        r
    }

    fn joined(&self) -> Option<UnitId> {
        self.link
            .lock()
            .unwrap()
            .with(|l| single_player::local_player(&l.host().game).map(|(p, _)| p))
            .unwrap()
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

    fn send(&mut self, msg: &[u8]) {
        self.link
            .lock()
            .unwrap()
            .send(SendQueue::Game, msg)
            .unwrap();
    }

    pub fn pos(&mut self) -> (i32, i32) {
        self.with(|sim, p| sim.events.action.sys.hooks.path_position(p))
    }

    fn mode(&mut self) -> u32 {
        self.with(|sim, p| sim.events.action.sys.units.get(p).map_or(0, |u| u.mode))
    }

    pub fn level(&mut self) -> Option<u32> {
        self.with(|sim, p| {
            let room = sim.game.lists.unit(p)?.room()?;
            sim.events.action.hooks().drlg.level_id(&sim.game, room)
        })
    }

    /// A run leg (C→S 0x03), then frames until the player stops.
    pub fn leg(&mut self, (x, y): (i32, i32)) {
        let mut m = vec![0x03];
        m.extend_from_slice(&(x as u16).to_le_bytes());
        m.extend_from_slice(&(y as u16).to_le_bytes());
        self.send(&m);
        self.step(2);
        for _ in 0..400 {
            if !test_fixtures::host::MOVING.contains(&self.mode()) {
                break;
            }
            self.step(1);
        }
    }

    /// Out of the Rogue Encampment into the Blood Moor on foot (run legs
    /// along the collision route, `test_fixtures::host::route`), then
    /// twenty sub-tiles on into the level (a monster in a town room is no
    /// target).
    pub fn leave_town(&mut self) {
        let (town, moor) = (single_player::ACT1_TOWN, single_player::BLOOD_MOOR);
        for _ in 0..12 {
            if self.level() == Some(moor) {
                break;
            }
            let start = self.pos();
            let legs = self.with(move |sim, _| {
                let h = sim.events.action.hooks();
                let d = h.drlg.dungeon.acts[0].as_ref().expect("Act I");
                let rect = |id| {
                    d.find_level(id)
                        .map(|l| d.level(l).rect)
                        .expect("level allocated")
                };
                test_fixtures::host::route(d, start, rect(town), rect(moor), 12)
            });
            for g in legs {
                self.leg(g);
                if self.level() == Some(moor) {
                    break;
                }
            }
        }
        assert_eq!(self.level(), Some(moor), "walked into the Blood Moor");
        // On past the border: a free spot with room around it.
        let p = self.pos();
        let moor_rect = self.with(move |sim, _| {
            let d = sim.events.action.hooks().drlg.dungeon.acts[0]
                .as_ref()
                .unwrap();
            let l = d.find_level(moor).unwrap();
            d.level(l).rect
        });
        let centre = (
            (moor_rect.x * 2 + moor_rect.w) * 5 / 2,
            (moor_rect.y * 2 + moor_rect.h) * 5 / 2,
        );
        self.leg((
            p.0 + (centre.0 - p.0).clamp(-20, 20),
            p.1 + (centre.1 - p.1).clamp(-20, 20),
        ));
        self.step(4);
    }

    /// Staging: mana and life 100, character level 30 (the highest
    /// `reqlevel` of the skills these tests learn), to-hit 1000, so a
    /// swing lands (`combat/hit.md` §3) and a cast is paid.
    pub fn strengthen(&mut self) {
        self.with(move |sim, p| {
            sim.events.action.with(&mut sim.game, |_, v| {
                for st in [stat::MANA, stat::MAXMANA, stat::HITPOINTS, stat::MAXHP] {
                    v.set_base(p, st, 100 << 8);
                }
                v.set_base(p, stat::LEVEL, 30);
                v.set_base(p, TOHIT, 1000);
            });
        });
    }

    /// A zombie ([`ZOMBIE`]) `dx` sub-tiles east of the player, created
    /// as monster creation leaves it (no AI started), with 100 life.
    pub fn spawn_monster(&mut self, dx: i32) -> UnitId {
        self.spawn_monster_at(dx, 0)
    }

    /// [`Rig::spawn_monster`] `dx`, `dy` sub-tiles from the player.
    pub fn spawn_monster_at(&mut self, dx: i32, dy: i32) -> UnitId {
        self.with(move |sim, p| {
            let a = &mut sim.events.action;
            let (px, py) = a.sys.hooks.path_position(p);
            let room = sim.game.lists.unit(p).and_then(|e| e.room()).expect("room");
            let req = AllocRequest {
                ty: UnitType::Monster,
                class: ZOMBIE,
                room: Some(room),
                add: true,
                fixed_guid: None,
                mode: 1,
                allied: false,
            };
            let m = a
                .with(&mut sim.game, |g, v| v.allocate(g, &req, px + dx, py + dy))
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

    /// C→S 0x0D: the right skill on a monster.
    pub fn right_click_unit(&mut self, m: UnitId) {
        let guid = self.with(move |sim, _| sim.game.lists.unit(m).expect("unit").guid);
        let mut msg = vec![0x0D];
        msg.extend(1u32.to_le_bytes());
        msg.extend(guid.to_le_bytes());
        self.send_traced(&msg);
    }

    /// C→S 0x0C: the right skill at a point `dx`, `dy` sub-tiles from the
    /// player.
    pub fn right_click_point(&mut self, dx: i32, dy: i32) {
        let (px, py) = self.pos();
        let mut msg = vec![0x0C];
        msg.extend(((px + dx) as u16).to_le_bytes());
        msg.extend(((py + dy) as u16).to_le_bytes());
        self.send_traced(&msg);
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

    /// The unit has a state list for `state`.
    pub fn has_state(&mut self, u: UnitId, state: u16) -> bool {
        self.with(move |sim, _| {
            sim.events
                .action
                .with(&mut sim.game, |_, v| v.state_list(u, state).is_some())
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
    pub fn state_stat(&mut self, state: u16, st: u16) -> Option<i32> {
        self.with(move |sim, p| {
            sim.events
                .action
                .with(&mut sim.game, |_, v| v.state_stat(p, state, st))
        })
    }

    pub fn player(&mut self) -> UnitId {
        self.with(|_, p| p)
    }

    /// [`Rig::send`]; with `RIG_DEBUG` set, the message's fate and the
    /// player's mode over the next frames are printed (diagnostics).
    fn send_traced(&mut self, msg: &[u8]) {
        if std::env::var_os("RIG_DEBUG").is_none() {
            self.send(msg);
            return;
        }
        let fate = self.send_for_fate(msg);
        let mut modes = Vec::new();
        for _ in 0..30 {
            let m = self.mode();
            if modes.last() != Some(&m) {
                modes.push(m);
            }
            self.step(1);
        }
        let (mana, used) = self.with(|sim, p| {
            let h = &sim.events.action.sys.hooks;
            (
                sim.events.action.sys.stats.unit_base(p, stat::MANA, 0) >> 8,
                format!("{:?}", h.used_skill_of(p)),
            )
        });
        eprintln!(
            "RIG {:02X?}: {fate}; modes {modes:?}; mana {mana}; used {used}",
            &msg[..1]
        );
    }

    /// The drained C→S messages of the last server frame (id, fate).
    pub fn drained(&mut self) -> Vec<(u8, String)> {
        self.link
            .lock()
            .unwrap()
            .with(|l| {
                l.last_frame()
                    .messages
                    .iter()
                    .map(|m| (m.id, format!("{:?}", m.handled)))
                    .collect()
            })
            .unwrap()
    }

    /// Sends `msg` (C→S) and steps one frame at a time until the server
    /// drains it; returns its fate.
    pub fn send_for_fate(&mut self, msg: &[u8]) -> String {
        let id = msg[0];
        self.send(msg);
        for _ in 0..5 {
            self.step(1);
            if let Some((_, f)) = self.drained().into_iter().find(|(i, _)| *i == id) {
                return f;
            }
        }
        "not drained".into()
    }

    pub fn errors(&mut self) -> String {
        self.with(move |sim, _| format!("{:?}", sim.events.action.sys.hooks.errors))
    }
}
