// Spec: specs/world/npc.md (§2–§7), specs/world/vendors.md (§4–§9), specs/ui/menus.md (§2), specs/ui/panels.md (§11), specs/client/bridge.md (§3, §6); preview fills: docs/handoff/q-smoke-town.md
//! (q-smoke-town) Town smoke tests over the real play path, headless:
//! the bridge on the app's in-process server thread (`single_player`),
//! the wired sim behind it, the original UI on top (synthetic fixtures
//! only, no Bevy window). In every act town of the synthetic game the
//! player walks up to each NPC, opens its menu (S→C 0x28) and runs every
//! menu entry the NPC has (trade: buy, sell, repair, gamble; hire;
//! Cain's identify; heal; resurrect), then uses the stash and the cube.
//!
//! Every step is checked the same way ([`Rig::check`]): the client
//! rejected no S→C message, dropped none (unit not in the model), had no
//! unowned id and discarded no bytes; the server answered every C→S
//! message the step sent with `Done` (no `Refused` / `Invalid` /
//! `Malformed` intent) and no transaction answer (S→C 0x2A) carried a
//! refusal code (7, 9–15, `npc.md` §9); and the client model or UI shows the step's result.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::palette::{add_act_palettes, ActPalettes};
use d2_client::app::play::{
    add_client_data, add_game, add_preview, add_walk, predict_link, send_create_game_for,
};
use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, GameData};
use d2_client::app::town_npcs;
use d2_client::app::ui::{add_original_ui_with, UiParts};
use d2_client::assets::path::MemorySource;
use d2_client::bridge::hover;
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::predict::Speeds;
use d2_client::bridge::world::UnitKey;
use d2_client::bridge::BridgeResource;
use d2_client::rules::camera::{moving_to_client, Camera, FrameSize, OpenMode};
use d2_client::rules::unit_composite::code;
use d2_client::ui::layout::{OptionKind, Screen};
use d2_client::ui::original::{FontMeasure, OriginalUi, UiConfig, CHARACTER_FONTS};
use d2_client::ui::{font_info, Point, PointerButton, UiEvent};
use d2_client::world_view::tile_assets::TileAssets;
use d2_client::world_view::unit_assets::UnitLooks;
use d2_client::world_view::walk::PreviewWalk;
use d2_client::world_view::WorldViewUi;
use d2_server::dispatch::Outcome;
use d2_server::host::Handled;
use d2_server::seams::{Clock, ResultCode};
use d2_sim::missiles::seams::MissileBodies;
use d2_sim::world::npc::class;

mod app_support;
use app_support::SharedLink;

// ---- fixtures (as `app_play_act3_town.rs`) ---------------------------------------------

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

/// A DC6 of one direction with `frames` frames of 2 × 2 literal pixels.
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

/// COF bytes (`formats/cof.md`): one direction, one frame, one layer.
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

/// A `.tbl` (`formats/font-tbl.md`): 256 records of width 6.
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

/// A `pal.pl2` of zeros with its 13 text colours.
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

// ---- the link tap ---------------------------------------------------------------------

type Server = app_support::Server<StepClock>;

/// What crossed the link since the last check.
#[derive(Default)]
struct Wire {
    /// C→S messages as the bridge sent them.
    sent: Vec<Vec<u8>>,
    /// S→C messages as the client received them.
    received: Vec<Vec<u8>>,
    /// The server's fate of each drained game message: (id, outcome).
    outcomes: Vec<(u8, Outcome)>,
}

struct Tap {
    inner: SharedLink<ThreadLink<single_player::Link<StepClock>>>,
    wire: Arc<Mutex<Wire>>,
}

impl ServerLink for Tap {
    fn protocol_version(&self) -> u32 {
        self.inner.protocol_version()
    }
    fn send(&mut self, queue: SendQueue, msg: &[u8]) -> Result<Sent, LinkError> {
        self.wire.lock().unwrap().sent.push(msg.to_vec());
        self.inner.send(queue, msg)
    }
    fn pump(&mut self) -> Result<Pumped, LinkError> {
        let p = self.inner.pump()?;
        let frame = self
            .inner
            .0
            .lock()
            .unwrap()
            .with(|l| l.last_frame().messages.clone())
            .unwrap();
        let mut w = self.wire.lock().unwrap();
        for m in frame {
            if let Handled::Game(o) = m.handled {
                w.outcomes.push((m.id, o));
            }
        }
        Ok(p)
    }
    fn receive(&mut self) -> Vec<Vec<u8>> {
        let v = self.inner.receive();
        self.wire.lock().unwrap().received.extend(v.iter().cloned());
        v
    }
}

