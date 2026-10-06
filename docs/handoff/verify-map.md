# Handoff: frame store + `map` verify case on the compute compositor — `claude/verify-map`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: integration from
clear specs, medium (METHODS M14). Branch `claude/verify-map`, from `main`
at `edad871` (repo only: no `game/`, no real GPU). HANDOFF §2 step 5
(the frame store and the `map` port; the in-app render node is not
part of it). Specs: `specs/client/render-pipeline.md` §A2, §A7 step 3,
§A9, §A10; `specs/client/assets.md` §A3–§A4; `specs/render/map-preview.md`
(unchanged). No spec, HANDOFF, PLAN, `app.rs`, `main.rs`, `world_view/`,
`Cargo.toml` or `tests/e2e_*.rs` edit.

## State

**(a) Frame store: implemented.** `frames::FrameStore` maps
`(FrameSetKey, index)` → `scene::FrameId` (dense ids, insertion order),
is a `scene::FrameSource`, and builds a C3 atlas whose `slots[n]` is
`FrameId(n)` (`FrameStore::atlas` → `gpu_compositor::AtlasFrames`, the
`SlotSource` numbering). `composite::build_with(…, frames: &impl
FrameIds)` takes every component's id from the store; the
`ComponentResolver::frame_id` hook is no longer asked on that path. The
verify `[[unit]]` fixture now uses it (all the case's `[[frame]]`s are one
set, `synthetic/case-frames.dc6` direction 0, so `[[frame]]` n is
`FrameId(n)` as before; the fixture has no `frame_id` any more).

**(b) `map` case ported: implemented, GPU half unverified on a real GPU.**
`verify/map.rs` no longer starts a Bevy app: the layout becomes scene
items one to one (layout order, opaque, unshaded, key 0, clip = the
whole view) and goes through `GpuCompositor` in chunks of at most
1024² (`CHUNK`; a whole map is up to 8192², above the compositor's
storage limits). The CPU reference is `map::cpu` (`map-preview.md`),
unchanged. Three byte-for-byte comparisons per run, all against the whole
view's reference: CPU binned compose of the scene list (indices), GPU
indices, GPU RGBA. `--perturb N` flips N reference indices and the red
top bit of the same N RGBA pixels, so each comparison reports exactly N
whatever the palette (M08). `cpu.png`, `gpu.png`, `diff.png` are written
as before.

Measured in the cloud (Mesa llvmpipe 25.2.8 / LLVM 20.1.2, Vulkan, CPU
adapter; `apt-get install mesa-vulkan-drivers libvulkan1`): the ignored
`verify::map::tests::gpu_map_matches_cpu_reference` (synthetic 100×70
map, 9 items, chunk sides 32 / 64 / 1024) `GPU indices: 0 of 7000 bytes
differ`, `GPU: 0 of 7000 pixels differ`, and with perturb 7 exactly 7 on
CPU binned, GPU indices and GPU RGBA; the ignored
`verify::tests::gpu_half_matches_cpu_on_every_synthetic_case` still
passes on all 10 synthetic cases (the COF cases now through the store).
The real `townN1` map case was not run (no game files).

**Material2d path: no longer used by verify, not deleted.** `render/`
(`PaletteMaterial`, `palette.wgsl`) and `app::Mode::Verify` /
`VerifyConfig` / `compare_rgba` are still compiled because `app.rs`
(owned by p6-window) uses `PaletteMaterial` for `view` and still has the
verify mode. Nothing calls `app::Mode::Verify` now. Deleting them is an
`app.rs` edit: p6-window (or the coordinator) removes `render/mod.rs`,
`render/palette.wgsl`, `PaletteRenderPlugin` / `PaletteMaterial` uses and
`Mode::Verify` with its systems once the window draws through the
compositor (§A9: "then is removed").

