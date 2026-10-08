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
use d2_client::app::synthetic_items::BUCKLER;
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
    /// The newest store record before the current trade opened.
    store_floor: u32,
}

impl Rig {
    /// A new sorceress in the Act I town of the synthetic game, joined,
    /// with the play UI over it.
    fn new() -> Rig {
        Rig::with_stash(STASH_XY)
    }

    /// [`Rig::new`] with the stash at `stash` (sub-tiles from the town
    /// room's origin).
    fn with_stash(stash: (i32, i32)) -> Rig {
        let data = GameData::Synthetic;
        let ms = Arc::new(AtomicU32::new(1000));
        let character = single_player::new_character("sorceress", "Test").unwrap();
        let (link, _) = single_player::start_with_town(
            data.clone(),
            single_player::DEFAULT_SEED,
            character.clone(),
            StepClock(ms.clone()),
            Vec::new(),
            Some(stash),
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
                // The right panel's click area of every `inventory.bin`
                // record (the user's file gives them; d2rs-own fixture:
                // the right half above the control panel, 800 × 600).
                inv_areas: Some(
                    (0..32)
                        .map(|_| d2_client::ui::original::InvArea {
                            left: 400,
                            right: 800,
                            top: 0,
                            bottom: 553,
                        })
                        .collect(),
                ),
                // An expansion install: the mercenary menus (Resurrect,
                // `panels-2.md` §14.2) are the expansion's.
                expansion_installed: true,
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
            store_floor: 0,
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
        let (x, y) = hover::unit_feet(&cam, u.key.unit_type, u.position.unwrap());
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

// ---- the shop ---------------------------------------------------------------------------

/// The store grid's cell (x, y) on screen (`ui/shop_ui.rs`: 29 px cells
/// at (`sx` + 15, `H` + `sy` − 400), R800).
fn store_cell(x: i32, y: i32) -> Point {
    let s = Screen::R800;
    Point::new(s.sx() + 15 + 29 * x + 10, s.h + s.sy() - 400 + 29 * y + 10)
}

/// The backpack cell (x, y) on screen (the inventory panel's grid beside
/// the shop, as `e2e_vendor.rs` reads it).
fn backpack_cell(x: u16, y: u16) -> Point {
    Point::new(
        339 + 80 + 29 * i32::from(x) + 10,
        255 + 60 + 29 * i32::from(y) + 10,
    )
}

/// Shop button `i` of a four-button bar (`panels::shop::BUTTON_X`).
fn shop_button(i: usize) -> Point {
    let s = Screen::R800;
    Point::new(
        s.sx() + d2_client::ui::panels::shop::BUTTON_X[3][i] + 10,
        s.h + s.sy() - 87,
    )
}

impl Rig {
    /// Steps until the client's gold differs from `from` (the vitals
    /// sync's forced run, at most 20 client updates) and returns it.
    fn gold_after(&mut self, from: i32) -> i32 {
        for _ in 0..40 {
            if self.gold() != from {
                break;
            }
            self.step(1);
        }
        self.gold()
    }

    fn gold(&self) -> i32 {
        let w = self.bridge().world();
        w.local_player.map_or(0, |me| w.total(me, 14, 0))
    }

    /// Server: the local player's gold := `n` (a new character has none).
    fn stage_gold(&mut self, n: i32) {
        app_support::with(&self.server, move |l| {
            let (p, _) = single_player::local_player(&l.host().game).unwrap();
            let g = &mut l.host_mut().game;
            g.events
                .action
                .with(&mut g.game, |_, v| v.set_base(p, 14, n));
        });
        // The vitals sync sends gold on its forced run (every 20 client
        // updates, `combat/vitals.md` §5.1 rule 2).
        for _ in 0..60 {
            self.step(1);
            if self.gold() == n {
                break;
            }
        }
    }

    /// Server: the durability (stat 72) of the item `guid` := `n`.
    fn stage_durability(&mut self, guid: u32, n: i32) {
        app_support::with(&self.server, move |l| {
            let g = &mut l.host_mut().game;
            let item = g
                .game
                .lists
                .find_unit(d2_sim::units::UnitType::Item, guid)
                .expect("the item is on the server");
            g.events
                .action
                .with(&mut g.game, |_, v| v.set_base(item, 72, n));
        });
        self.step(4);
    }

    /// The local player's backpack items (stored, page 0) of `code`.
    fn backpack(&self, code: &[u8; 4]) -> Vec<d2_client::bridge::items::ItemView> {
        d2_client::bridge::items::local_items(self.bridge().world())
            .into_iter()
            .filter(|i| i.code == Some(*code) && i.mode == 0 && i.page == 0 && !i.store)
            .collect()
    }

    /// The open store's page-0 items in the panel's packing order (every
    /// synthetic item is 1 × 1 without item art: cell k = (k mod 10, k / 10)).
    fn store_page0(&self) -> Vec<d2_client::bridge::items::ItemView> {
        let mut v: Vec<_> = d2_client::bridge::items::store_items(self.bridge().world())
            .into_iter()
            .filter(|i| i.page == 0 && i.store_seq > self.store_floor)
            .collect();
        v.sort_by_key(|i| i.store_seq);
        v
    }

    /// S→C 0x2A of the step: (kind, code).
    fn transactions(&self) -> Vec<(u8, u8)> {
        self.wire
            .lock()
            .unwrap()
            .received
            .iter()
            .filter(|m| m[0] == 0x2A)
            .map(|m| (m[1], m[2]))
            .collect()
    }

    /// Closes the shop's UI state (as Escape does); the close ends the
    /// chat (C→S 0x30).
    fn close_shop(&mut self) {
        self.app
            .world_mut()
            .non_send_mut::<WorldViewUi>()
            .original
            .as_mut()
            .unwrap()
            .set_ui(0x0C, 1, false)
            .unwrap();
        self.step(10);
        assert!(self.sent_ids().contains(&0x30), "{:02X?}", self.sent_ids());
        self.check("shop close");
    }

    /// Opens the shop of `class` through its menu row `kind`.
    fn open_shop(&mut self, class: u16, kind: OptionKind) {
        // The store records of an earlier trade stay in the model; the
        // shop shows only the newer ones (`ShopState` floor).
        self.store_floor = d2_client::bridge::items::store_items(self.bridge().world())
            .iter()
            .map(|i| i.store_seq)
            .max()
            .unwrap_or(0);
        self.choose(class, kind);
        self.step(10);
        assert!(
            self.with_ui(|u| u.is_open(0x0C)),
            "{class}: the shop is open"
        );
        assert!(
            !self.store_page0().is_empty(),
            "{class}: the store has items"
        );
    }

    /// Right-clicks the store item of `code` (quick buy, C→S 0x32): the
    /// bought copy's GUID.
    fn buy(&mut self, code: &[u8; 4]) -> u32 {
        let list = self.store_page0();
        let k = list
            .iter()
            .position(|i| i.code == Some(*code))
            .unwrap_or_else(|| panic!("{code:?} is in the store"));
        let before: Vec<u32> = self.backpack(code).iter().map(|i| i.key.guid).collect();
        let gold = self.gold();
        self.click_with(
            PointerButton::Right,
            store_cell(k as i32 % 10, k as i32 / 10),
        );
        self.step(10);
        assert!(self.sent_ids().contains(&0x32), "{:02X?}", self.sent_ids());
        let tx = self.transactions();
        assert!(
            tx.iter()
                .any(|&(kind, c)| c == 0 && (kind == 4 || kind == 5)),
            "bought: {tx:?}"
        );
        let after = self.backpack(code);
        let new = after
            .iter()
            .find(|i| !before.contains(&i.key.guid))
            .unwrap_or_else(|| panic!("the copy is in the backpack: {after:?}"));
        let now = self.gold_after(gold);
        assert!(now < gold, "paid: {gold} → {now}");
        let guid = new.key.guid;
        self.check("buy");
        guid
    }
}

// Covers: specs/world/vendors.md §7.1, §7.2, §8.1, §5.3; specs/ui/menus.md §4
#[test]
fn act1_traders_buy_sell_repair_and_gamble() {
    let mut rig = Rig::new();
    // A level-1 character carries at most 10 000 (`0x00622E70`).
    rig.stage_gold(5_000);
    rig.check("gold");
    assert_eq!(rig.gold(), 5_000, "the client model has the gold");

    // Akara: buy a buckler, then sell it back (lift it from the backpack,
    // drop it on the store grid: C→S 0x19, then 0x33).
    rig.open_shop(class::AKARA, OptionKind::Trade);
    let bought = rig.buy(&BUCKLER);
    let it = rig
        .backpack(&BUCKLER)
        .into_iter()
        .find(|i| i.key.guid == bought)
        .unwrap();
    rig.queue(UiEvent::Press {
        button: PointerButton::Left,
        at: backpack_cell(it.x, it.y),
    });
    rig.step(3);
    rig.queue(UiEvent::Release {
        button: PointerButton::Left,
        at: backpack_cell(it.x, it.y),
    });
    rig.step(6);
    assert_eq!(
        d2_client::bridge::items::cursor_item(rig.bridge().world()).map(|i| i.key.guid),
        Some(bought),
        "the buckler is on the cursor: {:02X?}",
        rig.wire.lock().unwrap().sent
    );
    rig.check("lift");
    let gold = rig.gold();
    rig.click(store_cell(9, 9));
    rig.step(10);
    assert!(rig.sent_ids().contains(&0x33), "{:02X?}", rig.sent_ids());
    assert!(
        rig.transactions().contains(&(3, 1)),
        "sold: {:?}",
        rig.transactions()
    );
    assert!(rig.gold_after(gold) > gold, "received the price");
    assert!(
        d2_client::bridge::items::local_items(rig.bridge().world())
            .iter()
            .all(|i| i.key.guid != bought || i.store),
        "the sold buckler left the player"
    );
    rig.check("sell");
    rig.close_shop();

    // Charsi: buy a buckler, wear it down, repair it (button 2, then the
    // item), wear it down again, repair all (button 3: worn items only).
    rig.open_shop(class::CHARSI, OptionKind::Trade);
    let bought = rig.buy(&BUCKLER);
    rig.stage_durability(bought, 3);
    rig.check("worn");
    let it = rig
        .backpack(&BUCKLER)
        .into_iter()
        .find(|i| i.key.guid == bought)
        .unwrap();
    rig.click(shop_button(2));
    assert!(
        rig.with_ui(|u| u.shop_state().repair_mode()),
        "repair armed"
    );
    let gold = rig.gold();
    rig.click(backpack_cell(it.x, it.y));
    rig.step(10);
    assert!(rig.sent_ids().contains(&0x35), "{:02X?}", rig.sent_ids());
    assert!(
        rig.transactions().contains(&(1, 2)),
        "repaired: {:?}",
        rig.transactions()
    );
    assert!(rig.gold_after(gold) < gold, "the repair was paid");
    rig.check("repair one");
    rig.stage_durability(bought, 2);
    rig.check("worn again");
    let gold = rig.gold();
    rig.click(shop_button(3));
    rig.step(10);
    assert!(rig.sent_ids().contains(&0x35), "{:02X?}", rig.sent_ids());
    assert!(
        rig.transactions().contains(&(1, 2)),
        "repaired all: {:?}",
        rig.transactions()
    );
    // Repair all covers the equipped items only (`vendors.md` §8.1 rule
    // 3): the worn buckler is in the backpack, so the total is 0 (code 2,
    // nothing paid) and the buckler stays worn.
    rig.step(30);
    assert_eq!(rig.gold(), gold, "nothing equipped needs repair");
    let dur = app_support::with(&rig.server, move |l| {
        let g = &mut l.host_mut().game;
        let item = g
            .game
            .lists
            .find_unit(d2_sim::units::UnitType::Item, bought)
            .unwrap();
        g.events.action.with(&mut g.game, |_, v| v.stat(item, 72))
    });
    assert_eq!(dur, 2, "the backpack buckler was not repaired");
    rig.check("repair all");
    rig.close_shop();

    // Gheed: the gamble window; a right click buys at the gamble price.
    rig.store_floor = d2_client::bridge::items::store_items(rig.bridge().world())
        .iter()
        .map(|i| i.store_seq)
        .max()
        .unwrap_or(0);
    rig.choose(class::GHEED, OptionKind::Gamble);
    rig.step(10);
    assert!(
        rig.with_ui(|u| u.is_open(0x0C) && u.shop_state().gamble()),
        "gamble window"
    );
    let list = rig.store_page0();
    assert!(!list.is_empty(), "the gamble list");
    let code = list[0].code.unwrap();
    rig.buy(&code);
    rig.close_shop();
}

// ---- mercenaries ------------------------------------------------------------------------

impl Rig {
    /// The model's units of `class` (type 1).
    fn monsters_of(&self, class: u32) -> Vec<UnitKey> {
        self.bridge()
            .world()
            .units
            .iter()
            .filter(|(k, u)| k.unit_type == 1 && u.class == class)
            .map(|(k, _)| *k)
            .collect()
    }

    /// Server: the mercenary `guid` dies (the host's pet-death queue, as
    /// `app_mercs_acts.rs`).
    fn kill_merc(&mut self, guid: u32) {
        app_support::with(&self.server, move |l| {
            let g = &mut l.host_mut().game;
            let merc = g
                .game
                .lists
                .find_unit(d2_sim::units::UnitType::Monster, guid)
                .expect("the mercenary is on the server");
            g.events
                .action
                .hooks()
                .pet_deaths
                .as_mut()
                .unwrap()
                .push(merc);
        });
        self.step(10);
    }
}

/// Asheara's mercenary (`town_npcs::synthetic_hire_rows`, class 357).
const ACT3_MERC: u32 = 357;

// Covers: specs/world/npc.md §7.1, §7.4; specs/ui/menus.md §2, §3; specs/ui/panels-2.md §14 r2
#[test]
fn asheara_hires_and_resurrects_a_mercenary_through_her_menu() {
    let mut rig = Rig::new();
    rig.go_to_town(d2_client::app::synthetic_chains::KURAST_DOCKS);
    rig.stage_gold(5_000);
    rig.check("gold");
    assert!(rig.monsters_of(ACT3_MERC).is_empty(), "no mercenary yet");

    // Hire: the menu's Hire row opens the hire list (S→C 0x4E / 0x4F),
    // a row click sends C→S 0x36 and the mercenary joins the model.
    rig.choose(class::ASHEARA, OptionKind::Hire);
    let offers = rig.with_ui(|u| u.hire_list().offers.len());
    assert!(offers > 0, "the hire list has offers");
    assert!(
        rig.with_ui(|u| u.hire_list().up.is_some()),
        "the list is up"
    );
    let (_, list) = d2_client::ui::panels::npc_menu::hire_geometry(800, 600);
    rig.click(Point::new(list.0 + 40, list.1 + 5));
    for _ in 0..60 {
        rig.step(1);
        if !rig.monsters_of(ACT3_MERC).is_empty() {
            break;
        }
    }
    assert!(
        rig.sent_ids().contains(&0x36),
        "C→S 0x36: {:02X?}",
        rig.sent_ids()
    );
    let merc = rig.monsters_of(ACT3_MERC);
    assert_eq!(merc.len(), 1, "the hired mercenary is in the model");
    rig.check("hire");
    // Leave the chat the hire left open.
    let npc = rig.npc(class::ASHEARA).guid;
    rig.app
        .world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .send_bytes(&d2_client::ui::panels::npc::msg_chat_end(npc))
        .unwrap();
    rig.step(10);
    rig.check("chat end");

    // The mercenary dies (S→C 0x9B: the client knows it is dead); her
    // menu now offers Resurrect with its cost, the row sends C→S 0x62,
    // the server answers S→C 0x2A code 5 and the mercenary is back.
    rig.kill_merc(merc[0].guid);
    rig.check("merc death");
    let kinds = rig.open_menu(class::ASHEARA);
    let i = kinds
        .iter()
        .position(|k| *k == Some(OptionKind::Resurrect))
        .unwrap_or_else(|| panic!("Resurrect is offered: {kinds:?}"));
    let cost = rig.with_ui(|u| u.npc_menu().unwrap().rows[i].cost);
    assert!(cost.is_some(), "the Resurrect caption has its cost");
    rig.click(Rig::row_at(i));
    for _ in 0..40 {
        rig.step(1);
        if rig.transactions().contains(&(0, 5)) {
            break;
        }
    }
    assert!(
        rig.sent_ids().contains(&0x62),
        "C→S 0x62: {:02X?}",
        rig.sent_ids()
    );
    assert!(
        rig.transactions().contains(&(0, 5)),
        "resurrected: {:?}",
        rig.transactions()
    );
    rig.step(10);
    assert_eq!(rig.monsters_of(ACT3_MERC).len(), 1, "the mercenary is back");
    rig.check("resurrect");
}

/// The walk to Akara ends on the stash's cell in the client's straight
/// line prediction (REC-51: no client path, no object footprint); the
/// server's position then snaps the view three cells and the click aimed
/// at Kashya before the snap lands on the stash. Settled with the client
/// path of REC-51.
// Covers: specs/ui/controls.md §6 r9
#[test]
#[ignore = "q-smoke-town break 5: the straight-line walk prediction ends inside the stash (REC-51)"]
fn a_click_on_an_npc_while_standing_on_the_stash_talks_to_the_npc() {
    // The stash right below Akara (the walk to her ends on its cell).
    let mut rig = Rig::with_stash((single_player::AKARA_X, single_player::UNIT_Y + 3));
    rig.open_menu(class::AKARA);
    rig.cancel(class::AKARA);
    let kinds = rig.open_menu(class::KASHYA);
    assert!(!rig.with_ui(|u| u.is_open(0x19)), "the stash stayed closed");
    assert!(kinds.contains(&Some(OptionKind::Talk)), "{kinds:?}");
    rig.cancel(class::KASHYA);
}

// ---- heal and identify -----------------------------------------------------------------

impl Rig {
    /// The local player's (life, max life) in the client model (stats 6,
    /// 7, 8.8 fixed point).
    fn life(&self) -> (i32, i32) {
        let w = self.bridge().world();
        w.local_player
            .map_or((0, 0), |me| (w.total(me, 6, 0), w.total(me, 7, 0)))
    }

    /// Server: the local player's life := `n` (8.8).
    fn stage_life(&mut self, n: i32) {
        app_support::with(&self.server, move |l| {
            let (p, _) = single_player::local_player(&l.host().game).unwrap();
            let g = &mut l.host_mut().game;
            g.events
                .action
                .with(&mut g.game, |_, v| v.set_base(p, 6, n));
        });
        for _ in 0..40 {
            self.step(1);
            if self.life().0 == n {
                break;
            }
        }
    }

    /// Opens a trade or gamble window at `class` (the store floor of this
    /// trade first).
    fn open_window(&mut self, class: u16, kind: OptionKind) {
        self.store_floor = d2_client::bridge::items::store_items(self.bridge().world())
            .iter()
            .map(|i| i.store_seq)
            .max()
            .unwrap_or(0);
        self.choose(class, kind);
        self.step(10);
        assert!(
            self.with_ui(|u| u.is_open(0x0C)),
            "{class}: the window is open"
        );
    }
}

// Covers: specs/world/npc.md §5, §6; specs/world/vendors.md §5.3; specs/ui/menus.md §2
#[test]
fn kurast_docks_heals_gambles_and_identifies() {
    let mut rig = Rig::new();
    rig.go_to_town(d2_client::app::synthetic_chains::KURAST_DOCKS);
    rig.stage_gold(5_000);
    rig.check("gold");

    // Ormus heals on the chat's start (`npc.md` §5): a wounded player's
    // life is full again in the client model.
    let (_, max) = rig.life();
    assert!(max > 0, "the player has life");
    rig.stage_life(max / 3);
    rig.check("wounded");
    assert!(rig.life().0 < max, "wounded in the model: {:?}", rig.life());
    rig.open_menu(class::ORMUS);
    for _ in 0..40 {
        if rig.life().0 == max {
            break;
        }
        rig.step(1);
    }
    assert_eq!(rig.life(), (max, max), "Ormus healed the player");
    rig.cancel(class::ORMUS);

    // Alkor's gamble window: the bought item is unidentified (flag 0x10).
    rig.open_window(class::ALKOR, OptionKind::Gamble);
    assert!(rig.with_ui(|u| u.shop_state().gamble()), "a gamble window");
    let list = rig.store_page0();
    let code = list[0].code.unwrap();
    let item = rig.buy(&code);
    rig.close_shop();
    let flags = |rig: &Rig| {
        d2_client::bridge::items::local_items(rig.bridge().world())
            .iter()
            .find(|i| i.key.guid == item)
            .map(|i| i.flags)
    };
    assert_eq!(flags(&rig).map(|f| f & 0x10), Some(0), "unidentified");

    // Cain: the Identify row costs 100 × 1; the row sends C→S 0x34, the
    // server identifies the item (S→C 0x2A code 3, the item's update) and
    // takes the price.
    let kinds = rig.open_menu(class::CAIN4);
    let i = kinds
        .iter()
        .position(|k| *k == Some(OptionKind::Identify))
        .unwrap_or_else(|| panic!("Cain offers Identify: {kinds:?}"));
    let cost = rig.with_ui(|u| u.npc_menu().unwrap().rows[i].cost);
    assert_eq!(cost, Some(100), "one item to identify");
    let gold = rig.gold();
    rig.click(Rig::row_at(i));
    for _ in 0..40 {
        rig.step(1);
        if flags(&rig).is_some_and(|f| f & 0x10 != 0) {
            break;
        }
    }
    assert!(
        rig.sent_ids().contains(&0x34),
        "C→S 0x34: {:02X?}",
        rig.sent_ids()
    );
    assert!(
        rig.transactions().contains(&(0, 3)),
        "identified: {:?}",
        rig.transactions()
    );
    assert_eq!(
        flags(&rig).map(|f| f & 0x10),
        Some(0x10),
        "identified in the model"
    );
    assert_eq!(rig.gold_after(gold), gold - 100, "Cain took 100");
    rig.check("identify");
}

// ---- the stash ----------------------------------------------------------------------------

/// The stash grid's cell (x, y) on screen (`panels/stash_items.rs`
/// fallback: (74, 82) + the 800 × 600 offset, 29 px cells).
fn stash_cell(x: u16, y: u16) -> Point {
    Point::new(154 + 29 * i32::from(x) + 10, 142 + 29 * i32::from(y) + 10)
}

/// The inventory gold button (`ui/gold.rs` `inventory_gold_hit`, 800 ×
/// 600) and the stash gold button (`stash_gold_rect_hit`, expansion).
const INV_GOLD_BUTTON: Point = Point::new(493, 462);
const STASH_GOLD_BUTTON: Point = Point::new(190, 93);

impl Rig {
    /// The local player's stash gold (stat 15) in the client model.
    fn stash_gold(&self) -> i32 {
        let w = self.bridge().world();
        w.local_player.map_or(0, |me| w.total(me, 15, 0))
    }

    /// Types `amount` into the open gold dialog and confirms it (Enter).
    fn gold_dialog(&mut self, amount: u32) {
        // The deposit box opens pre-filled with the maximum (§21 r3):
        // backspaces clear it first (the edit box rules of §28 r4).
        for _ in 0..10 {
            self.queue(UiEvent::Char(8));
        }
        for c in amount.to_string().bytes() {
            self.queue(UiEvent::Char(u16::from(c)));
        }
        self.queue(UiEvent::Char(0x0D));
        self.step(10);
    }

    /// The local player's item `guid` (page, mode).
    fn item_place(&self, guid: u32) -> Option<(u8, u8)> {
        d2_client::bridge::items::local_items(self.bridge().world())
            .iter()
            .find(|i| i.key.guid == guid)
            .map(|i| (i.page, i.mode))
    }
}

// Covers: specs/ui/panels.md §11 r1; specs/ui/panels-2.md §21; specs/world/vendors-2.md §10.2 r2, §10.2 r3; specs/items/inventory-moves.md §7
#[test]
fn act1_stash_keeps_an_item_and_gold() {
    let mut rig = Rig::new();
    rig.stage_gold(5_000);
    rig.check("gold");
    rig.open_shop(class::AKARA, OptionKind::Trade);
    let item = rig.buy(&BUCKLER);
    rig.close_shop();
    let gold = rig.gold();

    // Walk to the stash and click it: C→S 0x13, S→C 0x77 0x10, ui 0x19.
    let stash = *rig
        .bridge()
        .world()
        .units
        .iter()
        .find(|(k, u)| k.unit_type == 2 && u.class == single_player::STASH_CLASS)
        .expect("the stash is listed")
        .0;
    assert!(rig.walk_near(stash), "the stash is reachable");
    let at = rig.pick_point(stash);
    rig.click(at);
    for _ in 0..200 {
        rig.step(1);
        if rig.with_ui(|u| u.is_open(0x19)) {
            break;
        }
    }
    assert!(rig.with_ui(|u| u.is_open(0x19)), "the stash opened");
    rig.check("stash open");

    // The buckler: backpack → cursor → stash grid (C→S 0x19, 0x18 page 4).
    let (x, y) = d2_client::bridge::items::local_items(rig.bridge().world())
        .iter()
        .find(|i| i.key.guid == item)
        .map(|i| (i.x, i.y))
        .unwrap();
    rig.click(backpack_cell(x, y));
    rig.step(6);
    rig.click(stash_cell(0, 0));
    rig.step(10);
    assert_eq!(
        rig.item_place(item),
        Some((4, 0)),
        "the buckler is stored on the stash page"
    );
    rig.check("item to the stash");

    // Gold: the inventory gold button deposits (kind 3), the stash gold
    // button withdraws (kind 4); each moves stats 14 / 15.
    rig.click(INV_GOLD_BUTTON);
    rig.gold_dialog(1_000);
    let now = rig.gold_after(gold);
    assert_eq!(now, gold - 1_000, "deposited from the carried gold");
    for _ in 0..40 {
        if rig.stash_gold() == 1_000 {
            break;
        }
        rig.step(1);
    }
    assert_eq!(rig.stash_gold(), 1_000, "the stash holds the deposit");
    rig.check("deposit");
    rig.click(STASH_GOLD_BUTTON);
    rig.gold_dialog(400);
    assert_eq!(
        rig.gold_after(now),
        now + 400,
        "withdrawn to the carried gold"
    );
    assert_eq!(rig.stash_gold(), 600, "the stash keeps the rest");
    rig.check("withdraw");

    // The buckler back to its backpack cell (the start cube, REC-244,
    // holds (0, 0)), then the stash closes.
    rig.click(stash_cell(0, 0));
    rig.step(6);
    rig.click(backpack_cell(x, y));
    rig.step(10);
    assert_eq!(rig.item_place(item), Some((0, 0)), "back in the backpack");
    rig.check("item back");
    rig.app
        .world_mut()
        .non_send_mut::<WorldViewUi>()
        .original
        .as_mut()
        .unwrap()
        .set_ui(0x19, 1, false)
        .unwrap();
    rig.step(10);
    assert!(!rig.with_ui(|u| u.is_open(0x19)), "the stash closed");
    rig.check("stash close");
}

// ---- the cube -----------------------------------------------------------------------------

/// The cube grid's cell (x, y) on screen (`panels/cube_items.rs`
/// fallback: (116, 130) + the 800 × 600 offset, 29 px cells).
fn cube_cell(x: u16, y: u16) -> Point {
    Point::new(196 + 29 * i32::from(x) + 10, 190 + 29 * i32::from(y) + 10)
}

/// The transmute button and the cube's close button
/// (`panels/stash_input.rs` `transmute_hit`, `cube_close_hit`), centred.
fn transmute_button() -> Point {
    let s = Screen::R800;
    Point::new(s.sx() + 164, s.h + s.sy() - 203)
}

fn cube_close_button() -> Point {
    let s = Screen::R800;
    Point::new(s.sx() + 295, s.h + s.sy() - 80)
}

impl Rig {
    /// The local player's item with `code` (GUID, x, y).
    fn own_item(&self, code: &[u8; 4]) -> (u32, u16, u16) {
        d2_client::bridge::items::local_items(self.bridge().world())
            .iter()
            .find(|i| i.code == Some(*code))
            .map(|i| (i.key.guid, i.x, i.y))
            .unwrap_or_else(|| panic!("the player holds {code:?}"))
    }

    /// The C→S 0x4F ClickButton messages sent since the last check, by
    /// button.
    fn buttons_sent(&self) -> Vec<u8> {
        self.wire
            .lock()
            .unwrap()
            .sent
            .iter()
            .filter(|m| m[0] == 0x4F)
            .map(|m| m[1])
            .collect()
    }
}

// Covers: specs/ui/panels.md §12; specs/world/cube.md §1, §2; specs/items/inventory-moves.md §7
#[test]
fn act1_cube_holds_an_item_and_transmutes() {
    use d2_client::controls::Action;
    use d2_client::ui::panel::ActionId;
    use d2_client::ui::panels::inventory::UI_INVENTORY;
    use d2_client::ui::panels::stash_cube::UI_CUBE;
    let mut rig = Rig::new();
    rig.stage_gold(5_000);
    rig.check("gold");
    rig.open_shop(class::AKARA, OptionKind::Trade);
    let item = rig.buy(&BUCKLER);
    rig.close_shop();
    let (_, x, y) = rig.own_item(&BUCKLER);

    // The inventory (its toggle), then the start cube (REC-244) opened by
    // its use: a right click on it, C→S 0x20, ui 0x1A.
    rig.queue(UiEvent::Action(ActionId(
        Action::ToggleInventory.index() as u16
    )));
    rig.step(3);
    assert!(
        rig.with_ui(|u| u.is_open(UI_INVENTORY)),
        "the inventory opened"
    );
    let (_, cx, cy) = rig.own_item(b"box ");
    rig.click_with(PointerButton::Right, backpack_cell(cx, cy));
    for _ in 0..40 {
        if rig.with_ui(|u| u.is_open(UI_CUBE)) {
            break;
        }
        rig.step(1);
    }
    assert!(rig.with_ui(|u| u.is_open(UI_CUBE)), "the cube opened");
    rig.check("cube open");

    // The buckler: backpack → cursor → cube grid (page 3).
    rig.click(backpack_cell(x, y));
    rig.step(6);
    rig.click(cube_cell(0, 0));
    rig.step(10);
    assert_eq!(
        rig.item_place(item),
        Some((3, 0)),
        "the buckler is in the cube"
    );
    rig.check("item to the cube");

    // Transmute (C→S 0x4F 0x18): no recipe takes a lone buckler, so it
    // stays as it is (`cube.md` §2).
    rig.click(transmute_button());
    rig.step(10);
    assert_eq!(rig.buttons_sent(), [0x18], "the transmute is sent");
    assert_eq!(rig.item_place(item), Some((3, 0)), "the buckler stays");
    rig.check("transmute");

    // The buckler back to its backpack cell.
    rig.click(cube_cell(0, 0));
    rig.step(6);
    rig.click(backpack_cell(x, y));
    rig.step(10);
    assert_eq!(rig.item_place(item), Some((0, 0)), "back in the backpack");
    rig.check("item back");

    // The cube's close button: C→S 0x4F 0x17, ui 0x1A closed.
    rig.click(cube_close_button());
    rig.step(10);
    assert!(!rig.with_ui(|u| u.is_open(UI_CUBE)), "the cube closed");
    assert_eq!(rig.buttons_sent(), [0x17], "the close is sent");
    rig.check("cube close");
}
