// Spec: specs/world/quests.md (§4.3–§4.6, §6.2, §7.3), specs/world/quests-act1.md (§10), specs/world/quests-act2.md (§8), specs/world/quests-act3.md (§8), specs/world/quests-act4.md (§3–§5), specs/world/quests-act5.md, specs/world/npc.md (§2, §3, §8.3), specs/client/msg-ui.md (§16), specs/client/bridge.md (§6)
//! (q-smoke-quests) Readiness smoke test of the quest lines through the
//! real play path, headless: the bridge, the in-process server thread
//! and the sim of the synthetic single-player game, with the original UI
//! (the quest log) and no Bevy window. Each act's main line is walked as
//! far as the synthetic game's content allows; at every step the server
//! quest record moves, the client's quest record and quest log show it,
//! rewards land, and nothing is refused, dropped or left unhandled on
//! either side of the link (`docs/handoff/q-smoke-quests.md` lists what
//! the synthetic game lacks).
//!
//! Steps the synthetic game cannot drive from the client (a border
//! crossing between outdoor levels, a boss kill, a level only reachable by
//! a missing outdoor link) are done on the server the way the existing
//! per-quest app tests do them, and say so where they happen.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::palette::{add_act_palettes, ActPalettes};
use d2_client::app::play::{
    add_client_data, add_game, add_preview, add_walk, predict_link, send_create_game_for,
};
use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, GameData, Link};
use d2_client::app::ui::{add_original_ui_with, UiParts};
use d2_client::assets::path::MemorySource;
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::mirror::DynLink;
use d2_client::bridge::predict::Speeds;
use d2_client::bridge::world::{UnitKey, OBJECT, TILE};
use d2_client::bridge::BridgeResource;
use d2_client::controls::Action;
use d2_client::rules::unit_composite::code;
use d2_client::ui::layout::{OptionKind, Screen};
use d2_client::ui::original::{FontMeasure, OriginalUi, UiConfig, CHARACTER_FONTS};
use d2_client::ui::quest_log::IconState;
use d2_client::ui::{font_info, ActionId, Point, PointerButton, UiEvent};
use d2_client::world_view::tile_assets::TileAssets;
use d2_client::world_view::unit_assets::UnitLooks;
use d2_client::world_view::WorldViewUi;
use d2_server::dispatch::Outcome;
use d2_server::host::{Handled, HandledMessage};
use d2_server::seams::{Clock, ResultCode};
use d2_sim::units::{UnitId, UnitType};
use d2_sim::wiring::action::Pending;

mod app_support;

// ---------------------------------------------------------------- rig --

/// The C→S messages the client sent.
#[derive(Default)]
struct Wire {
    sent: Vec<Vec<u8>>,
}

type Handle = Arc<Mutex<ThreadLink<Link<StepClock>>>>;

struct Shared(Handle);