// ---- the rig --------------------------------------------------------------------------

/// The stash's sub-tile offset in the Act I town room.
const STASH_XY: (i32, i32) = (single_player::WAYPOINT_X, single_player::UNIT_Y + 10);

struct Rig {
    app: App,
    server: Server,
    ms: Arc<AtomicU32>,
    wire: Arc<Mutex<Wire>>,
    steps: u32,
    /// A test teleport moved the server player (the client's view of its
    /// position is stale from then on).
    teleported: bool,
}

impl Rig {
    /// A new sorceress in the Act I town of the synthetic game, joined,
    /// with the play UI over it.
    fn new() -> Rig {
        let data = GameData::Synthetic;
        let ms = Arc::new(AtomicU32::new(1000));
        let character = single_player::new_character("sorceress", "Test").unwrap();
        let (link, _) = single_player::start_with_town(
            data.clone(),
            single_player::DEFAULT_SEED,
            character.clone(),
            StepClock(ms.clone()),
            Vec::new(),
            Some(STASH_XY),
            town_npcs::ACT1.to_vec(),
        )
        .unwrap();
        let server: Server = Arc::new(Mutex::new(link));
        let source = Arc::new(files());
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_resource::<ButtonInput<MouseButton>>();
        let wire = Arc::new(Mutex::new(Wire::default()));
        let (link, tap) = predict_link(Box::new(Tap {
            inner: SharedLink(server.clone()),
            wire: wire.clone(),
        }));
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
            .set_unit_rows(single_player::synthetic_unit_rows());
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
        let mut rig = Rig {
            app,
            server,
            ms,
            wire,
            steps: 0,
            teleported: false,
        };
        while app_support::local_player(&rig.server).is_none() {
            rig.step(1);
        }
        rig.step(30);
        // The synthetic join sends no skill list (the synthetic game has no
        // `skills` rows): S→C 0x94 + 0x23, as every app rig does
        // (`app_play_act3_town.rs`).
        let guid = rig.bridge().world().local().expect("local player").key.guid;
        let mut msgs = vec![0x94, 1];
        msgs.extend_from_slice(&guid.to_le_bytes());
        msgs.extend_from_slice(&[0, 0, 1]);
        msgs.extend_from_slice(&[0x23, 0]);
        msgs.extend_from_slice(&guid.to_le_bytes());
        msgs.extend_from_slice(&[1, 0, 0]);
        msgs.extend_from_slice(&u32::MAX.to_le_bytes());
        rig.app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .receive_chunk(&msgs)
            .unwrap();
        rig.step(2);
        // Nor creation stats (no vitals tables): stage what 1.14d gives a
        // new character (stat 67 `velocitypercent` 100, `combat/vitals.md`
        // §1), else the server walks at the 25 % floor (`pathing.md` §8.1
        // r2) behind the client's prediction, as `app_level_border.rs`.
        app_support::with(&rig.server, |l| {
            let (p, _) = single_player::local_player(&l.host().game).unwrap();
            let g = &mut l.host_mut().game;
            g.events
                .action
                .with(&mut g.game, |_, v| v.set_base(p, 67, 100));
        });
        rig.check("join");
        rig
    }

    fn step(&mut self, n: usize) {
        for _ in 0..n {
            self.app.update();
            self.ms.fetch_add(40, Ordering::SeqCst);
            self.steps += 1;
            assert!(self.steps < 40_000, "the test finishes");
        }
    }

    fn bridge(&self) -> &d2_client::bridge::Bridge<d2_client::bridge::mirror::DynLink> {
        &self.app.world().resource::<BridgeResource>().0
    }

    /// The step's common checks (module doc), then forgets the step's
    /// traffic.
    fn check(&mut self, what: &str) {
        let b = self.bridge();
        let log = b.log();
        assert!(
            log.rejected.is_empty(),
            "{what}: rejected {:?}",
            log.rejected
        );
        assert!(log.dropped.is_empty(), "{what}: dropped {:?}", log.dropped);
        assert!(log.unowned.is_empty(), "{what}: unowned {:?}", log.unowned);
        assert!(
            log.discarded.is_empty(),
            "{what}: discarded {:?}",
            log.discarded
        );
        let mut w = self.wire.lock().unwrap();
        let bad: Vec<_> = w
            .outcomes
            .iter()
            .filter(|(_, o)| *o != Outcome::Dispatched(ResultCode::Done))
            .collect();
        assert!(bad.is_empty(), "{what}: server outcomes {bad:?}");
        let failed: Vec<_> = w
            .received
            .iter()
            .filter(|m| m[0] == 0x2A && matches!(m.get(2), Some(7 | 9..=15)))
            .collect();
        assert!(failed.is_empty(), "{what}: failed transactions {failed:?}");
        w.sent.clear();
        w.received.clear();
        w.outcomes.clear();
    }

