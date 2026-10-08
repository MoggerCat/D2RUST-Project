// Spec: specs/ui/controls.md (§6 r9.2), specs/client/model.md (§8 rule 7), specs/world/waypoints.md (§5.2), specs/client/msg-ui.md (§2); preview fills: docs/PLAN.md decisions D1–D2, docs/handoff/stitch-npc.md
//! The play preview's unit interaction headless, wired as
//! `d2-client play --new` wires it (synthetic fixtures only, as in
//! `app_play_e2e.rs`): a left click on the town's waypoint object picks
//! it as the hover target (d2rs-own preview pick), walks toward it with
//! the interaction pending and sends C→S 0x13 on arrival; the server
//! answers S→C 0x63, the waypoint menu (UI 0x14) opens with the known
//! destinations and a row click sends C→S 0x49.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::palette::{add_act_palettes, ActPalettes};
use d2_client::app::play::{
    add_client_data, add_game, add_preview, add_walk, predict_link, send_create_game_for,
};
use d2_client::app::single_player::{self, GameData};
use d2_client::app::ui::{add_original_ui_with, UiParts};
use d2_client::assets::path::MemorySource;
use d2_client::bridge::hover;
use d2_client::bridge::link::{LinkError, Pumped, SendQueue, Sent, ServerLink};
use d2_client::bridge::mirror::DynLink;
use d2_client::bridge::predict::Speeds;
use d2_client::bridge::BridgeResource;
use d2_client::rules::camera::{moving_to_client, Camera, FrameSize, OpenMode};
use d2_client::rules::unit_composite::code;
use d2_client::ui::layout::Screen;
use d2_client::ui::original::{FontMeasure, OriginalUi, UiConfig, CHARACTER_FONTS};
use d2_client::ui::{font_info, Point, PointerButton, UiEvent};
use d2_client::world_view::tile_assets::TileAssets;
use d2_client::world_view::unit_assets::UnitLooks;
use d2_client::world_view::walk::PreviewWalk;
use d2_client::world_view::{WorldViewState, WorldViewUi};
use d2_server::seams::Clock;

mod app_support;

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

/// A DT1 of one tile with one 32 × 32 RLE block (`formats/dt1.md`), the
/// fixture of `app_play_preview.rs`.
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

/// A DC6 of one direction with `frames` frames of 2 × 2 literal pixels
/// (`formats/dc6.md`).
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

/// COF bytes (`formats/cof.md`): one direction, one frame, one layer
/// (component 1, weapon class `hth`), animation rate 256.
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

/// A `pal.pl2` of zeros with its 13 text colours (`formats/palette.md`).
fn pl2() -> Vec<u8> {
    vec![0; 1024 + 1714 * 256 + 13 * (3 + 256)]
}

/// Invented unit tokens: every player class is `OY`, mode 5 `TN`,
/// component 1 `TR`. `OYTRlitTNhth` is the one player component file
/// name read as a DC6 (`unit-composite.md` §6 r2), so a DC6 fixture
/// draws the player.
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

/// Every file the play preview reads here.
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

fn step(app: &mut App, ms: &AtomicU32, n: usize) {
    for _ in 0..n {
        app.update();
        ms.fetch_add(40, Ordering::SeqCst);
    }
}

fn queue(app: &mut App, e: UiEvent) {
    app.world_mut()
        .non_send_mut::<WorldViewUi>()
        .queue
        .0
        .push(e);
}

