// Spec: specs/ui/controls.md (§6 r8.3 object case), specs/world/objects.md (§7.1), specs/world/waypoints.md (§5.2); preview fills: docs/PLAN.md decisions D1–D3
//! Clicking a world object in the play preview, headless, wired as
//! `app_play_e2e.rs` wires it (synthetic fixtures only): the synthetic
//! town's waypoint object is clicked, the player walks to it, C→S 0x13
//! goes out, the server operates the object (`Pending::object_preview_range`)
//! and the S→C object mode message changes the client's object.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use bevy::prelude::*;
use d2_client::app::palette::{add_act_palettes, ActPalettes};
use d2_client::app::play::{
    add_client_data, add_game, add_preview, add_walk, predict_link, send_create_game_for,
};
use d2_client::app::single_player::{self, GameData};
use d2_client::app::ui::{add_original_ui_with, UiParts};
use d2_client::assets::path::MemorySource;
use d2_client::bridge::hover;
use d2_client::bridge::predict::Speeds;
use d2_client::bridge::world::OBJECT;
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

// Covers: specs/ui/controls.md §6 r8; specs/world/objects.md §7.1; specs/world/waypoints.md §5.2
#[test]
fn clicking_an_object_walks_to_it_and_operates_it() {
    let data = GameData::Synthetic;
    let character = single_player::new_character("sorceress", "Test").unwrap();
    let ms = Arc::new(AtomicU32::new(1000));
    // A chest 5 sub-tiles from the player's start: in the interact reach.
    // (The synthetic `charstats` give the server player no walk speed, so
    // the server-side walk to a chest is not exercised here; the live
    // local check covers it, docs/handoff/stitch-objects.md.)
    let (link, _started) = single_player::start_with_chests(
        data.clone(),
        // A seed whose chest roll drops (`objects.md` §8.1 rule 5: 25% of
        // plain chests are empty).
        single_player::DEFAULT_SEED + 1,
        character.clone(),
        StepClock(ms.clone()),
        vec![(single_player::WAYPOINT_X + 8, single_player::UNIT_Y + 3)],
    )
    .unwrap();
    let source = Arc::new(files());
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_resource::<ButtonInput<MouseButton>>();
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
    add_walk(&mut app, tap, Some(Speeds { walk: 6, run: 9 }));
    step(&mut app, &ms, 10);

    // The synthetic join sends no skill list: skill 0 as the left skill.
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

    // The waypoint object is in the model, in its starting mode.
    let (key, mode0) = {
        let w = app.world().resource::<BridgeResource>().0.world();
        let (k, u) = w
            .units
            .iter()
            .find(|(k, u)| k.unit_type == OBJECT && u.class == single_player::SYNTHETIC_CHEST_CLASS)
            .expect("the chest is listed");
        (*k, u.mode)
    };

    // A screen point that lands on it (the preview's hover pick).
    let local_at = app
        .world()
        .resource::<PreviewWalk>()
        .predict
        .position()
        .expect("predicted player");
    let cam = Camera::new(
        FrameSize::D2RS,
        OpenMode::NONE,
        moving_to_client(local_at.0, local_at.1),
        (0, 0),
    );
    let cell = app.world().resource::<BridgeResource>().0.world().units[&key]
        .position
        .expect("object cell");
    let (x, y) = hover::unit_feet(&cam, key.unit_type, cell);
    let at = Point::new(x, y - 20);
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
    step(&mut app, &ms, 1);
    assert_eq!(
        app.world()
            .resource::<WorldViewState>()
            .interact
            .pending
            .map(|p| p.target),
        Some(key),
        "the click made the object the pending interact target"
    );
    step(&mut app, &ms, 200);
    let w = app.world().resource::<BridgeResource>().0.world();
    let mode = w.units.get(&key).expect("still listed").mode;
    assert_ne!(
        mode, mode0,
        "the server operated the chest (S→C 0x0E: opening)"
    );
    // The chest's drop (`treasure.md` §4, §7; REC-260): the opened chest's
    // item lies on the floor and the client heard it (0x9C ground item).
    // The synthetic chest's class is the item smoke test's (REC-281):
    // one pick of each entry, in order.
    let ground = d2_client::bridge::items::ground_items(w);
    let mut codes: Vec<[u8; 4]> = ground.iter().filter_map(|i| i.code).collect();
    codes.sort();
    let mut want = [*b"axe ", *b"hp1 ", *b"isc ", *b"tsc ", *b"cm1 ", *b"gsv "];
    want.sort();
    assert_eq!(codes, want, "the chest dropped its items on the floor");
}
