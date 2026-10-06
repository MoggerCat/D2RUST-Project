# Local buddy tri-server, 2026-10-07 (main 06a9726)

Lane A re-run of the d2-server game-file tests against the original 1.14d install (triage rows "d2-server" and "Local re-run"). Logs are outside the repo (`out-tri-server`). No expected value was changed.

Command, exactly as asked: `cargo test --release -p d2-server -- --ignored --nocapture`. cargo stops after the first failing test binary, so run 1 only reached the lib (`--lib`). To see the integration binaries too, the same command was repeated with `--no-fail-fast` (run 3). `game_wired_host` was run a second time on its own (run 2).

## Results per test

| Test | Triage expected | Actual | vs yesterday (local-buddy-a-server) |
|---|---|---|---|
| `world_data::tests::game::act1_placement_matches_the_recorded_vector` | PASS | PASS | same |
| `world_data::tests::game::den_of_evil_matches_the_maze_vector` | PASS | PASS | same |
| `world_data::tests::game::outdoor_levels_generate_through_the_dispatcher` | stays red, prints split | FAIL 97 vs 98 | same failure, now with split |
| `game_world_data::every_lvlprest_ds1_parses` | PASS | PASS | was FAIL (ids 580..582 empty) |
| `game_world_data::every_act1_level_generates` | PASS | PASS | not listed yesterday (2 of 3 passed there, so it passed) |
| `game_world_data::wired_game_on_live_tables_runs_100_ticks` | PASS | PASS | same |
| `game_wired_host` x7 (amazon, assassin, barbarian, druid, necromancer, paladin, sorceress) | PASS | 0 passed, 7 FAIL, both runs | same count, but the failure moved: was `:301` "no town preset object with operate function 23", now `:642` "stub record" |

## Cold Plains

`Cold Plains rooms: 97 total, 62 preset, 35 outdoor (recorded 98 = 61 + 37)`. Identical in runs 1 and 3. Q6 answer: both sides differ from the recording. Preset is one too many (62 vs 61), outdoor is two too few (35 vs 37), net 97. The earlier "one room short" reading was only the total. Which `0x00666F33` cell is extra and which outdoor cells are missing still needs the per-cell dump (Q6 asks for it); this output does not name cells. 222 room-seed draws = 37 x 6 in the recording suggests the outdoor side (two missing outdoor rooms, 12 draws) is the real gap.

## every_lvlprest_ds1_parses

PASSES now: the fix (counting unit type 2 with class >= 573 on the parser output) is confirmed on the live data, the expected map {580: 46, 581: 135, 582: 24} was not touched.

## act1_placement

Both pass, and `act1_placement_matches_the_recorded_vector` asserts the full list `[4, 3, 2, 1, 17, 39, 26, 7, 6, 27, 5, 8, 9, ..., 16]` plus the derived rects. This supports Q4 first part: the neighbour loop get-or-allocates ids 8..16 (the recorded order is reproduced). It does not settle the vis[j] lookup or the adjacency warps (Act III, V), which are not exercised in Act I. `every_act1_level_generates` also passes.

## Wired host (F1-F7)

F5 (town waypoint) is past: the failure is no longer at `town_waypoint`. All 7 classes now fail at step 4, `game_wired_host.rs:642` in `assert_stub` (`leg`), identically:

```text
assertion `left == right` failed: stub record
  left: None
 right: Some((0, 3, 5))
```
The client 0x03 (5 bytes) is dispatched, but the sim's `unhandled` list is empty: 0x03 no longer reaches the stub, because a walk handler took it. `docs/handoff/game-tests-wired-host.md` line 3 already says "Step 4 asserts the stub for C->S 0x03 and is stale since `wire-path-server` (HANDOFF section 2 step 7u(a)); fix it before the local run". So this is the known stale test expectation (F2), not a defect found in the host. Not fixed here (rule: a failure is a finding, expected values are not changed by this lane); the fix belongs to a cloud session: either drop the stub assertion for 0x03 or assert the walk handler's result.

No arrival position/frame, kill, pick-up or `digest` lines were printed (they come after step 4), so the 7 `digest` lines of the two runs could not be compared (both runs print none; they are trivially equal and show no nondeterminism evidence). Needs a re-run after the step-4 test is fixed.

## Claims

Only `every_act1_level_generates` passed with a non-"none" intended claim. Its "Intended claim" line became `// Covers: specs/drlg/levels.md §3 r2, §3 r3, §4 r1, §5` (the comment about world_data/tests/game.rs also claiming §4 r1 was dropped; no claim was added there). The other passing tests' intended claims are "none", so those lines stay as they are. All `game_wired_host` lines stay "Intended claim" (the tests fail). `py tools/coverage.py --check`: 5239 claims, 0 errors.

## Triage questions

- Q4: partly settled (see act1_placement above); vis[j] get-or-allocate and adjacency warps still open.
- Q5: not settled by this run (the d2-client `play` panic was not run here).
- Q6: partly settled: preset 62 (+1), outdoor 35 (-2); cell identities still open.
