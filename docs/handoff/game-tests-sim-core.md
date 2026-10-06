# Handoff: game-file tests for stats, units, vitals, skill levels, the world-data providers and a wired game (branch `claude/game-tests-sim-core`, 2026-10-06)

> To be folded into `docs/HANDOFF.md` (§1, §5) and `docs/PLAN.md` by a docs session; this file stays as the detailed record.

Cloud test session, repo only (no `game/`), medium effort, from
`claude/tender-meitner-mphas3` at `14f8185`. Read: `specs/`, `docs/`,
`crates/`. The diff adds two test files and this note. It changes no
library code, spec, `HANDOFF.md` or `PLAN.md`.

## 1. State

- 15 new `#[ignore]` tests (12 + 3) that read `D2_GAME_DIR`. They compile and
  pass clippy, but **none has run**: there are no game files in the cloud (M02).
- **Expected values unconfirmed.** Every value comes from a spec
  (Constants, Test vectors, a recorded vector) or from an invariant a
  spec rule states. None was observed on the live files. Following the
  `HANDOFF.md` §8 lesson, no test carries a `Covers:` claim. Each test
  has an `// Intended claim (unconfirmed until the first local run): …`
  line instead. Coverage is unchanged on this branch.
- Loading is the same as the other game-test files: `ArchiveSet::open_dir` →
  `bin::load` → `fixup::read_animdata` → `fixup::apply`. The server
  file adds `LevelTables::from_fixed` and `WorldFiles::load` with
  `archive::reader`, the same path `archive::load` takes.

## 2. Tests

### `crates/d2-sim/tests/game_core.rs` (12)

