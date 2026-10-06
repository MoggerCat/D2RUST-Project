# Handoff: Phase 6 C6 × C5 — verify harness GPU half + COF cases

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Scope: branch `claude/p6-verify-gpu`, from `claude/bold-ptolemy-jvyvxy`
at `a5b323a` (2026-10-06, cloud, repo only: no game files). Spec:
`specs/client/render-pipeline.md` §A7, §A9, §A10, Test vectors.
Integration task; no spec, HANDOFF, PLAN, `scene`, `composite`, `frames`,
`gpu_compositor` or other-crate edits.

## State

**Wired; GPU half proven on a software adapter only.** `d2-client verify`
runs every `synthetic` case CPU vs GPU (the C5 compute compositor,
headless) and compares both the index framebuffer (per byte) and the RGBA8
image (per pixel) against the CPU reference. On a real GPU it is
**unverified** until the local run below records its result (rule 10).

Measured in the cloud (Mesa llvmpipe 25.2.8 / LLVM 20.1.2, Vulkan, CPU
adapter; `apt-get install mesa-vulkan-drivers`, not part of
`tools/cloud-setup.sh`): all 10 synthetic cases `GPU indices: 0 of P
bytes differ`, `GPU: 0 of P pixels differ`, PASS, exit 0; with
`--perturb 7` every case reports exactly 7 on CPU binned, GPU indices and
GPU RGBA, `FAIL … CPU and GPU halves`, exit 1. With the Vulkan driver
hidden (`VK_ICD_FILENAMES=/nonexistent.json`): `NO ADAPTER`, summary
`1 no adapter`, exit 2. The `map` case was not run (no game files).

## What changed

