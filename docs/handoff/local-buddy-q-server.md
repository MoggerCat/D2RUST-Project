# Local buddy q-server, 2026-10-07 (main 674996d)

Lane A run of HANDOFF section 5 queue entries 59, 60 and 65 on the 1.14d install, plus the d2-server and d2-sim `--ignored` re-runs. Logs stay on the PC (`out-q-server`). No expected value or test was changed; no `// Covers:` line was added (see Claims).

## Entry 59 (seven wired-host tests, twice): FAIL

Command: `cargo test --release -p d2-server --test game_wired_host -- --ignored --nocapture --test-threads 1`, run twice (plus once more inside the full d2-server run).

| | Expected | Actual |
|---|---|---|
| Result | 7 passed, 0 failed | 0 passed, 7 failed, in all 3 runs |
| Failure | none | all 7 classes at `game_wired_host.rs:642:9` (`assert_stub`, step 4), `stub record`, left `None`, right `Some((0, 3, 5))` |
| Digest lines | 7 `class N: digest <hex>`, equal across the two runs | none printed (they come after step 4); comparison NOT POSSIBLE |
| Arrival position/frame, kill, pick-up notes | recorded per class | none printed |

Identical to the tri-server note (failure at `:642`). Cause is the already documented stale step-4 expectation (`game-tests-wired-host` line 3: the C->S 0x03 now goes to the walk handler of step 7u(a), so the stub list is empty). Not fixed here. Needs a cloud fix of the test, then a re-run for the digests.

## Entry 60 (`GameData::load` vs the hand-built setup): PARTIAL, digests NOT RUN

No test exists for it. A scratch crate outside the repo (`scratch-q-server`, path deps on the worktree) loaded the live set twice, once by the hand-built steps of `game_wired_host.rs` `live()` and once with `test_fixtures::game::GameData::load`, and compared `Debug` text:

| Part | Result |
|---|---|
| `BinSet` (18,112,158 bytes of text) | equal |
| `AnimData` | equal |
| `LevelTables` (level ids, drlg/preset/outdoor views) | equal |
| `WorldFiles` (22,040,507 bytes) | equal |
| `FixedSet` | text differs, but also differs between two `fixup::apply` calls on the same input (map iteration order); the sorted characters are equal, so same content |

So the constructor reproduces the loading half of the setup. The seeds and digests half cannot be compared while entry 59 fails at step 4, so the step 7e refactor is not yet confirmed. Re-run after the 59 fix. (`ActCreation::Full` was not exercised by the scratch program; it only covers `GameData::load`.)

## Entry 65 (Act III / Act V on live tables): PASS

`cargo test --release -p d2-sim --test game_drlg_tables act3_act5 -- --ignored`: `act3_act5_table_values` ok, `act3_act5_placement_on_live_tables` ok (2 passed). Stage-1 claims for these were already converted in earlier runs (no "Intended claim" lines remain in `game_drlg_tables.rs`), so nothing to edit.

## d2-server `--ignored --no-fail-fast` vs tri-server: no change

Same as tri-server: lib 2 pass (`act1_placement_matches_the_recorded_vector`, `den_of_evil_matches_the_maze_vector`), 1 fail (`outdoor_levels_generate_through_the_dispatcher`, `Cold Plains rooms: 97 total, 62 preset, 35 outdoor (recorded 98 = 61 + 37)`); `game_wired_host` 0/7 at `:642`; `game_world_data` 3/3 pass.

## d2-sim `--ignored` vs tri-sim

The lib test target of d2-sim could not be built: rustc crashed with `STATUS_ACCESS_VIOLATION` (0xc0000005) compiling `d2-sim` (lib test) in release, three times (jobs 8, jobs 4, and without sccache with jobs 2). Lib `--ignored` tests therefore NOT RUN this time (tri-sim: 8/8 pass). Probably a rustc 1.99.0 (2026-09-28) problem, not the test code. The eight `game_*` integration binaries were run with `--test` flags instead:

| Test | tri-sim | now |
|---|---|---|
| game_core 12 | pass | pass |
| game_drlg_tables | 9/10, `lvlprest_measurements` 80 vs 82 | same |
| game_inventory_path 6 | pass | pass |
| game_items | 7/8, `sweep_create_every_item_every_quality` 560 failures | same (560) |
| game_monsters | 17/19 (`ai_index_of_every_row` and `real_levels_rows` fail) | 18/19: **`ai_index_of_every_row` now passes**; `real_levels_rows` still fails and prints the same WarpDist list |
| game_skills | 15/16 (`every_skill_function_in_table`) | same |
| game_treasure 16 | pass | pass |
| game_world 10 | pass | pass |

Only change: `ai_index_of_every_row` red to green.

## Claims

None converted: the only tests that passed here (entry 65, game_world_data) either have no claim lines left or were converted before. `py tools/coverage.py --check` was run before the commit (result below).
