# Handoff: DRLG preset implementation (`d2_sim::drlg::preset`)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Branch `claude/impl-drlg-preset`, from `claude/bold-ptolemy-jvyvxy` at
a5b323a (cloud, 2026-10-06). Spec: `specs/drlg/preset.md` (draft) +
`specs/drlg/preset-tables.tsv`.

## State

**Implemented, unverified.** Every synthetic test vector and edge case of
the spec passes as a unit test (26 tests in
`crates/d2-sim/src/drlg/preset/tests.rs`; `cargo test -p d2-sim`: 537
passed, 3 ignored). The recorded vectors (level file draws, the 35-room
town split) are reproduced on synthetic inputs with the recorded seeds;
their replay against the recording waits for the level-type dispatcher
(below) and the local checks in "Checks to queue".

Changes outside `crates/d2-sim/src/drlg/preset/`: one line `pub mod
preset;` in `crates/d2-sim/src/drlg/mod.rs`. No dependency changes.
**d2-sim does not need a `d2-formats` dependency**: DS1 files arrive as
plain `Ds1Input` through the `Ds1Source` seam (provider notes below).

Gate run before push: `cargo fmt --all -- --check`, `cargo clippy -p
d2-sim --all-targets -- -D warnings`, `cargo test -p d2-sim`, `cargo run
-p depcheck`, `python3 tools/spec_index.py --check`, `python3
tools/methods.py check`, `python3 tools/coverage.py --check`: all clean.
Coverage: `specs/drlg/preset.md` 43 of 60 units claimed (unit tier).
Unclaimed: §1 (record layout), §2 r3 (load filter, `d2-data`), §3.1/§3.2
text and §3.2 r4 (automap, client only), §3.1 r4 and §4 text (callers'
order, owned elsewhere), §5.2 r1/r4/r7/r9/r11 (header, tile names, tag
layer, groups, trailing bytes: not kept, the preset path never reads
them), §5.2 text, §6 text, §6 r2 (grid allocation), §12 (presentation),
§13 (column list).

## Code map rows (for HANDOFF §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-sim/src/drlg/preset/mod.rs` | `Presets` (per-DRLG state: level info, map lists, maps, preset room data), `PresetCtx`, `PresetUnit`, `PresetMap`, `PresetRoom`, `PopEntry`, `PresetError` | `drlg/preset.md` |
| `crates/d2-sim/src/drlg/preset/data.rs` | `PresetData` (lvlprest `PresetDef`, monpreset rows + act ranges, counts, hdm item class), `PresetTables` (embedded `preset-tables.tsv`, strict parse, door lookup, objpreset count) | §2, §5.3, §11, §13 |
| `crates/d2-sim/src/drlg/preset/ds1.rs` | `Ds1Input` / `Ds1Source` seam, `Ds1File` (parser rules, object → unit conversion, paths), `Ds1Cache` (process-wide, by exact path, ref counted) | §5 |
| `crates/d2-sim/src/drlg/preset/map.rs` | DrlgType 2 init / generate / reset, map alloc, area build (warp flags, scan, pops, tile info, waypoints, rooms), unit filter, first activation (lazy load, navi, river objects) | §3, §4, §6, §7, §8 |
| `crates/d2-sim/src/drlg/preset/room.rs` | room grids (ORs into the shared DS1, passes, kill edges, extra grid, animate), unit transfer, tombstones, door units | §9, §10, §11 |
| `crates/d2-sim/src/drlg/preset/tests.rs` | spec vectors and edge cases, TSV check with perturbation test | |

## Public API (small, for the DrlgType 2 provider and the maze / outdoor generators)

State: one `Presets` per act `Drlg` (keys are that DRLG's `LevelIdx` /
`DrlgRoomId`); one `Ds1Cache` per process (shared between DRLGs, as the
1.14d global list: the §9 ORs persist in the shared record). Every call
takes `&mut Drlg` and a `PresetCtx { drlg: &DrlgData, data: &PresetData,
source: &dyn Ds1Source, cache: &mut Ds1Cache }`.

- DrlgType 2 (`LevelTypes` methods of the future dispatcher):
  `init_level(drlg, ctx, level)` (§3.1, `roll(Files)` on the level seed),
  `generate(drlg, ctx, level) -> Option<last room>` (§3.2; the caller has
  reset the level seed, as `Drlg::generate_level` does),
  `reset_level(ctx, level, keep)` (§3.3), `set_direction(level, dir)`
  (act layout overwrite, `0x006772C0`), `info(level)`.
- Any level type (maze `0x00673A60`, outdoor `0x006750F0`):
  `alloc_map(drlg, ctx, level, def, rect) -> MapId` (§4, draws on the
  level seed), `map_mut(map)` (set `picked_file` = `0x00666EC0`, set
  `link_grid`), `build_area(drlg, ctx, level, map, flags, single) ->
  Option<last room>` (§6), `picked_file_path`, `map`, `level_maps`,
  `PresetDef::size_x/size_y` (`0x00666FD0/FE0`).
