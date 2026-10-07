// Spec: specs/render/draw-order.md (§9, §10), specs/client/assets.md (§A4); preview fills: docs/PLAN.md decision D1
//! The play preview headless: `add_game` + `add_client_data` +
//! `add_preview` over the app's own single-player game (synthetic data).
//! The map is built from the client DRLG, the near rooms' DT1 tiles are
//! read from a memory source standing in for the user's archives (a
//! synthetic one-tile DT1, not a game file), and every frame is built:
//! resident tiles are drawn, the others skipped and logged once.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use d2_client::app::play::{add_client_data, add_game, add_preview, send_create_game};
use d2_client::app::single_player::{self, GameData};
use d2_client::assets::path::MemorySource;
use d2_client::bridge::BridgeResource;
use d2_client::rules::OpenMode;
use d2_client::world_view::tile_assets::{tile_key, TileAssets};
use d2_client::world_view::WorldViewState;
use d2_server::seams::Clock;

mod app_support;
use app_support::SharedLink;

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

// Covers: specs/render/draw-order.md §9; specs/client/assets.md §a4-residency
#[test]
fn the_preview_draws_the_map_from_the_client_drlg() {
    let data = GameData::Synthetic;
    let ms = Arc::new(AtomicU32::new(1000));
    let (link, _) = single_player::start(
        data.clone(),
        single_player::DEFAULT_SEED,
        StepClock(ms.clone()),
    )
    .unwrap();
    let server = Arc::new(Mutex::new(link));
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_resource::<ButtonInput<MouseButton>>();
    add_game(&mut app, Box::new(SharedLink(server.clone())), false).unwrap();
    send_create_game(&mut app).unwrap();
    let levels = single_player::client_level_rows(&data);
    add_client_data(
        &mut app,
        single_player::client_drlg_source(&data),
        levels.clone(),
    );
    let mut files = MemorySource::default();
    files.insert(r"DATA\GLOBAL\TILES\floor.dt1", dt1_bytes());
    add_preview(
        &mut app,
        levels,
        TileAssets::new(Some(Arc::new(files)), None),
    );
    app_support::synthetic_skill_rows(&mut app);
    app.world_mut()
        .resource_mut::<WorldViewState>()
        .feed
        .set_ui_open_mode(OpenMode::new(0).unwrap());
    for _ in 0..8 {
        app.update();
        ms.fetch_add(40, Ordering::SeqCst);
    }
    let b = &app.world().resource::<BridgeResource>().0;
    let w = b.world();
    assert!(w.local_room().is_some(), "joined into the town room");
    let state = app.world().resource::<WorldViewState>();
    assert!(state.preview);
    let last = state.last.expect("frames are built, none failed");
    assert_eq!(last.server_tick, w.server_ticks);
    assert!(last.items > 0, "the map's tiles are drawn: {last:?}");
    // Tile 0 of the synthetic DT1 is resident in the frame store.
    let key = tile_key(b"floor.dt1", 0).unwrap();
    assert!(state.assets.frames.contains(&key));
}
