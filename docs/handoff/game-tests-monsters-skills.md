# Game-file tests: monsters, missiles, skills, combat vitals

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Branch `claude/game-tests-monsters-skills`, from `claude/tender-meitner-mphas3`
at `4b5b0bf`. Cloud session (repo only, no game files), medium effort (M14).
Task class: tests from specs. Test code and this note only: no spec, no
library code, no `docs/HANDOFF.md` / `docs/PLAN.md` edit.

**None of these tests has run: every expected value is unconfirmed.** They
are `#[ignore]` and read the extracted 1.14d tables. Following the HANDOFF §8
lesson on blind-written game assertions (merged from
`claude/local-2026-10-06`), the tests carry **no `Covers:` claim**; the
intended claims are listed in §4 and are added by the session whose local
run (§3) passes. If a test fails, fix the side the spec says is wrong (code,
test or spec) before adding its claims.

## 1. What was added

| File | Tests | What |
|---|---|---|
| `crates/d2-sim/tests/game_common/mod.rs` | – | loader shared by both files: `D2_GAME_DIR/extracted/patch_d2/data/global/excel/<name>.bin`, the same path as the in-crate `skills::tests::game_loader` |
| `crates/d2-sim/tests/game_monsters.rs` | 19 | `monsters/init.md`, `monsters/ai.md`, `monsters/population.md` |
| `crates/d2-sim/tests/game_skills.rs` | 16 | `skills/levels.md`, `skills/use.md`, `missiles/missiles.md`, `combat/vitals.md` |

Each file has its own `InitHost` (`Host`), `SkillUnits` (`Bare`: no unit
state) or `VitalsUnits` (`Player`: base stats only) on the public API. Expected
values come only from the specs: recorded vectors, the "Real 1.14d" tables,
values the Constants and Rules sections state, or an invariant a rule states.

### 1.1 `game_monsters.rs`

Sweeps (no panic, plus the stated invariant):

- `stats_and_skills_every_class_level_difficulty`: every monstats row × every
  level id 0..=row count × difficulty 0–2 (expansion, game type 3, 1 player):
  level per §7, every stat of §6 step 6, HP within the §8 range (cap
  0x7FFFFF) and maxhp = hitpoints, AC and XP of §8.1, hpregen per §6 step 10,
  components below their counts (§10 step 2), the exact `give_skill` calls of
  §6 step 14 with the `MonsterSkillBonus` of the game difficulty, unit flags
  of step 15.
- `type_init_every_class`: every row × difficulty, expansion and classic: the
  §5 steps (flags 0x0A, class, level id, AI alloc and install state 0, quest
  chain attach for mode neutral).
- `every_umod_on_every_class`: every `umods.tsv` id on every row, unique and
  not, every difficulty: an `init_fn` `-` umod, and a `unique_gate` `yes` umod
  called with unique 0, change no stat, no monster data and call nothing.
- `choose_umods_every_class`: `eligible` for every row × umod id (and past
  the table); `choose_umods` per row, difficulty, champion allowed or not: a
  champion gets ≤ 1 umod from {16, 36–39}; a unique ≤ 1 + d, distinct, each
  with `upick` > 0, `champion` 0, eligible.
- `level_stats_every_row`: `monster_level` / `stats_by_level` for every row,
  level id −1..=count, d, L-flag; out-of-range ids give 1 (§7 step 2); levels
  past monlvl clamp (§8.1).
- `ai_index_of_every_row`: every monstats `AI` < 148; per-index row counts
  equal `ai-functions.tsv` `monstats_rows`; each named row uses that index.
- `regions_every_level`: `Regions::create` for every difficulty × expansion
  flag × 3 game seeds: one game-seed step, a region per level id, every §2.2
  field equals its levels column, entries ≤ min(NumMon, 13, list), each from
  the difficulty's list and `isSpawn`, rarity total = sum, variants within the
  component counts; Blood Moor 3 entries, total 6; Cold Plains keeps 3 of 4.

