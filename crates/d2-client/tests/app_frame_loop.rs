// Spec: specs/client/bridge.md (§3, §8), specs/client/render-pipeline.md (A1, A9)
//! The play mode's frame loop, headless: the app's own wiring
//! (`app::play::add_game`) over the app's single-player game
//! (`app::single_player`, synthetic tables, on its server thread) in a
//! Bevy `App` without a window, driven by `App::update`.
//!
//! - every update runs one bridge frame (pump: drain → tick → flush;
//!   receive), the server ticks on its host clock, the bridge receives
//!   the server's S→C message, and the world view builds a frame from the
//!   bridge's model;
//! - the GPU compositor's render-graph node runs when an adapter exists:
//!   its output, read back from the presented texture, equals the CPU
//!   reference byte for byte. Without an adapter the GPU test reports the
//!   skip and passes (the CPU test still runs the whole loop).
//!
//! The host clock is a manual one shared with the test (`bridge.md` §3
//! rule 3: the link owns it; Bevy time never reaches the server). Rules
//! in the GPU test are a test fixture (one synthetic tile), not an
//! original rule.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use bevy::app::PluginGroup;
use bevy::prelude::*;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::window::ExitCondition;
use d2_client::app::play::add_game;
use d2_client::app::server_thread::ThreadLink;
use d2_client::app::single_player::{self, GameData, Link, Started, COLD_PLAINS, DEFAULT_SEED};
use d2_client::bridge::link::Sent;
use d2_client::bridge::world::ClientWorld;
use d2_client::bridge::{BridgeResource, ClientUnit};
use d2_client::composite::{ComponentFrame, ComponentRequest, CompositeError, UnitParams};
use d2_client::frames::{FramePart, FrameSet, FrameSetKey, IndexFrame};
use d2_client::gpu_compositor::Gpu;
use d2_client::scene::{BlendOp, DrawKey, Rect, ShadeChain};
use d2_client::ui::{ImageRequest, TextRequest};
use d2_client::world_view::node::NodeRuns;
use d2_client::world_view::present::{FrameStats, WorldViewTarget};
use d2_client::world_view::{
    build, compose_cpu, TileDraw, UiRules, UiSprite, UnitPose, Unspecified, ViewAssets, ViewError,
    ViewRules, WorldViewState, VIEW,
};
use d2_formats::palette::{Palette, Rgb};
use d2_proto::client::TakeOrCloseWp;
use d2_server::seams::Clock;

/// The host clock, advanced by the test.
struct StepClock(Arc<AtomicU32>);

impl Clock for StepClock {
    fn now_ms(&mut self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }
}

/// The app's game on clock `ms`, with the waypoint menu of the player
/// open (staged as the bridge's end-to-end test does).
fn game(ms: &Arc<AtomicU32>) -> (ThreadLink<Link<StepClock>>, Started) {
    let (mut link, started) =
        single_player::start(GameData::Synthetic, DEFAULT_SEED, StepClock(ms.clone())).unwrap();
    let (player, guid) = (started.player, started.waypoint_guid);
    link.with(move |l| {
        l.host_mut()
            .game
            .events
            .hooks()
            .x
            .interact
            .insert(player, (2, guid));
    })
    .unwrap();
    (link, started)
}

fn bridge(app: &App) -> &BridgeResource {
    app.world().resource::<BridgeResource>()
}

fn stats(app: &App) -> FrameStats {
    app.world()
        .resource::<WorldViewState>()
        .last
        .expect("a world view frame")
}