    fn sent_ids(&self) -> Vec<u8> {
        self.wire
            .lock()
            .unwrap()
            .sent
            .iter()
            .map(|m| m[0])
            .collect()
    }

    fn sent(&self, msg: &[u8]) -> bool {
        self.wire.lock().unwrap().sent.iter().any(|m| m == msg)
    }

    fn queue(&mut self, e: UiEvent) {
        self.app
            .world_mut()
            .non_send_mut::<WorldViewUi>()
            .queue
            .0
            .push(e);
    }

    fn click_with(&mut self, button: PointerButton, at: Point) {
        self.queue(UiEvent::Press { button, at });
        self.queue(UiEvent::Release { button, at });
        self.step(2);
    }

    fn click(&mut self, at: Point) {
        self.click_with(PointerButton::Left, at);
    }

    /// Runs `f` on the original UI.
    fn with_ui<R>(&self, f: impl FnOnce(&OriginalUi) -> R) -> R {
        let ui = self.app.world().non_send::<WorldViewUi>();
        f(ui.original.as_ref().expect("original ui"))
    }

    /// The server player's level id.
    fn level(&self) -> Option<u32> {
        app_support::with(&self.server, |l| {
            let (p, _) = single_player::local_player(&l.host().game)?;
            let g = &mut l.host_mut().game;
            let room = g.game.lists.unit(p)?.room()?;
            g.events.action.hooks().drlg.level_id(&g.game, room)
        })
    }

    /// The act change to the town `level` (Warriv's / Meshif's / Tyrael's
    /// travel through the hooks queue, as `app_a2_town.rs`).
    fn go_to_town(&mut self, level: u32) {
        app_support::with(&self.server, move |l| {
            let (p, _) = single_player::local_player(&l.host().game).unwrap();
            l.host_mut()
                .game
                .events
                .action
                .hooks()
                .act_changes
                .push((p, level, 0));
        });
        while self.level() != Some(level) {
            self.step(1);
        }
        self.step(30);
        self.check(&format!("act change to {level}"));
    }

    /// The model key of the NPC of `class`.
    fn npc(&self, class: u16) -> UnitKey {
        let w = self.bridge().world();
        *w.units
            .iter()
            .find(|(k, u)| k.unit_type == 1 && u.class == u32::from(class) && u.position.is_some())
            .unwrap_or_else(|| panic!("NPC {class} is in the model"))
            .0
    }

    /// The screen point of a unit's feet, as the click's camera sees it.
    fn on_screen(&self, key: UnitKey) -> Point {
        let u = &self.bridge().world().units[&key];
        let at = self
            .app
            .world()
            .resource::<PreviewWalk>()
            .predict
            .position()
            .expect("predicted player");
        let cam = Camera::new(
            FrameSize::D2RS,
            OpenMode::NONE,
            moving_to_client(at.0, at.1),
            (0, 0),
        );
        let (x, y) = hover::feet(&cam, u.position.unwrap());
        Point::new(x, y - 20)
    }

    /// A screen point the preview's hover pick resolves to `key` (its
    /// feet first, then up its body and to the sides: another unit may
    /// stand in front of it).
    fn pick_point(&self, key: UnitKey) -> Point {
        let feet = self.on_screen(key);
        let at = self
            .app
            .world()
            .resource::<PreviewWalk>()
            .predict
            .position()
            .expect("predicted player");
        let cam = Camera::new(
            FrameSize::D2RS,
            OpenMode::NONE,
            moving_to_client(at.0, at.1),
            (0, 0),
        );
        let w = self.bridge().world();
        for dy in [0, -10, -20, -30, 10, -40, 15] {
            for dx in [0, -8, 8, -16, 16] {
                let p = Point::new(feet.x + dx, feet.y + dy);
                if hover::pick(w, &cam, (p.x, p.y)) == Some(key) {
                    return p;
                }
            }
        }
        panic!("no screen point picks {key:?}");
    }

    /// Moves the server player to `(x, y)` (absolute sub-tiles).
    fn teleport(&mut self, (x, y): (i32, i32)) {
        app_support::with(&self.server, move |l| {
            let (p, _) = single_player::local_player(&l.host().game).unwrap();
            let g = &mut l.host_mut().game;
            let room = g.game.lists.unit(p).and_then(|u| u.room());
            g.events
                .action
                .with(&mut g.game, |g, v| v.path_teleport(g, p, room, x, y));
        });
        self.step(4);
    }

