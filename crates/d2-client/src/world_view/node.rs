// Spec: specs/client/render-pipeline.md (A9, A1 stage 4b)
//! The in-app GPU compositor: a system of Bevy's render graph (the
//! `RenderGraph` schedule of the render world, set
//! `RenderGraphSystems::Render`, before the camera driver) that runs the
//! compute compositor ([`Gpu::encode_rgba`]) on Bevy's own device
//! (`Gpu::from_device(render_device.wgpu_device().clone(), queue)`, HANDOFF
//! §2 step 5) and copies its RGBA rows into the presented image's texture.
//! No readback: the frame never leaves the GPU.
//!
//! The main world packs the frame ([`super::GpuAtlas::pack`]) and hands
//! the render world a [`ComposeJob`]: the packed buffers, the atlas pages
//! (shared, replaced only when a frame set was added), the palette and the
//! target image. The node owns the atlas array texture and re-uploads the
//! pages only when their version changes. Pixels are the compositor's:
//! this module moves bytes and decides none.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use bevy::core_pipeline::schedule::camera_driver;
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::renderer::{
    RenderContext, RenderDevice, RenderGraph, RenderGraphSystems, RenderQueue,
};
use bevy::render::texture::GpuImage;
use bevy::render::RenderApp;
use d2_formats::palette::Palette;

use crate::frames::atlas::AtlasPage;
use crate::gpu_compositor::{Gpu, Packed};

/// One frame for the node, extracted to the render world when it changes.
#[derive(Resource, Clone)]
pub struct ComposeJob {
    /// Job number, one per built frame: a job is composed once.
    pub seq: u64,
    /// The bridge frame it shows (diagnostics).
    pub frame: u64,
    pub packed: Arc<Packed>,
    pub pages: Arc<Vec<AtlasPage>>,
    /// Changes whenever `pages` changes (a frame set was added).
    pub pages_version: u64,
    pub palette: Arc<Palette>,
    pub target: Handle<Image>,
}

impl ExtractResource for ComposeJob {
    type Source = ComposeJob;

    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

/// Frames the node has composed into the target, shared by both worlds
/// (logs and tests: the main world reads it).
#[derive(Resource, Clone, Default)]
pub struct NodeRuns(pub Arc<AtomicU64>);

impl NodeRuns {
    pub fn get(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }
}

/// The node's own GPU state (render world).
#[derive(Resource, Default)]
struct NodeState {
    gpu: Option<Gpu>,
    /// The atlas array: pages version, texture, page count.
    atlas: Option<(u64, wgpu::Texture, u32)>,
    /// The last composed job's `seq`.
    last: Option<u64>,
}

/// Adds the node and the job's extraction when there is a render world.
/// Returns whether it did (no render world: no node, the CPU reference is
/// presented).
pub fn add_node(app: &mut App) -> bool {
    let runs = NodeRuns::default();
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return false;
    };
    render_app
        .insert_resource(runs.clone())
        .init_resource::<NodeState>()
        .add_systems(
            RenderGraph,
            compose_node
                .in_set(RenderGraphSystems::Render)
                .before(camera_driver),
        );
    app.insert_resource(runs)
        .add_plugins(ExtractResourcePlugin::<ComposeJob>::default());
    true
}

/// Bytes per presented pixel (RGBA8).
const PIXEL: u32 = 4;

fn compose_node(
    job: Option<Res<ComposeJob>>,
    mut state: ResMut<NodeState>,
    runs: Res<NodeRuns>,
    images: Res<RenderAssets<GpuImage>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    mut ctx: RenderContext,
) -> Result {
    let Some(job) = job else { return Ok(()) };
    if state.last == Some(job.seq) {
        return Ok(());
    }
    // The target is uploaded during the render world's prepare step; until
    // it is, wait (the job stays and is composed next frame).
    let Some(target) = images.get(&job.target) else {
        return Ok(());
    };
    let state = &mut *state;
    let gpu = state
        .gpu
        .get_or_insert_with(|| Gpu::from_device(device.wgpu_device().clone(), (**queue.0).clone()));
    let pages = job.pages.len() as u32;
    if state.atlas.as_ref().map(|a| a.0) != Some(job.pages_version) {
        state.atlas = Some((job.pages_version, gpu.atlas_texture(&job.pages), pages));
    }
    let (_, atlas, atlas_pages) = state.atlas.as_ref().ok_or("atlas missing")?;
    let p = job.packed.params;
    let size = target.texture_descriptor.size;
    if (size.width, size.height) != (p.width, p.height) {
        return Err(format!(
            "target is {}x{}, the frame {}x{}",
            size.width, size.height, p.width, p.height
        )
        .into());
    }
    let encoder = ctx.command_encoder();
    let Some(rgba) = gpu.encode_rgba(encoder, &job.packed, atlas, *atlas_pages, &job.palette)?
    else {
        return Ok(());
    };
    // One copy per row: the buffer's rows are `width × 4` bytes, which
    // need not meet the 256-byte row alignment of a multi-row copy.
    let row = u64::from(p.width * PIXEL);
    for y in 0..p.height {
        encoder.copy_buffer_to_texture(
            wgpu::TexelCopyBufferInfo {
                buffer: &rgba,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: u64::from(y) * row,
                    bytes_per_row: None,
                    rows_per_image: None,
                },
            },
            wgpu::TexelCopyTextureInfo {
                texture: &target.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: p.width,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }
    state.last = Some(job.seq);
    runs.0.fetch_add(1, Ordering::AcqRel);
    Ok(())
}
