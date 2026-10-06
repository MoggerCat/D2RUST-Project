# Local buddy a-sim 2026-10-07 (d2-sim re-run)

Branch `claude/local-buddy-a-sim-2026-10-07` from main `e5e7c94`. Machine and
`D2_GAME_DIR` as in `local-buddy-2026-10-06.md` (1.14d install, verified
identical there). Commands exactly as in `docs/LOCAL-RUN.md`; no expected
value or test changed. Logs stay on the PC.

| # | Expect | Actual | Result | vs 2026-10-06 |
|---|---|---|---|---|
| 2.1 C1 `cargo test -p d2-sim -- --ignored` | 5 listed pass (7 ignored tests now) | 6 pass, 1 fail (`real_grid_belt_and_type_tables`) | FAIL | same failure |
| 2.4 C34 `game_world` | 10 pass | 7 pass, 3 fail: `cubemain_vector_records`, `waypoint_objects`, `vendor_columns_from_live_items` | FAIL | same failures |
| 2.4 C34 `game_drlg_tables` | 8 pass | 6 pass, 2 fail: `lvlprest_measurements`, `act1_placement_on_live_tables` | FAIL | same failures |
| 2.6 C37 `game_core` (release) | 12 pass | 12 pass | PASS | still passing |
| 2.10 C45 `game_monsters` (release) | 19 pass | 16 pass, 3 fail: `real_levels_rows`, `ai_index_of_every_row`, `type_init_every_class` | FAIL | same failures |
| 5.1 `tick_replay` | 7 pass | 7 pass | PASS | still passing |

Nothing new failed and nothing newly passes: every result equals the
2026-10-06 run. The worldgen / monster-init / cube changes on main did not
change any of these outcomes.

2.6 printed lines: `3558 AnimData records scheduled, 0 with a speed past
i16`; `734 monstats rows, 673 with DamageRegen`; `357 skills, 78804 values
evaluated`. (The DS1 histogram and Act I room counts are not printed by
this test version.)

## Failure messages (verbatim, short)

```text
2.1  tests.rs:1540:57: ...game/extracted/patch_d2/data/global/excel/inventory.bin: The system cannot find the file specified. (os error 2)
2.4a game_world.rs:146:9: FrameCnt1 of 119  left: 3840 right: 15
2.4a game_world.rs:345:5: left: 129 right: 130
2.4a game_world.rs:427:36: weapons.HratliMin
2.4b game_drlg_tables.rs:229:5: left: 1079 right: 82
2.4b game_drlg_tables.rs:329:5: left: [5, 27, 6, 7, 26, 39, 17, 1, 2, 3, 4]  right: [16, 15, 14, 13, 12, 11, 10, 9, 8, 5, 27, 6, 7, 26, 39, 17, 1, 2, 3, 4]
2.10 game_monsters.rs:1037:13: level 15  left: 3800 right: 2025
2.10 game_monsters.rs:948:13: row 528 drehyaiced  left: 129 right: 31
2.10 game_monsters.rs:508:17: class 0 d 1 expansion false  left: 115456 right: 57728
```

The 2.10 expected evilhut row (528 or 529) was not reached: the AI test
fails first at row 528 `drehyaiced` (`ai_index` 129 vs 31 expected).

## Claims unlocked

Only the passing `game_core` (2.6) tests: seven `// Intended claim` lines in
`crates/d2-sim/tests/game_core.rs` became `// Covers:` lines
(`fixups.md` §2 r3; `vitals.md` §4.1, §1, §2, §3; `stat-lists.md` §7.2 r2;
`units.md` §4.2; `levels.md` §2, §4, §5). Five "none" intended-claim lines
left as they are. `py tools/coverage.py --check`: `4250 claims, 0 errors`.
`--summary` total: 4250 claims, verified 249/3339 rules (7.5%), up from
4240 claims, 227/3339 (6.8%) on main. No claim for 2.1, 2.4, 2.10 or 5.1
(no such lines in scope or the tests failed).