Vectors and table facts: `real_resistances_and_block` (6 classes × 3
difficulties), `real_umod_constants` (K, `MonsterSkillBonus` 0/3/7,
`ChampionDamageBonus` 90/75/66, the champion candidates),
`real_brute1_normal_mods` (BaseId 24 → [13], recorded), `real_brute1_champion_pack`
(recorded [13, 16], champion + unique), `real_champion_fallenshaman1` ([16]
and ghostly [36] vectors), `real_unique_fallen1`, `real_boss_hp_factors` (NM /
Hell factors), `real_component_counts` (recorded zombie1 counts),
`real_recorded_ai_params` (`ai-bodies.md` §9.1 table), `real_levels_rows`,
`real_monstats_population_rows` (Rarity, groups, parties, placespawn,
sparsePopulate, superuniques 0–9, monumod row 0), `real_level_list_facts`
(§2.3 step 3).

### 1.2 `game_skills.rs`

Sweeps: `every_skill_formula_evaluates` (all 36 `skillscode` fields of every
skills row, levels 0..=99, no unit and a bare player; −1 → 0),
`every_missile_formula_evaluates` (7 `misscode` fields of every missiles row),
`every_skill_value_evaluates` (all 73 skill and 43 missile special codes, the
damage / length / to-hit / mana functions; shifted mana ≥ 0),
`every_skill_function_in_table` (every skills `srvstfunc`, `srvdofunc`,
`srvprgfunc1–3`, `ItemEffect` and states `srvactivefunc` is a filled slot of
`table::FUNCS`; referenced slots = the `mapped` ones; unreferenced = 53, 138,
142), `every_missile_function_in_catalogue` (server-do / server-hit /
server-damage in range and non-null; per-index counts = `srvdo.tsv` /
`srvhit.tsv` `rows`; 551 / 53 / 591), `every_missile_flags_dword` (§R1.2
bits), `every_missile_creation_velocity` (§R2.3 steps 5, 7, levels 0..=99),
`every_class_every_level` (vitals: every class, acts 0–5, levels 2–99: §3
gains, thresholds bracket each level).

Vectors and table facts: `real_level_vectors` (Critical Strike, Dodge, Lower
Resist, Amplify Damage, Bash specials; Teleport `usmc` / `mana`; Fire Bolt
`mana`; Sacrifice to-hit; Tornado physical), `real_skills_table_facts`
(`levels.md` Constants counts and mana columns), `real_use_table_facts`
(`use.md` Constants flag counts, delays, Might perdelay),
`real_use_vectors`, `real_missiles_record_layout` (684 × 420, row 568
0xFDB4), `real_recorded_missiles` (§R10 table, speeds 1,920 / 1,536 / 3,840 /
4,608, lifetimes), `real_charstats_and_experience`, `real_vitals_vectors`.

Not covered here: `combat/hit.md` and `combat/damage.md` (their live-table
facts, charstats `ToHitFactor` / `BlockFactor` and the difficultylevels
columns, are already checked by the in-crate `combat::tests::real_table_constants`;
the block / hit vectors need a `CombatWorld` provider); the synergy vectors
of `levels.md` (need a unit with skill levels) and Kick; the population
recordings (no positions or RNG, as the spec says).

## 2. Readings and spec questions found

These are places where a test had to read the spec; the local run decides.

1. **evilhut row: 528 or 529.** `population.md` Test vectors says
   "sparsePopulate rows | 528 evilhut = 40 only"; `ai-functions.tsv` pairs
   `529 evilhut`. The test checks only "exactly one row, value 40". Owner:
   `population.md` (spec edit after the local run prints the row).
2. `init.md` "Real 1.14d values" gives one `block` per class after three
   difficulty sets; `real_resistances_and_block` reads it as all three.
