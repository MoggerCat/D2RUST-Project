// Spec: specs/world/npc.md (§2 rule 3 approach), specs/ui/controls.md (§6 r9.2); preview fills: docs/PLAN.md decisions D1–D3, docs/handoff/q-npc-approach.md
//! An NPC clicked from out of the talk distance: the walk ends 7–8
//! sub-tiles away, the server approaches and starts the talk on arrival
//! (no second click).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::palette::{add_act_palettes, ActPalettes};
use d2_client::app::play::{
    add_client_data, add_game, add_preview, add_walk, predict_link, send_create_game_for,
};
use d2_client::app::single_player::{self, GameData};
use d2_client::app::town_npcs;
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
use d2_client::world_view::WorldViewUi;
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
fn play_app(ms: &Arc<AtomicU32>, wire: &Arc<Mutex<Wire>>, npcs: Vec<(u16, i32)>) -> App {
    let data = GameData::Synthetic;
    let character = single_player::new_character("sorceress", "Test").unwrap();
    let (link, _) = single_player::start_with_town(
        data.clone(),
        single_player::DEFAULT_SEED,
        character.clone(),
        StepClock(ms.clone()),
        Vec::new(),
        None,
        npcs,
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
    set_npc_rows(&mut app);
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

/// The client's monster rows: Akara is an `npc` and `interact` class,
/// attackable-flagged so the pick takes her (`isAtt`, `bridge/click.rs`).
fn set_npc_rows(app: &mut App) {
    app.world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .set_unit_rows(single_player::synthetic_unit_rows());
}

/// The GUID and screen point of the NPC of `class`, as the click's camera
/// sees it.
fn npc_on_screen(app: &App, class: u16) -> (u32, Point) {
    let w = app.world().resource::<BridgeResource>().0.world();
    let (key, u) = w
        .units
        .iter()
        .find(|(k, u)| k.unit_type == 1 && u.class == u32::from(class) && u.position.is_some())
        .unwrap_or_else(|| panic!("NPC {class} in the model"));
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

fn click(app: &mut App, ms: &AtomicU32, at: Point) {
    for button in [true, false] {
        let e = if button {
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
        queue(app, e);
    }
    step(app, ms, 2);
}

fn ids(wire: &Arc<Mutex<Wire>>) -> Vec<u8> {
    wire.lock().unwrap().sent.iter().map(|m| m[0]).collect()
}

/// Clicks the NPC of `class` and waits for its menu (S→C 0x28); the
/// menu's row kinds and the NPC's GUID.
fn open_menu(
    app: &mut App,
    ms: &AtomicU32,
    wire: &Arc<Mutex<Wire>>,
    class: u16,
) -> (u32, Vec<Option<d2_client::ui::layout::OptionKind>>) {
    // A click walks up to the NPC and talks; when the walk ends outside
    // the talk distance the server only approaches, so click again.
    let mut guid = 0;
    for _ in 0..4 {
        let (g, at) = npc_on_screen(app, class);
        guid = g;
        assert!(
            (0..800).contains(&at.x) && (0..560).contains(&at.y),
            "NPC {class} is on screen: {at:?}"
        );
        click(app, ms, at);
        step(app, ms, 40);
        if app
            .world()
            .non_send::<WorldViewUi>()
            .original
            .as_ref()
            .unwrap()
            .npc_menu()
            .is_some()
        {
            break;
        }
    }
    let mut want = vec![0x13, 1, 0, 0, 0];
    want.extend_from_slice(&guid.to_le_bytes());
    assert!(
        wire.lock().unwrap().sent.contains(&want),
        "C→S 0x13 on NPC {class}: {:?}",
        ids(wire)
    );
    let menu = app
        .world()
        .non_send::<WorldViewUi>()
        .original
        .as_ref()
        .unwrap()
        .npc_menu()
        .unwrap_or_else(|| panic!("the menu of NPC {class} is open"));
    assert_eq!(menu.guid, guid);
    let kinds = menu.rows.iter().map(|r| r.kind).collect();
    // Leave through the last row (Cancel, R800: box x 300, y 150, rows
    // from y 170, 20 high) so the next NPC starts clean.
    let rows = menu_rows(app);
    click(app, ms, Point::new(400, 170 + 20 * (rows as i32 - 1) + 5));
    step(app, ms, 3);

fn presses_on(wire: &Arc<Mutex<Wire>>, guid: u32) -> usize {
    let mut want = vec![0x13, 1, 0, 0, 0];
    want.extend_from_slice(&guid.to_le_bytes());
    wire.lock()
        .unwrap()
        .sent
        .iter()
        .filter(|m| **m == want)
        .count()
}

/// One click on the NPC placed `x` sub-tiles from the town origin; true
/// when the talk menu opened with no further click.
fn talks_after_one_click(x: i32) -> (bool, usize) {
    let ms = Arc::new(AtomicU32::new(1000));
    let wire = Arc::new(Mutex::new(Wire::default()));
    let class = d2_sim::world::npc::class::ORMUS;
    let mut app = play_app(&ms, &wire, vec![(class, x)]);
    let (guid, at) = npc_on_screen(&app, class);
    click(&mut app, &ms, at);
    step(&mut app, &ms, 120);
    let open = app
        .world()
        .non_send::<WorldViewUi>()
        .original
        .as_ref()
        .unwrap()
        .npc_menu()
        .is_some();
    (open, presses_on(&wire, guid))
}

// Covers: specs/world/npc.md §2 rule 3
#[test]
fn a_click_from_far_walks_up_and_talks_without_a_second_click() {
    for x in [24, 30, 34, 38] {
        let (open, sent) = talks_after_one_click(x);
        assert!(open, "NPC at offset {x}: the talk menu is open");
        assert!(sent >= 1, "NPC at offset {x}: C→S 0x13 sent");
    }
}