## Code map rows (for HANDOFF §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-client/src/frames/store.rs`, `store/tests.rs` | `FrameStore` (`insert`, `id`, `frame`, `owner`, `contains`, `len`, `frames`, `atlas`), `StoreError::{Duplicate, NotResident, Index, Full}`; `scene::FrameSource` impl | `client/render-pipeline.md` §A2, §A7 step 3; `client/assets.md` §A4 |
| `crates/d2-client/src/composite/mod.rs`, `store_tests.rs` | `FrameIds` (implemented by `FrameStore`), `build_with` (ids from the store); `build` unchanged (ids from `ComponentResolver::frame_id`, which now has a refusing default) | `client/render-pipeline.md` §A7 |
| `crates/d2-client/src/verify/map.rs`, `map/tests.rs` | case kind `map` through `GpuCompositor`: `MapScene`, `compare_layout`, `chunks`, `chunk`, `MapRun` (`verdict`), `GpuResult`, `run` (own `Wgpu`), `run_with` (caller's compositor), `CHUNK` = 1024; `archives`, `file_stem`, `full_view`, `MAX_TEXTURE_SIDE` kept | `render/map-preview.md`; `client/render-pipeline.md` §A9, §A10 |
| `crates/d2-client/src/verify/mod.rs` | `[[unit]]` fixture on the store: `case_frames_key`, `composite::build_with` | §A7, §A10 |

## Signature changes

- `composite::ComponentResolver::frame_id` now has a default body
  (returns `Unresolved { what: "frame_id" }`). Existing implementors
  (`world_view::UnitResolver`, test fixtures) compile unchanged. New:
  `composite::FrameIds`, `composite::build_with`, `frames::store`,
  `frames::{FrameStore, StoreError}` re-exports, `verify::case_frames_key`.
- `verify::map`: `run(name, case, out, perturb)` keeps its signature (it
  opens its own headless `Wgpu`); new `run_with(…, gpu: &mut dyn
  GpuCompositor)`. The Bevy `app::run` call is gone from verify.
- For p6-window (`main.rs`, `world_view/`): call `verify::map::run_with(…,
  &mut gpu)` with the runner's shared `Wgpu` so one process opens one
  device (today the map case opens a second headless device; both are
  plain wgpu, no Bevy app any more); `world_view::FrameTable` can become a
  `FrameStore` (or implement `FrameIds`) and `UnitResolver` can drop
  `frame_id` and call `composite::build_with`.

## Decisions (d2rs-own, for the PLAN decisions log)

1. Case-file format stays **version 1**: the `map` case file is
   unchanged (`ds1`, `wall_base`, optional `view`); only the runner
   changed.
2. The map view is composed in chunks of ≤ 1024² (one `GpuJob` each,
   only the items and images touching the chunk). Per-pixel results do
   not depend on chunking (each pixel walks the same items in the same
   order); the GPU test checks sides 32, 64 and 1024.
3. `--perturb` on the map case corrupts indices and RGBA independently at
   the same pixels (before: RGBA only), so the count is exactly N on all
   three comparisons for any palette.
4. Frame ids in the store are dense and follow insertion order; a store
   is append-only (eviction rebuilds it, residency stays C2's).

## Questions

1. **Index 0 (`render/composition.md` §B2):** the map reference paints
   index 0 opaque black (`map-preview.md` §Palette shading); the
   compositor maps it through the palette. If an act palette's entry 0 is
   not black, the map case FAILs on the RGBA comparison with exactly the
   background pixel count while indices agree (test
   `index_zero_question_is_visible`). Not decided here; the local run
   shows which.
2. `GpuJob` / `Wgpu` cap the atlas at 4 pages per job (`verify::gpu::MAX_PAGES`);
   a chunk whose tiles need more fails loudly (`GpuError`). 1024² chunks
   should stay far below 16 M texels; if a map hits it, lower `CHUNK`.

## Local checks to queue (HANDOFF §5 C; real GPU, `D2_GAME_DIR` set)

1. `cargo run --release -p d2-client -- verify --case map`
   Expect: `verify data\global\tiles\ACT1\TOWN\townN1.ds1: <K> draw items,
   view <W>x<H> at <L>,<T>` with the same K, W×H, L,T as the last recorded
   Phase 1b run; `images in game/renders/verify-townN1`; `PASS: GPU render
   matches the CPU reference exactly`; report lines `<C> chunks of at most
   1024x1024`, `CPU binned: 0 of <W·H> bytes differ`, `GPU indices: 0 of
   <W·H> bytes differ`, `GPU: 0 of <W·H> pixels differ`; `PASS map (map)`;
   exit 0. If only the `GPU:` (RGBA) line is non-zero and `GPU indices` is
   0, record `palette[0]` of the act palette: that is question 1, not a
   compositor bug.
2. `cargo run --release -p d2-client -- verify --case map --perturb 7`
   Expect: `CPU binned: 7 of …`, `GPU indices: 7 of …`, `GPU: 7 of …`,
   `FAIL map (map): CPU and GPU halves`, exit 1. Any other count fails
   the check (M08).
3. `cargo run --release -p d2-client -- verify` (all 11 cases)
   Expect: as the p6-verify-gpu queue entry, with the map lines above;
   `summary: 11 pass, 0 fail, 0 error, 0 GPU not wired, 0 no adapter`,
   exit 0. Record whether two headless devices in one process cause any
   problem (see signature changes).
4. `cargo test -p d2-client --lib verify::map::tests::gpu_map -- --ignored --nocapture`
   Expect: first line `adapter: <real GPU name> (<backend>, …)`, test
   passes.

Record adapter name, backend and driver with each result. Until then the
`map` case's GPU half is proven on llvmpipe only (synthetic layout).

## Gate results

All pass on this branch (cloud, 2026-10-06): `cargo fmt --all -- --check`;
`cargo clippy --workspace --all-targets -- -D warnings`; `cargo test
--workspace` (0 failed; `d2-client` lib 224 pass, 6 ignored: 19 new
tests, 1 new ignored GPU test); `cargo run -p depcheck` (8 crates OK);
`python3 tools/spec_index.py --check`; `python3 tools/methods.py check`
(21 OK); `python3 tools/coverage.py --check` (3,197 claims, 0 errors) and
`--selftest` (ok). On llvmpipe: `cargo test -p d2-client --lib --
--ignored verify::map::tests::gpu_map verify::tests::gpu_half` both pass.
No generated files in the diff. `Covers:` claims: two, on
`render-pipeline.md §a10-verify-harness-extension` (the map case's
comparison and its exact `--perturb` count).
