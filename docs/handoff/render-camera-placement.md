# Handoff: camera and sprite placement rules — `claude/render-camera-placement`

> Waiting to be folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md`; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: implementation
from a clear spec (medium). Branch `claude/render-camera-placement`, from
`claude/specs-staging` at `c6e40f9`. Specs: `specs/render/camera.md`
(§1–§10) and `specs/render/sprite-placement.md` (§1–§8), both drafts
(RE on 1.14d, no capture yet). Repo only: no game files, no GPU.

## State

**Implemented, unverified** (METHODS M02). Every synthetic test vector of
both specs runs as a unit test (pixel positions); a CPU-compositor golden
scene places tiles and units through the new rules and compares the whole
800 × 600 index framebuffer against hand-painted spec coordinates. No
pixel of the original has been compared: the capture cases `camera-0001`
and `placement-0001` and three game-file counts are queued below.

Nothing is wired into the app yet (`p6-integrate` owns `world_view/` and
`app*`); see "Wiring" for the one change.

## Code map

| File | What |
|---|---|
| `crates/d2-client/src/rules/mod.rs` | module root, re-exports |
| `rules/camera.rs` | camera.md: `FrameSize` (W × H, play height `H − 40`, frame rect), `OpenMode` 0–3, `ViewRect` (§1 table, `shift_x`), `moving_to_client` / `static_to_client` / `cell_origin` / `tile_entry` (§2), `Camera::new` (tile and unit origins once per frame, shake added, §3), `unit_draw` (§4), `tile_handed` / `handed_at` / `block_origin` (§5, §6), `floor_roof_visible` / `wall_block_visible` (§7), `Shake` (`start`, `amplitude`, `time_of` = 40 × ticks) and `shake_offsets` (two `roll_range(−a, 2a)` draws, §8, §9) |
| `rules/placement.rs` | sprite-placement.md: `Cel` (`columns`, `rows`, `row_plan`; `Cel::dc6`, `Cel::dcc` from header fields, §1–§4), `RowPlan` (the rasterizer's skip / start / count, §5), `draw_position` (§8 table), `place` / `place_cel` → `Placed { x, y, clip }` (top-down cels get the clip that reproduces the rasterizer's rows), `block_pixel` (§7) |
| `rules/view.rs` | `OriginalView<'a, R, S>`: implements `ViewRules` (and `UiRules` by delegation) over wrapped rules `R` and a `ViewSource` `S`; `MapTile`, `BlockRect` (`of_tile(&Dt1Tile)`), `ViewSource` |
| `rules/tests.rs` | 16 tests (below) |
| `frames/mod.rs` | **minimal data change**: `IndexFrame.anchor: FrameAnchor` (`Top` default, `Bottom` = DC6 `flip = 0`, `TopDown` = DC6 `flip = 1`), `IndexFrame::with_anchor`; `from_dc6` sets the anchor and refuses `flip ∉ {0, 1}` (`FrameError::Dc6Flip`); `from_dcc` refuses an odd `variable0` (`FrameError::DccVariable0`) |
| `frames/tests.rs` | struct literal gets `anchor`; 2 new tests |
| `lib.rs` | `pub mod rules;` |

`IndexFrame::new` signature is unchanged (anchor `Top`), so no other
caller changed. A struct literal of `IndexFrame` elsewhere (other
branches) needs `anchor: FrameAnchor::Top`.

## Hook implementations

