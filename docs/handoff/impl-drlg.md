# Handoff: DRLG implementation (`d2_sim::drlg`)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Branch `claude/impl-drlg`, from `claude/bold-ptolemy-jvyvxy` at 4c7a7c7
(cloud, 2026-10-06). Specs: `specs/drlg/levels.md`, `specs/drlg/rooms.md`.

## State

**Implemented, unverified.** All synthetic test vectors of both specs pass
as unit tests (48 tests in `crates/d2-sim/src/drlg/tests/`; `cargo test
-p d2-sim`: 104 total). Both specs are drafts with queued confirmations
(HANDOFF §5) and the level type specs (`drlg/preset.md`, `maze.md`,
`outdoor.md`) are not written, so no real level can be generated yet: the
recorded vectors (§9.9 town table, the 20-level Act 1 list with real
placer draws) wait for those specs and the local checks below.

Changes outside `crates/d2-sim/src/drlg/`: one line `pub mod drlg;` in
`crates/d2-sim/src/lib.rs`. No dependency changes. d2-sim does **not**
need a `d2-formats` dependency: DT1 tiles arrive as plain
`TileInfo` records through the `TileSource` seam (the d2-server / world
side converts `d2_formats::dt1::Dt1Tile`: `orientation`, `main_index`,
`sub_index`, `rarity`, `material_flags`, `subtile_flags`), DS1 grids as
`RoomGrids` through `LevelTypes::room_grids`.

Gate run before push: `cargo fmt --all -- --check`, `cargo clippy -p
d2-sim --all-targets -- -D warnings`, `cargo test -p d2-sim`, `cargo run
-p depcheck`, `python3 tools/spec_index.py --check`, `python3
tools/methods.py check`: all clean.

## Code map rows (for HANDOFF §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-sim/src/drlg/mod.rs` | ids (`DrlgRoomId`, `LevelIdx`), `TileRect`, room flags, act-of-level / town tests, `DrlgError` | `drlg/levels.md`, `drlg/rooms.md` |
| `crates/d2-sim/src/drlg/data.rs` | `DrlgData`: leveldefs / lvlwarp / lvltypes / objects view from `d2_data::tables` records; `WallRemap`, `DoorTables` inputs | `levels.md` "Data", `rooms.md` §9 |
| `crates/d2-sim/src/drlg/seams.rs` | seams `LevelTypes`, `ActRooms`, `TileSource`; `Services` bundle | |
| `crates/d2-sim/src/drlg/level.rs` | `Drlg` (act DRLG, creation draws), `Dungeon`, levels (get-or-allocate, seeds, position), vis/warp records, `room_at`, activity counts, freeing, spawn room | `levels.md` §2–§10 |
| `crates/d2-sim/src/drlg/room.rs` | `DrlgRoom`, room creation seeds, near gaps/sort, rooms-near build + warp links + town border, statuses, set/unset handlers, propagate, stream/build | `rooms.md` §2–§4 |
| `crates/d2-sim/src/drlg/active.rs` | `ActiveRoom`, creation, adjacency fill/removal, clients, inactivity counter, removal test, removal, client room change; `ActRooms for UnitLists` | `rooms.md` §5–§8, `levels.md` §9.1 |
| `crates/d2-sim/src/drlg/tiles.rs` | tile library, lookup, tile choice, record flags, grid fill, cell rules, linked cells, animation, tile freeing | `rooms.md` §9 |
| `crates/d2-sim/src/drlg/collision.rs` | collision grid build, update, queries | `rooms.md` §10 |
| `crates/d2-sim/src/drlg/tests/` | spec vectors and rules on fakes (`fakes.rs`: scripted level types, DT1 source, `UnitLists`) | |

## Seams (traits) and expected providers