// Covers: specs/client/bridge.md §8 r1, §8 r2, §8 r3
#[test]
fn frame_loop_ticks_the_server_and_feeds_the_world_view() {
    let ms = Arc::new(AtomicU32::new(1000));
    let (link, started) = game(&ms);
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_resource::<ButtonInput<MouseButton>>();
    add_game(&mut app, Box::new(link), true).unwrap();

    // Frame 1 at 1000 ms: the host starts its clock, no tick; the world
    // view composes the (empty) model on the CPU (no render world).
    app.update();
    let w = &bridge(&app).0.world();
    assert_eq!((w.frames, w.server_ticks), (1, 0));
    assert_eq!(
        stats(&app),
        FrameStats {
            bridge_frame: 1,
            items: 0,
            units_drawn: 0,
            units_hidden: 0,
            ui_sent: 0,
            ui_unhandled: 0,
            gpu: false,
        }
    );
    assert_eq!(
        app.world().get_resource::<NodeRuns>().map(|r| r.get()),
        None
    );

    // An intent sent between frames 1 and 2 is drained by frame 2's pump;
    // the tick's flush reaches the bridge in the same frame: S→C 0x0D,
    // which has no owner spec yet, so it is recorded as unowned.
    let sent = app
        .world_mut()
        .resource_mut::<BridgeResource>()
        .0
        .send(&TakeOrCloseWp {
            wp: started.waypoint_guid,
            level: COLD_PLAINS as u16,
        })
        .unwrap();
    assert_eq!(sent, Sent::Queued);
    ms.fetch_add(40, Ordering::SeqCst);
    app.update();
    let b = &bridge(&app).0;
    assert_eq!((b.world().frames, b.world().server_ticks), (2, 1));
    assert_eq!(b.log().unowned.get(&0x0D), Some(&1));
    assert!(b.log().rejected.is_empty() && b.log().discarded.is_empty());
    assert_eq!(stats(&app).bridge_frame, 2);

    // A frame shorter than a tick pumps without ticking.
    app.update();
    let w = bridge(&app).0.world();
    assert_eq!((w.frames, w.server_ticks), (3, 1));

    // A frame much longer than a tick still runs one tick: no catch-up.
    ms.fetch_add(200, Ordering::SeqCst);
    app.update();
    let w = bridge(&app).0.world();
    assert_eq!((w.frames, w.server_ticks), (4, 2));

    // N frames of 40 ms: one tick each, and the world view follows. 300
    // ticks run past the DRLG's room inactivity removal (tick step 9): the
    // player's room stays, as the client's room switch registered it.
    for _ in 0..300 {
        ms.fetch_add(40, Ordering::SeqCst);
        app.update();
    }
    let w = bridge(&app).0.world();
    assert_eq!((w.frames, w.server_ticks), (304, 302));
    assert_eq!(stats(&app).bridge_frame, 304);
    assert_eq!(bridge(&app).0.log().unowned.get(&0x0D), Some(&1));

    // The presented image is the CPU reference of the empty list.
    let state = app.world().resource::<WorldViewState>();
    let frame = build(&ClientWorld::default(), &[], &Unspecified, &state.assets).unwrap();
    let want = compose_cpu(&frame, &state.assets).unwrap();
    let target = app.world().resource::<WorldViewTarget>();
    let image = app
        .world()
        .resource::<Assets<Image>>()
        .get(&target.image)
        .unwrap();
    assert_eq!(image.data.as_deref(), Some(&want[..]));
}

// ---- GPU node -------------------------------------------------------------------------

fn tile_key() -> FrameSetKey {
    FrameSetKey::new("data/global/tiles/app-test.dt1", FramePart::Tile(0)).unwrap()
}

/// A palette whose entries are distinct, so RGBA reveals the index.
fn palette() -> Palette {
    let mut p = Palette {
        colors: [Rgb::default(); 256],
    };
    for (i, c) in p.colors.iter_mut().enumerate() {
        let i = i as u8;
        *c = Rgb {
            r: i,
            g: 255 - i,
            b: i ^ 0x5a,
        };
    }
    p
}

/// One 37×23 frame of varying indices (0 = transparent).
fn assets() -> ViewAssets {
    let (w, h) = (37u32, 23u32);
    let pixels = (0..w * h).map(|i| (i % 251) as u8).collect();
    let mut a = ViewAssets::new(palette());
    a.sets.insert(
        tile_key(),
        FrameSet {
            frames: vec![IndexFrame::new(w, h, 0, 0, pixels).unwrap()],
        },
    );
    a
}

/// Fixture rules: the tile twice, overlapping, partly off the frame's
/// right edge; nothing else.
struct TestRules;

