// Spec: specs/client/render-pipeline.md (A1 stages 4–5, A9 presentation), specs/client/ui.md (A4)
//! Bevy edge of the world view: after the bridge frame (`PreUpdate`,
//! `bridge.md` §8), one `Update` system runs UI → [`super::build`] →
//! compose (the GPU compute compositor when [`WorldViewGpu`] exists, else
//! the CPU reference) → the 800×600 image shown by a sprite, scaled by the
//! integer presentation factor (§A9, outside the verify boundary).
//!
//! Inert until the app inserts [`crate::bridge::BridgeResource`] and
//! [`WorldViewState`]: nothing here builds a server or chooses rules.
//! Errors go to Bevy's error handler; there is no fallback from a failed
//! GPU frame to the CPU one (METHODS M07).

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::view::Msaa;
use bevy::window::PrimaryWindow;

use crate::bridge::BridgeResource;
use crate::gpu_compositor::Gpu;
use crate::ui::{edge, FramePos, PointerButton, StringLookup, UiEvent, UiRoot};

use super::ui_bind::{run_ui, UiQueue, UiRules};
use super::{build, compose_cpu, GpuAtlas, ViewAssets, ViewRules, VIEW};

/// Render layer of the presented frame and its camera, so the world view
/// never mixes with other sprites of the app.
pub const PRESENT_LAYER: usize = 31;

/// Atlas pages the in-app GPU path may use (64 × 4 MiB).
pub const GPU_ATLAS_PAGES: u32 = 64;

/// Rules of both halves, as one object.
pub trait WorldRules: ViewRules + UiRules {}
impl<T: ViewRules + UiRules + ?Sized> WorldRules for T {}

/// The world view's inputs besides the bridge.
#[derive(Resource)]
pub struct WorldViewState {
    pub assets: ViewAssets,
    pub rules: Box<dyn WorldRules + Send + Sync>,
    /// Counts of the last frame, for logs and tests.
    pub last: Option<FrameStats>,
}

impl WorldViewState {
    pub fn new(assets: ViewAssets, rules: Box<dyn WorldRules + Send + Sync>) -> Self {
        WorldViewState {
            assets,
            rules,
            last: None,
        }
    }
}

/// What the last presented frame held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameStats {
    pub bridge_frame: u64,
    pub items: usize,
    pub units_drawn: usize,
    pub units_hidden: usize,
    pub ui_sent: usize,
    pub ui_unhandled: usize,
    pub gpu: bool,
}

/// The UI core (non-send: panels are plain trait objects). Optional: with
/// none inserted the frame has no UI items.
pub struct WorldViewUi {
    pub root: UiRoot,
    pub strings: Box<dyn StringLookup>,
    pub queue: UiQueue,
    /// Last cursor position sent, so moves are reported once.
    cursor: Option<FramePos>,
}

impl WorldViewUi {
    pub fn new(root: UiRoot, strings: Box<dyn StringLookup>) -> Self {
        WorldViewUi {
            root,
            strings,
            queue: UiQueue::default(),
            cursor: None,
        }
    }
}

/// The compute compositor on Bevy's own device, with its atlas.
#[derive(Resource)]
pub struct WorldViewGpu {
    pub gpu: Gpu,
    pub atlas: GpuAtlas,
}

/// The presented image and its sprite.
#[derive(Resource)]
pub struct WorldViewTarget {
    pub image: Handle<Image>,
    pub sprite: Entity,
}

#[derive(Component)]
struct WorldViewSprite;

/// Adds the world view systems. `gpu`: create [`WorldViewGpu`] from
/// Bevy's render device once a [`WorldViewState`] exists (no device, no
/// GPU path: the CPU reference is presented).
pub struct WorldViewPlugin {
    pub gpu: bool,
}

impl Default for WorldViewPlugin {
    fn default() -> Self {
        WorldViewPlugin { gpu: true }
    }
}

#[derive(Resource)]
struct GpuWanted(bool);

impl Plugin for WorldViewPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GpuWanted(self.gpu)).add_systems(
            Update,
            (init_gpu, ui_input, world_view_frame, present_scale)
                .chain()
                .run_if(resource_exists::<BridgeResource>)
                .run_if(resource_exists::<WorldViewState>),
        );
    }
}

/// Creates the GPU path once, from the main world's render device.
fn init_gpu(
    mut commands: Commands,
    wanted: Res<GpuWanted>,
    mut tried: Local<bool>,
    device: Option<Res<RenderDevice>>,
    queue: Option<Res<RenderQueue>>,
) -> Result {
    if *tried || !wanted.0 {
        return Ok(());
    }
    *tried = true;
    if let (Some(device), Some(queue)) = (device, queue) {
        let gpu = Gpu::from_device(device.wgpu_device().clone(), (**queue.0).clone());
        commands.insert_resource(WorldViewGpu {
            gpu,
            atlas: GpuAtlas::new(GPU_ATLAS_PAGES)?,
        });
    }
    Ok(())
}

