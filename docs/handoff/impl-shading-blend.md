# Handoff: shading and blend modes in code — `claude/impl-shading-blend`

Cloud implementation session, 2026-10-06, medium. Repo only, synthetic
fixtures, no game files. Implements `specs/render/shading.md` and
`specs/render/blend-modes.md` (both draft, from
`claude/spec-shading-blend`, merged on `main` at d09ab24) and the
`render-pipeline.md` §A5 correction (`IndexTable` row = destination, new
`IndexTableSrcRow`).

Base: this branch merges `origin/claude/render-followups-impl` (not yet on
`main`), which holds `scene::PixelTables`; the only conflict was
`specs/render/sprite-placement.md` Edge cases, kept as on `main`
(orientation bit is r5), and the two `frames/tests.rs` claims of that
branch were moved from r2 to r5 to match. Merge this branch after (or
instead of) `render-followups-impl`.

## What changed

| Spec rule | Code | Tests (`// Covers:`) |
|---|---|---|
| render-pipeline §A5, blend-modes §2: `IndexTable` = `map[base + d][s]` (row = destination, already so since `render-composition`); new `IndexTableSrcRow` = `map[base + s][d]` | `scene/item.rs` `BlendOp::IndexTableSrcRow`, `BlendOp::table()`; `gpu_compositor/compositor.wgsl` op 2; `pack.rs` `BLEND_INDEX_TABLE_SRC_ROW`, `pack_item`, `emulate` | `rules/shading_blend_tests.rs` `translucent_walls_read_the_transpose`; harness cases `shading-blocks`, `shading-stress` (CPU = emulate = GPU); `tests/prop_scene.rs` model and strategy |
| shading §4 (wall r4 / RLE floor l2 r3 gradient: per-pixel light map `G[a_r][b_r][x]`) | `scene/item.rs` `LightGradient { kind, x, y, corners, light0 }`, `GradientKind::{Wall, RleFloor}`, `scene::gradient` (`G`); `ShadeChain::with_gradient` (applied after the chain's maps, per pixel); `resolve` refuses a drawn area outside the block (`SceneError::GradientArea`) and light maps past the table (`SceneError::LightMaps`). GPU: item grew to 5 × `vec4<u32>` (`ITEM_SIZE` 80), `light` = kind, light map 0 row, corners, block offset of the area; WGSL `gradient_row`, `pack::gradient_row` | `gradient_light_per_pixel_in_the_compositor`, `a_gradient_item_must_stay_inside_its_block`, `wall_gradient_rows_and_columns`, `gradient_table_vectors`; harness cases |
| shading §1, §2, §5, §8 (block as map-table rows, `H`, `R`, nearest) | `rules/shading.rs` `ShadeTables::push` (light maps, `H`, `R`, `Z`, 128 remaps, `A0`–`A2`, `ADD`, `MUL`, `MAX`, all unchanged / `[dest][src]`), `highlight_map`, `red_map`, `nearest` | `the_block_is_pushed_unchanged`, `highlight_is_nearest_of_170_percent`, `red_map_is_nearest_red` |
| shading §3 (cel `L = map v >> 3`, 0xFF none, mode 7 → `H`), §5 hover light byte | `ShadeTables::cel_light`, `cel_light_level`, `hover_light`; `rules/blend.rs` `cel_tables` | `cel_light_is_v_shifted_by_3`, `hover_light_doubles_and_clamps` |
| shading §4 walls r2–r4, floors l2 r1–r3 | `wall_block_light`, `floor_block_light` (c2 bug reproduced), `ShadeTables::block_chain`, `floor_block_chain` | `wall_block_light_vectors`, `floor_block_light_vectors` |
| shading §6 r1, r4 (unit palette index `p − 1`, item color `(t, c)`) | `ShadeTables::unit_remap`, `item_color`, `ITEM_PALETTE_FILES` | `unit_palette_index_selects_map_p_minus_1`, `item_color_selection` |
| shading §7 (mapped 0 drawn as index 0) | `ShadeChain::apply` (TODO removed) | `a_mapped_zero_is_drawn_as_index_0` |
| blend-modes §1–§2 (mode → table, cel ops) | `rules/blend.rs` `mode_table`, `BlendTable::base`, `cel_ops` → **`PixelTables::ops`** (its first caller) | `draw_mode_tables`, `cel_modes_write_row_destination` (every mode, `P`, `L`, `T`, mode 7, mode 8) |
| blend-modes §3, §4 | `unit_override`, `component_mode`, `hover_highlighted`, `missile_mode`, `item_mode`, `overlay_mode` | `component_mode_decision`, `single_cel_modes` |
| blend-modes §5 | `unit_shadow_ops` (`[Z]` + `A0` / opaque), `shadow_image` (sheared shape as a `FrameImage` + top-left), `shadow_tile_ops` | `unit_shadow_pixels`, `unit_shadow_shape`, `shadow_tiles` |
| blend-modes §6 | `wall_draw`, `wall_block_ops` (translucent: gradient always + `IndexTableSrcRow`) | `translucent_walls_read_the_transpose` |

Also, outside the two specs:

- `scene/gaps_numbered_tests.rs` `world_is_skipped_in_open_mode_3_and_the_ui_still_drawn`
  failed on this branch's base (7f684ad, which already holds
  `impl-draw-order`): `rules::OriginalView` now answers `ui_pass`, so the
  UI request reaches `ui_image` and fails there (`ViewError::Ui { 0,
  "UI image" }`) instead of at the pass. The expectation was updated to
  that error; the test's point (the UI is still built in open mode 3, no
  tile error) is unchanged.