| Seam | Methods | Provider |
|---|---|---|
| `LevelTypes` | `create_act_levels` (act placer `0x00678AD0`), `init_level` (type init at allocation), `generate` (rooms of a level), `reset_level`, `add_preset_units` (status-3 handler), `preset_units` (waypoint position), `room_grids` (packed cell grids, after the seed reset), `free_room_tiles`, `door_unit`, `warp_unit` | the level type specs `drlg/preset.md`, `drlg/maze.md`, `drlg/outdoor.md` (future DRLG sessions). `NoLevelTypes` generates nothing. |
| `ActRooms` | `create_active_room`, `set_adjacent`, `remove_active_room` | `crate::units::UnitLists` (implemented in `drlg/active.rs`) |
| `TileSource` | `dt1(path) -> &[TileInfo]` | whoever holds parsed DT1s (d2-server / world). Paths: lvltypes `File` strings verbatim, and `tiles::FIXED_LIBRARY` |

Tick integration (for the world session; `tick` is not edited here):

- step 9 `TickHooks::room_inactivity(room)` → `Drlg::room_inactivity(room)`;
  `act_allows_room_removal(act, room)` → `Drlg::allows_removal(room)`.
- step 9 then compresses units and calls `UnitLists::deactivate_room`;
  the rest of the removal (neighbour arrays §6.3, other flags, tile free,
  record free) is `Drlg::remove_active_room(svc, room)`, which tolerates
  the already-unlinked room. **Seam request:** `tick` has no hook after
  the unlink; add one (e.g. `TickHooks::room_removed(game, act, room)`)
  so the provider can call `remove_active_room` in the same pass (tick
  open question T5).
- step 10 `free_inactive_rooms(act)` → `Drlg::free_inactive_levels`.
- `client_level_change` (`0x00537B50`) and client leave (`0x00539DA0`)
  → `Drlg::client_changes_room(svc, client, old, new)`.
- Unit placement at coordinates (`0x00553720`) → `Drlg::room_at` then
  `Drlg::stream_room`.

## Public API (for missiles/AI and world)

- Acts: `Dungeon::get_or_create(act, init_seed, difficulty, arena_level,
  data, types)`, `Drlg::create(...)`; fields `seed`, `start_seed`,
  `staff_tomb`, `boss_tomb`, `jungle_link`.
- Levels: `get_or_alloc_level`, `find_level`, `level_list`, `level`,
  `generate_level`, `level_rooms`, `room_count`, `level_at`,
  `set_level_position_and_size`, `free_inactive_levels`,
  `level_free_test`, `free_level_rooms`, `update_level_activity`.
- Warps: `vis_array`, `warp_array`, `warp_id`, `warp_record_mut`,
  `set_warp`, `lvlwarp_row`, `warp_records`; `DrlgRoom::warp_links`.
- Rooms: `alloc_room`, `link_room` (for type specs), `room`, `room_mut`,
  `try_room`, `room_at`, `build_near`, `DrlgRoom::near`, `status_list`,
  `change_status_room`, `set_and_propagate`, `unpropagate`,
  `stream_room`.
- Active rooms: `drlg_room_of(RoomId)`, `active_room`, `adjacent_rooms`,
  `active_rooms`, `add_room_client`, `remove_room_client`,
  `room_inactivity`, `allows_removal`, `remove_active_room`,
  `client_changes_room`; `ActiveRoom::seed` (active-room seed).
- Spawn: `spawn_room(svc, level_id, tile_index) -> SpawnPoint`.
- Tiles: `lookup_tiles`, `choose_tile`, `tile_info`, `animate_tiles`,
  `DrlgRoom::tiles`.
- Collision: `collision_at(sx, sy)`, `collision_at_mut(sx, sy)` (sub-tile
  coordinates; units set and clear run-time bits), `ActiveRoom::collision`,
  `collision::bits`.

## Open questions (each has a `TODO` in code)

Spec gaps found while implementing (for the next local spec session):

1. **Wall remap table values** (`rooms.md` §9.6): table `0x006EF578`
   is described as "identical to D2MOO's `nWallTileTypeRemap`" but not
   transcribed. `DrlgData::wall_remap` is an input; a linked wall over a
   normal wall returns `DrlgError::MissingWallRemap` until it is filled.