/// Window input → UI events (§A4): cursor in frame coordinates, pointer
/// buttons routed as-is (their meaning is `TODO(spec: ui/controls.md)`).
/// Keyboard actions come from the controls layer (C9), not wired here.
fn ui_input(
    ui: Option<NonSendMut<WorldViewUi>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    buttons: Res<ButtonInput<MouseButton>>,
) -> Result {
    let (Some(mut ui), Ok(window)) = (ui, windows.single()) else {
        return Ok(());
    };
    // A window below 800×600 has no frame mapping (`ui.md` open question
    // 1 of the C8 notes): pointer input is dropped as outside the frame.
    let pos = edge::cursor_frame_pos(window).unwrap_or(FramePos::Outside);
    if ui.cursor != Some(pos) {
        let e = match pos {
            FramePos::Inside(p) => UiEvent::CursorMoved(p),
            FramePos::Outside => UiEvent::CursorLeft,
        };
        ui.queue.0.push(e);
        ui.cursor = Some(pos);
    }
    let FramePos::Inside(at) = pos else {
        return Ok(());
    };
    for (bevy, button) in [
        (MouseButton::Left, PointerButton::Left),
        (MouseButton::Right, PointerButton::Right),
        (MouseButton::Middle, PointerButton::Middle),
    ] {
        if buttons.just_pressed(bevy) {
            ui.queue.0.push(UiEvent::Press { button, at });
        }
        if buttons.just_released(bevy) {
            ui.queue.0.push(UiEvent::Release { button, at });
        }
    }
    Ok(())
}

fn rgba_image(rgba: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: VIEW.width,
            height: VIEW.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    image
}

/// UI frame, draw list, composition, image update.
#[allow(clippy::too_many_arguments)]
fn world_view_frame(
    mut commands: Commands,
    mut bridge: ResMut<BridgeResource>,
    mut state: ResMut<WorldViewState>,
    ui: Option<NonSendMut<WorldViewUi>>,
    gpu: Option<ResMut<WorldViewGpu>>,
    target: Option<Res<WorldViewTarget>>,
    mut images: ResMut<Assets<Image>>,
) -> Result {
    let ui_frame = match ui {
        Some(mut ui) => {
            let ui = &mut *ui;
            Some(run_ui(
                &mut ui.root,
                &mut ui.queue,
                &mut bridge.0,
                ui.strings.as_ref(),
            )?)
        }
        None => None,
    };
    let draws = ui_frame.as_ref().map_or(&[][..], |f| &f.draws[..]);
    let state = &mut *state;
    let frame = build(bridge.0.world(), draws, state.rules.as_ref(), &state.assets)?;
    let use_gpu = gpu.is_some();
    let rgba = match gpu {
        Some(mut g) => {
            let g = &mut *g;
            g.atlas.compose(&g.gpu, &frame, &state.assets)?
        }
        None => compose_cpu(&frame, &state.assets)?,
    };
    state.last = Some(FrameStats {
        bridge_frame: bridge.0.world().frames,
        items: frame.items.len(),
        units_drawn: frame.units_drawn,
        units_hidden: frame.units_hidden,
        ui_sent: ui_frame.as_ref().map_or(0, |f| f.sent),
        ui_unhandled: ui_frame.as_ref().map_or(0, |f| f.unhandled.len()),
        gpu: use_gpu,
    });
    match target {
        Some(t) => {
            let mut image = images
                .get_mut(&t.image)
                .ok_or("world view image asset is gone")?;
            image.data = Some(rgba);
        }
        None => {
            let image = images.add(rgba_image(rgba));
            let layer = RenderLayers::layer(PRESENT_LAYER);
            commands.spawn((
                Camera2d,
                Camera {
                    order: 1,
                    clear_color: ClearColorConfig::Custom(Color::srgb_u8(0, 0, 0)),
                    ..default()
                },
                Msaa::Off,
                Tonemapping::None,
                layer.clone(),
            ));
            let sprite = commands
                .spawn((Sprite::from_image(image.clone()), WorldViewSprite, layer))
                .id();
            commands.insert_resource(WorldViewTarget { image, sprite });
        }
    }
    Ok(())
}

/// Integer presentation scale (§A9; same factor as `ui::Presentation`, so
/// the cursor mapping matches), converted to logical units for Bevy.
fn present_scale(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut sprites: Query<&mut Transform, With<WorldViewSprite>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok(p) = edge::presentation(window) else {
        return;
    };
    let s = p.scale as f32 / window.scale_factor();
    for mut t in &mut sprites {
        t.scale = Vec3::new(s, s, 1.0);
    }
}
