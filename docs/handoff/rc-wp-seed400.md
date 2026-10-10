# rc-wp-seed400 hand-back

Branch `claude/rc-wp-seed400` (merged `claude/rc-ai-special`). Checks: `gen-wp-*` (39).

## Before / after (state ticks equal of 17940)
- Before (staging + rc-ai-special): 17330 equal, DIVERGED 22 (wp-30..38 at frame 430, wp-21 at 401, others).
- After path-target fix: 17749, DIVERGED 4 (wp-2, 21, 28, 37).
- After water-point fix: 17809, DIVERGED 3 (wp-2, 28, 37). wp-18..26 now match.

## What changed
1. `0x00553540` (path target unit) returns none when the target is the unit
   itself. Malah (class 513) ends S1 (self-targeted) and 1.14d requests
   neutral toward (0,0), clearing path tx/ty; d2rs kept (x,y).
   `wiring/path/monsters.rs::path_target`; note in `specs/monsters/ai.md` 1.4.
   Fixed gen-wp-30..38 frame 430.
2. Water point `0x005B2700`: tile records were never water (TODO). Now the
   record's DT1 material flags & 2 (`0x00604BC0`). The Slime Prince pack
   (class 249, spawnCol 1) failed to place, so 2 monsters and 2 game-seed
   draws were missing at frame 401 (gen-wp-21).
   `wiring/worldgen/population.rs::tile_records`; note in `population.md` 9.2.

## Open
- gen-wp-2 (frame 400): player x 5098 vs 5338; the poke or arrival position
  differs (size S, not examined).
- gen-wp-28 (frame 401): monster 1:29 class 403 (1.14d) vs 308 (d2rs): a
  different pack class picked (population pick; size M).
- gen-wp-37 (frame 451): monster 1:10 class 503 unit seed differs (size S-M).
- `prop_walk_motion::chase_a_moving_target` failed once, then passed
  4760/4760 on rerun: proptest randomness. Not diagnosed (no seed saved).