| Test | Checks (source) | Intended claim |
|---|---|---|
| `itemstatcost_columns_as_stated` | 359 records; ValShift 8 exactly on 6–11, 216, 217; keepzero {8, 10}; fMin {0, 1, 2, 3, 7, 9, 11} with MinAccr 1,1,1,1,1,0,0; Saved 0–15; fCallback 35 stats, first 7, 9, 11, 78, 81, 83, last 204; damagerelated 104; MaxStat 6→7, 8→9, 10→11, 72→73; UpdateAnimRate 67–69; +0x04 bits besides 5–8 = {0–4, 9–12} (`stats.md` Constants, §2.2, §3) | none (data facts) |
| `itemstatcost_ops_as_stated` | the stats of every op as §6.3 lists them, the op histogram, op base 12 for ops 2/4/5, 42 targets (A52), entries(7), deps(12) = 214…250, A53 = 214, 215, 218, 219, the §6.3 targets of ops 1, 8, 9, 11; every entry's op is a `stat-ops.tsv` row (rows 1–13) and `op_row` gives it; ops 3, 10, 12 unused (`stats.md` §6.3, Constants, Test vectors "Real data") | none (data facts) |
| `op_tables_rebuild_from_the_columns` | `fixups.md` §2 step 3 rebuilt in the test from the typed op columns equals A51 / A52 / A53 / deps / entries of every fixed-up record; the op graph has no cycle (`stats.md` edge case 5) | `fixups.md` §2 r3 |
| `charstats_and_experience_as_stated` | the 13 charstats columns of all 7 classes (`vitals.md` Constants); MaxLvl 99, threshold(c, 1) = 500 and threshold(c, 99) = 3,837,739,017 for every class; `level_from_exp(0, 499 / 500 / u32::MAX)` = 1 / 2 / 99; MaxLvl of class 0 = `LEVEL_CAP_114D` (`levels.md` OQ2 answer) | `vitals.md` §4.1 |
| `player_creation_every_class` | `vitals.md` §1 (act 0) for all 7 classes through the real `VitalsView` on live stat lists: every stat of the §1 table, the three maxima; Sorceress 10240 / 8960 / 18944, Barbarian 14080 / 2560 / 23552 | `vitals.md` §1 |
| `level_up_and_stat_point_vectors` | Sorceress and Barbarian level 1 → 10 (maxima, refill, statpts +45, newskills +9, nextexp); Barbarian +10 vitality (+10240 / +2560, then a failed 11th spend); Sorceress +10 energy (+5120) (`vitals.md` Test vectors) | `vitals.md` §2, §3 |
| `stat_list_vector_on_live_records` | the `stat-lists.md` vector steps 1–4, 6 on the live itemstatcost / states and the live Barbarian record (LifePerVitality 16, StaminaPerVitality 4: the vector's class): the same callbacks, full arrays, mods, state expiry | none (the synthetic run claims these rules) |
| `monster_damage_regen_every_class` | `stat-lists.md` §7.2 r2 on every monstats row: max life 100 points → stat 74 = (100 · DamageRegen) >> 4, else 0 | `stat-lists.md` §7.2 r2 |
| `anim_schedule_every_record` | `units.md` §4.2 main form (b = 0, f = 100, s = the record's speed, F = frames · 256) on every AnimData record: U4 shape (frame order, a1 ∈ 1–4, numbered a2, one event 1, last), end = f + max(⌈F / s⌉, 2), +0x44 = f · 256; with s ≤ 256 the action count equals the event bytes 1–4 of frames < min(frames, 144). Records whose speed exceeds i16 are skipped and counted | `units.md` §4.2 |
| `every_skill_level_calc_evaluates` | every formula field of every skill (34 fields), plus elemental / physical damage, length, mana, shifted mana and to-hit, at levels 1, 2, 5, 10, 20, 25, 30, 60, 99, with no unit and with a bare unit: evaluates (no panic, no endless recursion) and repeats to the same value | none (completion only) |
| `skills_table_facts` | 357 rows; EDmgSymPerCalc 64, DmgSymPerCalc 6, ELenSymPerCalc 4, ToHitCalc 3, SrcDam 63, skpoints 0, HitShift 8 × 316 / 7 × 20; 7 negative `lvlmana`, Teleport (54) among them (`levels.md` Constants) | none |
| `special_value_vectors` | the `levels.md` vectors that `skills::tests::real_skill_vectors` does not cover: dm12 Critical Strike (L1…99) and Dodge, dm56 Lower Resist, ln34 Amplify Damage, ln12 Bash, Sacrifice to-hit, Teleport L25 shifted 0 and L30 `mana` −5, `usmc` 0 / −1280. Each skill id (9, 13, 91, 66, 126, 96) is first checked against the parameters the vector names | `levels.md` §2, §4, §5 |

### `crates/d2-server/tests/game_world_data.rs` (3)

| Test | Checks (source) | Intended claim |
|---|---|---|
| `every_lvlprest_ds1_parses` | through `WorldFiles::load`: every distinct lvlprest file string loaded, 2,043 of them, versions 12–18, preset units type 1 × 2,267 and type 2 × 14,105, one record with flags 1, object ids ≥ 573 = 580 × 46, 581 × 135, 582 × 24; 1,054 rows set SizeX/SizeY and each of their files has that stored size (`preset.md` §5.3, §6 step 5) | none (measurements) |
| `every_act1_level_generates` | act 0 on the recorded seed: `dwStartSeed`, DRLG seed, level list 4 … 5 (recorded); then every `levels` row with `Act` 0 (id > 0) allocated, generated through `WorldTypes` and its first room streamed: no level-type error, ≥ 1 room. Prints (id, rooms, preset rooms) | `levels.md` §3 r2, r3, §4 r1 (recorded part), §5 |
| `wired_game_on_live_tables_runs_100_ticks` | `SimGame<WorldSim<_>, NoWorld>` on the live tables (stat data, unit data, skills, combat, vitals, missiles, levels, AnimData, population / init tables) and files; act 0 created, regions on the game seed, town generated and a room streamed; a Sorceress allocated there with her §1 stats (life 10240) joins; 100 `Tick::tick` calls: frame + 100, `WorldSim::errors()` empty, the player still in the game, the client has a room, life unchanged | none (integration run) |

`world_data/tests/game.rs` (drlg-data, still unrun) already holds the
recorded placement and the Den of Evil / outdoor vectors. This file
repeats only the recorded part of the placement, before it generates
every Act I level.

## 3. Findings and blind spots (to settle on the first run)

1. **`threshold(c, 99)`**: `vitals.md` says "level 99 → 3,837,739,017".
   The test reads that as experience row 100 (`threshold(c, 99)`, under
   §4.1 "row L + 1 is level L"). If the table has only 100 rows, the
   lookup panics past the table. Then the spec's "level 99" is row 99
   (`threshold(c, 98)`) and its wording needs fixing.
2. **2,043 and the patch MPQ.** `preset.md` measured the d2data / d2exp
   copies (its OQ5: the patch MPQ has no listfile). The provider reads
   through the archive set, so patch overrides apply. A differing unit
   or id count while every file parses points at OQ5, not at the
   parser. The "1,054 rows" count is read as rows with both SizeX and
   SizeY ≠ 0. If it fails alone, try "either".
3. **Op 1 stats 162, 163.** `stats.md` §6.3 lists "(162, 163 →
   maxstamina)" under op 1. The test reads it as stats 162 and 163
   having op 1, each with an entry in entries(11).
4. **fCallback list.** The spec elides the middle of the list ("7, 9, 11,
   78, 81, 83, …, 204"). The test asserts the count, the first six and
   the last.
5. **Animation speed.** §4.3 (the rate function) has no spec yet. The
   sweep uses the AnimData speed as s, which is the rate at 100 %.
6. **Wired game seams.** `Unprovided` keeps every `Pending` /
   `WorldPending` default. `NamedIds` stays default (no monteleport /
   bloodraven id; `init.md` §14.2 / §19.6 are not reached in town).
   The player's stats go through `VitalsView` with `NoHost` (the stat host
   the game would use is not wired to vitals). A non-empty `errors()`
   lists the adapter that failed. Treat it as a wiring finding before
   suspecting the tables.

## 4. Local run queue (fold into `HANDOFF.md` §5 C)

With `D2_GAME_DIR=<install>`:

```
cargo test --release -p d2-sim --test game_core -- --ignored --nocapture
cargo test --release -p d2-server --test game_world_data -- --ignored --nocapture --test-threads 1
```

Expected: `game_core` 12 passed, `game_world_data` 3 passed, 0 failed.
Look at the printed lines: AnimData records scheduled / skipped,
monstats rows with DamageRegen, the evaluated skill-value count, the
DS1 version / type / id histograms, the Act I room counts, and the
frame and message count after 100 ticks.

Record the results in `HANDOFF.md` §5 Done and §1. For each test that
passes, turn its `Intended claim` line into a `// Covers:` claim (same
IDs), then rerun `py tools/coverage.py --check`. Correct a failing
test from the observation, or turn the spec fact into an open
question (findings 1–4 first).

## 5. Gate (this branch)

`sh tools/gate.sh` after `sh tools/cloud-setup.sh`: GATE PASS (all 13
steps; the 15 new tests are listed as ignored). Non-source files in the
diff: this note only.