| Path | What |
|---|---|
| `crates/d2-client/src/verify/gpu.rs` (new) | `Wgpu`: `GpuCompositor` over `gpu_compositor::Gpu::headless`, opened lazily on the first synthetic job (`open()` gives the adapter line). Per job: `AtlasFrames::from_images` (4 pages max), `pack` against the runner's own bins (mismatch = error), `compose_rgba` **twice**; two different readbacks are an error (Phase 1b stable-capture rule). `GpuError::Adapter` → `GpuOutcome::NoAdapter`. |
| `crates/d2-client/src/verify/mod.rs` | `GpuOutcome::Image { indices, rgba }` (was RGBA only) and `GpuOutcome::NoAdapter`; `Status::NoAdapter` (label `NO ADAPTER`, exit 2 like not-wired, never a pass); `Summary::no_adapter`. `compare_indices` / `ByteMismatch`. `--perturb` now flips the index top bit of the reference (`perturb_indices`, same pixel spacing as `perturb`); the RGBA reference is derived from it, so under the synthetic palette (red = index) exactly N bytes and N pixels differ. `run_cpu` returns a `Reference { indices, rgba }`. The GPU half now runs even when the CPU half failed, so `--perturb N` shows N on both halves (`Fail("CPU and GPU halves")`). `[[unit]]` building: `UnitFixture` (a `ComponentResolver` from the case's per-component answers) → `d2_formats::cof::Cof::parse` → `composite::build` → items appended after `[[item]]`s, then `scene::order`. New `BuildError` variants `UnitUndefined`, `UnitScene`, `Cof`, `Composite`. |
| `crates/d2-client/src/verify/case.rs` | Case format v1 gains `[[unit]]` (`cof` hex string, `dir`, `frame`, `key = [pass, major, minor]`, optional `clip`, `[[unit.component]]` with `component`, `frame`, `x`, `y`, optional `shade`, `table`). A synthetic case needs ≥ 1 `[[item]]` or `[[unit]]`. Strict as before: bad hex, odd digits, duplicate component, component > 15, unknown keys are errors with key paths. |
| `crates/d2-client/src/verify/tests.rs` | Echo stand-in returns both images; no-adapter status and exit code; `--perturb 5` with a matching GPU stand-in reports 5 on CPU binned, GPU indices and GPU RGBA; COF cases build one item per layer with `sub` = slot and cross-unit key order; unit parsing/building strictness; ignored `gpu_half_matches_cpu_on_every_synthetic_case` (real adapter; no adapter fails it). |
| `crates/d2-client/render-cases/synth-cof-units.toml` (new) | COF bytes (3 layers HD/TR/RH, 2 dirs × 2 frames, `cof.md` layout) → two units over a floor item: d0 f1 with a shaded RH, d1 f0 with an xor-blended RH; unit 1 listed later but `major` 50 < 100 draws first. 10 expectations. |
| `crates/d2-client/render-cases/synth-cof-frames.toml` (new) | Same COF, same component layout at d1 f1 vs d0 f0: different front component from the draw order alone; equal unit keys keep list order. 5 expectations. |
| `crates/d2-client/src/main.rs` | `verify` uses `verify::gpu::Wgpu` (replaces `NotWired`, the `TODO(C5)`), prints `GPU compositor: adapter: …` / `no adapter: …` before the first synthetic case (after the `map` case, which sorts first and starts its own Bevy app). All arguments unchanged. |

Decision (d2rs-own): `[[unit]]` extends format **version 1** instead of
bumping it. A reader without it rejects the key (strict, unknown key), so
no file is ever misread; only this crate reads case files. Bump if the
coordinator prefers.

No behavior change in `scene`/`composite`/`frames`/`gpu_compositor`; no
bug found there.

## Local run queue (for `docs/HANDOFF.md` §5; not edited here)

On the developer PC with a real GPU, `D2_GAME_DIR` set:

1. `cargo run --release -p d2-client -- verify`
   Expect: `verify: 11 cases from …/render-cases`; the `map` case lines as
   in the last recorded `d2-client verify` (same K items, W×H, L,T; `PASS:
   GPU render matches the CPU reference exactly`); then `GPU compositor:
   adapter: <real GPU name> (<backend>, DiscreteGpu|IntegratedGpu, …)`;
   then for each of `synth-bins-span`, `synth-clip`, `synth-cof-frames`,
   `synth-cof-units`, `synth-index-table`, `synth-opaque`,
   `synth-order-keys`, `synth-order-stable`, `synth-shade-chain`,
   `synth-shade-one`: `CPU binned: 0 of P pixels differ`, `GPU indices: 0
   of P bytes differ`, `GPU: 0 of P pixels differ`, `PASS`. Last line
   `summary: 11 pass, 0 fail, 0 error, 0 GPU not wired, 0 no adapter`,
   exit 0. If the process panics or hangs after the map case's Bevy app
   (two wgpu instances in one process), record it; workaround: run
   `verify --case map` and the synth cases separately.
2. `cargo run --release -p d2-client -- verify --perturb 7`
   Expect: every synthetic case `CPU binned: 7 of …`, `GPU indices: 7 of
   … bytes differ`, `GPU: 7 of … pixels differ`, `FAIL … CPU and GPU
   halves`; the map case FAILs with 7 (its app log); exit 1. Any count
   other than 7 fails the check (M08).
3. `cargo test -p d2-client --lib verify::tests::gpu_half -- --ignored --nocapture`
   Expect: first line `adapter: <real GPU>`, test passes.

Record adapter name, backend and driver with the result. Until then §A9
(and the C7 COF path through the GPU) is proven on llvmpipe only.

## Open questions

1. Exit code 2 now covers "GPU not wired" and "no adapter"; `NotWired` is
   kept for tests and builds without a compositor.
2. The stable-capture rule is applied as "two dispatches agree". For a
   compute dispatch with readback there is no frame pacing to wait out;
   the coordinator may judge one dispatch enough.

## Gate run

All pass on this branch: `cargo fmt --all -- --check`, `cargo clippy -p
d2-client --all-targets -- -D warnings`, `cargo test -p d2-client` (191
lib tests pass, 5 ignored incl. the new GPU test), `cargo run -p
depcheck` (8 crates OK), `python3 tools/spec_index.py --check`, `python3
tools/methods.py check` (21 OK), `python3 tools/coverage.py --check` (0
errors).