- `rules/gaps_numbered_tests.rs` used `Shake::amplitude(..).unwrap()`
  (main's `Result`); `render-followups-impl` made it `Option<u32>`, so the
  four asserts drop the `unwrap()` (merge fix, same values).
- `tests/mutants_client/assets_size.rs` built a `Pl2` without the
  `base_palette` field `render-followups-impl` added (merge fix; the field
  is inline, so the expected size is unchanged).
- `tests/game_assets.rs` (ignored) `evaluate` still read `IndexTable`
as row = source (left over from before `render-composition`); fixed to row
= destination, plus the new op. Its local run would have failed.

## Not wired (open)

1. **`ViewRules::shade` / `blend` and `ComponentResolver::shade` /
   `blend` still return the wrapped rules' answer** (`rules/view.rs`
   forwards them). The rules functions above need per-unit inputs the
   client model does not carry yet (light byte `v` — `render/lighting.md`
   unwritten; fade stat, ghostly flag, hover target, item `transparent` /
   ethereal; the COF layer override bytes) and the act's `ShadeTables` in
   the world view's map table. The bridge / client-model session owns
   those inputs; wiring is a follow-up once they exist. Tile draws
   (`TileDraw`) likewise need the block corners from `lighting.md`.
2. Isometric floor blocks with a gradient: `ShadingError::IsometricFloorGradient`
   (shading OQ1).
3. Unit palette index `p` > 128: `ShadingError::RemapIndex` (spec silent;
   only 128 maps exist).
4. `shadow_image` gives the shape only; the shadow position
   (`blend-modes.md` OQ1) is the caller's.
5. Item color `c` beyond the state rule (shading OQ4), monster palette
   shift (OQ2), green blood option (OQ3): not implemented, spec open.
6. `verify` case files (`render-cases/*.toml`) have no key for
   `IndexTableSrcRow` or gradients; the CPU = GPU proof for them is the
   harness (`gpu_compositor::harness::cases`). Add a case key if a local
   case needs one.

## Gate and GPU

- `sh tools/gate.sh`: **FAIL, not from this branch.** Passing: spec
  index, methods, coverage, trace checkers, hook selftest, fmt, depcheck,
  clippy (workspace, `-D warnings`), tests of the other crates, doc-tests.
  Failing on the base 7f684ad already (this branch changes no file they
  read):
  - `test d2-client`: 46 tests, all from the bridge dispatch table
    (`bridge::tests::dispatch_table_matches_spec` and every test that
    builds a `LocalBridge`, e2e / app frame loop / ui intents / prop
    bridge): `Mismatch([NoHandler { id: 0 }, … id: 172])` — the
    `client/msg-*.md` specs merged on the base give owners to S→C ids
    that have no handler yet. `impl-client-model`'s work (bridge).
  - `test d2-sim + conformance`: `missiles::tests_bodies::bodies_match_catalogue_status`
    (`spec'd-here [1, 2, 3, 5, 7, 8, 10, 25], implemented [1]`),
    `bodies_check_catches_perturbations`, `monsters::ai::tests::implemented_matches_catalogue`:
    the skill/AI/missile body specs merged on the base mark more bodies
    specified than implemented.
  - The d2-client lib tests of `scene`, `rules`, `gpu_compositor` (143)
    pass; `rules::gaps_numbered_tests` and `tests/mutants_client` needed
    the merge fixes above (`Shake::amplitude`, `Pl2::base_palette`).
- `cargo test -p d2-client --lib gpu_compositor -- --ignored` on llvmpipe
  (LLVM 20.1.2, Vulkan; `apt-get install mesa-vulkan-drivers`): every
  harness case, `shading-blocks` (21 items) and `shading-stress` (250
  items) included, 0 differing bytes CPU vs GPU; `--perturb 7` reports
  exactly 7 on each case. Real GPU: local check below.
- `py tools/coverage.py --check`: 0 errors; `blend-modes.md` 15 / 15
  rule units claimed, `shading.md` 24 / 29 (unclaimed: §6 text, §6 r2,
  r3 — open questions 2–3, §6 r5 — `ui/text.md`'s, §9 — one palette,
  nothing to implement here).

## Local checks (game files / captures)

- `D2_GAME_DIR=<install> cargo test -p d2-client --test game_shading -- --ignored --nocapture`:
  both tests pass (`act1_shading_vectors`, `act1_blend_vectors`: every act 1
  value of the two specs' test vectors through `ShadeTables`, `cel_ops`
  and the CPU compositor). Then add `// Covers:` claims to them
  (shading §3, §5, §6 r1, §8; blend-modes §1, §2, §5, §6).
- `D2_GAME_DIR=<install> cargo test --release -p d2-client --test game_assets -- --ignored --nocapture --test-threads 1`:
  `cpu_compositor_on_real_frames` and `gpu_compositor_on_real_frames`
  pass with the fixed `evaluate` (row = destination).
- `cargo run -p d2-client --example gpu_compare` (or the ignored GPU
  harness test) on the real GPU: the new cases `shading-blocks` and
  `shading-stress` report 0 differing bytes (`--perturb N` → exactly N).
- Captures (blend-modes OQ2): a blended unit shadow or a ghostly / Fade /
  ethereal unit over a known background (static camera) to decide the
  orientation on an asymmetric table; a wall fading as the player walks
  behind it (§6, the non-monotonic 100 % → 25 % → 50 % fade); a lit floor
  at night (shading OQ1). Each compared pixel for pixel against the CPU
  reference once the world view is wired (Not wired 1).