impl ServerLink for Shared {
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

/// A DT1 of one tile with one 32 × 32 RLE block (`formats/dt1.md`).
fn dt1_bytes() -> Vec<u8> {
    let encoded = [0x00u8, 0x02, 0x0A, 0x0B];
    let mut d = Vec::new();
    d.extend_from_slice(&7u32.to_le_bytes());
    d.extend_from_slice(&6u32.to_le_bytes());
    d.extend_from_slice(&[0; 260]);
    d.extend_from_slice(&1u32.to_le_bytes());
    d.extend_from_slice(&276u32.to_le_bytes());
    let mut tile = vec![0u8; 96];
    tile[0x48..0x4C].copy_from_slice(&372u32.to_le_bytes());
    tile[0x50..0x54].copy_from_slice(&1u32.to_le_bytes());
    d.extend_from_slice(&tile);
    for v in [0u16, 0, 0] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    d.extend_from_slice(&[0, 0]);
    d.extend_from_slice(&0x1001u16.to_le_bytes());
    d.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
    d.extend_from_slice(&0u16.to_le_bytes());
    d.extend_from_slice(&20u32.to_le_bytes());
    d.extend_from_slice(&encoded);
    d
}

/// A DC6 of one direction with `frames` frames of 2 × 2 pixels.
fn dc6(frames: u32) -> Vec<u8> {
    let rows = [2u8, 1, 2, 0x80, 2, 3, 4, 0x80];
    let mut d = Vec::new();
    for v in [6i32, 1, 0] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    d.extend_from_slice(&[0xEE; 4]);
    d.extend_from_slice(&1u32.to_le_bytes());
    d.extend_from_slice(&frames.to_le_bytes());
    let mut at = d.len() + 4 * frames as usize;
    let mut body = Vec::new();
    for _ in 0..frames {
        d.extend_from_slice(&(at as u32).to_le_bytes());
        for v in [0u32, 2, 2, 0, 0, 0, 0, rows.len() as u32] {
            body.extend_from_slice(&v.to_le_bytes());
        }
        body.extend_from_slice(&rows);
        body.extend_from_slice(&[0xEE; 3]);
        at += 32 + rows.len() + 3;
    }
    d.extend(body);
    d
}

/// A one-layer COF (`formats/cof.md`).
fn cof_bytes() -> Vec<u8> {
    let mut v = vec![1, 1, 1, 20, 0, 0, 0, 0];
    for x in [-10i32, 10, -20, 0] {
        v.extend_from_slice(&x.to_le_bytes());
    }
    v.extend_from_slice(&256u32.to_le_bytes());
    v.extend_from_slice(&[1, 0, 1, 0, 0]);
    v.extend_from_slice(b"hth\0");
    v.push(0);
    v.push(1);
    v
}

/// A `.tbl` of 256 records of width 6 (`formats/font-tbl.md`).
fn tbl() -> Vec<u8> {
    let mut d = b"Woo!".to_vec();
    d.extend_from_slice(&1u16.to_le_bytes());
    d.extend_from_slice(&0u16.to_le_bytes());
    d.extend_from_slice(&256u16.to_le_bytes());
    d.extend_from_slice(&[10, 0]);
    for i in 0..256u16 {
        d.extend_from_slice(&i.to_le_bytes());
        d.extend_from_slice(&[0, 6, 10, 0, 0, 0]);
        d.extend_from_slice(&i.to_le_bytes());
        d.extend_from_slice(&[0; 4]);
    }
    d
}

fn pl2() -> Vec<u8> {
    vec![0; 1024 + 1714 * 256 + 13 * (3 + 256)]
}

fn looks() -> UnitLooks {
    UnitLooks {
        player_tokens: vec![code(b"OY"); 7],
        player_modes: [b"DT", b"NU", b"WL", b"RN", b"GH", b"TN", b"TW"]
            .iter()
            .map(|m| code(*m))
            .collect(),
        components: vec![code(b"HD"), code(b"TR")],
        ..Default::default()
    }
}

fn files() -> MemorySource {
    let mut s = MemorySource::default();
    s.insert(r"DATA\GLOBAL\TILES\floor.dt1", dt1_bytes());
    s.insert(r"data\global\chars\OY\cof\OYTNhth.cof", cof_bytes());
    s.insert(r"data\global\chars\OY\TR\OYTRlitTNhth.dc6", dc6(1));
    let config = UiConfig {
        screen: Screen::R800,
        expansion_installed: false,
    };
    let ui = OriginalUi::new(config, None).unwrap();
    for name in ui.files().names() {
        s.insert(&format!("data\\global\\ui\\{name}.dc6"), dc6(64));
    }
    for id in 0..14 {
        let Some(f) = font_info(id) else { continue };
        s.insert(f.tbl_path, tbl());
        s.insert(f.dc6_path, dc6(256));
    }
    s
}

/// One server message the host did not run to completion.
type Refusal = (u8, String);

/// The play app over the synthetic game, the server thread and what the
/// smoke test watches.
struct Smoke {
    app: App,
    ms: Arc<AtomicU32>,
    h: Handle,
    wire: Arc<Mutex<Wire>>,
    /// C→S messages the server dispatched with a result other than done,
    /// or dropped at the gate.
    refused: Vec<Refusal>,
    frames: u32,
}

impl Smoke {
    fn new(class: &str) -> Self {
        let ms = Arc::new(AtomicU32::new(1000));
        let wire = Arc::new(Mutex::new(Wire::default()));
        let data = GameData::Synthetic;
        let character = single_player::new_character(class, "Smoke").unwrap();
        let (link, _) = single_player::start_with(
            data.clone(),
            single_player::DEFAULT_SEED,
            character.clone(),
            StepClock(ms.clone()),
        )
        .unwrap();
        let h: Handle = Arc::new(Mutex::new(link));
        let source = Arc::new(files());
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<ButtonInput<KeyCode>>();
        let link = Recorder {
            inner: Box::new(Shared(h.clone())),
            wire: wire.clone(),
        };
        let (link, tap) = predict_link(Box::new(link));
        add_game(&mut app, link, false).unwrap();
        send_create_game_for(&mut app, &character).unwrap();
        let levels = single_player::client_level_rows(&data);
        add_client_data(
            &mut app,
            single_player::client_drlg_source(&data),
            levels.clone(),
        );
        add_preview(
            &mut app,
            levels,
            TileAssets::new(Some(source.clone()), None),
        );
        app_support::synthetic_skill_rows(&mut app);
        app.world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .set_unit_rows(unit_rows());
        add_act_palettes(
            &mut app,
            ActPalettes {
                pl2: std::array::from_fn(|_| pl2()),
                shown: None,
            },
        );
        let fonts = FontMeasure::load(source.as_ref(), &CHARACTER_FONTS).unwrap();
        add_original_ui_with(
            &mut app,
            UiParts {
                source: source.clone(),
                inv_areas: None,
                expansion_installed: false,
                fonts: Some(fonts),
                resist_penalties: Some(vec![0, 20, 50]),
            },
            looks(),
        )
        .unwrap();
        d2_client::app::ui::set_waypoint_map(&mut app, single_player::client_waypoint_map(&data));
        add_walk(&mut app, tap, Some(Speeds { walk: 6, run: 9 }));
        let mut s = Smoke {
            app,
            ms,
            h,
            wire,
            refused: Vec::new(),
            frames: 0,
        };
        while s.player().is_none() {
            s.step(1);
        }
        s.step(30);
        s.check("join");
        s
    }

