# Local buddy q9-a: C87-C91 against the original 1.14d install

Lane A run. **Base: `origin/claude/specs-staging` at 913d3b0 (ninth fold).** PC: Windows 11, D2_GAME_DIR = the main checkout's `game`. d2-sim tests built in debug. Logs are outside the repo (`out-q9-a`). No spec expected value was changed. Comparison runs: `local-buddy-stg-formats-2026-10-07`, `local-buddy-stg-server-2026-10-07`.

| Entry | Command | Result |
|---|---|---|
| C87 | `cargo test --no-fail-fast -p d2-sim --test game_monsters -- --ignored ai_index_of_every_row` | PASS (1 passed; 0 differing counts or rows) |
| C88 | `cargo test --no-fail-fast -p d2-formats --test game_sweep -- --ignored cof_every_live_file_parses string_tables_every_key_resolves` | PASS (2 passed). Matches stg-formats exactly: 3,605 cof parse; the only failure is `d2char.mpq` `amblxbow.cof` (72 bytes) and 3 files are 42-byte padded; ambl1hs / ambl1ht / amblhth / amblxbow each in d2char.mpq only; 29 tables (20 paths), 10 languages, 63,167 used entries, 4,236 earlier-duplicate, 16,786 non-ASCII, 0 raw FF, 130 `C3 BF` |
| C89 | `cargo test --no-fail-fast -p d2-server world_data -- --ignored`; `cargo test -p seed-finder --test game_seed_finder -- --ignored` | 1a `act1_placement_matches_the_recorded_vector` PASS; 1b `den_of_evil_matches_the_maze_vector` PASS (no WorldgenError or panic); 1c `outdoor_levels_generate_through_the_dispatcher` FAIL, unchanged from stg-server: `Cold Plains rooms: 97 total, 62 preset, 35 outdoor (recorded 98 = 61 + 37)` (assert at game.rs:199, 97 vs 98); seed-finder `den_of_evil_and_blood_moor_build_on_live_tables` PASS |
| C90 | new `world::hirelings::tests::game::test_vector_rows_on_the_live_tables` | PASS, all 8 vector rows: every offer column and every unit column equal the spec |
| C91 | new `world::quests::act2::tests_q6::staff_hand_in_period_from_the_live_missiles_range` | PASS: live missiles row 338 Range = 440, orifice timer period 18 |

## C89: room-population monsters the views list (seeds 1-8, identical to stg-server)

Den of Evil (level 8): 27 rooms and entrance (7520, 5140) on every seed; monsters 66, 75, 62, 79, 78, 100, 96, 94.
Blood Moor (level 2): rooms / monsters / entrance per seed: 79/82 (5580,5940), 82/94 (5100,6260), 83/93 (4620,4780), 84/102 (5620,4860), 83/109 (4140,5380), 82/85 (5020,4380), 84/86 (5620,4860), 79/101 (4780,4700). Each built twice with the same result.

The 1c finding is unchanged since the earlier runs (preset one too many, outdoor two too few): carried, not new.

## C90 test notes

- Added in `crates/d2-sim/src/world/hirelings/tests/game.rs` (`#[ignore]`, reads D2_GAME_DIR through a small local loader, same pattern as `skills::tests::game_loader`); `// Covers: hirelings.md §1.2 r2, §2, §4` (promoted after the passing run; `coverage.py --check`: 8,230 claims, 0 errors).
- `HirelingTables::from_tables` takes the raw extracted `hireling` and `pettype` `.bin` (no fixup pass; the name-id fixups of fixups.md §7 do not touch the columns tested) and `MaxLvl` 99 as a constant.
- Offer: `offer` rolls Id and level from a seed, so the test scans seeds for one that gives the vector's Id and level (player level L+2); the Id's act and difficulty come from its lowest bracket row.
- Unit: `apply_level` runs on the recording fake with `skills` `reqlevel` from the live table. The spec's "row N" is read as the bracket whose `Level` is N (rows.rs `row_at`), not the table index; the test asserts that.
- Skill ids are hard-coded from the standard skills.txt (Inner Sight 8, Jab 10, Cold Arrow 11, Inferno 41, Fire Ball 47, Prayer 99, Thorns 103, Bash 126, Stun 139); the bin has no names. All matched, which confirms the ids.
- Resists compared on the fire stat only (all four are set the same way in `apply_level`).

## C91 notes

A typed `Missiles` decode is reachable from d2-sim, so the test reads `missiles.bin` row 338 directly, asserts Range 440, feeds it to the quest fake's `missile_ranges` and checks the `LairObjects` timer period 18 after `staff_inserted`. The quest tests have no shared loader; the test carries its own read.

## Checks

`cargo fmt -p d2-sim --check` clean; `cargo clippy -p d2-sim --all-targets -- -D warnings` clean; both new tests also run as ignored (not in CI).