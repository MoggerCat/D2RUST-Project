# PC 2 Wave B: game-file tests for HANDOFF §5 C10, C11, C12, C14, C18, C19

Local run on PC 2 (Windows 11, the user's 1.14d install in `D2_GAME_DIR`),
branch `claude/pc2-waveb-tests` from `origin/claude/specs-staging-4`
(`a8a7458`). Debug build (rustc 1.99.0 crashes on the d2-sim lib test unit
in `--release` on this PC).

Following the §5 third fold and sixth fold: C14 and C19 have test code
(C34, `game_world`); C10 and C11 have test code (C37 `game_core`, C45
`game_monsters` / `game_skills`); C18's three parts also have test code in
C45 (`real_recorded_ai_params`, `real_missiles_record_layout`,
`real_recorded_missiles`), so they were run, not rewritten. Only C12 had no
test: one was written.

Command (all entries, one run):

```
D2_GAME_DIR=<install> cargo test -p d2-sim --no-fail-fast --test game_drlg_tables \
  --test game_world --test game_core --test game_monsters --test game_skills -- --ignored
```

Binary totals: `game_core` 12/12 pass; `game_world` 10/10 pass;
`game_drlg_tables` 9 pass, 2 fail (11 incl. the new test); `game_monsters`
18 pass, 1 fail; `game_skills` 15 pass, 1 fail.

## Per entry

| Entry | Tests | Written / existing | Expected | Actual | Result |
|---|---|---|---|---|---|
| C10 vitals + use | `game_core::charstats_and_experience_as_stated`, `player_creation_every_class`, `level_up_and_stat_point_vectors`; `game_skills::real_charstats_and_experience`, `real_vitals_vectors`, `every_class_every_level`, `real_use_vectors` | existing | spec vectors (threshold(0,1) 500, level_from_exp 499/500 → 1/2, Fire Bolt 640, Fire Ball L10 2,432, Teleport 6,144 / 1,280 / 0 / −1,280, Multiple Shot L10 3,328) | as expected | PASS |
| C11 population | `game_monsters::real_monstats_population_rows`, `real_level_list_facts`, `regions_every_level`, `real_levels_rows` | existing | "Real" rows of `population.md` | 3 pass; `real_levels_rows` FAIL: Act 0 rows with WarpDist ≠ 2025: `[(15, 3800), (20, 100), (21, 100), (23, 100), (25, 100)]` (spec §8 "Act 1 WarpDist is 2025" read as every Act 0 row) | FAIL (1 of 4) |
| C12 maze data | `game_drlg_tables::maze_defs_exist_in_live_lvlprest` (new), `lvlmaze_rows_as_stated`, `every_level_drlg_type_dispatches` | new + existing | 81 lvlmaze rows; every DrlgType 1 level has a row; every non-zero `shape_def(type, mask 1..15, rooms_one)` of the maze level types, every `maze-specials.tsv` replace def and the fixed defs (167, 288–290, 333, 336, 444–447, 480, 735–738, 836, 852–856, 1038–1041, 1074–1077) in lvlprest with `Files` ≥ 1 | 81 rows and every level row: pass. 15 defs with `Files` 0: type 19 masks 3, 5, 6, 7, 9–15 (defs 512, 514–516, 518–524), type 33 with `Rooms[d]` = 1 masks 1, 2, 3 (defs 1, 2, 3), fixed def 167. Specials: all 54 kinds present in lvlprest with Files ≥ 1 | FAIL (new test) |
| C14 vendors | `game_world::vendor_columns_from_live_items`, `recorded_stores_fit_live_columns`, `npc_txt_multipliers`, `npc_records_from_live_monstats` | existing | Charsi column (`aqv`, `cqv` permanent; `axe` 1/1/1/1/1), `npc` = §9.3, difficultylevels odds | as expected (`vendor_columns_from_live_items`, which panicked on `weapons.HratliMin` in the 2026-10-06 run, now passes) | PASS |
| C18 combat / AI / missiles | `game_monsters::real_recorded_ai_params`; `game_skills::real_missiles_record_layout`, `real_recorded_missiles` | existing | ai-bodies §9.1, 684 × 420-byte missile records, R10 values | as expected | PASS |
| C19 quests | `game_world::quest_tables_against_live_data` | existing | every quest NPC id a valid monstats row | as expected | PASS |

Other failures in the same binaries (not one of the six entries, recorded):

- `game_drlg_tables::lvlprest_measurements`: rows naming a file beyond
  `Files` 80, expected 82 (after the 1,079 → 82 fix of the earlier run).
- `game_skills::every_skill_function_in_table`: the `used` (kind, index)
  set over the live tables is non-empty while the `Mapped` set of the
  function catalogue is empty (`right: {}`): either the catalogue has no
  `Mapped` entries any more or its status parse changed; finding for
  `skills/use.md` §8 / the catalogue, not a data value.

## C12 test decisions (recorded, user away)

- "each maze level type": the (LevelType, `Rooms[d]` = 1) pairs of every
  DrlgType 1 level over the three difficulties (22 pairs on the live
  set); `shape_def` results of 0 are skipped ("non-zero").
- "special def": the `replace_def` (`SpecialRow::special`) of every
  `maze-specials.tsv` row; find defs are shape defs already checked.
- `Files` is the first lvlprest row of each def (`MazeData::files`).
- Interpretation for the owner of `maze.md`: type 19's 510 + mask base and
  the `Rooms` = 1 remap of type 33 (def = mask) may name defs the code never
  builds from a file (or the shape remap is wrong for them); def 167 is the
  type-7 base (167 + mask), so it may not be a real fixed def. No expected
  value changed; no `Covers:` added.

## Covers changes

Added `// Covers:` (claim table of `game-tests-monsters-skills` §4,
`game-tests-drlg-world` §2) only on tests that passed:
`game_skills`: `real_use_vectors`, `real_charstats_and_experience`,
`real_vitals_vectors`, `every_class_every_level`,
`real_missiles_record_layout`, `real_recorded_missiles`; `game_monsters`:
`real_recorded_ai_params`, `real_monstats_population_rows`,
`real_level_list_facts`, `regions_every_level`; `game_world`:
`npc_txt_multipliers` (`specs/world/vendors.md` §9.3).
`py tools/coverage.py --check`: 8,920 claims, 0 errors. Nothing added for
`real_levels_rows` (failed) or the new C12 test (failed).

Gates: `cargo fmt -p d2-sim`, `cargo clippy -p d2-sim --tests -- -D
warnings` pass.
