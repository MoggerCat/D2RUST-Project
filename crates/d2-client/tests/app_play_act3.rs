// Spec: specs/world/quests-act3.md, specs/world/quests-act3-2.md, specs/world/quests.md (§4, §6, §7), specs/world/npc.md (§2, §3)
//! Act III played headless on the user's install (task `q-play-act3`):
//! a character saved at the act's start by `d2s-tool` (Acts I and II
//! done, Kurast Docks known) joins in Kurast Docks, wired as `d2-client
//! play` wires it (`add_live_client`); the test walks, clicks NPCs and
//! objects through the client and sets up far states with pokes
//! (`tools/poke.md`), then checks the server's quest state and the
//! client's model. Every call the server's host had no provider for
//! (`AppRest::log`) fails the test.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{add_live_client, LiveClient};
use d2_client::app::single_player::{self, GameData};
use d2_client::bridge::hover;
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::mirror::DynLink;
use d2_client::bridge::BridgeResource;
use d2_client::rules::camera::{moving_to_client, Camera, FrameSize, OpenMode};
use d2_client::ui::{Point, PointerButton, UiEvent};
use d2_client::world_view::walk::PreviewWalk;
use d2_client::world_view::WorldViewUi;
use d2_server::seams::Clock;
use d2_sim::world::quests::act3::npc;

mod app_support;

use app_support::{Server, SharedLink};

/// What crossed the link: the C→S messages.
#[derive(Default)]
struct Wire {
    sent: Vec<Vec<u8>>,
}

/// A link that records what crosses it.
struct Recorder {
    inner: DynLink,
    wire: Arc<Mutex<Wire>>,
}

impl ServerLink for Recorder {
    fn protocol_version(&self) -> u32 {
        self.inner.protocol_version()
    }
    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        self.wire.lock().unwrap().sent.push(msg.to_vec());
        self.inner.send(queue, msg)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        self.inner.pump()
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        self.inner.receive()
    }
}

struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

/// The game in play: the app, its server thread, the clock, the wire.
struct Play {
    app: App,
    server: Server<StepClock>,
    ms: Arc<AtomicU32>,
    wire: Arc<Mutex<Wire>>,
}

/// The `d2s-tool new` flags of the Act III start save: a level 25
/// sorceress, Acts I and II done (every quest's bit 0 and the act
/// transitions), the waypoints of Acts I–II and Kurast Docks, standing in
/// Act III.
const ACT3_QUESTS: &str =
    "0:acts=2,0:1.0,0:2.0,0:3.0,0:4.0,0:5.0,0:6.0,0:9.0,0:10.0,0:11.0,0:12.0,0:13.0,0:14.0";
const ACT3_SAVE: &[&str] = &[
    "--class",
    "sor",
    "--level",
    "25",
    "--expansion",
    "--waypoints",
    "0:0,0:1,0:2,0:3,0:4,0:5,0:6,0:7,0:8,0:9,0:10,0:11,0:12,0:13,0:14,0:15,0:16,0:17,0:18",
    "--act",
    "2",
    "--gold",
    "50000",
    "--all-skills",
    "1",
    "--right-skill",
    "36",
];

