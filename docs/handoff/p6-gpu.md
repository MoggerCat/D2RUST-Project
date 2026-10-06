# Handoff: Phase 6 C5 — GPU compute compositor (`d2_client::gpu_compositor`)

Scope: branch `claude/p6-gpu`, from `claude/bold-ptolemy-jvyvxy` at
`ed7236e` (2026-10-06, cloud). Spec: `specs/client/render-pipeline.md`
§A9 (+ the §A10 diff/perturb pieces the harness needs), d2rs-own design
draft. Repo only; no game files.

## State

**Implemented; CI half proven; GPU half proven on a software adapter
only.** The compositor consumes exactly the C4 draw list and bins and
produces the index framebuffer of `scene::compose` (plus the RGBA8 image
of `scene::to_rgba`), integer math only, no fixed-function blending, no
sampler. Byte-exact on the developer GPU is **unverified** until the local
run below records its result.

Measured in the cloud container (Mesa llvmpipe 25.2.8, LLVM 20.1.2,
Vulkan, CPU adapter; installed with `apt-get install mesa-vulkan-drivers
libvulkan1`, not part of `tools/cloud-setup.sh`): all 12 synthetic cases
0 differing bytes (indices) and 0 differing pixels (RGBA); `--perturb 7`
reports exactly 7 / 7 on every case. A software rasterizer says nothing
about a real driver's integer and texture paths, hence the queue entry.

Changes outside the new module: `pub mod gpu_compositor;` in
`crates/d2-client/src/lib.rs`; one dependency in
`crates/d2-client/Cargo.toml`: `wgpu = "29.0.3"` with Bevy's own backend
features (`wgsl, dx12, metal, vulkan, naga-ir`) — the exact wgpu Bevy
0.19.1 already builds, so nothing new is downloaded or compiled (the only
`Cargo.lock` change is the dependency line); `wgpu::naga` (the same naga)
validates the shader in CI. New example `crates/d2-client/examples/gpu_compare.rs`.
No `scene`, `frames`, `render`, `bridge`, spec, HANDOFF or PLAN edit.

## Code map