    /// `n` client frames of 40 ms; each frame's server part is audited.
    fn step(&mut self, n: u32) {
        for _ in 0..n {
            self.ms.fetch_add(40, Ordering::SeqCst);
            self.app.update();
            self.frames += 1;
            assert!(self.frames < 100_000, "runaway");
            let msgs: Vec<HandledMessage> = self
                .h
                .lock()
                .unwrap()
                .with(|l| l.last_frame().messages.clone())
                .unwrap();
            for m in msgs {
                match m.handled {
                    Handled::Game(Outcome::Dispatched(ResultCode::Done)) | Handled::System => {}
                    other => self.refused.push((m.id, format!("{other:?}"))),
                }
            }
        }
    }

    /// No C→S message refused by the server, no S→C message dropped,
    /// unowned, discarded or rejected by the client, so far.
    fn check(&self, at: &str) {
        let b = &self.app.world().resource::<BridgeResource>().0;
        let log = b.log();
        assert!(
            self.refused.is_empty(),
            "{at}: server refused C→S {:02X?}",
            self.refused
        );
        assert!(log.rejected.is_empty(), "{at}: rejected {:?}", log.rejected);
        assert!(log.dropped.is_empty(), "{at}: dropped {:?}", log.dropped);
        assert!(log.unowned.is_empty(), "{at}: unowned {:?}", log.unowned);
        assert!(
            log.discarded.is_empty(),
            "{at}: discarded {:?}",
            log.discarded
        );
    }