/// Writes the Act III start save with `d2s-tool` (`quests`: more quest
/// bits, `d2s-tool --quests` items, after [`ACT3_QUESTS`]) and loads it
/// as `play --save` does.
fn act3_character(quests: &str) -> single_player::Character {
    let dir = std::env::temp_dir().join(format!(
        "d2rs-play-act3-{}-{}",
        std::process::id(),
        std::thread::current()
            .name()
            .unwrap_or("t")
            .replace("::", "-")
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ActThree.d2s");
    let mut args: Vec<String> = ["new", "--name", "ActThree"]
        .iter()
        .chain(ACT3_SAVE)
        .map(|s| s.to_string())
        .collect();
    args.push("--quests".into());
    args.push(if quests.is_empty() {
        ACT3_QUESTS.to_string()
    } else {
        format!("{ACT3_QUESTS},{quests}")
    });
    args.push("-o".into());
    args.push(path.display().to_string());
    assert_eq!(d2s_tool::cli::run(&args, &mut std::io::sink()).unwrap(), 0);
    let c = single_player::load_character(&app_support::game_data(), &path, 0).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    c
}

impl Play {
    /// The play app on the user's install, joined with the Act III save
    /// (`quests`: as [`act3_character`]).
    fn start(quests: &str) -> Self {
        let ms = Arc::new(AtomicU32::new(1000));
        let wire = Arc::new(Mutex::new(Wire::default()));
        let data = app_support::game_data();
        let GameData::Live(live) = data.clone();
        let character = act3_character(quests);
        let speeds = single_player::walk_speeds(&data, &character).unwrap();
        let (link, started) = single_player::start_with(
            data,
            single_player::DEFAULT_SEED,
            character.clone(),
            StepClock(ms.clone()),
        )
        .unwrap();
        let server: Server<StepClock> = Arc::new(Mutex::new(link));
        let mut app = App::new();
        app.insert_resource(d2_client::bridge::mirror::ScriptedClock(ms.clone()));
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_resource::<ButtonInput<MouseButton>>();
        let link = Recorder {
            inner: Box::new(SharedLink(server.clone())),
            wire: wire.clone(),
        };
        add_live_client(
            &mut app,
            Box::new(link),
            LiveClient {
                data: &live,
                request: &character,
                start_flags: None,
                prices: started.prices,
                speeds,
                hardcore: false,
                automap_files: None,
                gpu: false,
            },
        )
        .unwrap();
        let mut p = Play {
            app,
            server,
            ms,
            wire,
        };
        let mut n = 0;
        while app_support::local_player(&p.server).is_none() {
            p.step(1);
            n += 1;
            assert!(n < 2000, "the join");
        }
        p.step(10);
        p
    }

    fn step(&mut self, n: usize) {
        for _ in 0..n {
            self.app.update();
            self.ms.fetch_add(40, Ordering::SeqCst);
        }
    }

    fn queue(&mut self, e: UiEvent) {
        self.app
            .world_mut()
            .non_send_mut::<WorldViewUi>()
            .queue
            .0
            .push(e);
    }

    fn click(&mut self, at: Point) {
        self.queue(UiEvent::Press {
            button: PointerButton::Left,
            at,
        });
        self.queue(UiEvent::Release {
            button: PointerButton::Left,
            at,
        });
        self.step(2);
    }

    fn level(&self) -> Option<u32> {
        app_support::server_level(&self.server)
    }

    /// The calls the server's host had no provider for, so far.
    fn unhandled(&self) -> Vec<String> {
        app_support::with(&self.server, |l| l.host().game.world.rest.log.clone())
    }

    fn assert_clean(&self, what: &str) {
        let log = self.unhandled();
        assert!(log.is_empty(), "{what}: unhandled host calls {log:?}");
        let b = &self.app.world().resource::<BridgeResource>().0;
        assert!(
            b.log().rejected.is_empty(),
            "{what}: rejected {:?}",
            b.log().rejected
        );
    }

    /// The local player's quest bit `slot.bit` (normal) on the server.
    fn bit(&self, slot: u8, bit: u8) -> bool {
        app_support::with(&self.server, move |l| {
            let g = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(g).expect("joined");
            g.world
                .rest
                .quests
                .get(&p)
                .is_some_and(|q| q.flags[0].get(slot, bit))
        })
    }

    /// (state, status) of quest chain `chain` on the server.
    fn chain(&self, chain: u8) -> Option<(u8, u8)> {
        app_support::with(&self.server, move |l| {
            let r = l.host().game.world.quests.record(chain)?;
            Some((r.state, r.status))
        })
    }

    /// The C→S 0x31 messages sent so far, as (GUID, message).
    fn quest_messages(&self) -> Vec<(u32, u32)> {
        self.wire
            .lock()
            .unwrap()
            .sent
            .iter()
            .filter(|m| m[0] == 0x31)
            .map(|m| {
                (
                    u32::from_le_bytes(m[1..5].try_into().unwrap()),
                    u32::from_le_bytes(m[5..9].try_into().unwrap()),
                )
            })
            .collect()
    }

    fn ids(&self) -> Vec<u8> {
        self.wire
            .lock()
            .unwrap()
            .sent
            .iter()
            .map(|m| m[0])
            .collect()
    }

    /// The screen point of the model's monster of `class`, as the click's
    /// camera sees it.
    fn npc_on_screen(&self, class: u16) -> Option<(u32, Point)> {
        let w = self.app.world().resource::<BridgeResource>().0.world();
        let (key, u) = w.units.iter().find(|(k, u)| {
            k.unit_type == 1 && u.class == u32::from(class) && u.position.is_some()
        })?;
        let at = self
            .app
            .world()
            .resource::<PreviewWalk>()
            .predict
            .position()?;
        let cam = Camera::new(
            FrameSize::D2RS,
            OpenMode::NONE,
            moving_to_client(at.0, at.1),
            (0, 0),
        );
        let (x, y) = hover::unit_feet(&cam, u.key.unit_type, u.position.unwrap());
        Some((key.guid, Point::new(x, y - 20)))
    }

    /// Walks to the town NPC of `class`, clicks it and waits for its menu
    /// (S→C 0x28 and the menu box), then leaves the menu by its last row.
    /// The quest messages (C→S 0x31) the talk sent.
    fn talk(&mut self, class: u16) -> Vec<u32> {
        app_support::approach(
            &mut self.app,
            &self.server,
            &self.ms,
            1,
            &[u32::from(class)],
        );
        // The client refuses a repeat interact on the same monster within
        // 200 ms (`client/model.md` §8 r7).
        self.step(10);
        let (guid, at) = self.npc_on_screen(class).expect("the NPC in the model");
        assert!(
            (0..800).contains(&at.x) && (0..560).contains(&at.y),
            "NPC {class} on screen: {at:?}"
        );
        let mut want = vec![0x13, 1, 0, 0, 0];
        want.extend_from_slice(&guid.to_le_bytes());
        let count = |p: &Play| {
            p.wire
                .lock()
                .unwrap()
                .sent
                .iter()
                .filter(|m| **m == want)
                .count()
        };
        let npc_at = app_support::server_unit(&self.server, 1, &[u32::from(class)]).map(|u| u.1);
        let me = app_support::server_pos(&self.server);
        eprintln!("talk {class}: player {me:?} NPC {npc_at:?}");
        let before_msgs = self.quest_messages().len();
        let before = count(self);
        self.click(at);
        for _ in 0..300 {
            self.step(1);
            if count(self) > before {
                break;
            }
        }
        assert!(
            count(self) > before,
            "C→S 0x13 on NPC {class}: {:?}",
            self.ids()
        );
        for _ in 0..80 {
            self.step(1);
            if app_support::npc_menu_len(&self.app) > 0 {
                break;
            }
        }
        self.step(2);
        let n = app_support::npc_menu_len(&self.app);
        assert!(
            n > 0,
            "NPC {class}'s menu opens; host log {:?}, sent {:?}",
            self.unhandled(),
            self.ids()
        );
        let p = app_support::npc_menu_row(&self.app, n - 1);
        self.click(p);
        self.step(3);
        self.quest_messages()[before_msgs..]
            .iter()
            .filter(|m| m.0 == guid)
            .map(|m| m.1)
            .collect()
    }

    fn send(&mut self, m: &[u8]) {
        self.app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .send_bytes(m)
            .unwrap();
    }

    /// Runs the poke `line` (`tools/poke.md` §1) on the server game now.
    fn poke(&mut self, line: &str) -> d2_sim::poke::PokeResult {
        let toks: Vec<String> = line.split_whitespace().map(String::from).collect();
        app_support::with(&self.server, move |l| {
            let t: Vec<&str> = toks.iter().map(|s| s.as_str()).collect();
            let op = d2_sim::poke::parse_op(&t).expect("poke parses");
            d2_client::app::poke::apply_now(&mut l.host_mut().game, &op)
        })
    }

    /// `warp <level>` and the frames until the client's model follows.
    fn warp(&mut self, level: u32) {
        let r = self.poke(&format!("warp {level}"));
        assert!(
            matches!(r, d2_sim::poke::PokeResult::Ok(_)),
            "warp {level}: {r:?}"
        );
        for _ in 0..400 {
            self.step(1);
            if self.level() == Some(level) && self.model_level() == Some(level as u16) {
                break;
            }
        }
        assert_eq!(self.level(), Some(level), "the server player in {level}");
        self.step(20);
        assert_eq!(
            self.model_level(),
            Some(level as u16),
            "the model in {level}"
        );
    }

    fn model_level(&self) -> Option<u16> {
        self.app
            .world()
            .resource::<BridgeResource>()
            .0
            .world()
            .player_level()
    }

    /// Whether the server's unit `ty`/`guid` is gone or dead.
    fn dead(&self, ty: u8, guid: u32) -> bool {
        app_support::with(&self.server, move |l| {
            let g = &mut l.host_mut().game;
            let st = match ty {
                1 => d2_sim::units::UnitType::Monster,
                _ => d2_sim::units::UnitType::Object,
            };
            let Some(u) = g.game.lists.find_unit(st, guid) else {
                return true;
            };
            g.events
                .action
                .sys
                .units
                .get(u)
                .is_none_or(|r| matches!(r.mode, 0 | 12))
        })
    }

    /// Kills monster `guid`: its life set to 1 by a poke, the player
    /// placed beside it, then right-skill casts (C→S 0x0D) on it until
    /// it dies.
    fn kill(&mut self, guid: u32) {
        let at = self.unit_pos(1, guid).expect("the monster");
        self.poke(&format!("pos @player {} {}", at.0 + 3, at.1 + 3));
        self.step(5);
        self.poke(&format!("stat 1/{guid} 6 0 256"));
        for _ in 0..40 {
            let mut m = vec![0x0D];
            m.extend_from_slice(&1u32.to_le_bytes());
            m.extend_from_slice(&guid.to_le_bytes());
            self.send(&m);
            self.step(15);
            if self.dead(1, guid) {
                break;
            }
        }
        assert!(self.dead(1, guid), "monster {guid} dies");
        self.step(30);
    }

    fn unit_pos(&self, ty: u8, guid: u32) -> Option<(i32, i32)> {
        app_support::with(&self.server, move |l| {
            let g = &mut l.host_mut().game;
            let st = match ty {
                1 => d2_sim::units::UnitType::Monster,
                4 => d2_sim::units::UnitType::Item,
                _ => d2_sim::units::UnitType::Object,
            };
            let u = g.game.lists.find_unit(st, guid)?;
            Some(g.events.action.hooks().path_position(u))
        })
    }

    /// The model's ground item of `code`, picked up (C→S 0x16 to the
    /// cursor... no: 0x16 cursor 0 puts it in the inventory).
    fn pick_up(&mut self, code: &[u8; 4]) {
        let item = {
            let w = self.app.world().resource::<BridgeResource>().0.world();
            d2_client::bridge::items::ground_items(w)
                .into_iter()
                .find(|i| i.code.as_ref() == Some(code))
                .unwrap_or_else(|| panic!("{} on the ground", String::from_utf8_lossy(code)))
        };
        let (x, y) = (i32::from(item.x), i32::from(item.y));
        self.poke(&format!("pos @player {} {}", x + 1, y + 1));
        self.step(5);
        let mut m = vec![0x16];
        m.extend_from_slice(&4u32.to_le_bytes());
        m.extend_from_slice(&item.key.guid.to_le_bytes());
        m.extend_from_slice(&0u32.to_le_bytes());
        self.send(&m);
        self.step(40);
        assert!(
            self.holds(code),
            "{} picked up",
            String::from_utf8_lossy(code)
        );
    }

    /// The model's item GUID of the local player's item `code`.
    fn item_guid(&self, code: &[u8; 4]) -> Option<u32> {
        let w = self.app.world().resource::<BridgeResource>().0.world();
        d2_client::bridge::items::local_items(w)
            .iter()
            .find(|i| i.code.as_ref() == Some(code))
            .map(|i| i.key.guid)
    }

    /// The local player's base stat `stat` (layer 0) on the server.
    fn player_stat(&self, stat: u16) -> i32 {
        app_support::with(&self.server, move |l| {
            let g = &mut l.host_mut().game;
            let (p, _) = single_player::local_player(g).expect("joined");
            g.events.action.sys.stats.unit_base(p, stat, 0)
        })
    }

    /// Whether the model's player holds an item of `code`.
    fn holds(&self, code: &[u8; 4]) -> bool {
        let w = self.app.world().resource::<BridgeResource>().0.world();
        d2_client::bridge::items::local_items(w)
            .iter()
            .any(|i| i.code.as_ref() == Some(code))
    }

    /// Chain 18's chosen boss (its GUID), if any.
    fn bird_boss(&self) -> Option<u32> {
        app_support::with(&self.server, |l| {
            let r = l.host().game.world.quests.record(18)?;
            let x = &r.extra.act3.q4;
            x.chosen.then_some(x.boss_guid)
        })
    }

    /// Places the player at the centre of each room of its level in turn
    /// (the rooms come into play and populate) until `done`.
    fn tour<F: FnMut(&Play) -> bool>(&mut self, mut done: F) -> bool {
        let (level, act) = app_support::level_act(&self.server);
        let rooms: Vec<(i32, i32)> = app_support::with(&self.server, move |l| {
            let g = &mut l.host_mut().game;
            let d = g.events.action.hooks().drlg.dungeon.acts[act]
                .as_ref()
                .expect("the act");
            let lv = d.find_level(level).expect("the level");
            d.level_rooms(lv)
                .into_iter()
                .map(|r| {
                    let t = d.room(r).rect;
                    ((t.x * 2 + t.w) * 5 / 2, (t.y * 2 + t.h) * 5 / 2)
                })
                .collect()
        });
        for (x, y) in rooms {
            if done(self) {
                return true;
            }
            self.poke(&format!("pos @player {x} {y}"));
            self.step(8);
        }
        done(self)
    }

    /// The server object of `class` in the player's level nearest the
    /// player (the level's rooms toured until one exists): its GUID.
    fn find_object(&mut self, class: u32) -> u32 {
        let found = |p: &Play| app_support::server_unit(&p.server, 2, &[class]);
        assert!(
            self.tour(|p| found(p).is_some()),
            "object {class} in the level"
        );
        found(self).unwrap().0
    }

    /// Operates object `guid` (C→S 0x13 type 2) from beside it.
    fn operate(&mut self, guid: u32) {
        let at = self.unit_pos(2, guid).expect("the object");
        self.poke(&format!("pos @player {} {}", at.0 + 2, at.1 + 2));
        self.step(5);
        let mut m = vec![0x13];
        m.extend_from_slice(&2u32.to_le_bytes());
        m.extend_from_slice(&guid.to_le_bytes());
        self.send(&m);
        self.step(40);
    }
}

// Covers: specs/world/npc.md §2, §3; specs/world/quests-act3.md §9
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn kurast_docks_arrival_and_every_town_npc_talks() {
    let mut p = Play::start("");
    assert_eq!(p.level(), Some(75), "the save joins in Kurast Docks");
    {
        let b = &p.app.world().resource::<BridgeResource>().0;
        assert_eq!(b.world().act.as_ref().map(|a| a.act), Some(2));
    }
    p.assert_clean("arrival");
    for class in [
        npc::CAIN3,
        npc::ORMUS,
        npc::ALKOR,
        npc::ASHEARA,
        npc::HRATLI,
        npc::MESHIF2,
    ] {
        let msgs = p.talk(class);
        eprintln!("NPC {class}: quest messages {msgs:?}");
        p.assert_clean(&format!("talk to {class}"));
    }
}

// Covers: specs/world/quests-act3.md §6.2–§6.7
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn the_golden_bird_from_the_jungle_boss_to_the_potion_of_life() {
    let mut p = Play::start("");
    p.warp(76);
    assert!(
        p.tour(|p| p.bird_boss().is_some()),
        "a Spider Forest boss carries the figurine"
    );
    let boss = p.bird_boss().unwrap();
    p.kill(boss);
    p.assert_clean("the bird boss's death");
    assert_eq!(p.chain(18).map(|c| c.0), Some(1), "the figurine dropped");
    p.pick_up(b"j34 ");
    p.warp(75);
    let cain = p.talk(npc::CAIN3);
    assert!(cain.contains(&527), "Cain on the figurine: {cain:?}");
    assert!(p.bit(20, 2));
    let meshif = p.talk(npc::MESHIF2);
    assert!(
        meshif.contains(&529),
        "Meshif takes the figurine: {meshif:?}"
    );
    p.step(10);
    assert!(p.holds(b"g34 "), "the Golden Bird given");
    assert!(!p.holds(b"j34 "), "the figurine taken");
    let cain = p.talk(npc::CAIN3);
    assert!(cain.contains(&531), "Cain on the bird: {cain:?}");
    let alkor = p.talk(npc::ALKOR);
    assert!(alkor.contains(&534), "Alkor takes the bird: {alkor:?}");
    assert!(p.bit(20, 1), "the reward pending");
    p.step(100);
    let alkor = p.talk(npc::ALKOR);
    assert!(alkor.contains(&538), "Alkor's reward: {alkor:?}");
    p.step(10);
    assert!(p.bit(20, 0), "the quest done");
    assert!(p.holds(b"xyz "), "the Potion of Life");
    // Drinking it (C→S 0x20 UseGridItem): +20 maximum life, bit 20.5
    // cleared, the potion gone (`quests-act3.md` §6.6).
    let maxhp = |p: &Play| p.player_stat(7);
    let before = maxhp(&p);
    let potion = p.item_guid(b"xyz ").unwrap();
    let mut m = vec![0x20];
    m.extend_from_slice(&potion.to_le_bytes());
    m.extend_from_slice(&0u32.to_le_bytes());
    m.extend_from_slice(&0u32.to_le_bytes());
    p.send(&m);
    p.step(20);
    assert_eq!(maxhp(&p), before + 20 * 256, "the potion adds 20 life");
    assert!(!p.bit(20, 5));
    assert!(!p.holds(b"xyz "), "the potion is used up");
    p.assert_clean("the Golden Bird");
}

/// The Gidbinn decoy object (`objects.txt` row 252, `quests-act3.md` §1.4).
const GIDBINN_DECOY: u32 = 252;

// Covers: specs/world/quests-act3.md §5.3–§5.8
#[test]
#[ignore = "real data: needs D2_GAME_DIR (tools/realdata-gate.sh)"]
fn the_blade_of_the_old_religion_from_hratli_to_ormus_and_asheara() {
    // The Golden Bird done: the sequence starts the Blade (§1.3).
    let mut p = Play::start("0:20.0");
    let hratli = p.talk(npc::HRATLI);
    assert!(hratli.contains(&571), "Hratli starts the Blade: {hratli:?}");
    p.step(10);
    assert_eq!(p.chain(17).map(|c| c.1), Some(2), "status 2 after the chat");
    p.warp(78);
    let decoy = p.find_object(GIDBINN_DECOY);
    p.operate(decoy);
    let boss = app_support::with(&p.server, |l| {
        let r = l.host().game.world.quests.record(17)?;
        let e = &r.extra.act3.q3;
        e.boss_spawned.then_some(e.boss_guid)
    });
    let boss = boss.expect("the decoy spawns the Gidbinn guardian");
    p.kill(boss);
    p.assert_clean("the guardian's death");
    p.pick_up(b"g33 ");
    assert!(p.bit(19, 5), "19.5: holds the Gidbinn");
    p.warp(75);
    let ormus = p.talk(npc::ORMUS);
    assert!(ormus.contains(&587), "Ormus takes the Gidbinn: {ormus:?}");
    assert!(p.bit(19, 6));
    assert!(!p.holds(b"g33 "));
    p.step(50);
    let ormus = p.talk(npc::ORMUS);
    assert!(ormus.contains(&593), "Ormus' ring: {ormus:?}");
    p.step(10);
    assert!(p.holds(b"rin "), "the ring");
    let asheara = p.talk(npc::ASHEARA);
    assert!(asheara.contains(&589), "Asheara's mercenary: {asheara:?}");
    assert!(p.bit(19, 0), "the quest done");
    p.assert_clean("the Blade");
}