| Path | What | Spec |
|---|---|---|
| `crates/d2-client/src/gpu_compositor/mod.rs` | `SHADER` (WGSL source), `WORKGROUP` = 16, `GpuError`, re-exports | §A9 |
| `crates/d2-client/src/gpu_compositor/pack.rs` | `pack` (validate + serialize), `Packed`, `GpuItem`, `Params` (explicit `to_le_bytes`/`from_le_bytes`), `SlotSource` (`FrameId` → `AtlasSlot`), `emulate` (the shader's algorithm on the packed bytes, CPU), `AtlasFrames` (synthetic frames → C3 atlas) | §A9 |
| `crates/d2-client/src/gpu_compositor/compositor.wgsl` | `compose` (per pixel: walk the bin's list in order; frame index 0 skips; shade chain; `Opaque`/`IndexTable`) and `to_rgba` (palette word bit copy) | §A4, §A5, §A9 |
| `crates/d2-client/src/gpu_compositor/device.rs` | `Gpu` (`headless`, `from_device`, `compose`, `compose_rgba`): pipelines, upload, limits check, dispatch, readback | §A9 |
| `crates/d2-client/src/gpu_compositor/harness.rs` | 12 synthetic cases (`cases`), `compare`, `prepare`, `diff`, `perturb`, `Report` | §A10, Test vectors |
| `crates/d2-client/src/gpu_compositor/tests.rs` | 11 CI tests + 2 ignored GPU tests | |
| `crates/d2-client/examples/gpu_compare.rs` | headless CLI over the cases, `--case`, `--perturb` | §A10 |

## Entry point (for C6)

```rust
use d2_client::gpu_compositor::{pack, Gpu, harness};
let (gpu, adapter_info) = Gpu::headless()?;            // once
// per case: items in draw order (scene::order applied), frames, maps, view
let bins   = scene::bin(&items, &frames, &maps, view)?;
let packed = pack(&items, &bins, &frames, &slots, &maps, atlas.pages().len() as u32)?;
let (indices, rgba) = gpu.compose_rgba(&packed, atlas.pages(), &palette)?;
// or, for a harness::Case: harness::compare(&gpu, &case, perturb_n)? -> Report
```

- `slots: impl SlotSource` maps each `FrameId` to its C3 `AtlasSlot`
  (`[AtlasSlot]`/`Vec<AtlasSlot>`: `FrameId(n)` = element n). Synthetic
  cases: `AtlasFrames::from_images(&frames, max_pages)` packs a
  `[FrameImage]` with C3's packer and gives `atlas` + `slots`.
- `pack` re-bins with `scene::bin` (so every CPU validation applies, same
  `SceneError`s) and requires the given bins to be equal
  (`GpuError::BinsMismatch`); the view is the bins' view. On top: slot
  missing / wrong size / outside the given pages are errors.
- `harness::compare` builds the CPU reference with `scene::compose`,
  perturbs it, runs the GPU, and reports per-byte (indices) and per-pixel
  (RGBA) diffs with the first mismatch. `Report` prints one line:
  `case NAME: P pixels, I items, G pages: D differing bytes (indices), R differing pixels (rgba)`.
- In-app (render-graph node, not written yet): `Gpu::from_device(
  render_device.wgpu_device().clone(), (**render_queue.0).clone())`;
  same pipelines.

## Design decisions taken inside the spec's latitude (d2rs-own)

- **Workgroups 16×16, not one 32×32 workgroup per bin.** WebGPU's default
  `max_compute_invocations_per_workgroup` is 256; a 1024-invocation
  workgroup is not portable. Each invocation still walks only its own
  bin's list (bin = pixel / 32), so the result is the spec's. §A9's "one
  workgroup per bin" wording could say "per-bin lists, workgroup size
  free" — for the spec owner; not edited here.
- **Output is a storage buffer of one u32 per pixel**, not an R8Uint
  storage texture (r8uint is not a core WebGPU storage format). Readback
  keeps the low byte. Same for RGBA (one u32 = r, g, b, a bytes).
- **RGBA pass is a bit copy of the palette bytes**, not a write to an sRGB
  target, so the verify image is the exact palette bytes like
  `scene::to_rgba`. Presentation to the window (sRGB target, integer
  scale) is outside the verify boundary and not written.
- **Precomputed areas.** `pack` stores each item's drawable area
  (image ∩ clip ∩ view, the CPU's formula) in view coordinates and the
  atlas texel of the area's corner, so the shader does only unsigned
  integer compares and adds; off-view items keep a zero area.
- **Atlas as one R8Uint 2048² texture array** (layer = page), uploaded
  from `Atlas::pages()` per compose call (4 MiB per page). C3's
  `AtlasTextures` holds separate `Image`s per page; an in-app node would
  copy those pages into array layers (texture copy, exact) or own the
  array. Per-call full upload is a verify-harness simplification.
- **Maps** are uploaded as the map table's bytes unchanged; the shader
  reads byte `b` of row `r` as bits `8·(b%4)` of word `64r + b/4`
  (little-endian, WGSL host-shareable layout). Empty buffers are padded
  with one zeroed record nothing references.
- **Limits** (storage buffer size, texture layers, workgroups per
  dimension) are checked before dispatch: `GpuError::Limit`, never a
  partial frame.

## TODO(spec) hooks

None new. The shader implements C4's neutral behaviors exactly (mapped
result 0 draws index 0; clear value 0; one palette; `flip_x` rejected by
`pack` via `scene::bin`), so a §B answer changes `scene` and the WGSL
together; `emulated_shader_matches_cpu_on_all_cases` and the GPU run
catch a one-sided change.

## Open questions

1. §A9 wording (one workgroup per bin), above.
2. Spec OQ 2 (60 fps on a full town scene): not measured; the stress case
   (400 items, 800×600) is a correctness case, not a benchmark.

## Checks to queue (local run queue, `docs/HANDOFF.md` §5)

Needs a real GPU (no game files). From the repo root, on the developer PC:

1. `cargo run -p d2-client --example gpu_compare`
   Expect: an `adapter:` line naming the real GPU, then 12 lines
   `PASS case …: 0 differing bytes (indices), 0 differing pixels (rgba)`
   (cases `vector-opaque`, `vector-chain1`, `vector-chain2`,
   `vector-key-order`, `vector-equal-keys`, `vector-index-table`,
   `vector-clip`, `vector-four-bins`, `empty`, `two-pages`, `offset-view`,
   `stress`), last line `all 12 cases: 0 differing bytes`, exit 0.
2. `cargo run -p d2-client --example gpu_compare -- --perturb 7`
   Expect (M08): 12 lines `FAIL case …: 7 differing bytes (indices), 7
   differing pixels (rgba), 7 perturbed; first at (0,0): …`, then
   `Error: 12 of 12 cases differ`, exit 1. Any count other than 7 is a
   failure of the check.
3. Same as an assertion: `cargo test -p d2-client --lib gpu_compositor::tests::gpu -- --ignored --nocapture --test-threads 1`
   Expect: `gpu_matches_cpu` and `gpu_perturb_reports_exactly_n` pass.

Record the adapter name and backend with the result. When these pass,
§A9 is proven on that GPU for the synthetic cases; the `map`/`sprite`/
`unit` cases come with C6.

## Gate run

All pass on this branch: `cargo fmt --all -- --check`, `cargo clippy -p
d2-client --all-targets -- -D warnings`, `cargo test -p d2-client` (168
lib tests pass, 3 ignored: 2 GPU + C3's game-file test), `cargo run -p
depcheck` (8 crates OK), `python3 tools/spec_index.py --check`,
`python3 tools/methods.py check` (21 OK), `python3 tools/coverage.py
--check` (400 claims, 0 errors).