    /// Runs `f` on the game inside the server thread.
    fn server<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut single_player::Sim) -> R + Send + 'static,
    ) -> R {
        self.h
            .lock()
            .unwrap()
            .with(move |l| f(&mut l.host_mut().game))
            .unwrap()
    }

    fn player(&self) -> Option<(UnitId, u32)> {
        self.server(|s| single_player::local_player(s))
    }

    fn level(&self) -> Option<u32> {
        self.server(|s| {
            let (p, _) = single_player::local_player(s)?;
            let room = s.game.lists.unit(p)?.room()?;
            s.events.action.hooks().drlg.level_id(&s.game, room)
        })
    }

    /// The server's quest record of `chain`: (state, status).
    fn quest(&self, chain: u8) -> (u8, u8) {
        self.server(move |s| {
            let r = s.world.quests.record(chain).expect("quest record");
            (r.state, r.status)
        })
    }

    /// The local player's quest flags of `slot` on the server (bits set).
    fn flags(&self, slot: u8) -> Vec<u8> {
        self.server(move |s| {
            let (p, _) = single_player::local_player(s).unwrap();
            let f = s.world.rest.quests.get(&p).expect("player quest record");
            (0..16).filter(|&b| f.flags[0].get(slot, b)).collect()
        })
    }

    /// A base stat of the local player on the server.
    fn stat(&self, id: u16) -> i32 {
        self.server(move |s| {
            let (p, _) = single_player::local_player(s).unwrap();
            s.events.action.with(&mut s.game, |_, v| v.stat(p, id))
        })
    }

    /// Puts the player in `level` (a border crossing or a level the
    /// synthetic world joins by no tile: the act-change queue, as the
    /// per-quest tests do).
    fn put_in(&mut self, level: u32) {
        self.server(move |s| {
            let (p, _) = single_player::local_player(s).unwrap();
            s.events.action.hooks().act_changes.push((p, level, 0));
        });
        for _ in 0..400 {
            if self.level() == Some(level) {
                break;
            }
            self.step(1);
        }
        assert_eq!(self.level(), Some(level), "moved to level {level}");
        self.step(30);
    }

    fn unit(&self, ty: u8, class: u32) -> Option<UnitKey> {
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

    /// The client interacts with `key` (the click's C→S 0x13).
    fn interact(&mut self, key: UnitKey) {
        self.app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .interact(key)
            .unwrap();
    }

    /// Takes the warp tile of `class` and waits to be in `to`.
    fn take(&mut self, class: u32, to: u32) {
        let t = self
            .unit(TILE, class)
            .unwrap_or_else(|| panic!("warp tile {class} in the client model"));
        self.interact(t);
        for _ in 0..600 {
            if self.level() == Some(to) {
                break;
            }
            self.step(1);
        }
        assert_eq!(
            self.level(),
            Some(to),
            "the warp tile {class} leads to {to}"
        );
        self.step(30);
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
        for press in [true, false] {
            let e = if press {
                UiEvent::Press {
                    button: PointerButton::Left,
                    at,
                }
            } else {
                UiEvent::Release {
                    button: PointerButton::Left,
                    at,
                }
            };
            self.queue(e);
        }
        self.step(2);
    }

    fn menu(&self) -> Option<d2_client::ui::npc_menu_ui::Open> {
        let ui = self.app.world().non_send::<WorldViewUi>();
        ui.original.as_ref().unwrap().npc_menu()
    }

    /// Opens the menu of the NPC of `class` (C→S 0x13, S→C 0x28).
    fn open_menu(&mut self, class: u32) -> u32 {
        let key = self
            .unit(1, class)
            .unwrap_or_else(|| panic!("NPC {class} in the client model"));
        self.interact(key);
        for _ in 0..400 {
            if self.menu().is_some() {
                break;
            }
            self.step(1);
        }
        let menu = self
            .menu()
            .unwrap_or_else(|| panic!("NPC {class}'s menu opened"));
        assert_eq!(menu.guid, key.guid);
        self.step(2);
        key.guid
    }

    /// Clicks the menu row `i` (R800: box rows from y 170, 20 high).
    fn menu_row(&mut self, i: usize) {
        self.click(Point::new(400, 170 + 20 * i as i32 + 5));
        self.step(3);
    }

    fn menu_row_of(&self, kind: Option<OptionKind>) -> Option<usize> {
        self.menu()?.rows.iter().position(|r| r.kind == kind)
    }

    /// Talks to the NPC of `class`: its menu, Talk, then Cancel.
    fn talk(&mut self, class: u32) -> u32 {
        let guid = self.open_menu(class);
        if let Some(t) = self.menu_row_of(Some(OptionKind::Talk)) {
            self.menu_row(t);
        }
        if self.menu().is_some() {
            let c = self.menu_row_of(None).expect("a Cancel row");
            self.menu_row(c);
        }
        self.step(5);
        guid
    }

    /// The C→S 0x31 messages to `guid`, in order.
    fn said(&self, guid: u32) -> Vec<u32> {
        self.wire
            .lock()
            .unwrap()
            .sent
            .iter()
            .filter(|m| m[0] == 0x31 && m[1..5] == guid.to_le_bytes())
            .map(|m| u32::from_le_bytes(m[5..9].try_into().unwrap()))
            .collect()
    }

    /// Opens the quest log (Q), reads the rows of tab `act`, closes it.
    fn log(&mut self, act: u8) -> Vec<(u8, IconState, u8)> {
        let q = UiEvent::Action(ActionId(Action::ToggleQuests.index() as u16));
        self.queue(q);
        self.step(4);
        let rows = {
            let ui = self.app.world().non_send::<WorldViewUi>();
            let o = ui.original.as_ref().unwrap();
            assert!(o.is_open(0x0F), "the quest log opened");
            o.quest_rows(act, false)
                .iter()
                .map(|r| (r.row.quest, r.row.icon, r.row.shown))
                .collect()
        };
        self.queue(q);
        self.step(4);
        rows
    }

    fn row(&mut self, act: u8, quest: u8) -> (IconState, u8) {
        let rows = self.log(act);
        rows.iter()
            .find(|r| r.0 == quest)
            .map(|r| (r.1, r.2))
            .unwrap_or_else(|| panic!("quest {quest} in the log's tab {act}: {rows:?}"))
    }

    /// The client's quest record (S→C 0x28, `[0x007C0D43]`).
    fn client_flag(&self, slot: usize, bit: u8) -> bool {
        let ui = self.app.world().non_send::<WorldViewUi>();
        let r = ui.original.as_ref().unwrap().more().client_quest;
        let w = u16::from_le_bytes([r[slot * 2], r[slot * 2 + 1]]);
        w & (1 << bit) != 0
    }

    /// A monster of `class` next to the player on the server.
    fn spawn(&mut self, class: u32) -> (UnitId, u32) {
        let r = self.server(move |s| {
            let (p, _) = single_player::local_player(s).unwrap();
            let a = &mut s.events.action;
            let (px, py) = a.sys.hooks.path_position(p);
            let room = s.game.lists.unit(p).and_then(|e| e.room()).expect("room");
            let req = d2_sim::units::lifecycle::AllocRequest {
                ty: UnitType::Monster,
                class,
                room: Some(room),
                add: true,
                fixed_guid: None,
                mode: 1,
                allied: false,
            };
            let m = a
                .with(&mut s.game, |g, v| v.allocate(g, &req, px + 3, py))
                .expect("monster");
            (m, s.game.lists.unit(m).expect("entry").guid)
        });
        self.step(10);
        r
    }

    /// The player kills `m` (the kill reaction; the synthetic monsters
    /// have no fight to win).
    fn kill(&mut self, m: UnitId) {
        self.server(move |s| {
            let (p, _) = single_player::local_player(s).unwrap();
            s.events.action.combat(&mut s.game, |w, _| {
                d2_sim::wiring::action::reaction::kill(w, m, p);
            });
        });
        self.step(40);
    }

    /// The kill's quest parse step for `m` (`KillStep::QuestKill`): the
    /// synthetic `monstats` marks only a few classes killable (Blood Raven,
    /// Izual, Duriel, the Act IV bosses), so other quest monsters die by
    /// this step alone, as in the per-quest tests.
    fn quest_kill(&mut self, m: UnitId) {
        self.server(move |s| {
            let (p, _) = single_player::local_player(s).unwrap();
            s.events.action.hooks().x.kill_step(
                &mut s.game,
                d2_sim::wiring::action::KillStep::QuestKill,
                m,
                p,
            );
        });
        self.step(40);
    }

    /// The server monster of `class` in the player's room, if any.
    fn placed(&self, class: u32) -> Option<UnitId> {
        self.server(move |s| {
            s.game
                .lists
                .units_of_type(UnitType::Monster)
                .into_iter()
                .find(|&u| {
                    s.events
                        .action
                        .sys
                        .units
                        .get(u)
                        .is_some_and(|r| r.class == class)
                })
        })
    }

    fn client_act(&self) -> Option<u8> {
        self.app
            .world()
            .resource::<BridgeResource>()
            .0
            .world()
            .act
            .as_ref()
            .map(|a| a.act)
    }
}