    /// Walks toward `key` with ground clicks until it is on screen near
    /// the player (each click walks part of the way; the walk ends);
    /// `false`: the walk made no way and the player was put beside it.
    fn walk_near(&mut self, key: UnitKey) -> bool {
        for _ in 0..if self.teleported { 0 } else { 6 } {
            let at = self.on_screen(key);
            let (cx, cy) = (400, 280);
            if (at.x - cx).abs() < 160 && (at.y - cy).abs() < 120 {
                return true;
            }
            // A ground point toward the unit, kept on screen.
            let x = (cx + (at.x - cx) / 2).clamp(120, 680);
            let y = (cy + (at.y - cy) / 2).clamp(100, 460);
            // Moved off the line when a unit is there, so the click lands
            // on the ground (the hover pick takes a unit under it).
            let units: Vec<Point> = self
                .bridge()
                .world()
                .units
                .iter()
                .filter(|(k, u)| k.unit_type != 0 && u.position.is_some())
                .map(|(k, _)| self.on_screen(*k))
                .collect();
            let mut grid = Vec::new();
            for gy in (100..=460).step_by(20) {
                for gx in (120..=680).step_by(20) {
                    grid.push(Point::new(gx, gy));
                }
            }
            let free = grid
                .into_iter()
                .filter(|p| {
                    units
                        .iter()
                        .all(|u| (u.x - p.x).abs() > 50 || (u.y - p.y).abs() > 60)
                })
                .min_by_key(|p| (p.x - x).pow(2) + (p.y - y).pow(2))
                .expect("a free ground point");
            self.click(free);
            for _ in 0..200 {
                self.step(1);
                let walking = self
                    .app
                    .world()
                    .resource::<PreviewWalk>()
                    .predict
                    .walking()
                    .is_some();
                if !walking {
                    break;
                }
            }
        }
        // The synthetic act towns have no walkable path across their room
        // (`app_a2_town.rs`): stand beside the NPC on the server, as the
        // other town rigs do. The test teleport sends no placement, so the
        // client's view stays put: the caller interacts through the bridge
        // (the C→S 0x13 a click sends on arrival).
        let (nx, ny) = self.bridge().world().units[&key].position.unwrap();
        self.teleport((i32::from(nx), i32::from(ny) + 3));
        self.teleported = true;
        false
    }

    /// Walks up to the NPC of `class` and clicks it (the walk, C→S 0x13
    /// on arrival) and waits for its menu.
    fn open_menu(&mut self, class: u16) -> Vec<Option<OptionKind>> {
        let key = self.npc(class);
        if self.walk_near(key) {
            let at = self.pick_point(key);
            self.click(at);
        } else {
            self.app
                .world_mut()
                .resource_mut::<BridgeResource>()
                .0
                .interact(key)
                .unwrap();
        }
        for _ in 0..200 {
            self.step(1);
            if self.with_ui(|u| u.npc_menu().is_some()) {
                break;
            }
        }
        let mut talk = vec![0x13, 1, 0, 0, 0];
        talk.extend_from_slice(&key.guid.to_le_bytes());
        assert!(
            self.sent(&talk),
            "C→S 0x13 on NPC {class} ({:08X}): {:02X?}",
            key.guid,
            self.wire.lock().unwrap().sent
        );
        let menu = self.with_ui(|u| u.npc_menu()).unwrap_or_else(|| {
            let w = self.wire.lock().unwrap();
            panic!(
                "the menu of NPC {class} is open: sent {:02X?}, received {:02X?}, outcomes {:?}",
                w.sent,
                w.received.iter().map(|m| m[0]).collect::<Vec<_>>(),
                w.outcomes
            )
        });
        assert_eq!(menu.guid, key.guid);
        self.check(&format!("menu of {class}"));
        menu.rows.iter().map(|r| r.kind).collect()
    }

    /// The screen point of menu row `i` (R800: box x 300, y 150, rows
    /// from y 170, 20 high).
    fn row_at(i: usize) -> Point {
        Point::new(400, 170 + 20 * i as i32 + 5)
    }

