# impl-draw-order — `render/draw-order.md` in d2-client

Scope: branch `claude/impl-draw-order`, from `claude/tender-meitner-mphas3`
@ 62fcef6 (main + the draw-order / unit-composite specs). Cloud, repo only.

## What changed

- New `crates/d2-client/src/rules/draw_order/` (`// Spec:
  specs/render/draw-order.md`):
  - `mod.rs`: the draw-cell grid (§2: `q`, `T`, `DrawGrid` from the
    camera's view rectangle and tile origin), the §9 inputs (`NearRooms`,
    `Room`, `TileRecord`, `Dt1Facts`, `Fade`, `RoomUnit`, `UnitFacts`,
    `LevelFacts`, `FadeClock`), filling (§3: room test, wall / shadow
    arrays, units, flat units, 3,000-entry pool, cell flag word, grid
    flags), layer-sorted insertion (§4, last element never compared),
    the unit entry (§5 r1, r3, r4 with flag writes), the fade targets
    and ramp (§8), the passes (§1, §6 r1–r5, open mode 3 draws no
    world) and the §10 keys (`OrderKey`, `Ordered`, `UnitSlot`).
  - `source.rs`: wiring. `ordered_source` reads the feed's near rooms,
    asks each room unit's position from `ViewSource::unit_position`,
    orders the frame, maps every tile item to a `MapTile` (art from
    `ViewFeed::tile_art`, `DrawKey` from §10, lower walls and shadow
    tiles placed as `TileList::Wall`) and wraps the feed in
    `OrderedSource` (a `ViewSource`: `map_tiles` = ordered tiles,
    `unit_slot` = drawn key or not drawn).
  - `tests.rs`: 27 tests from the spec's vectors and rules, each with a
    `// Covers:` line (§1, §2, §3 r1–r4, §4, §5 r1, r3, §6 r1–r5, §8, §9,
    §10).
- `scene/order.rs`: `pass` constants 1–11 (§10); the `DrawKey` TODO now
  points at them.
- `rules/view.rs`: `ViewSource::unit_slot` (default `Unordered`);
  `OriginalView::unit_pose` returns `None` for a `NotDrawn` unit,
  `unit_params` takes pass / major / minor from the order; `ui_pass`
  returns pass 11 (§10). `MapTile.key` / `map_tiles` docs updated.
- `world_view/feed.rs`: `ViewFeed` hooks with defaults — `near_rooms`
  (`None` = no map, the old behavior), `fade_clock` and `tile_art`
  (errors until their owners exist); `build_frame` orders the frame
  when the feed states near rooms. Existing feeds are unchanged.
- No edits to `camera.rs`, capture, composition or sprite-placement code.

Tests: d2-client 390 run, 390 passed (16 skipped = ignored game-file
tests). Gate: see the session summary.

## Open questions (for the spec owner)

- DO1 **§Test vectors `T(−160, 0)`**: the table says (−2, −1); the §2 rule
  gives `ty = q(2·0 − (−160)) = q(160) = 1`. The test asserts only the x
  half (`tile_of(-160, 0).0 == -2`) with a TODO; fix the vector or the
  rule.
- DO2 `TileList` (camera.rs, render-followups' file) should gain
  `LowerWall` and `ShadowTile` kinds placed as walls (§10). Until then
  `draw_order::source::placement_list` maps both to `TileList::Wall`; the
  kind travels in `OrderedTile.kind` to the `tile_art` hook (blend:
  shadow tiles draw mode 4).
- DO3 Grid flags (view +0x38) are set when an entry of that kind is
  filed, also when the pool drops it (§1 says "when it files"; silent on
  pool overflow).
- DO4 Record flag 0x20000 is set for walls (pass 6) and roofs at the
  order decision; the spec's Outputs say "when a wall or roof draws":
  whether lower walls set it and whether the drawer's §7 culling comes
  first is not stated.
- DO5 Fade end time compared unsigned (`now >= end`); `GetTickCount` wrap
  and the d2rs time base belong to `render/blend-modes.md` (`FadeClock`
  hook). `instant` = `0x00477730 ≤ 3` is a lighting hook.
- DO6 Items the order emits but no spec draws yet fail the frame
  (strict, METHODS M07): unit shadows (OQ3), drawn water floors (OQ11;
  checked with camera §7 before the error), levels 74 / 120 (OQ1),
  `DrawEdges` at open mode 0 (OQ10), fade group mode (OQ6), a sight-tested
  unit without an answer (OQ9). Passes 4, 8, 9 (OQ2) and the screen fade
  (pass 10, no d2rs input) emit nothing; marked `TODO(spec)` in code.
- DO7 Unit draw keys: units go through the existing composite with the
  order's pass / major / minor and `sub` = slot index; `unit-composite.md`
  §10 owns `sub` (the other session). The unit's own placement and
  offsets stay with `ViewSource::unit_offset`.

## Local checks to queue (§5)

- `order-0001`, `order-0002`, `order-0003` (spec §Test vectors): record
  with `record_frames.py` once RW1 records the lists, then
  `d2-client verify` the capture cases with the CPU reference composed in
  this order. Expected: identical pixels; `order-0003` settles OQ7
  (townN1 river cells 44,32 / 51,32 drawn or hidden).
- Cheap check of the grid read (`capture.md` §3 recorder): for one
  recorded frame, compare view +0xEAB0 (side) and +0xEAA0 / +0xEAA4
  (origin) against `DrawGrid::of_camera` of the recorded camera
  (expected 34 and `T(cx_t, cy_t) − (3, 16)` at 800 × 600), and the
  per-cell list contents against `rules::draw_order::fill` fed from the
  recorded rooms.
