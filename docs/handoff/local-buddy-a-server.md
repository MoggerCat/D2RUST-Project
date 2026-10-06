# Local buddy a-server, 2026-10-07 (main e5e7c94)

Lane A game-file checks against the original 1.14d install. All three checks FAIL, all identical to the 2026-10-06 results (main 0472619). No failure names a boss's umods, so the montype-nesting change in umod eligibility is not implicated. No test, expected value or `// Covers:` line was changed.

| Check | Expected | Actual | Verdict | vs yesterday |
|---|---|---|---|---|
| 2.7 (C37) `game_world_data` | 3 passed | 2 passed, 1 failed | FAIL | same |
| 2.8 (C23) `world_data` | 3 pass | 2 passed, 1 failed | FAIL | same |
| 2.13 `game_wired_host` (x2) | 7 passed | 0 passed, 7 failed, both runs | FAIL | same |

## 2.7
```text
thread 'every_lvlprest_ds1_parses' panicked at crates\d2-server\tests\game_world_data.rs:167:5:
assertion `left == right` failed: object ids >= 573
  left: {}
 right: {580: 46, 581: 135, 582: 24}
```
The test expects object ids 580..582 to appear in the parsed DS1 set and none did; likely owner spec is the DS1/lvlprest parse or object-id table (worldgen/ds1 spec). `wired_game_on_live_tables_runs_100_ticks` passed (no adapter errors).

## 2.8
```text
world_data::tests::game::outdoor_levels_generate_through_the_dispatcher panicked at crates\d2-server\src\world_data\tests\game.rs:179:5:
  left: 97
 right: 98
```
Cold Plains (or the room count asserted at line 179) is 97 against the recorded 98; one room short. Likely owner: worldgen dispatcher / outdoor generation spec (`wire-worldgen.md`). `act1_placement_matches_the_recorded_vector` and `den_of_evil_matches_the_maze_vector` passed.

## 2.13 (two runs)
Both runs, all seven classes (amazon, assassin, barbarian, druid, necromancer, paladin, sorceress):
```text
game_wired_host.rs:301:5: no town preset object with operate function 23
```
The wired host finds no town preset object with operate function 23 (likely a waypoint/portal-type object in the Rogue Encampment preset), which is related to the 2.7 missing object ids; same root as 2.7 is plausible. No Blood Moor arrival, kill or `digest` lines were printed, so the seven digest lines could not be compared between runs. Likely owner: game-tests-wired-host F1-F7 setup / object-table wiring.

Logs: C:\Users\zffit\Desktop\D2test\out-a-server\ (outside the repo).