impl ViewRules for TestRules {
    fn tiles(&self, _: &ClientWorld, _: &ViewAssets) -> Result<Vec<TileDraw>, ViewError> {
        let tile = |x, y, minor| TileDraw {
            frame: ComponentFrame {
                set: tile_key(),
                index: 0,
            },
            x,
            y,
            clip: Rect::FRAME,
            shade: ShadeChain::EMPTY,
            blend: BlendOp::Opaque,
            key: DrawKey::new(0, 0, minor, 0).unwrap(),
            cell: (minor as i32, 0),
        };
        Ok(vec![tile(100, 90, 0), tile(120, 100, 1), tile(780, 300, 2)])
    }
    fn unit_pose(&self, w: &ClientWorld, u: &ClientUnit) -> Result<Option<UnitPose>, ViewError> {
        Unspecified.unit_pose(w, u)
    }
    fn unit_params(
        &self,
        w: &ClientWorld,
        u: &ClientUnit,
        p: &UnitPose,
    ) -> Result<UnitParams, ViewError> {
        Unspecified.unit_params(w, u, p)
    }
    fn component_frame(
        &self,
        u: &ClientUnit,
        p: &UnitPose,
        r: &ComponentRequest<'_>,
    ) -> Result<ComponentFrame, CompositeError> {
        Unspecified.component_frame(u, p, r)
    }
    fn place(
        &self,
        u: &ClientUnit,
        p: &UnitPose,
        r: &ComponentRequest<'_>,
        i: &IndexFrame,
    ) -> Result<(i32, i32), CompositeError> {
        Unspecified.place(u, p, r, i)
    }
    fn shade(
        &self,
        u: &ClientUnit,
        r: &ComponentRequest<'_>,
    ) -> Result<ShadeChain, CompositeError> {
        Unspecified.shade(u, r)
    }
    fn blend(&self, u: &ClientUnit, r: &ComponentRequest<'_>) -> Result<BlendOp, CompositeError> {
        Unspecified.blend(u, r)
    }
}

impl UiRules for TestRules {
    fn ui_image(&self, r: &ImageRequest, a: &ViewAssets) -> Result<UiSprite, ViewError> {
        Unspecified.ui_image(r, a)
    }
    fn ui_text(&self, r: &TextRequest, a: &ViewAssets) -> Result<Vec<UiSprite>, ViewError> {
        Unspecified.ui_text(r, a)
    }
    fn ui_pass(&self) -> Result<u32, ViewError> {
        Unspecified.ui_pass()
    }
}

/// Removes the row padding of a texture readback.
fn unpad(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let row = (width * 4) as usize;
    let stride = data.len() / height as usize;
    assert!(stride >= row && data.len() == stride * height as usize);
    data.chunks_exact(stride)
        .flat_map(|r| &r[..row])
        .copied()
        .collect()
}

#[test]
fn gpu_node_composes_the_frame_into_the_presented_texture() {
    let adapter = match Gpu::headless() {
        Ok((_, info)) => info,
        Err(e) => {
            eprintln!("skipped: no GPU adapter ({e}); the CPU frame loop test covers the rest");
            return;
        }
    };
    eprintln!(
        "adapter: {} ({:?}, {})",
        adapter.name, adapter.backend, adapter.driver
    );
    let ms = Arc::new(AtomicU32::new(1000));
    let (link, _) = game(&ms);
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .disable::<bevy::winit::WinitPlugin>()
            .disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>()
            .disable::<bevy::audio::AudioPlugin>()
            .disable::<bevy::log::LogPlugin>(),
    );
    add_game(&mut app, Box::new(link), true).unwrap();
    app.insert_resource(WorldViewState::new(assets(), Box::new(TestRules)));
    app.finish();
    app.cleanup();

    let captured: Arc<Mutex<Option<Vec<u8>>>> = Arc::default();
    let mut armed = false;
    for _ in 0..60 {
        ms.fetch_add(40, Ordering::SeqCst);
        app.update();
        let runs = app.world().resource::<NodeRuns>().get();
        if runs > 0 && !armed {
            // Read the presented texture back once the node has written it.
            let image = app.world().resource::<WorldViewTarget>().image.clone();
            let sink = captured.clone();
            app.world_mut().spawn(Readback::texture(image)).observe(
                move |ev: On<ReadbackComplete>, mut commands: Commands| {
                    let mut s = sink.lock().unwrap();
                    if s.is_none() {
                        *s = Some(ev.data.clone());
                        commands.entity(ev.entity).despawn();
                    }
                },
            );
            armed = true;
        }
        if captured.lock().unwrap().is_some() {
            break;
        }
    }
    let s = stats(&app);
    assert!(s.gpu, "the GPU path was chosen");
    assert_eq!(s.items, 3);
    assert!(app.world().resource::<NodeRuns>().get() > 0, "the node ran");
    let data = captured
        .lock()
        .unwrap()
        .take()
        .expect("the presented texture was read back");

    let world = ClientWorld::default();
    let a = assets();
    let frame = build(&world, &[], &TestRules, &a).unwrap();
    let want = compose_cpu(&frame, &a).unwrap();
    let got = unpad(&data, VIEW.width, VIEW.height);
    let differ = got
        .as_chunks::<4>()
        .0
        .iter()
        .zip(want.as_chunks::<4>().0)
        .filter(|(g, w)| g != w)
        .count();
    assert_eq!(differ, 0, "pixels differing from the CPU reference");
}