| Hook | Answer |
|---|---|
| `ViewRules::tiles` | `ViewSource::map_tiles` lists `MapTile { cell, list: Floor / Wall / Roof { roof_height }, frame, blocks, shade, blend, key }` in draw order; each is placed by `OriginalView::tile`: handed (X, Y) per camera §6, floors/roofs culled on the handed point (§7), walls culled per block (§7) and expressed as one clip (the frame when all blocks are kept; the kept blocks' bounding box when some are culled; an error when a culled block overlaps that box); then `placement::place` on the DT1 image (`(X + x0, Y + y0)`, frame clip) |
| `ViewRules::unit_params` | the wrapped rules' draw key and tag; `clip` = the frame (camera §10) |
| `ViewRules::place` | `ViewSource::unit_position` → client pixels (§2), `ViewSource::unit_offset` (`unit-composite.md` `(ox, oy)`), `Camera::unit_draw` (§4), `placement::place` (§8). A top-down DC6 cel whose rasterizer rows differ from the frame clip is an error (a unit's components share one clip) |
| `unit_pose`, `component_frame`, `shade`, `blend`, UI hooks | delegated to `R` unchanged |
| `ComponentResolver::frame_id` | not touched: `world_view::UnitResolver` answers it from the `FrameTable` (owned by `p6-integrate`) |

`ViewSource` is new: the client world model has no unit position, map or
per-unit offset yet. Its methods are `TODO(spec: …)` hooks for the owners
(S→C position messages / units path, `unit-composite.md`, `draw-order.md`
and DRLG); until then an implementation returns an error.

## Wiring (the app's one-line change)

In `crates/d2-client/src/world_view/present.rs` line 253, build through
the original's rules, with the frame's camera computed once:

```rust
let view = rules::OriginalView::new(rules::Camera::new(rules::FrameSize::D2RS, open_mode, player, shake), state.rules.as_ref(), &source);
let frame = build(bridge.0.world(), draws, &view, &state.assets)?;
```

`open_mode` (UI state, `ui/panels.md`), `player` (the local player's
`UnitPosition::client()`), `shake` (`(0, 0)` until an effect starts one;
`Shake::amplitude(Shake::time_of(ticks))` then `shake_offsets`) and
`source: impl ViewSource` do not exist in the app yet; they come with the
owners named above. `state.rules` (`dyn WorldRules`) keeps answering the
delegated hooks.

## Tests (`cargo test -p d2-client --lib rules::`, 16 pass)

Camera: `moving_static_and_tile_coordinates` (§2: (0, 160), −1 floor vs
D2MOO 0, (80, 152), cell (3, 1) / entry (80, 240)),
`origins_and_player_position_at_800x600` (§1, §3, §4: origins 600 / 1720
/ 600 / 1716, player (400, 292), mode 1 (200, 292), modes 2 / 3, 640 × 480
(320, 232)), `shake_offsets_move_both_origins`,
`floor_wall_and_roof_positions` (§5, §6: handed (40, 0), block (−40, 0),
wall block (−40, 16), roof, panel shift), `units_sit_12_rows_below_the_floor_vertex`,
`view_culling` (§7: −81 / −80 / 553, wall block ranges per mode),
`shake_envelope_and_offsets` (§8: t = 50 → 5, every boundary, `t2 = 0`
ignored, offsets equal two `roll_range(−5, 10)` draws, range [−5, 4]; §9).
Placement: `dc6_cels_both_orientations` (§2, §4, §8 vectors),
`dcc_frame_box` (396 / 272), `row_clipping` (§5: Y = 2, Y = H + 1),
`top_down_cels_clip_as_bottom_up` (edge case), `dt1_block_position`
((32, 236)). View: `cpu_golden_scene_through_original_view` (full index
framebuffer of a floor, a wall, a moving player and a static object; one
culled floor), `golden_scene_catches_a_one_pixel_camera_move` (M08:
exactly the 12 expected pixels change, the player's none),
`wall_blocks_culled_in_mode_2`, `a_cut_top_down_unit_cel_is_an_error`.
Frames: `dc6_frames_carry_the_orientation_bit_and_refuse_other_flips`,
`dcc_frames_are_top_anchored_and_refuse_an_odd_variable0`.

M08 by hand: unit `+ 8` → `+ 9` failed 3 tests; `>> 1` → `/ 2` failed
the §2 test; view clip `H − 47` → `H − 46` failed `view_culling`;
dropping the `Y + yoff + 1` row limit failed 3 tests.

`Covers:` claims (docs/COVERAGE.md): camera.md §1–§10 and the edge-cases
section; sprite-placement.md §2–§5, §7, §8 and the edge-cases section. Not claimed: placement §1 (driver path, nothing to
implement), §6 (transparency is the decoders' index 0; OQ1).

## Findings for the spec owners (not fixed here)

1. **Roof drawer.** camera.md §6 hands roofs to `DrawGroundTile` (floor
   drawer, −80 and panel shift); sprite-placement.md §7 lists the roof
   drawer with the wall drawer `0x005131B0`. Implemented per camera.md
   (owner of tile positions). Spec session: reconcile.
2. **DC6 `flip` other than 0/1.** The original tests bit 0 only
   (placement §1); `d2-formats` `dc6.rs` decodes any non-zero `flip`
   top-down. `from_dc6` refuses such frames rather than draw rows the
   original would not. The game-file check below shows whether any exist.
3. **Top-down rows below the frame.** For a top-down cel starting at
   `H − 1` the original writes its following rows past the surface; d2rs
   clips them. Unreproducible by definition; noted under placement Edge
   cases.
4. **Shake arithmetic.** `t3 = 0` at `t = t1 + t2` divides by zero in the
   original (§8 table); a product `A × t` over 32 bits is not specified.
   Both are errors (`ShakeError`), never a guessed value.
5. **Wall culling as one clip.** Per-block culling becomes a clip of the
   assembled DT1 image; a culled block overlapping kept ones is an error.
   With 32-pixel blocks on a 32 grid it does not happen; if live tiles
   trigger it, the tile image needs per-block draw items.
6. **Unit culling** (camera OQ2) is not implemented: units are not culled
   beyond the frame clip, which changes no pixel unless the original
   skips visible units.

## Local checks (add to `docs/HANDOFF.md` §5)

- **C (game files).** Extend `mpq-tool formats` with three counts and run
  it: (a) DC6 frames with `flip ∉ {0, 1}` (expect 0; else finding 2
  needs a decoder fix), (b) DCC frames with odd `variable0` (expect 0,
  placement OQ2), (c) zero bytes inside DC6 copy runs, DT1 RLE runs and DT1
  iso diamonds (placement OQ1; any non-zero count means `IndexFrame`
  needs an opaque zero). Then `D2_GAME_DIR=… cargo test -p d2-client --
  --ignored` to confirm `from_dc6` / `from_dcc` still accept every live
  file used by the ignored tests.
- **A (capture).** `capture.md` cases `camera-0001` (walk 5 s, every
  frame with state) and `placement-0001` (standing still): CPU reference
  from the recorded positions through `rules::Camera` and
  `rules::placement` equals the captured index frame; the recorded origins
  equal `Camera::new`. Needs the `ViewSource` and scene cases of the
  `render-capture` session.

## Gate (this branch)

`cargo fmt --check` ok; `cargo clippy --workspace --all-targets -- -D
warnings` ok; `cargo test -p d2-client`: lib 268 pass, 6 ignored, all
integration tests pass; `cargo run -p depcheck` OK; `python3
tools/spec_index.py --check` ok; `python3 tools/methods.py check` 21 OK;
`python3 tools/coverage.py --check` 3,319 claims, 0 errors; `--selftest`
ok.
