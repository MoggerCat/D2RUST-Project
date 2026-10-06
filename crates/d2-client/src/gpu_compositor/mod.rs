// Spec: specs/client/render-pipeline.md
//! GPU compute compositor (§A9): composes exactly the C4 draw list
//! ([`crate::scene`]) into the same index framebuffer as
//! [`crate::scene::compose`], with integer math in a compute shader (no
//! fixed-function blending, no float, no sampler).
//!
//! Three layers, so everything up to the dispatch runs in CI without a GPU:
//!
//! 1. [`pack`] (plain Rust): validates the list exactly as the CPU does,
//!    checks the bins, and serializes items, bins, map table and parameters
//!    into the little-endian byte buffers the shader reads ([`Packed`]).
//!    [`pack::emulate`] runs the shader's algorithm on those bytes, so CI
//!    proves the layout against the CPU reference.
//! 2. `compositor.wgsl`: one invocation per pixel walks its 32×32 bin's
//!    item list in order (§A9); a second entry point maps indices to RGBA8
//!    bytes through the frame palette (a bit copy, no sRGB conversion).
//! 3. [`device`]: wgpu pipelines, upload, dispatch, readback. Runs headless
//!    ([`Gpu::headless`]) or on Bevy's device ([`Gpu::from_device`]).
//!
//! [`harness`] holds the synthetic cases and the byte diff used by the
//! `gpu-compare` example and by C6's verify runner. Original-game behavior
//! (§B) is not decided here: the formulas are the C4 ones and its
//! `TODO(spec: …)` hooks apply unchanged.

pub mod device;
pub mod harness;
pub mod pack;

#[cfg(test)]
mod tests;

pub use device::Gpu;
pub use pack::{pack, AtlasFrames, GpuItem, Packed, SlotSource};

use crate::frames::{AtlasError, FrameError};
use crate::scene::{FrameId, SceneError};

/// The compute shader (WGSL).
pub const SHADER: &str = include_str!("compositor.wgsl");

/// Workgroup side in invocations. A 32×32 bin is covered by 2×2 workgroups
/// of 16×16: WebGPU's default limit is 256 invocations per workgroup, so one
/// 1024-invocation workgroup per bin is not portable. Each invocation still
/// reads only its own bin's list, so the result is the same.
pub const WORKGROUP: u32 = 16;

/// Errors of packing and dispatch. Inputs are strict (M07), as in `scene`.
#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    #[error(transparent)]
    Scene(#[from] SceneError),
    #[error(transparent)]
    Atlas(#[from] AtlasError),
    #[error(transparent)]
    Frame(#[from] FrameError),
    #[error("bins differ from the bins of this draw list and view")]
    BinsMismatch,
    #[error("draw item {index}: frame {frame:?} has no atlas slot")]
    SlotMissing { index: usize, frame: FrameId },
    #[error("draw item {index}: atlas slot is {slot_w}x{slot_h}, frame is {width}x{height}")]
    SlotSize {
        index: usize,
        slot_w: u32,
        slot_h: u32,
        width: u32,
        height: u32,
    },
    #[error("draw item {index}: atlas slot outside page {page} of {pages}")]
    SlotPage { index: usize, page: u32, pages: u32 },
    #[error("{what} needs {needed}, the device allows {limit}")]
    Limit {
        what: &'static str,
        needed: u64,
        limit: u64,
    },
    #[error("{pages} atlas pages given, the list was packed for {packed}")]
    PageCount { pages: usize, packed: u32 },
    #[error("no GPU adapter: {0}")]
    Adapter(String),
    #[error("GPU device: {0}")]
    Device(String),
    #[error("GPU readback: {0}")]
    Readback(String),
}