    /// Leaves the open menu through its last row (cancel: C→S 0x30).
    fn cancel(&mut self, class: u16) {
        let n = self.with_ui(|u| u.npc_menu().map_or(0, |m| m.rows.len()));
        assert!(n > 0, "a menu is open");
        self.click(Self::row_at(n - 1));
        self.step(10);
        assert!(self.with_ui(|u| u.npc_menu().is_none()), "the menu closed");
        assert!(
            self.sent_ids().contains(&0x30),
            "cancel ends the chat: {:?}",
            self.sent_ids()
        );
        self.check(&format!("cancel {class}"));
    }
}

// Covers: specs/world/npc.md §2, §3; specs/ui/menus.md §2
#[test]
fn act1_town_every_npc_talks_and_cancels() {
    let mut rig = Rig::new();
    for (class, _) in town_npcs::ACT1 {
        let kinds = rig.open_menu(class);
        assert!(
            kinds.contains(&Some(OptionKind::Talk)),
            "{class}: {kinds:?}"
        );
        rig.cancel(class);
    }
    let _ = class::AKARA;
}

/// The town of each act after the act change, with its NPC classes.
fn towns() -> Vec<(u32, Vec<u16>)> {
    vec![
        (single_player::ACT2_TOWN, single_player::ACT2_NPCS.to_vec()),
        (
            d2_client::app::synthetic_chains::KURAST_DOCKS,
            town_npcs::ACT3.iter().map(|&(c, _)| c).collect(),
        ),
        (
            d2_client::app::synthetic_act4::FORTRESS,
            d2_client::app::synthetic_act4::NPCS.to_vec(),
        ),
        (
            single_player::ACT5_TOWN,
            town_npcs::ACT5.iter().map(|&(c, _)| c).collect(),
        ),
    ]
}

// Covers: specs/world/npc.md §2, §3; specs/ui/menus.md §2
#[test]
fn every_act_town_npc_talks_and_cancels() {
    for (level, npcs) in towns() {
        let mut rig = Rig::new();
        rig.go_to_town(level);
        for class in npcs {
            let kinds = rig.open_menu(class);
            // The `npc-menus.tsv` rows, then cancel (Jamella has no Talk).
            assert!(
                kinds.len() >= 2 && kinds.last() == Some(&None),
                "{level}/{class}: {kinds:?}"
            );
            rig.cancel(class);
        }
    }
}

impl Rig {
    /// Opens the menu of `class` and clicks its row of `kind`; the C→S
    /// messages the row sent.
    fn choose(&mut self, class: u16, kind: OptionKind) -> Vec<Vec<u8>> {
        let kinds = self.open_menu(class);
        let i = kinds
            .iter()
            .position(|k| *k == Some(kind))
            .unwrap_or_else(|| panic!("{class} offers {kind:?}: {kinds:?}"));
        self.click(Self::row_at(i));
        self.step(20);
        let sent = self.wire.lock().unwrap().sent.clone();
        self.check(&format!("{class} {kind:?}"));
        sent
    }

    /// The store items the client model holds (S→C 0x9C action 11).
    fn store_len(&self) -> usize {
        d2_client::bridge::items::store_items(self.bridge().world()).len()
    }
}

/// C→S 0x38 `[action][NPC GUID][0]`.
fn entity_action(action: u32, npc: u32) -> Vec<u8> {
    let mut m = vec![0x38];
    m.extend_from_slice(&action.to_le_bytes());
    m.extend_from_slice(&npc.to_le_bytes());
    m.extend_from_slice(&0u32.to_le_bytes());
    m
}

// Covers: specs/ui/menus.md §2; specs/world/vendors.md §4
#[test]
fn act1_trade_and_gamble_rows_open_the_shop() {
    let mut rig = Rig::new();
    for (class, kind, action) in [
        (class::AKARA, OptionKind::Trade, 1),
        (class::CHARSI, OptionKind::Trade, 1),
        (class::GHEED, OptionKind::Gamble, 2),
    ] {
        let npc = rig.npc(class).guid;
        let sent = rig.choose(class, kind);
        assert!(
            sent.contains(&entity_action(action, npc)),
            "{class} {kind:?}: {sent:02X?}"
        );
        // The trade open shows the store (S→C 0x9C action 11) and the
        // shop panel (ui 0x0C) opens on it.
        assert!(rig.store_len() > 0, "{class}: store items arrived");
        assert!(rig.with_ui(|u| u.is_open(0x0C)), "{class}: the shop opened");
        // Leave the chat.
        let mut end = vec![0x30, 1, 0, 0, 0];
        end.extend_from_slice(&npc.to_le_bytes());
        rig.app
            .world_mut()
            .resource_mut::<BridgeResource>()
            .0
            .send_bytes(&end)
            .unwrap();
        rig.step(10);
        rig.check(&format!("{class} chat end"));
    }
}