3. `population.md` Real: "MonDen 600" (Den of Evil), "1056" (Crypt /
   Mausoleum) and "MonDen 0" (town) have no "×3": only Normal is checked.
   `rangedspawn` "set only on" levels 110–119, 123–131, 135 is checked as
   exactly those levels. `WarpDist` 2025 is checked on every level with
   `Act` 0.
4. `levels.md` Constants "SrcDam 63" is read as 63 rows with SrcDam ≠ 0;
   "skpoints 0" as no row with a `skpoints` formula. `use.md` "periodic 2
   (Thunder Storm, Blade Shield)" as rows {57, 277}.
5. `every_skill_function_in_table` reads `functions.tsv` status `mapped` as
   "referenced by some live row" (the inverse of `unreferenced`).
6. `every_missile_function_in_catalogue` requires a non-zero
   `pSrvDmgFunc` to name a filled server-damage slot (1–14, §R9.1); §R9 does
   not say a live row never names a null one.

## 3. Local run queue entry (for `docs/HANDOFF.md` §5 C)

Needs `game/` with the extracted tables (`mpq-tool extract`, the same as the
existing `real_skill_vectors`):

```
D2_GAME_DIR=<install> cargo test -p d2-sim --test game_monsters -- --ignored
D2_GAME_DIR=<install> cargo test -p d2-sim --test game_skills -- --ignored
```

Expected: `game_monsters` 19 passed, `game_skills` 16 passed, 0 failed. For
speed, `--release` is fine (the sweeps run about 300,000 stats inits and
1.5 M formula evaluations). On a failure: print the failing row and fix the
side the spec says is wrong; for §2 item 1 record the evilhut row in
`population.md`. Then record the result here and in §5 Done, and only then
count the claims as verified.

## 4. Claims to add after the local run (not in the code yet)

Add each line as `// Covers: <specs>` above its test once that test passes
locally (game tier). Measured with the claims in place, before the merge of
`claude/local-2026-10-06`, they raised the game tier from 187 to 277 units
(total `coverage.py --summary`); without them coverage is unchanged.