2. **Merge case analysis** (§9.6 step 3): "keep" (does it keep R's or the
   new type?), what a row does when R's type is outside 1..7, and how the
   door edge cases combine with the table are not stated. Reading used:
   see `tiles.rs` `merged_type` doc.
3. **Warp tiles** (§9.5 "Warp tiles"): how a cell finds "the room's warp
   entry for the cell's destination" is not given, so the LitVersion
   records (4 floor choices / 1 wall choice, i.e. draws) and the chaining
   are not modelled. Rooms with lit exits will draw fewer times than 1.14d.
4. **Door tables** (`rooms.md` OQ 10): not transcribed; door units are a
   seam call (`LevelTypes::door_unit`), whose provider draws the
   `roll(3)` for objects 91–92 on the room seed.
5. **Keep argument** of `0x0066F1A0` from `0x0066B4C0` (§9.2): not
   stated; keep = 0 used (flag 0x200000 never set).
6. **Link list order** (§9.6 "head insertion, 12-byte node"): read as the
   list node being inserted at the head of the room's link lists and the
   record appended to that list's chain. Only matters when two records at
   one position both match a find.
7. **Animation frame records' flags** (§9.7): only "flag 0x8" is stated;
   frames copy the base record's flags plus 0x8.
8. **All-zero rarities** (§9.4): the literal walk runs past the array;
   the spec's "Consequences" (first entry, no draw) is implemented.
9. **Empty lvltypes `File` names** for a set DT1 mask bit (§9.3): skipped.
10. **Spawn-tile class rule** (`levels.md` §10.2): read as "record index
    = t, or class[t].a ≠ 0 and class[t].b = class[record].b". Spawn-tile
    records and their coordinates (tiles assumed) come from the type
    specs. `Position` ≠ 0 with index 13 and no waypoint room returns
    `NoWaypointRoom` (unspecified). The spawn record's room lookup uses
    no level (§8.1 level search).
11. **Overflows** kept unbounded with TODOs: near candidates > 30
    (`rooms.md` OQ 5), warp-room centres > 9 (`levels.md` edge case 3),
    populated memory sized once (§9.4).
12. Carried from the spec: set handlers 0, 2 and unset handler 2 follow
    D2MOO (`rooms.md` OQ 1); `0x0066C0B0` in the removal test is treated
    as passing (OQ 2); client arrays sort by client slot as the stand-in
    for record addresses (OQ 4); collision build assumes every listed
    room has its grid (OQ 11).
13. Not implemented (no server outcome): the client-copy build timer
    (§4.6), the animation phase copy `0x0066D750` (§9.7), tile record
    screen coordinates, act tile-library cache (`levels.md` §3.5), unit
    flags of units left in a removed room (§8.2, units spec).

## Checks to queue (HANDOFF §5, local)

These need the level type specs implemented first (no real level can be
generated without them); queue them with those sessions:

1. **Act 1 level list** (`levels.md` Test vectors): with the outdoor
   placer implemented, `Drlg::create(0, 644409375, 0, 1, false, ...)` on
   the live tables must give the list 16, 15, …, 4 (head first) with
   seeds `init_low(4014346869 + id)`, the DRLG seed after creation and
   the 47 town room seeds of `20261005-232125-rng.jsonl` seq 2425–2591.
2. **Town tiles** (`rooms.md` §9.9): an ignored test (`D2_GAME_DIR`)
   that builds `TownW1.ds1` rooms through the preset type spec with the
   real Act 1 DT1s, in the server order 15, 7, 14, 8, 9, 21, 16, 22, 23
   and the client order 34 → 0, and compares per-room mod/mask draw
   counts with the §9.9 table and the first 12 draws (entries and `r`)
   of seq 6822–6833. Expected: every cell of the table equal.
3. **Adjacency order** (`rooms.md` queued checks C1–C4): replay of
   `ract`/`rdeact` from the tick recordings against `client_changes_room`
   / `remove_active_room` once the world session drives them.

No checker was added in this session (M08 perturbation test not
applicable); the tests compare against hand-derived spec vectors.
