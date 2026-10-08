# q-dungeon-builds: Act I's tree dungeons built by the maze generator

Branch `claude/q-dungeon-builds`. Nothing here is verified against 1.14d (rule 10). Open point: REC-261 (`docs/HANDOFF.md` §7).

## Links connected

| Link | State before | Now |
|---|---|---|
| Cave 2, Underground Passage 1–2, Hole 1–2, Pit 1–2 | flat 8×8-tile rooms (`synthetic_chains::room_origin`) | maze levels: `app/synthetic_a1_maze.rs` (`LEVELS`, `base_rooms`, `add_levels`), registered in `synthetic_tower::maze_levels` (lvlmaze rows), `synthetic_maze::is_maze`, `SyntheticTypes::generate` / `preset_units`, and `single_player::synthetic_drlg_data` (drlg type 1, level type 3) |
| Their warp tiles | flat room tiles | the tree's slots (`synthetic_chains::slots` / `tile_xy`) stand in the maze level's first room, flags `WARP_0 << slot`; `ACT1_TREE` untouched |

Test: `crates/d2-client/tests/app_dungeon_builds.rs` enters each of the seven levels from its parent, checks the maze built at least 4 preset rooms (flat: 1) and leaves by the way-back tile; also the passage / hole / pit one level deeper and back. `app_a1_dungeons` and `app_levels_warps_all` still pass.

## PROVISIONAL (REC-261)

Row sizes, room counts and the tile room are made up (`// d2rs-own, unverified`).

## Left

- Den of Evil stays flat (preset build needs live lvlprest/DS1 rows); Crypt, Mausoleum, Tristram and the Monastery–Catacombs levels stay flat.

## The user's local check

`cargo nextest run -p d2-client --test app_dungeon_builds --test app_a1_dungeons` (no game files). With game files: `cargo run -p d2-client --release -- play --new sorceress Test`, walk to Cold Plains → Cave 2 and Dark Wood → Underground Passage; the levels should be several-room caves, with an exit tile in the first room.