| Test | Covers |
|---|---|
| `game_monsters::stats_and_skills_every_class_level_difficulty` | specs/monsters/init.md §6 r1, §6 r4, §6 r5, §6 r6, §6 r7, §6 r8, §6 r9, §6 r10, §6 r14, §6 r15, §7 r1, §7 r2, §7 r3, §9 r1, §9 r3, §10 r2 |
| `game_monsters::type_init_every_class` | specs/monsters/init.md §5 r1, §5 r2, §5 r3, §5 r5, §5 r6, §5 r7 |
| `game_monsters::real_resistances_and_block` | specs/monsters/init.md §6 r6 |
| `game_monsters::real_umod_constants` | specs/monsters/init.md §9 r3, §17 r1, §17.1, §19 text |
| `game_monsters::real_brute1_normal_mods` | specs/monsters/init.md §14.1 |
| `game_monsters::real_brute1_champion_pack` | specs/monsters/init.md §16.2 r1, §16.2 r2, §16.2 r3, §16.2 r4 |
| `game_monsters::real_champion_fallenshaman1` | specs/monsters/init.md §16.1 r3, §18 r1, §18 r2, §19.1, §19.2, §19.6 |
| `game_monsters::real_unique_fallen1` | specs/monsters/init.md §16.1 r2, §16.1 r3, §17 r2, §17 r3, §17.2, §18 r1, §18 r2, §19.1 |
| `game_monsters::real_boss_hp_factors` | specs/monsters/init.md §19.1 |
| `game_monsters::every_umod_on_every_class` | specs/monsters/init.md §19 text |
| `game_monsters::choose_umods_every_class` | specs/monsters/init.md §17 r1, §17 r2, §17 r3, §17.1, §17.2, §17.3 r1, §17.3 r2, §17.3 r3 |
| `game_monsters::real_component_counts` | specs/monsters/init.md §10 r2 |
| `game_monsters::level_stats_every_row` | specs/monsters/init.md §7 r2, §8.1 |
| `game_monsters::ai_index_of_every_row` | specs/monsters/ai.md §10, §4 |
| `game_monsters::real_recorded_ai_params` | specs/monsters/ai-bodies.md §9.1 |
| `game_monsters::real_levels_rows` | specs/monsters/population.md §2.2, §3.1 r1 |
| `game_monsters::real_monstats_population_rows` | specs/monsters/population.md §4 r4, §7 r1, §10.1 r1, §11.4 text |
| `game_monsters::real_level_list_facts` | specs/monsters/population.md §2.3 r3 |
| `game_monsters::regions_every_level` | specs/monsters/population.md §2.1 r1, §2.1 r2, §2.2, §2.3 r1, §2.3 r2, §2.4 r1, §2.4 r2, §2.4 r3, §2.4 r5 |
| `game_skills::every_skill_formula_evaluates` | specs/skills/levels.md §2 |
| `game_skills::every_missile_formula_evaluates` | specs/skills/levels.md §2 |
| `game_skills::every_skill_value_evaluates` | specs/skills/levels.md §2, §3.1 text, §3.2, §3.3 text, §3.4, §4, §5 |
| `game_skills::real_level_vectors` | specs/skills/levels.md §2, §3.3 text, §4, §5 |
| `game_skills::real_skills_table_facts` | specs/skills/levels.md §4 |
| `game_skills::every_skill_function_in_table` | specs/skills/use.md §8 |
| `game_skills::real_use_table_facts` | specs/skills/use.md §6, §7 |
| `game_skills::real_use_vectors` | specs/skills/use.md §5.1, §5.2 text |
| `game_skills::real_missiles_record_layout` | specs/missiles/missiles.md §r1-data-the-server-keeps-per-missile r1, §r1-data-the-server-keeps-per-missile r3 |
| `game_skills::every_missile_flags_dword` | specs/missiles/missiles.md §r1-data-the-server-keeps-per-missile r2 |
| `game_skills::every_missile_function_in_catalogue` | specs/missiles/missiles.md §r3-per-tick-dispatch r1, §r3-per-tick-dispatch r7, §r9-1-tables-dumped-from-game-exe-1-14d-confirmed, §r9-2-tsv-columns-srvdo-tsv-srvhit-tsv |
| `game_skills::real_recorded_missiles` | specs/missiles/missiles.md §r10-behaviour-of-the-recorded-missiles text, §r10-behaviour-of-the-recorded-missiles r5, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r5, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r7, §r7-lifetime-and-expiry r2 |
| `game_skills::every_missile_creation_velocity` | specs/missiles/missiles.md §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r5, §r2-3-steps-in-order-0x0059fa30-1-14d-confirmed r7 |
| `game_skills::real_charstats_and_experience` | specs/combat/vitals.md §4.1 |
| `game_skills::real_vitals_vectors` | specs/combat/vitals.md §1, §2 text, §3 r1, §3 r2, §3 r3, §3 r4, §3 r5, §3 r6 |
| `game_skills::every_class_every_level` | specs/combat/vitals.md §1, §3 r1, §3 r2, §3 r3, §3 r4, §3 r5, §3 r6, §4.1 |

## 5. Gate

All pass on this branch: `cargo fmt --all -- --check`;
`cargo clippy --workspace --all-targets -- -D warnings`; `cargo test -p d2-sim`
(1232 passed, 5 ignored in the lib; the two new binaries 19 + 16 ignored);
`cargo run -p depcheck`; `python3 tools/spec_index.py --check`;
`python3 tools/methods.py check`; `python3 tools/coverage.py --check` and
`--selftest`. Base merged: `origin/claude/tender-meitner-mphas3` at `5edceb8`
(the local group C results); none of its observations touches a value
asserted here (its live counts skills 357 and monstats 734 match).
