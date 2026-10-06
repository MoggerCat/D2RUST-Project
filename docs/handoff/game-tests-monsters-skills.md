# Game-file tests: monsters, missiles, skills, combat vitals

> To be folded into `docs/HANDOFF.md` (§1, §4, §5 C) and `docs/PLAN.md` by a docs session; this file stays as the detailed record.

Branch `claude/game-tests-monsters-skills`, from `claude/tender-meitner-mphas3`
at `4b5b0bf`. Cloud session (repo only, no game files), medium effort (M14).
Task class: tests from specs. Test code and this note only: no spec, no
library code, no `docs/HANDOFF.md` / `docs/PLAN.md` edit.

**None of these tests has run.** They are `#[ignore]` and read the extracted
1.14d tables, so every game-tier claim below is **unverified** until the local
run in §3 passes (METHODS M02, `docs/COVERAGE.md` §3). If a test fails there,
fix the code or the test (whichever the spec says is wrong) or remove its claim
in the same session.

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
`real_recorded_ai_params` (`ai.md` §9.1 table), `real_levels_rows`,
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

## 4. Coverage (claims on `#[ignore]` tests: game tier, unverified)

`python3 tools/coverage.py --summary`, before → after (any tier unchanged
except where noted):

| spec | game-file before → after | any before → after |
|---|---|---|
| `monsters/init.md` | 3 → 44 | 111 → 111 |
| `monsters/population.md` | 0 → 15 | 158 → 158 |
| `monsters/ai.md` | 0 → 3 | 108 → 109 |
| `missiles/missiles.md` | 0 → 12 | 104 → 106 |
| `skills/levels.md` | 3 → 8 | 43 → 45 |
| `skills/use.md` | 0 → 5 | 64 → 64 |
| `combat/vitals.md` | 0 → 9 | 19 → 19 |
| total | 187 → 277 (6.9 % → 10.2 %) | 2459 → 2464 |

Read the game column as an upper bound until §3 passes.

## 5. Gate

All pass on this branch: `cargo fmt --all -- --check`;
`cargo clippy --workspace --all-targets -- -D warnings`; `cargo test -p d2-sim`
(1232 passed, 5 ignored in the lib; the two new binaries 19 + 16 ignored);
`cargo run -p depcheck`; `python3 tools/spec_index.py --check`;
`python3 tools/methods.py check`; `python3 tools/coverage.py --check` (3387
claims, 0 errors) and `--selftest`.
