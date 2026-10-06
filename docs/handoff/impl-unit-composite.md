# Handoff: unit composites (`d2-client::rules::unit_composite`) — `claude/impl-unit-composite`

Cloud implementation session, 2026-10-06, task class: implementation
from a clear spec, medium (METHODS M14). Repo only. Spec:
`specs/render/unit-composite.md` (+ `render/unit-directions.tsv`, the
`formats/cof.md` / `formats/animdata.md` notes). For the coordinator to
fold into `docs/HANDOFF.md` / `docs/PLAN.md` (not edited here).

## 1. State

**Implemented, unverified** (draft spec; the capture `unit-0001` of the
Test vectors has not run).

- New files: `crates/d2-client/src/rules/unit_composite.rs`,
  `rules/unit_composite_tests.rs`. One line `pub mod unit_composite;`
  (plus the module doc and `Spec:` line) in `rules/mod.rs`.
- Changed infrastructure (C7 / p6-cof), where the spec says otherwise:
  - `composite::slot_order` reads the row as 1.14d does (§3 r6, new
    `composite::game_row`): offset `28 + 9L + F + (dir × F + frame) × L`,
    i.e. `event_padding ++ draw_order`, so the three padded `f9` COFs
    read 3 bytes early. No bound check on `frame` (a frame ≥ `F` reads
    the next direction's rows); a row past the file end is the new
    `CompositeError::RowPastEnd` (replaces `CompositeError::Frame`).
  - A component without a layer record is a slot that draws nothing
    (§5.1, §10), not `CompositeError::NoLayer` (removed). S7 (14) slots
    have no own graphic (§5 r1) and are left out; `composite::inline_slot`
    gives the S7 slot for the inline linked unit. Remaining slots keep
    their index `s` (= draw key `sub`, §10).
  - "Slot draws nothing" (§5 r2 failed request, §6 r4 missing file):
    additive hooks with defaults, so no existing implementor changed:
    `ComponentResolver::slot_frame` (default `frame(..).map(Some)`, used
    by `build` / `build_with`) and `ViewRules::component_slot_frame`
    (forwarded by `UnitResolver`, `OriginalView`, the feed wrapper).
- Tests: 26 new unit tests in `unit_composite_tests.rs`, 5 new / 2
  rewritten in `composite/tests.rs` (old `out_of_range_direction_and_frame_are_errors`
  and `draw_order_component_without_layer_is_an_error` encoded the
  behaviors the spec changes).
- `unit-directions.tsv` (docs/PLAN.md "Machine tables in code"): the code
  computes both tables from the spec's formulas (`cof_direction`,
  `file_direction` with the `P_D` interleave); test
  `directions_tsv_matches_code_row_by_row` embeds the TSV (test-only
  `include_str!`) and compares all 448 rows strictly (header, every
  (D, dir64) once); `directions_check_catches_perturbations` (M08) changes
  every value of every row, drops / repeats a row and swaps the header.

## 2. Code map rows

| Path | What | Spec |
|---|---|---|
| `rules/unit_composite.rs` | `Code` helpers (`part`, `code`, `LIT` …); `CompositeKind`; `CofName` (short = AnimData key, full path, TH rule); `mode_token` (override pairs); `monster_weapon_class`, `player_weapon_class`, `player_cof_weapon_class`; `cof_direction`, `file_direction`, `snap`, `expected_directions`, `unit_direction` (snap + write-back), `frame_index`, `component_cel`; `cof_box_visible`; `armor_class`, `ComponentCodes` (request, name, path), `file_format` (DC6 lists); `slot_frame`, `unit_pose` (the §10 hooks as plain functions); `unit_map`, `colormap_source`; `MotionRecord::update` / `draw_offset`, `unit_offset` (`ViewSource::unit_offset`); `missile_file`, `item_graphic`, `gold_direction`, `flippy_file` | §2–§10 |
| `composite/mod.rs` | `game_row`, `slot_order` (fixed), `inline_slot`, `S7`, `ComponentResolver::slot_frame`, `CompositeError::RowPastEnd` | §3 r6, §5, §10 |
| `world_view/mod.rs`, `rules/view.rs`, `world_view/feed.rs` | `ViewRules::component_slot_frame` and its forwarding | §5 r2, §6 r4 |

## 3. Integration notes (wiring left minimal)

- Nothing calls the new functions from a `ViewRules` implementation yet:
  the client unit model (`ClientUnit` = key only) holds no mode,
  direction, frame, inventory, monster choices or table rows. The
  functions take those as plain values; the S→C owner specs and a data
  provider fill them. `unit_pose` returns `(UnitPose, UnitDirection)`;
  the snapped `dir64` (for `slot_frame`) and the write-back are not in
  `UnitPose` (its fields are the spec's `cof`, `dir`, `frame`).
- `UnitParams.sub` (§10): `build` already uses the slot index; overlays
  (back at sub 0 before slot 0, front at 255) are not built (OQ13, no
  owner); the inline unit at the host slot is the draw-order session's
  (`inline_slot` gives the slot).
- The draw-time write-back (§3 r5) is returned, not applied (client unit
  path state does not exist yet).

## 4. Readings where the spec is silent (`TODO(spec: …)` at the site)

1. Duplicate layer records for one component stay an error
   (`CompositeError::DuplicateLayer`): §5.1's layer walk would take the
   first; the spec does not say, no 1.14d COF is known to have one.
2. An `armtype` index above 2 fails the request (the original reads past
   the table, Edge cases; no 1.14d row has one).
3. §8 r3 bounce reading: the bounce-count decrement (§8 r3) runs
   only when the limit is hit ("then" read as inside the hit branch).
4. `OYTRlitTNhth` (§6 r2) is compared byte for byte (case not stated).
5. Refused as `UnitCompositeError::Unresolved`: Barbarian dual-wield
   class (OQ3), monster armor override tables (OQ5), linked-unit
   inventory (OQ2), motion-record follow branch (OQ7). Mode override
   pairs (OQ1) and the per-mode monster direction table (OQ4) are caller
   inputs.

## 5. Local run queue (game files / captures)

1. Capture `unit-0001` (spec Test vectors): local player standing still,
   re-faced through 16 directions, each frame captured (`capture.md`);
   the CPU reference built with these rules must equal the capture per
   direction. Needs the client unit model wired first.
2. `D2_GAME_DIR=… cargo test -p d2-client --lib all_live_cofs_give_slot_orders -- --ignored --nocapture`:
   now with the game's row reading; expect `failures 0` (before this
   change a draw-order component without a layer record was a failure).
3. Suggested new ignored test (blind, M02): for every live player COF,
   `cof_direction(cof.directions, dir64)` < `cof.directions` and the
   §6 r1 component paths of `AM*NU1hs` resolve in the archives
   (`AMHDcapNU1hs.dcc` exists).

## 6. Open questions for the spec owner

- None new beyond the spec's OQ1–OQ13; the readings of §4 above should be
  confirmed or written into the spec.
