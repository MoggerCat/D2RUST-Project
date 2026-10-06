# Local buddy stg-server, 2026-10-07 (specs-staging 06db38f)

Lane A run of the room-population game-file checks (`impl-room-population.md` §4 items 1-2, `seed-finder.md` §4 item 1) against the original 1.14d install. **Base is `origin/claude/specs-staging` at 06db38f, not main.** Logs are outside the repo (`out-stg-server`). No expected value was changed.

## Results

| # | Command | Expected | Actual |
|---|---|---|---|
| 1a | `world_data::tests::game::act1_placement_matches_the_recorded_vector` | PASS | PASS |
| 1b | `world_data::tests::game::den_of_evil_matches_the_maze_vector` | PASS (Den rooms build coordinate lists, no WorldgenError) | PASS; no panic or WorldgenError naming any room |
| 1c | `world_data::tests::game::outdoor_levels_generate_through_the_dispatcher` | unchanged (red, prints split) | FAIL, same as before: `Cold Plains rooms: 97 total, 62 preset, 35 outdoor (recorded 98 = 61 + 37)` |
| 2 | `seed-finder --test game_seed_finder` `den_of_evil_and_blood_moor_build_on_live_tables` | PASS | PASS (twice-identical assert holds) |
| 3 | `seed-finder --release -- --ignored` same test | PASS | PASS (identical output to run 2) |

1c is a FINDING carried over unchanged from `local-buddy-tri-server` (97 vs 98; preset one too many, outdoor two too few). The room-population change did not move it. The failing assertion is the total (97 vs 98); no room name is reported by it, and the Blood Moor / Den rooms raised no error in 1.

## Printed lines (seeds 1-8; identical in debug run 2 and release run 3)

Den of Evil (level 8, `--from 2`): 27 rooms on every seed, entrance (7520, 5140) on every seed. Monsters per seed 1-8: 66, 75, 62, 79, 78, 100, 96, 94.

Blood Moor (level 2, `--from 1`): rooms and entrance per seed:

| seed | rooms | monsters | entrance |
|---|---|---|---|
| 1 | 79 | 82 | (5580, 5940) |
| 2 | 82 | 94 | (5100, 6260) |
| 3 | 83 | 93 | (4620, 4780) |
| 4 | 84 | 102 | (5620, 4860) |
| 5 | 83 | 109 | (4140, 5380) |
| 6 | 82 | 85 | (5020, 4380) |
| 7 | 84 | 86 | (5620, 4860) |
| 8 | 79 | 101 | (4780, 4700) |

seed-finder.md §4 said "the Den's room count varying by seed": it does not vary (27 for all eight seeds); only the monster count varies. Every line has an entrance, including the Blood Moor, so no Rogue Encampment rect problem.

## Coverage

No `Covers:` lines were promoted. `world_data/tests/game.rs` and `tools/seed-finder/tests/game_seed_finder.rs` carry no "Intended claim"/"Claim once" lines (the latter only says "no `Covers:` claim" in its header, and names no rule section to claim); the claim lines in `game_world_data.rs` belong to a binary not in this run. `tools/coverage.py --check` was not touched.
