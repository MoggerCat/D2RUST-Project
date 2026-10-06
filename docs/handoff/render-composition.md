# Handoff: render composition (framebuffer, frame cycle, pixel write) — `claude/render-composition`

> Waiting to be folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md`; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: implementation from
a clear spec, medium (METHODS M14). Branch `claude/render-composition`,
from `claude/specs-staging` at `c6e40f9` (repo only: no `game/`, no real
GPU). Spec: `specs/render/composition.md` (draft, 1.14d RE, no capture
yet), with `specs/client/render-pipeline.md` §A4, §A5, §A8, §A9. No spec,
HANDOFF, PLAN, `app.rs`, `main.rs`, `world_view/`, `render/` or
`Cargo.toml` edit.

## State

**Implemented in both compositors, byte-identical on llvmpipe;
unverified against 1.14d** (no capture: `composition.md` test vector
`composition-0001` and `capture.md` are queued below). Every synthetic
test vector of `composition.md` is a unit test.

| Rule | Where | What |
|---|---|---|
| §5 pixel write, row = destination | `scene/item.rs` `BlendOp::IndexTable`, `compositor.wgsl`, `pack::emulate` | **behavior change**: `dest' = map[base + dest][src]` (was `map[base + src][dest]`); `MapTable::push_table` takes `table[dest][src]`, the PL2 layout, so a PL2 table is pushed unchanged. No `T`: `L[P[s]]` = the shade chain then `Opaque`. |
| §2 framebuffer, §3 frame cycle | `scene/frame.rs` `FrameCycle`, `FramePlan`, `UNCLEARED_ROWS` = 47 | persistent W × H index framebuffer; `plan(blank_screen)` = `{clear_rows: H − 47 or 0, clear_after: counter > 0}`; `compose` (CPU frame) / `commit` (a frame composed elsewhere, e.g. GPU readback) step the counter `[0x0070F2C0]`; `set_post_clear` (`0x0044E100` writes 1, OQ4) |
| §3 clears in the compositors | `scene::compose_frame`, `compose_binned_frame`; WGSL params `clear_rows`, `clear_after`, binding 9 `base`; `Packed::with_frame` | start value 0 in rows `< clear_rows`, else the base (previous frame); draws; `clear_after` → 0. `compose` / `compose_binned` / `pack` keep their signatures: base all 0, no clear (§6: a single-frame case starts from all 0) |
| §4 palette | `scene::present_palette(pl2)`, `PL2_PALETTE_BYTES` | index `i` = `pl2[4i..4i+3]` (R, G, B), 0 included; `to_rgba` = `(R, G, B, 255)`, one palette per frame |
| §6 answers | doc comments, TODOs removed | domain indexed (no `Rgb` op); clear rule; RGBA |
| §7 DirectDraw | not implemented | display type 3 only, outside the GDI reference (module doc of `scene/frame.rs`) |

Kept as a TODO: `composition.md` OQ2 (order of `L` and `T` when both are
present): the compositor applies the whole shade chain, then `T`
(`BlendOp` doc). Nothing builds such an item yet (`blend-modes.md`
unwritten).

GPU layout changes (`pack.rs` table): params 32 → 48 bytes (`clear_rows`,
`clear_after`, 3 pad words); new storage binding 9 `base` (one byte per
view pixel, four per word, little-endian, zero-padded to a word). The
in-app node (`encode_rgba`) and `verify::gpu` need no change: `pack`
gives a zero base.

Other edits: `verify/mod.rs` fills `[[table]]` rows as `table[dest][src]
= rule(src, dest)`, so case files keep their meaning (all current rules
are symmetric or unchanged by it); `verify/case.rs` and
`render-cases/synth-index-table.toml` comments.

## Checks (this branch)

- `cargo test -p d2-client`: lib 257 pass, 6 ignored (was 249); new
  `scene::tests::{frame_cycle_clears, framebuffer_persists_between_frames,
  frame_cycle_strict_inputs, pixel_write_vectors,
  present_palette_from_pl2, binned_frame_matches_reference}`,
  `gpu_compositor::tests::pack_frame_base_and_plan`; updated
  `index_table_every_src_and_dest`, `chain_applies_before_blend` (row =
  destination), layout / binding / little-endian tests.
