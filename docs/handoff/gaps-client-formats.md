# Gap tests: client, render and format specs

> To be folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` by a docs session; this file stays as the detailed record.

Branch `claude/gaps-client-formats`, based on `main` at `edad871`. Cloud
session (repo only, no game files). Task class: tests from specs, medium
effort (M14). The diff adds tests and this note. It changes no non-test
code, spec, `docs/HANDOFF.md` or `docs/PLAN.md`.

## 1. State

- 18 more units are covered: 17 by unit tests that run in CI, and 1 by a
  game test that has not been run yet (§5).
- No test showed code that deviates from its spec, so nothing was fixed.
- Coverage on this branch (`python3 tools/coverage.py --check`) has 3213
  claims and 0 errors.

| spec | units | any before | any after | game before | game after |
|---|---|---|---|---|---|
| client/assets.md | 12 | 7 | 8 | 1 | 1 |
| client/bridge.md | 39 | 22 | 33 | 0 | 0 |
| client/ui.md | 13 | 8 | 9 | 0 | 0 |
| formats/cof.md | 5 | 4 | 5 | 3 | 3 |
| formats/dcc.md | 14 | 13 | 14 | 12 | 12 |
| formats/ds1.md | 16 | 15 | 16 | 15 | 16 |
| render/map-preview.md | 14 | 11 | 13 | 0 | 0 |
| **repo total** | 2704 | 2401 (88.8%) | 2419 (89.5%) | 186 | 187 |

## 2. Code map (tests added)

| Test | File | Claims |
|---|---|---|
| `bridge_modules_except_mirror_have_no_bevy_type` | `crates/d2-client/src/bridge/tests.rs` | bridge §1 r3 (scans the code lines of every bridge module except `mirror` for `bevy`; also checks that the scan can fail on `mirror.rs`) |
| `model_changes_only_through_dispatched_messages` | same | bridge §1 r1 |
| `bytes_cross_the_boundary_unchanged` | same | bridge §1 r4 |
| `buffer_and_node_chunks_split_alike` | same | bridge §2 r1 |
| `chunks_are_handled_in_delivery_order` | same | bridge §2 r5 |
| `units_iterate_in_type_then_guid_order` | same | bridge §5 r2 |
| `every_row_is_tbd_and_no_handler_is_registered` | same | bridge §6 r2 (must change when the first owner spec takes an id) |
| `views_are_overwritten_from_the_model` | same | bridge §7 r2 |
| `one_pump_per_frame_whether_or_not_it_ticked` | same | bridge §8 r2 |
| `errors_stop_the_frame_records_do_not` | same | bridge §8 r4 |
| `size_zero_ids_end_the_split_and_one_byte_unit_ids_address_nothing` | same | bridge §edge-cases-original-bugs |
| `one_logical_frame_of_800_by_600` | `crates/d2-client/src/ui/tests.rs` | ui §a5-logical-resolution |
| `resolved_frame_has_every_listed_key_even_over_budget` | `crates/d2-client/src/assets/cache.rs` (tests) | assets §a4-residency text |
| `floors_layer_by_layer_row_major` | `crates/d2-client/src/map/layout.rs` (tests) | map-preview §draw-order r1 |
| `walls_sorted_by_depth_x_layer_piece` | same | map-preview §draw-order r3 |
| `integers_are_little_endian` | `crates/d2-formats/src/cof.rs` (tests) | cof §rules text |
| `equal_cell_copies_from_last_draw_of_its_cell` | `crates/d2-formats/src/dcc.rs` (tests) | dcc §edge-cases-original-bugs |
| `four_walls_two_floors_one_shadow` | `crates/d2-formats/src/ds1.rs` (tests) | none: it covers half of the section; the game test below claims all of it |
| `ds1_layer_limits_and_truncated_trees_groups` (`#[ignore]`) | `crates/d2-formats/tests/formats_game.rs` | ds1 §edge-cases-original-bugs (game tier) |

The DCC test builds three 1×1 frames. Frame 1 sits in a different
direction cell, so it skips cell 0. Frame 2 is an equal cell, and it copies
frame 0's pixel from the persistent buffer. Riiablo's newer decoder would
copy frame 1's output instead and give 0.

## 3. Signature changes

None.

## 4. Seams reached (no spec or no code, so no test)

- `client/ui.md` §a3-text: `layout_text` does not exist yet. It waits on
  `TODO(spec: ui/text.md §B3)` in `ui/draw.rs`.
- `client/audio.md` §a1-decode-path text, r1, r3: there is no
  `d2-formats::wav` and no sound pool. The owner spec `formats/wav.md` is
  not written yet.
- `client/render-pipeline.md` §a7 r1: the COF path rule is §B4, which this
  spec does not specify yet.
- `client/assets.md` §a4 r1 and r3:
  - r1: the stage-1 key list belongs to the render stage. There is no
    function to test against this rule alone.
  - r3: prefetch is not implemented.

## 5. Local checks to queue (for `docs/HANDOFF.md` §5)

```
D2_GAME_DIR=<install> cargo test -p d2-formats --test formats_game -- --ignored ds1_layer_limits_and_truncated_trees_groups
```

Expected: the test passes.
- The synthetic half (4 walls, 2 floors, one shadow layer) already passes
  in the cloud: run without game files, the test only panics later, at
  `D2_GAME_DIR`.
- The game half checks `data\global\tiles\ACT1\OUTDOORS\trees.ds1`, read
  from `d2exp.mpq` or else `d2data.mpq`. Expected values: version 12,
  tag_type 1, 14 groups, `groups_truncated`, and the 14th group's y, width
  and height read as 0.
- The test also checks that `files` contains the embedded developer path
  `C:\D2\DATA\GLOBAL\TILES\ACT1\TOWN\trees.tg1`. The comparison ignores
  ASCII case, and the path is taken from the spec's text.

If it fails, fix the expected value from what the file shows, or remove
the claim in the same session (COVERAGE.md §3).

## 6. Left uncovered (not testable here, or nothing to test)

- **bridge**
  - §1 r2: needs a scan of `app.rs`, which this session does not own.
  - §3 r4: online play (Phase 7).
  - §5 r1: a statement about meaning.
  - §7 r4: interpolation is owned by the render specs.
  - §9 r2, r3: nothing is persisted.
- **assets** §a6-writes: a policy about files on disk. No writer exists
  to test.
- **ui**
  - §a1-why-not-bevy-ui: rationale only.
  - §edge-cases: empty, "to be listed by §B owners".
- **map-preview** §edge-cases: a list of simplifications deferred to
  Phase 6. "First match" could be tested, but the section as a whole
  cannot be claimed by one test until translucent-tile handling is
  specified.
- **animdata** §1, §5, §7, §expfield-d2: unchanged; see
  `gaps-data-formats.md` §4.
- **§b-original-behavior-to-reproduce-not-specified-here** (assets,
  audio, render-pipeline, ui): not specified yet.

## 7. Questions

1. `bridge.md` §6 r2 says "Today every row is `TBD`". The test pins this
   statement, so the session that takes the first id must update the
   spec and the test together. Should this sentence move to Status
   instead of staying a rule?

## 8. Gate

All pass on this branch:
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test -p d2-formats`
- `cargo test -p d2-client`
- `cargo run -p depcheck`
- `tools/spec_index.py --check`
- `tools/methods.py check`
- `tools/coverage.py --check`
- `tools/coverage.py --selftest`

Non-source files in the diff: this note only.
