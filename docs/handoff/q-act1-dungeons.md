# q-act1-dungeons: entering a maze level

Stitching session `q-act1-dungeons`, branch `claude/q-act1-dungeons`.

## The path, with the links that were missing

| Link | State before | Now |
|---|---|---|
| Maze generator, preset rooms, DS1 grids (`drlg/maze.md`, `drlg/preset.md`) in `d2-sim` (`WorldTypes`) | existed and tested, live game only | unchanged |
| Synthetic world | flat 8×8 rooms only (`Types`), no maze level | `app/synthetic_maze.rs`: `SyntheticTypes` sends Cave Level 1 (level 9, level type 3, maze row, one 24×24 floor DS1 for every cell) to the real `WorldTypes`, everything else to the flat `Types`; used by the server DRLG, the client DRLG copy (`client_drlg_source`) and the server's world state |
| Warp pair Den ↔ Cave Level 1 | none | lvlwarp rows 2 and 3 (`DEN_TO_CAVE` 13, `CAVE_TO_DEN` 14), Den vis/warp slot 1, cave slot 0; the Den room carries `WARP_0 << 1` and a stairs tile preset; the maze's first room carries `WARP_0` and the way-back tile preset |
| Entering | `warp_dest` / `warp_player` (q-warps) | unchanged: the Den's `build_warp_links` generates the maze level on streaming; the arrival room is streamed and the player placed in it |

Test: `crates/d2-client/tests/app_maze_level.rs`: Blood Moor → Den → click the stairs; the server player ends in level 9, the maze has ≥ 9 preset rooms, the client follows (level 9, way-back tile in `units`), no rejected message. Did not compile / run before the change (no maze level, no `synthetic_maze`). `app_level_warp` still passes.

## PROVISIONAL (REC-480 in `docs/HANDOFF.md` §7; was a second REC-117, renumbered by q-fix-prov-rec-ids)

Which maze room carries the exit warp and where the tile sits. The original takes exits from the cells' DS1 warp units (`rooms.md` §9.5.1 `warp_unit`, the door/warp tables are untranscribed). Here: the first room of the level, tile at sub-tile (20, 20). All `// d2rs-own, unverified`.

## What's left

- The real levels: with game files the live `WorldTypes` already builds the Den, caves, crypt, catacombs etc. by `drlg/maze.md`; their entrances/stairs depend on the DS1 warp units (`warp_unit`) and the level's `lvlwarp` links. Not exercised without game files; see the local check.
- Waypoints in the maze levels (Catacombs L2 etc.) rely on the existing waypoint object path and were not touched.
- Stairs tiles are clicked via `Bridge::interact` in the test; on-screen pickability is as for the cave entrance (q-warps "What's left").

## The user's local check (game files)

```
cargo run -p d2-client --release -- play --new sorceress Test
```

Walk to the Blood Moor, enter the Den of Evil, then find the stairs down to Cave Level 1 and click them (or enter Cave Level 1 of the Cave via its entrance). Expect: the level switches to maze-built tiles (rooms joined, no void), the player is placed on a floor tile, no panic and no `rejected` line. Note the console lines if the stairs are not clickable or the player lands off-floor.