const NEWSKILLS: u16 = 5;

/// A Den of Evil monster (`monstats` row 1, a plain fallen class).
const DEN_MONSTER: u32 = 1;

/// The monster classes this test places by hand that the synthetic
/// client rows lack (only Blood Raven, Izual and Duriel have one): the
/// client ignores a 0xAC of a class without a `monstats` row (`msg-units.md`
/// §1.2 rule 2) and then drops its mode messages. Follow-up: the synthetic
/// client rows for the quest monsters (`docs/handoff/q-smoke-quests.md`).
const QUEST_MONSTERS: [u32; 3] = [DEN_MONSTER, 45, 156];

/// The synthetic client rows plus plain rows for [`QUEST_MONSTERS`].
fn unit_rows() -> d2_client::bridge::world::UnitRows {
    let mut rows = single_player::synthetic_unit_rows();
    let raven = rows.monsters[d2_client::app::synthetic_burial::BLOOD_RAVEN as usize];
    for c in QUEST_MONSTERS {
        let c = c as usize;
        if rows.monsters.len() <= c {
            rows.monsters.resize(c + 1, None);
        }
        if rows.monsters[c].is_none() {
            rows.monsters[c] = raven;
        }
    }
    rows
}

// -------------------------------------------------------------- Act I --

// Covers: specs/world/quests-act1.md §10.1, §10.5, §10.7, §10.8; specs/world/quests.md §4.4, §6.2, §7.3; specs/world/npc.md §8.3
#[test]
fn act1_den_and_burial_grounds() {
    use d2_client::app::synthetic_burial as burial;
    use d2_sim::world::npc::class;
    let mut s = Smoke::new("sorceress");
    assert_eq!(s.row(0, 1).1, 0, "Den not started");

    // Den of Evil: Akara's introduction, then the quest (message 64).
    let akara = s.talk(u32::from(class::AKARA));
    s.talk(u32::from(class::AKARA));
    assert!(s.said(akara).contains(&64), "Akara's Den of Evil (64)");
    assert_eq!(s.quest(1).0, 2, "A1Q1 state 2 (talked)");
    // The client's record (`[0x007C0D43]`) changes only with S→C 0x28,
    // which comes with the next chat's start.
    assert!(!s.client_flag(1, 2));
    s.open_menu(u32::from(class::AKARA));
    let cancel = s.menu_row_of(None).expect("a Cancel row");
    s.menu_row(cancel);
    s.step(5);
    assert!(s.client_flag(1, 2), "the client's quest record (S→C 0x28)");
    assert_eq!(s.row(0, 1), (IconState::InProgress, 1), "log: Den started");
    s.check("Den of Evil started");
    // The Den (level 8) by its tile from the Blood Moor (the town to Blood
    // Moor border is not a tile).
    s.put_in(single_player::BLOOD_MOOR);
    s.take(single_player::BLOOD_MOOR_TO_DEN, single_player::DEN_OF_EVIL);
    assert_eq!(s.quest(1).0, 3, "A1Q1 state 3 (entered the Den)");
    s.check("entered the Den");
    // The Den's population: the synthetic Den has none, so one monster
    // with the chain-1 link monster init gives Den monsters
    // (follow-up: Den population and the `den_region` seam).
    let (m, _) = s.spawn(DEN_MONSTER);
    s.check("Den spawn");
    s.server(move |sim| {
        sim.events.action.hooks().x.monster_quest_chain(m, 1);
    });
    s.step(2);
    s.quest_kill(m);
    assert_eq!(s.quest(1).0, 4, "A1Q1 state 4 (Den cleared)");
    assert!(s
        .flags(1)
        .contains(&d2_sim::world::quests::bit::REWARD_PENDING));
    s.check("Den cleared");
    // Back to town, Akara's reward (message 76): one skill point.
    s.take(single_player::DEN_TO_BLOOD_MOOR, single_player::BLOOD_MOOR);
    s.put_in(single_player::ACT1_TOWN);
    let points = s.stat(NEWSKILLS);
    s.talk(u32::from(class::AKARA));
    assert!(s.said(akara).contains(&76), "Akara's reward (76)");
    assert_eq!(s.stat(NEWSKILLS), points + 1, "+1 skill point");
    assert_eq!(s.quest(1).0, 5, "A1Q1 done");
    let den = s.row(0, 1);
    assert!(
        matches!(den.0, IconState::Completed | IconState::JustCompleted),
        "log: Den done {den:?}"
    );
    s.check("Den rewarded");

    // Sisters' Burial Grounds: Kashya (81), the Burial Grounds off Cold
    // Plains, Blood Raven (placed with her chain link), Kashya's reward (92).
    let kashya = s.talk(u32::from(class::KASHYA));
    if !s.said(kashya).contains(&81) {
        s.talk(u32::from(class::KASHYA));
    }
    assert!(s.said(kashya).contains(&81), "Kashya's quest (81)");
    assert_eq!(s.quest(2).0, 2, "A1Q2 started");
    assert_eq!(
        s.row(0, 2),
        (IconState::InProgress, 1),
        "log: Burial started"
    );
    s.put_in(single_player::COLD_PLAINS);
    let to_burial = d2_client::app::synthetic_chains::slots(single_player::COLD_PLAINS)
        .into_iter()
        .find(|e| e.1 == burial::BURIAL_GROUNDS)
        .expect("Cold Plains leads to the Burial Grounds")
        .2;
    s.take(to_burial, burial::BURIAL_GROUNDS);
    assert_eq!(s.quest(2).0, 3, "A1Q2 state 3 (entered)");
    let raven = s.placed(burial::BLOOD_RAVEN).expect("Blood Raven placed");
    s.kill(raven);
    assert_eq!(s.quest(2).0, 4, "A1Q2 state 4 (Blood Raven dead)");
    s.check("Blood Raven dead");
    // Back to town at once: after about 300 frames away the synthetic
    // town's NPCs are not announced again (follow-up in the handoff).
    s.put_in(single_player::ACT1_TOWN);
    s.step(20 * 17);
    s.talk(u32::from(class::KASHYA));
    assert!(s.said(kashya).contains(&92), "Kashya's reward (92)");
    assert_eq!(s.quest(2).0, 5, "A1Q2 done");
    s.check("Burial rewarded");
}

