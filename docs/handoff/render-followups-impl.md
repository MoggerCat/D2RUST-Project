# Handoff: render spec follow-ups in code — `claude/render-followups-impl`

Cloud implementation session, 2026-10-06, medium. Repo only, synthetic
fixtures, no game files. Brings the code in line with the render spec
changes merged from `claude/spec-render-followups` (diff of `f3d1ff8^1`
→ `f3d1ff8`: `render/camera.md`, `capture.md`, `composition.md`,
`sprite-placement.md`, `formats/palette.md`, `formats/dc6.md`).

## What changed, per spec change

| Spec change | Code | Tests (`// Covers:`) |
|---|---|---|
| camera §8: shake arithmetic is unsigned 32-bit (product keeps its low 32 bits, `t3 = 0` gives `a = 0` at `t = t1 + t2`, no zero divisor) | `rules/camera.rs` `Shake::amplitude` now returns `Option<u32>` with `wrapping_*` u32 math; `ShakeError` removed; `world_view/feed.rs` `frame_shake` no longer maps an error (one-line change, not the frame-cycle code) | `rules/tests.rs` `shake_envelope_is_unsigned_32_bit` (the three new spec vectors); `feed/tests.rs` `shake_runs_on_the_tick_time_base` (the old "divides by zero" error case now asserts a = 0, no draw) |
| camera §7: wall-block culling for lit **and** translucent wall drawers, kept block drawn whole; units have no view test; roofs use the floor drawer's culling | already so in `rules/view.rs`; no code change | `wall_blocks_culled_in_mode_1_lit_and_translucent` (spec vector 399/400), `roofs_and_floors_are_not_culled_per_block`, `units_are_not_culled_by_the_view` |
| sprite-placement §7 (floor drawer culls nothing per block), edge case 1 (rows past the surface are clipped), edge case 5 (refuse DC6 `flip ∉ {0,1}`, DCC odd `variable0`) | refusals already in `frames/mod.rs`; no code change | claims `§edge-cases-original-bugs r5` added to the two `frames/tests.rs` refusal tests; `top_down_cels_clip_as_bottom_up` gains the "first row at H − 2" case |
| composition §5: with `L` and `T`, `d' = T[256 × d + L[s]]`, `P` dropped | `scene/item.rs` new `PixelTables { remap, light, blend }::ops() -> (ShadeChain, BlendOp)` (exported from `scene`); `BlendOp` doc no longer carries the OQ2 TODO. The compositors (CPU and GPU) are generic chain-then-table and need no change | `scene/tests.rs` `lit_blend_drops_the_remap` (all §5 rows through `compose_frame`) |
| palette.md: base palette is R, G, B, x and presented; alpha tables `[level][destination][source]` | `d2-formats` `Pl2::base_palette` (new field, parsed R, G, B, x); `Pl2::alpha_blend` doc fixed | `palette.rs` `pl2_base_palette_is_rgbx_and_blend_rows_are_destinations` |
| dc6.md: note only (drawer tests bit 0) | none | existing |
| capture §4–§7: `frames-raw-2`, frames numbered by `seq`, state key with `cursor_key`, `light_key`, level; §6 initial framebuffer = frame `seq − 1` of an `--every 1` recording; a frame without a draw log cannot be composed | `verify/capture.rs` reads both `frames-raw-1` and `frames-raw-2` (`RawFormat`; each refuses the other's records/fields; raw-2: `capture` record second, `celfile` records, `seq` 1, 2, … in order, cursor / level / seeds / light / weather / draw log typed, `cursor_key` and `light_key` checked against the state they come from). `Frame::seq` (raw-1: position). `StateKey` + cursor/light/level; groups and repeated ticks report `seq`. `capture_case.rs`: selection, previous frame and report lines by `seq`; previous only when `capture.every == 1`. `scene_source.rs`: `NotRecorded` names the first missing input (`NO_DRAW_LOG`, `NO_INITIAL_FRAME`, then `RECORDER_GAP`, rewritten for raw-2). Case files: key `draws` (draw counters) replaced by `seqs` (no committed case used it) | `capture_tests.rs` fixtures now write raw-2 (raw-1 on request): `raw_reader_reads_every_field` (both formats), `raw_reader_is_strict` (+7 raw-2 cases), `the_source_gets_frame_seq_minus_1_of_an_every_draw_recording`, `the_state_key_holds_cursor_light_and_level`; `scene_tests.rs` `a_frame_needs_its_draw_log_and_initial_framebuffer`; existing tests updated to `seq` |

Not touched (other sessions): the world view's frame cycle
(`scene/frame.rs` `FrameCycle`), `ui/` text layout, draw order, unit
composite.

## Open items / questions

1. **composition §3 step 4** (the post-draw clear counter is set to 1 by
   the act load, S→C 0x03): belongs in the frame-cycle code
   (`FrameCycle` / world view), which this session had to avoid. The
   frame-cycle session or a follow-up should set the clear on the act-load
   message.
2. **`PixelTables` has no caller yet**: which draws pass `P`, `L`, `T` is
   `blend-modes.md` (TODO(spec)). Item builders should use `ops()` once
   that spec exists, so a lit translucent remapped draw drops `P`.
3. `Pl2::base_palette` duplicates `scene::present_palette` (raw bytes);
   the frame-cycle owner may switch to the field.
4. Scene case files: `draws = [...]` is now an unknown-key error; use
   `seqs = [...]` (recorder `seq`). Coordinator: mention in HANDOFF if any
   local case file used `draws`.

## Local checks (game files / captures)

- After the first `frames-raw-2` recording (capture.md §8
  `stability-0001`): `cargo run -p d2-client -- verify --cases
  crates/d2-client/capture-cases` — the reader must accept the file
  (`raw … (frames-raw-2)` in the report); `stability-0001` reports
  PASS with ≥ 2 repeated keys; `--perturb 1` gives exactly
  `re-hash: 1 of N frames differ`. Compare cases must report SCENE NOT
  WIRED with `NO_INITIAL_FRAME` / `RECORDER_GAP` seams (never PASS), and
  the camera note on every frame.
- The two `frames-raw-1` runs still read: the same command pointed at
  `traces/raw/20261006-140102-frames-run1b.jsonl` (case `raw = '<path>'`)
  must report `(frames-raw-1)`, the camera check equal on every frame,
  and `initial framebuffer: none`.
