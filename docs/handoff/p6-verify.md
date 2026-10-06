# Handoff: Phase 6 C6 — verify harness (`d2_client::verify`)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Branch `claude/p6-verify`, from `claude/bold-ptolemy-jvyvxy` at `ed7236e`
(2026-10-06, cloud, repo only: no game files, no GPU). Spec:
`specs/client/render-pipeline.md` §A10 and Test vectors (d2rs-own design
draft); the `map` case is `specs/render/map-preview.md` unchanged.

## State

**Implemented; CPU half proven in CI, GPU half of `synthetic` cases not
wired yet (C5).** `d2-client verify` is a runner over versioned case
files. Every Test vectors row with a CPU half is a case file whose CPU
half runs in `cargo test -p d2-client verify`, together with the M08
perturbation for each case. The `map` case is today's verify, moved, not
changed (same CPU reference, same GPU app, same lines, same perturbation
rule). Nothing here reproduces original-game behavior.

Changes outside the new module: `pub mod verify;` + doc lines in
`lib.rs`; `main.rs` `verify` dispatch and options (the map code moved to
`verify/map.rs`). No dependency, spec, `scene/`, `app.rs` or other-crate
edits.

## Code map rows

| Path | What | Spec |
|---|---|---|
| `crates/d2-client/src/verify/mod.rs` | `GpuCompositor` trait + `GpuJob` + `GpuOutcome`, `NotWired`; `build` (case → ordered `DrawItem`s, `MapTable`, frames, synthetic palette); `run_cpu` / `run_synthetic`; `compare` (count + first mismatch, screen coords); `perturb` (exactly N, N > pixels is an error); `Status`, `Summary` (exit codes); `load_dir` / `load_file` | `client/render-pipeline.md` §A10 |
| `crates/d2-client/src/verify/case.rs` | case file format v1, strict parser (`CaseError { at: "item[2].frame", kind }`) | §A10, M07, M20 |
| `crates/d2-client/src/verify/map.rs` | case kind `map`: today's verify (`archives`, `file_stem`, `full_view`, `run`) | `render/map-preview.md` |
| `crates/d2-client/src/verify/tests.rs` | every synthetic case's CPU half, perturbation per case, strict parsing, seam statuses | §A10, Test vectors |
| `crates/d2-client/render-cases/*.toml` | `map` + 8 `synth-*` cases (one per Test vectors row 1–8) | §Test vectors |

## Command line

- `d2-client verify` — all cases in `crates/d2-client/render-cases/`
  (sorted by file name). New: `--case NAME` (repeatable), `--cases DIR`,
  `--perturb N` (applies to each selected case).
- Any map flag (`--ds1`, `--wall-base`, `--view`, `--out`) runs today's
  single-map verify from the flags, exactly as before (combining it with
  `--case`/`--cases` is an error). `--perturb N` with N above the pixel
  count is now an error (before: fewer than N pixels were corrupted).
- Exit codes of the case run: `0` all PASS; `1` any FAIL or ERROR; `2`
  none failed but some case reports `GPU NOT WIRED` (incomplete, not a
  pass). **Change:** bare `verify` used to be the map verify only (exit 0
  on match); it now also runs the synthetic cases and exits 2 until C5
  wires the GPU. `verify --case map` is the old behavior and exit code.

## Case file format (version 1)

Documented at the top of `verify/case.rs`. Root: `version = 1` (missing
or other = error, checked first), `kind` (`synthetic` | `map`), optional
`description`. `synthetic`: `view` (default 800×600 frame), `[[frame]]`
(`width`, `height`, `pixels` or `fill`), `[[map]]` (`base` identity|zero,
`set` pairs), `[[table]]` (`rule` src|dest|add|xor, 256×256), `[[item]]`
(`frame`, `x`, `y`, optional `clip`, `shade`, `table`, `key`, `flip_x`),
`[[expect]]` (`x`, `y`, `index` on the index framebuffer). `map`: `ds1`,
`wall_base`, optional `view`. Unknown keys, wrong types, out-of-range
numbers, duplicate `set` indices, undefined frame/map/table references
are errors. The runner applies `scene::order` before composing (as play
will). The synthetic palette is `rgb(i, 255 − i, 37·i mod 256)`.

Per synthetic case: compose (reference) → `[[expect]]` checks → perturb
reference by N → `bin` + `compose_binned` vs reference (CPU, CI) → GPU
via `GpuCompositor` vs reference. With `--perturb N` the CPU binned
comparison reports exactly N and the case FAILs, in CI too.

## Seam for C5 (GPU compositor)

```rust
pub trait GpuCompositor { fn compose(&mut self, job: &GpuJob<'_>) -> GpuOutcome; }
pub struct GpuJob<'a> { case, items /* ordered */, bins, frames: &[FrameImage],
                        maps: &MapTable, palette: &Palette, view: Rect }
pub enum GpuOutcome { Image(Vec<u8> /* RGBA8 view-sized, alpha 255 */), NotWired, Error(String) }
```

The implementation runs headless/offscreen, applies the Phase 1b
stable-capture rule itself and returns the image; the harness compares.
Wiring: replace `let mut gpu = verify::NotWired;` in `main.rs` `verify`
(marked `TODO(C5)`). Note: several Bevy apps in one process (the `map`
case starts one via `app::run`) may not be supported; if not, C5's
implementation should keep one app for all synthetic jobs, or the runner
should run the map case last / in a child process — decide when wiring.
When §A9 ports the `map` case to the compositor, `verify/map.rs` moves
to `GpuCompositor` too and the `Material2d` path goes.

## Local run queue (for `docs/HANDOFF.md` §5; not edited here)

`cargo run --release -p d2-client -- verify` with `D2_GAME_DIR` set, on a
machine with a GPU. Expected output (Bevy log lines interleaved):

```
verify: 9 cases from …/crates/d2-client/render-cases
case map (map)
verify data\global\tiles\ACT1\TOWN\townN1.ds1: <K> draw items, view <W>x<H> at <L>,<T>
images in game/renders/verify-townN1
PASS: GPU render matches the CPU reference exactly
PASS map (map)
case synth-bins-span (synthetic)
  3 items, view 800x600 at 0,0, 6 expectations
  CPU binned: 0 of 480000 pixels differ
GPU NOT WIRED synth-bins-span (synthetic)
… (same for synth-clip, synth-index-table, synth-opaque, synth-order-keys,
   synth-order-stable, synth-shade-chain, synth-shade-one)
summary: 1 pass, 0 fail, 0 error, 8 GPU not wired
```

Exit code 2. Look for: the map lines identical to the last recorded
`d2-client verify` result (same K, W×H, L,T; PASS). Then
`cargo run --release -p d2-client -- verify --case map --perturb 5`: map
FAILs (app log reports 5 pixels differ), exit 1. If the second Bevy app
panics after the map case, record it (see seam note).

## Open questions

1. Exit code 2 for "GPU not wired": chosen so not-wired is never a pass;
   the coordinator may prefer another code.
2. Synthetic images are not written to disk (no game art; the report has
   count and first mismatch). C5 may want `gpu.png`/`cpu.png` for
   debugging a mismatch.

## Gate run

See the commit: `cargo fmt --all -- --check`, `cargo clippy -p d2-client
--all-targets -- -D warnings`, `cargo test -p d2-client`, `cargo run -p
depcheck`, `python3 tools/spec_index.py --check`, `python3
tools/methods.py check`, `python3 tools/coverage.py --check`.