// Covers: specs/world/quests-act1.md §10.7, §10.8; specs/world/quests.md §4.4, §6.2; specs/world/npc.md §8.3
#[test]
fn act1_tower_andariel_and_the_way_east() {
    use d2_client::app::synthetic_tower as tower;
    use d2_sim::world::npc::class;
    // A fresh game: the Moldy Tome is host-placed at build and is lost
    // once the Black Marsh's room is freed (follow-up in the handoff), so
    // the Tower comes first.
    let mut s = Smoke::new("sorceress");

    // The Forgotten Tower: the Moldy Tome in the Black Marsh, the cellars,
    // the Countess (hand-placed with her init link, as `app_tower_quest`).
    s.put_in(tower::BLACK_MARSH);
    let tome = s.unit(OBJECT, tower::TOME_CLASS).expect("the Moldy Tome");
    s.interact(tome);
    s.step(60);
    assert_eq!(s.quest(5).0, 2, "A1Q5 state 2 (tome read)");
    s.take(tower::MARSH_TO_TOWER, tower::FORGOTTEN_TOWER);
    for i in 0..tower::TOWER_LEVELS.len() - 1 {
        s.take(tower::down(i), tower::TOWER_LEVELS[i + 1]);
    }
    assert_eq!(s.quest(5).0, 3, "A1Q5 state 3 (Cellar 5)");
    // The tome sets no status (`0x00594E70`); the Tower's levels do.
    let tower_row = s.row(0, 5);
    assert_eq!(
        tower_row.0,
        IconState::InProgress,
        "log: Tower {tower_row:?}"
    );
    let (countess, _) = s.spawn(tower::COUNTESS_CLASS);
    s.server(move |sim| {
        let x = &mut sim.events.action.hooks().x;
        x.monster_quest_chain(countess, tower::TOWER_CHAIN);
    });
    s.step(2);
    s.quest_kill(countess);
    assert_eq!(s.quest(5).0, 5, "A1Q5 done (the Countess)");
    s.check("Countess dead");

    // Sisters to the Slaughter: Catacombs 4, Andariel, Warriv (183), and
    // his travel row to Lut Gholein.
    s.put_in(single_player::CATACOMBS_4);
    assert_eq!(s.quest(6).0, 3, "A1Q6 state 3 (Catacombs 4)");
    let (andariel, _) = s.spawn(156);
    s.quest_kill(andariel);
    let f = s.flags(6);
    assert!(
        f.contains(&d2_sim::world::quests::bit::REWARD_PENDING),
        "Andariel credited: {f:?}"
    );
    s.check("Andariel dead");
    s.put_in(single_player::ACT1_TOWN);
    // The synthetic Rogue Encampment has no Warriv (a class only; follow-up
    // for the town set): he is placed by the player, as `app_andariel`.
    if s.unit(1, u32::from(class::WARRIV1)).is_none() {
        s.spawn(u32::from(class::WARRIV1));
    }
    let warriv = s.talk(u32::from(class::WARRIV1));
    for _ in 0..3 {
        if !s.said(warriv).contains(&183) {
            s.talk(u32::from(class::WARRIV1));
        }
    }
    assert!(
        s.said(warriv).contains(&183),
        "Warriv's message 183: {:?}",
        s.said(warriv)
    );
    assert!(s
        .flags(6)
        .contains(&d2_sim::world::quests::bit::REWARD_GRANTED));
    // The "Go East" row is a runtime insert the spec leaves open
    // (`ui/panels.md` open question 8, `panels-2.md` §14.8): the client
    // menu has no row for it yet (follow-up), so the row's intent is sent
    // as `app_andariel` does: C→S 0x38 action 0 on Warriv.
    let mut m = vec![0x38];
    m.extend_from_slice(&0u32.to_le_bytes());
    m.extend_from_slice(&warriv.to_le_bytes());
    m.extend_from_slice(&1u32.to_le_bytes());
    s.app
        .world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .send_bytes(&m)
        .unwrap();
    for _ in 0..400 {
        if s.level() == Some(single_player::ACT2_TOWN) {
            break;
        }
        s.step(1);
    }
    assert_eq!(s.level(), Some(single_player::ACT2_TOWN), "Act II");
    s.step(40);
    assert_eq!(s.client_act(), Some(1), "the client is in Act II");
    s.check("Act II");
}
