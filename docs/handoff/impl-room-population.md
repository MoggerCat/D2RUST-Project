# Handoff: live room population (branch `claude/impl-room-population`, 2026-10-06)

Cloud implementation session, repo only (no `game/`), from
`claude/specs-staging` at `5844674`. Read: `specs/`, `docs/`, `crates/`,
`tools/`. Implements `specs/drlg/levels.md` §11 (logical rooms and the
population reads, §11.6 map) and wires it into
`d2_sim::wiring::worldgen`, closing HANDOFF finding F1 ("with the
default `WorldPending` no room population runs, only presets place
monsters"). **Implemented, unverified** (M02): no recording of DRLG
room +0x64 lists and no spawn RNG trace exist yet.

## 1. What changed

| Area | Change | Spec |
|---|---|---|
| `drlg/logic.rs` (new) | `LogicInfo` (room +0x64), `CoordRec`, `LogicGrids`; `build_logic_one` (`0x0066CCB0`), `build_logic_grid` (`0x0066D110`: blocker grid, regions, fill with T1 / T2 and the five dwords before T2, rectangles, merge `0x0066D040`, rename `0x0066C770`), lookups `coord_first` (`0x0066CF30`), `coord_at` (`0x0066CEB0`) | `levels.md` §11.1–§11.4 |
| `drlg/tiles.rs` | after the tile fill (and animation): grid build for preset rooms with `RoomGrids::logicals`, else one record; `free_room_tiles` drops the info | §11.2 |
| `drlg/preset/room.rs` | `room_grids` fills `RoomGrids::logicals` (wall 0 orientation, floor 0, wall 0 cuts) when lvlprest `Logicals` ≠ 0 | §11.2 step 1 |
| `drlg/level.rs` | `Level::coord_counter` (+0x1DC); `populated_level`, `populated_room_count` (allocates, §4.3), `warp_points`, `kind11_location` (the §10 choice with tile 11, (−1, −1) when no room) | §11.5 |
| `drlg/room.rs` | `DrlgRoom::logic` and `logic()` | §11.1 |
| `monsters/population/seams.rs` | `PopWorld::populated_room_count` and `spawn_location` take `&mut self` (both can allocate a level or stream a room; `placement::near_warp` takes `&mut Ctx`) | §11.5 items 2, 4 |
| `wiring/worldgen/population.rs` | `PopWorld` answers every §11.6 read from the act DRLG: coordinate list / record (clipped box, node, index), `0x0061B130` through `find_room` (none → 0, null record → −1), populated level and count, warp points, kind-11 location through `with_act` | §11.4–§11.6 |
| `wiring/worldgen/mod.rs` | the seven DRLG-read defaults removed from `WorldPending` (only `nearest_free_point`, the path-provider fallback, stays) | — |
| `tools/seed-finder` | `FinderHost` is any `WorldPending`; `ROOM_POPULATION`, `rooms_streamed`, `StreamedRoom`, `supported` and `Kind::needs_room_population` removed: champion / unique queries are answered on the live host; the run note now says the build order is the finder's own (§11.6 rule 4) | — |
| `tools/scenario-run` | no change needed: it never relied on presets for monsters (spawn steps call population directly; the start area is the town, density 0); room population now also runs in its tick room pass | — |

## 2. Tests

- `drlg::tests::logic` (13): the §11 synthetic vectors (one-record
  room; grid builds: open room, blockers with orientation 1,
  orientation 0 reading before T2), unreached blocker with a zero
  clipped box, blocker test exclusions, node flag keys, counter rules,
  merge and its conditions, rename recursion and rules. M08: zeroing
  the d = −1 dword before T2 and disabling rename recursion fail 4 tests.
- `wiring::worldgen::tests::population::drlg_population_reads_are_the_act_drlgs`:
  populated level / count (with and without flag 0x800000), `0x0061B130`
  across the adjacency array, warp points, kind-11 location (8020, 8004)
  (the room holding the level centre (8018, 8007)). M08: dropping the
  flag test in `populated_room_count` fails it.
- Changed because room population now runs (no assertion weakened):
  `worldgen::tests::e2e::generated_level_is_populated_and_ticks_deterministically`
  and `prop_wired_path` `regress_*` tell the preset monster (pending log
  `preset` line) from room population's, and the e2e test asserts that
  population placed more monsters, all in the two rooms;
  `worldgen::tests::population::populate` asserts the real list of room
  A is the one record the fake used to inject. seed-finder:
  `the_live_host_refuses_random_boss_queries` became
  `the_live_host_answers_random_boss_queries` (same `UNIQUE_SEEDS` as
  the old fake: a one-record room's list is that record); the binary
  test checks a unique query is accepted. All pinned seeds unchanged.
- Cold Plains 98 rooms: `world_data::tests::game::outdoor_levels_generate_through_the_dispatcher`
  already asserts 98 (61 + 37); no test in the tree assumed 97. The live
  run gave 97 (HANDOFF "97 vs 98"): an outdoor-generation finding, not a
  population one, still open.

Gates (this branch): `cargo test -p d2-sim -p d2-server -p conformance
-p seed-finder -p scenario-run` all pass except the 10 known red tests of
other sessions (`monsters::ai::tests::specd_here_*`,
`skills::use_::tests::*`, `skills::mutant_tests::table_check_mutants::*`,
`missiles::tests_bodies::*`); clippy `-D warnings` and fmt clean;
`py tools/coverage.py --check` 5,272 claims, 0 errors;
`py tools/spec_index.py --check` clean. `d2-client` not built (disk).

## 3. Open questions (TODOs at their sites)

1. `levels.md` §11.3 step 6: orientations above 19 read past T1 (no
   bound check, values not given); read as rule 0
   (`TODO(spec: levels.md §11.3 r6)`).
2. `levels.md` §11.3 step 4: the local blocker grid's layout (1,024
   cells, 256 rows) is not given; a (W+1) × (H+1) grid is used, own wall
   records outside it skipped (never reached on room-relative records).
3. `levels.md` §11.4: lookups outside the (W+1) × (H+1) cells read
   outside the grid in 1.14d; here they give no record (`coord_index_at`
   −1). Not known to be reached.
4. Step 8 "same index": the full cell value is compared (equal to the
   masked index while the counter stays below 0x0FFFFFFF).
5. §11.3 step 9 (wall record +0x10) not stored: read only by drawing.

## 4. Local run queue (for `HANDOFF.md` §5; M02)

1. `cargo test -p d2-server world_data -- --ignored` with `D2_GAME_DIR`:
   unchanged expectations; additionally the Den of Evil / Blood Moor
   rooms now build coordinate lists (crypt-style `Logicals` rows run the
   grid build): any panic or `WorldgenError` names the room.
2. `cargo test -p seed-finder --test game_seed_finder -- --ignored`:
   Den of Evil and Blood Moor seeds 1–8 build twice with the same result;
   the views now list room-population monsters (record them).
3. `levels.md` open question 7: dump DRLG room +0x64 lists of a crypt
   level (`Logicals` 1) and an outdoor level after activation and compare
   with `drlg::logic` on the same tiles.
4. HANDOFF §5 item 11 / `population.md` "Recordings": the first
   population of a Blood Moor room through `WorldSim` against the spawn
   RNG recording (now runnable: population no longer stops at the empty
   coordinate list).

## 5. Next

- Run the queue above; on a mismatch, the spec owner of §11 decides.
- `seed-finder.md` F1 is closed by this branch; F2 (activation order)
  still holds and is now the main reason a hit is only a candidate.