- Room build: `add_preset_units(drlg, ctx, room)` (status-3 handler, §8),
  `room_grids(drlg, ctx, room) -> RoomGrids` (§9–§10, also transfers the
  map's units into the room), `door_unit(drlg, ctx, room, wx, wy, cell,
  orientation) -> DoorOutcome` (§11), `room_units(room)` (full records,
  head first), `preset_units(room)` (the `LevelTypes::preset_units` seam
  view), `tombstones(room)` (`0x00666A80`), `room(room)`.
- Tables: `PresetData::{def_for_level, def}`, `PresetDef::from_record`
  (`d2_data::tables::Lvlprest`), `MonPresetRow::from_record_bytes` (raw
  4-byte monpreset record: the generated `Monpreset` decodes only `Act`),
  `PresetTables::{spec, from_tsv, door, objpreset_count}`.

### Providers and seam requests

- **`Ds1Source`** (d2-server / world, from `d2_formats::ds1::Ds1`):
  `width`/`height` are the **stored** values (d2_formats adds 1); act as
  i32; orientations **as stored** (d2_formats already remaps v < 7 files
  with the 25-entry table and rejects values 25–41 that 1.14d maps; no
  lvlprest DS1 is below v12, so no current file is affected); path
  actions as stored. Paths: lvlprest `File1`–`File6` strings after
  `data/fixups.md` §12, resolved by the provider.
- **`PresetData`**: `monpreset_acts` = `d2_data::fixup::FixedSet::
  monpreset`; `hdm_item` = the item-code lookup of `hdm ` (`0x00633640`,
  items spec), −1 if absent.
- **Errors:** `LevelTypes` methods return `DrlgError`, which has no
  variant for preset failures; the dispatcher needs one (e.g.
  `DrlgError::Preset(PresetError)` or a generic level-type error) — not
  added here (other module's file).
- **Door seam:** `LevelTypes::door_unit` passes no orientation; §11 needs
  it (right door = orientation 9). The door-record flag 0x20 is the tile
  code's: skip the call when set, set it on `DoorOutcome::Placed`.
- **`RoomGrids`** has no `Logicals` field (§10): the tile code does not
  model the logical coordinate lists.
- `DrlgData::doors` (`data.rs`, "not transcribed") is superseded by
  `PresetTables::doors` from the TSV.

## Open questions (each has a `TODO` in code)

1. `0x0066B970` list position (§6 step 10): head insert, read from
   `rooms.md` §9.9 (the client streams the town's rooms 34 → 0 in list
   order). Confirm against the recording.
2. Which step writes the level preset info's map field (§1 +0): set in
   `generate` after the direction sync.
3. Door flag 0x20 when `roll(3)` gives 0 or the position is outside the
   room (§11): not stated; `DoorOutcome` reports each case, the caller
   decides.
4. Out-of-grid waypoint cells (§6 step 9): a unit far outside the map
   writes past the cell grid in 1.14d; skipped (in-allocation row
   overflow follows the linear index).
5. Negative DS1 act (§5.2 step 2 keeps it): out-of-table read of the
   objpreset / monpreset tables → `PresetError::NegativeAct`.
6. v < 7 orientation beyond 41, item ids ≥ 1, door unit types other than
   1/2: errors (`BadDs1`, `ItemCodeBeyondTable`, `UnsupportedDoorType`);
   none occurs in lvlprest data.
7. Pop and tile-info capacity (§6 step 7): unbounded Vecs.
8. Link record bit (§6 step 10, link +0x0C |= 1) belongs to the outdoor
   record; `PresetRoom::link` carries the value. The link-grid layout
   (indexed like the §6 cell grid) is this API's contract with outdoor.
9. Spec open question 3 (§8 before §9 for rooms built before their map's
   first activation): the API needs `add_preset_units` before
   `room_grids` (the DS1 must be loaded: `Ds1NotLoaded` otherwise), as
   `rooms.md` §9.2 orders them.
10. Not implemented: §3.2 step 4 (client automap callbacks: generates
    Vis levels and streams rooms on a client DRLG), §12 pops at run time
    (presentation, wall clock), type-2 tile free `0x00666610` (not
    specified), DS1 tag layer and groups (outdoor's).

## Checks to queue (HANDOFF §5, local)

1. **DS1 survey through the sim's parser rules** (needs a crate with both
   `d2-formats` and `d2-sim`, e.g. an ignored test in `conformance`;
   `D2_GAME_DIR`): convert every DS1 named by lvlprest (2,043 files,
   d2data + d2exp) to `Ds1Input`, run `Ds1File::from_input` with the live
   `PresetData`. Expected: no error; monster records 2,267 and object
   records 14,105 before conversion; one record with flags ≠ 0 (value 1);
   kept class ids 580/581/582 = 46/135/24 records; every DS1 size equals
   its row's `SizeX`/`SizeY` (1,054 rows) and every DrlgType 2 level's
   size. Command: `cargo test -p conformance -- --ignored preset_ds1_survey`
   (test to write).
2. **Survey vectors on real files** (same test): `TownN1.ds1` at origin
   (X, Y) → tile info (X+26, Y+7, 0), (X+28, Y+7, 10), (X+30, Y+14, 11);
   `Act2/Town/LutN.ds1` (Def 301) → pops (8, 8, group 1, (24, 3, 7×7)),
   (13, 13, 2, (10, 27, 4×5)), (12, 13, 2, (15, 33, 3×3)) and 5 tile-info
   entries; `MetroTemple2.ds1` → style 8 rect (2, 2, 5×10), style 9 rect
   (0, 0, 12×7), both group 1.
3. **Recording `20261005-232125-rng`** (after the level-type dispatcher
   and the act placer exist): Act 1 with start seed 4014346869 gives
   `roll(1)` → 0 at `0x0066749F` for levels 26, 13–16 (seq 2433–2453),
   `roll(3)` lo' 2260552554 → 0 for level 27; level 1 generation: 35
   level-seed steps at `0x0066B42E` (seq 2455–2559), first lo'
   2928842600, no filter draws, rooms row-major (7 × 5). Then the town
   tile counts of `rooms.md` §9.9 (queued by `impl-drlg`).
4. Outdoor maps of levels 2 and 3: 48 / 61 `roll(Files)` steps per build
   at `0x00666F33` (with the outdoor session's code).

No checker beyond the TSV check was added; `tables_tsv_check_catches_
perturbations` is its M08 test (header, duplicate key, range, extra
cell, non-contiguous door level, unknown table; a changed value changes
exactly that entry).