/// The play app over the synthetic game, joined, with a left skill.
fn play_app(ms: &Arc<AtomicU32>, wire: &Arc<Mutex<Wire>>) -> App {
    let data = GameData::Synthetic;
    let character = single_player::new_character("sorceress", "Test").unwrap();
    let (link, _) = single_player::start_with(
        data.clone(),
        single_player::DEFAULT_SEED,
        character.clone(),
        StepClock(ms.clone()),
    )
    .unwrap();
    let source = Arc::new(files());
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_resource::<ButtonInput<MouseButton>>();
    let link = Recorder {
        inner: Box::new(link),
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
    step(&mut app, ms, 10);
    // The synthetic join sends no skill list: S→C 0x94 + 0x23 (as in
    // `app_play_e2e.rs`).
    let guid = app
        .world()
        .resource::<BridgeResource>()
        .0
        .world()
        .local()
        .expect("local player")
        .key
        .guid;
    let mut msgs = vec![0x94, 1];
    msgs.extend_from_slice(&guid.to_le_bytes());
    msgs.extend_from_slice(&[0, 0, 1]);
    msgs.extend_from_slice(&[0x23, 0]);
    msgs.extend_from_slice(&guid.to_le_bytes());
    msgs.extend_from_slice(&[1, 0, 0]);
    msgs.extend_from_slice(&u32::MAX.to_le_bytes());
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .receive_chunk(&msgs)
        .unwrap();
    app
}

/// The screen point of the waypoint object's feet, as the click's
/// camera sees it (centred on the predicted player).
fn waypoint_on_screen(app: &App) -> (u32, Point) {
    let w = app.world().resource::<BridgeResource>().0.world();
    let (key, u) = w
        .units
        .iter()
        .find(|(k, u)| k.unit_type == 2 && u.position.is_some())
        .expect("the waypoint object in the model");
    let at = app
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
    (key.guid, Point::new(x, y - 20))
}

fn waypoint_open(app: &App) -> Option<u32> {
    let ui = app.world().non_send::<WorldViewUi>();
    let o = ui.original.as_ref().unwrap();
    o.msg_state().waypoint.as_ref().map(|w| w.guid)
}

// Covers: specs/ui/controls.md §6 r9; specs/client/model.md §8 r7
#[test]
fn clicking_the_waypoint_walks_there_and_interacts() {
    let ms = Arc::new(AtomicU32::new(1000));
    let wire = Arc::new(Mutex::new(Wire::default()));
    let mut app = play_app(&ms, &wire);
    let (guid, at) = waypoint_on_screen(&app);
    assert!(
        (0..800).contains(&at.x) && (0..560).contains(&at.y),
        "the waypoint is on screen: {at:?}"
    );
    assert_eq!(waypoint_open(&app), None);
    queue(
        &mut app,
        UiEvent::Press {
            button: PointerButton::Left,
            at,
        },
    );
    queue(
        &mut app,
        UiEvent::Release {
            button: PointerButton::Left,
            at,
        },
    );
    step(&mut app, &ms, 2);
    assert!(
        app.world()
            .resource::<WorldViewState>()
            .interact
            .pending
            .is_some_and(|p| p.target.guid == guid),
        "the click on the waypoint left the interaction pending"
    );
    let mut want = vec![0x13, 2, 0, 0, 0];
    want.extend_from_slice(&guid.to_le_bytes());
    for _ in 0..200 {
        step(&mut app, &ms, 1);
        if wire.lock().unwrap().sent.contains(&want) {
            break;
        }
    }
    let w = wire.lock().unwrap();
    assert!(
        w.sent.contains(&want),
        "C→S 0x13 on the waypoint: {:?}",
        w.sent.iter().map(|m| m[0]).collect::<Vec<_>>()
    );
    // The interaction ran once: the pending record is gone.
    assert!(app
        .world()
        .resource::<WorldViewState>()
        .interact
        .pending
        .is_none());
    drop(w);
    // The server operates the waypoint (`waypoints.md` §5.2 step 3, the
    // interact range PROVISIONAL REC-96) and answers S→C 0x63: the menu
    // (UI 0x14) opens on the waypoint's GUID.
    step(&mut app, &ms, 4);
    assert_eq!(waypoint_open(&app), Some(guid));
    let o = app.world().non_send::<WorldViewUi>();
    let o = o.original.as_ref().unwrap();
    assert!(o.is_open(0x14), "the waypoint menu is open");
    // The known destinations: the town (current) and Cold Plains (the
    // synthetic record knows it, `single_player` staging).
    let rows: Vec<(u16, bool, bool)> = o
        .waypoint_rows()
        .iter()
        .map(|r| (r.level, r.known, r.current))
        .collect();
    assert_eq!(rows, [(1, true, true), (3, true, false)]);
    // A click on row 1 (`ui/menus.md` §1.2 / §1.6 hit, R800: x' = x − 80,
    // y' = y − 60) sends C→S 0x49 [GUID][level 3] and closes the menu.
    let at = Point::new(80 + 150, 60 + 110);
    for e in [
        UiEvent::Press {
            button: PointerButton::Left,
            at,
        },
        UiEvent::Release {
            button: PointerButton::Left,
            at,
        },
    ] {
        queue(&mut app, e);
    }
    step(&mut app, &ms, 2);
    let mut take = vec![0x49];
    take.extend_from_slice(&guid.to_le_bytes());
    take.extend_from_slice(&3u32.to_le_bytes());
    assert!(
        wire.lock().unwrap().sent.contains(&take),
        "C→S 0x49 to level 3: {:?}",
        wire.lock()
            .unwrap()
            .sent
            .iter()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
    );
    let ui = app.world().non_send::<WorldViewUi>();
    assert!(!ui.original.as_ref().unwrap().is_open(0x14), "menu closed");
}