- `gpu_compare` cases 12 → 18: `frame-blank-screen`, `frame-no-clear`,
  `frame-post-clear` (§3 vectors over an all-5 base, one item across row
  553), `pixel-write` (§5 vectors), `blend-ops` (every op over every
  (dest, src) pair, asymmetric table), `frame-stress` (random previous
  frame + BlankScreen clear + 300 items). `emulated_shader_matches_cpu_on_all_cases`
  runs all 18 in CI.
- llvmpipe (Mesa 25.2.8, LLVM 20.1.2, Vulkan; `apt-get install
  mesa-vulkan-drivers libvulkan1`): `gpu_compare` 18/18 0 differing bytes
  and pixels; `--perturb 7` 7 / 7 on every case; ignored
  `gpu_compositor::tests::gpu*` 2 pass, `verify::tests::gpu_half` pass;
  `d2-client verify` 10 synthetic PASS (map: no `D2_GAME_DIR`).
- M08 by hand: the old `[src][dest]` read in `BlendOp::apply` fails 5
  tests (`index_table_every_src_and_dest`, `chain_applies_before_blend`,
  `pixel_write_vectors`, `emulated_shader_matches_cpu_on_all_cases`,
  `perturbations_are_reported_exactly`); ignoring the base in `emulate`
  fails `emulated_shader_matches_cpu_on_all_cases`.
- Gates: `cargo fmt --check`, `cargo clippy --workspace --all-targets --
  -D warnings`, `cargo run -p depcheck`, `spec_index.py --check`,
  `methods.py check`, `coverage.py --check` (3,306 claims, 0 errors) and
  `--selftest`: all pass. `composition.md`: 9 of 16 units claimed (unit
  tier): §2, §3 text, §3 r2, r4, r5, §4, §5, §6, edge cases. Not claimed:
  §1 (reference choice), §3 r1 / r3 (`camera.md`, `draw-order.md`), §7.

## Local run queue (add to HANDOFF §5 C)

- **C-rc1 real GPU** (C15 style): `cargo run -p d2-client --example
  gpu_compare` → 18/18 `0 differing bytes`; `-- --perturb 7` → every case
  FAIL with exactly 7 / 7, exit 1; `cargo test -p d2-client --lib
  gpu_compositor::tests::gpu -- --ignored --nocapture --test-threads 1`
  → 2 pass. Record adapter name, backend, driver.
- **C-rc2 verify** (C16 style): `cargo run --release -p d2-client --
  verify` → map and 10 synthetic cases PASS (the map case has no blend
  table, so the orientation change cannot move it); `--perturb 5` → 5.
- **C-rc3 palette** (`composition.md` OQ1): for each act, the first 1,024
  bytes of `pal.pl2` through `scene::present_palette` vs the `.dat`
  palette, and entry 0 = (0, 0, 0). Decides whether the app may keep
  reading `.dat` (`map::cpu::to_rgba` paints 0 black).
- **Capture `composition-0001`** (`capture.md`, after `stability-0001`):
  a translucent sprite on a known floor; CPU reference with §5 equals the
  captured index frame. Until then every rule above is unverified.

## Seams for others

- `p6-integrate` (app / `world_view`): to run the frame cycle, keep a
  `FrameCycle`, compose with `plan = cycle.plan(level.blank_screen)` —
  CPU: `cycle.compose(...)`; GPU: `pack(...)?.with_frame(cycle.pixels(),
  plan)?`, read back the indices, `cycle.commit(plan, indices)` — and
  build the palette with `present_palette`. BlankScreen comes from
  `Levels.txt` of the player's level (all 137 live rows are 1).
- `render-capture` (verify scene cases): a case that records the previous
  frame passes it as the base; otherwise the base is all 0 (§6).
- Anyone pushing PL2 blend tables: push them unchanged (`[dest][src]`);
  `formats/palette.md`'s `Pl2::alpha_blend` doc still says
  `[level][source]` (spec says the reverse; d2-formats doc edit is a
  separate task).
