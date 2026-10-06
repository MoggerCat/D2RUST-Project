# Handoff: combat and skill levels (branch `claude/impl-combat`, 2026-10-06)

Scope: implementation of `specs/combat/hit.md`, `specs/combat/damage.md`,
`specs/skills/levels.md` (+ `skillcalc.tsv`, `misscalc.tsv`) in `d2-sim`,
started from `claude/bold-ptolemy-jvyvxy` at `4c7a7c7` (cloud session,
repo only). Nothing outside `crates/d2-sim/src/{combat,skills}/`, the two
`pub mod` lines in `crates/d2-sim/src/lib.rs` (plus its doc line) and this
file was changed. No spec content, Cargo dependency, `tick`, `units`,
`game.rs` or `rng.rs` was edited.

## 1. State

**Implemented, unverified.** The three specs are drafts; their trace
checks (§5 below) have not run. Gate on this branch: `cargo fmt --all --
--check`, `cargo clippy -p d2-sim --all-targets -- -D warnings`, `cargo test
-p d2-sim` (128 passed, 3 ignored game-file tests), `cargo run -p
depcheck`, `python3 tools/spec_index.py --check`, `python3
tools/methods.py check`: all pass.

Tests cover every synthetic vector of the three specs (one disagreement,
§4 Q1), the 1.14d skills vectors re-created as synthetic records where the
spec gives enough data, the calc evaluator vectors of
`data/calc-expressions.md`, and the draw order of each §Randomness table
(seed state compared after each case). Real-data vectors are `#[ignore]`
tests (§5).

## 2. Code map

| Path | What | Spec |
|---|---|---|
| `crates/d2-sim/src/skills/mod.rs` | `SkillUnits` seam, `SkillEntry`, `SkillTables` (typed `d2_data` records + `skillscode`/`misscode` buffers), `LEVEL_CAP_114D` | `skills/levels.md` |
| `crates/d2-sim/src/skills/levels.rs` | skill level, bonus level, highest entry (§1); `special` / `miss_special`, `dm`, `ln`, mastery, formula contexts (§2); bracket, elemental / physical / missile damage, weapon mastery, concentration, kick, the two rolls (§3); mana cost / afford / consume (§4); to-hit (§5); learning and the 0x3B validator (§6) | `skills/levels.md` |
| `crates/d2-sim/src/skills/special.rs` | the 73 skills and 43 missile special values as data (`SKILL_SPECIALS`, `MISS_SPECIALS`, codes); `check_skillcalc` / `check_misscalc` compare them with the TSVs row by row (M05), perturbation test (M08) | `skills/levels.md` §2, `skillcalc.tsv`, `misscalc.tsv` |
| `crates/d2-sim/src/skills/calc.rs` | the runtime formula evaluator over a code buffer (`eval`, `run`, `CalcContext`), `rand` | `data/calc-expressions.md` §3 |
| `crates/d2-sim/src/combat/mod.rs` | `pct` (`0x00483360`), `scale`, `CombatWorld` seam, `CombatTables`, `CombatEntry`, `RoomKind` | `combat/damage.md` §0 |
| `crates/d2-sim/src/combat/hit.rs` | attack rating, defense, hit terms / chance / test, melee result flags, block chance, block / dodge / avoid / evade / weapon block | `combat/hit.md` |
| `crates/d2-sim/src/combat/damage.rs` | `DamageRecord`, `fill`, `bonuses`, `element`, `roll_in_range`, `start_combat`, `totals` (+ resistance rows), `apply`, `apply_melee`, leech, heal / add mana, stun, cold, freeze, poison, burn, element hit class, get-hit test, crushing blow, open wounds, durability | `combat/damage.md` |
| `crates/d2-sim/src/{skills,combat}/tests.rs`, `skills/fake.rs` | vectors; `Fake`: a `BTreeMap`-backed fake of every seam that logs each effect | |

## 3. Seams and public API

### Seams (traits other sessions provide)

| Trait | Methods | Expected provider |
|---|---|---|
| `skills::SkillUnits` | unit type / class; stat getters `stat` (`0x00625480`), `item_stat` (`0x00625500`), `base_stat` (`0x006253B0`), `formula_stat` (calc `stat(s, mode)`), `stat_entries` (`0x006261D0`); `has_state`, `state_stat`; `seed`; `skill_list`, `used_skill`; `current_weapon` (`0x00535BC0`), `weapon` (`0x00623990`), `item_at`, `item_is` (`0x00629BB0`), `itype_is`, `wield_type`, `item_damage`, `str_dex_bonus`, `item_flag_throw` (`0x0062BA80`), `missile_level` | units/stats session (`sim/stats.md`, `sim/stat-lists.md`, `sim/units.md`); item queries: items session |
| `skills::ManaUnits` | `shapeshifted`, `consume_charges`, `pay_life` (OQ8), `set_stat` | units/stats + skills-use |
| `skills::LearnUnits` | `is_class_skill` (`0x0056C700`), `add_skill_level` (§6.4 step 4 effects) | units/stats, `sim/intents-events.md` handler |
| `skills::KickItems` | `toggle_weapon_lists` (`0x00627910`), `boots_damage` | items / stat lists |
| `combat::CombatWorld` (extends `SkillUnits`) | game frame / expansion / difficulty, the process-wide hit-class byte; ident, mode, moving mode, monster flags (`0x005A0180`), boss / hireling / demon / undead / prime evil / revived / alignment; hostility (`0x00554200`), melee range (`0x00622870`, `0x00622C40`); shields (`0x0063C8F0`, composit part of `0x006225F0`); weapon class / hit class; montype nest (`0x0057A830`); room / town; dead; monster mode; mode conversion (`0x00645270`); item durability; player-count bonus (`0x005738F0`). Effects: `set_stat`, `unit_event` (`0x005C0C30`), `set_state`, `curse` (`0x0056E970`), state stat lists (expiry / create / set stat), `schedule_timer`, `cancel_timers`, `overlay`, `refresh_anim_rate`, `set_last_attacker`, the two monster hooks (`0x005A4390`, `0x005D6410`), `dual_wield_switch`, `combat_list`, `durability_loss`, `thorns` (`0x005D10C0`), `reaction` (`0x0057CEE0`) | units/stats (most), tick (timers), monsters branch (flags, hooks, player-count bonus), items (durability, composits), skills-use (curse, thorns) |

### Public API (for missiles/AI and skill functions)

- Hit: `hit_test(w, st, ct, a, d, bonus, missile) -> bool`,
  `melee_result(w, st, ct, a, d, bonus, range_offset) -> u16`,
  `block_or_dodge(w, ct, a, d, avoid, block) -> BlockResult` (the missile
  path maps `Evade` to `0x0200`), `hit::result::*` flags.
- Damage: `start_combat(w, st, ct, a, d, &mut rec, srcdam)` (melee roll +
  combat list), `apply_melee(w, ct, a, d)` (damage frame),
  `apply(w, ct, a, d, missile, &mut rec)` (missiles, auras),
  `totals`, `DamageRecord`, `damage::hitflag::*`.
- Event functions for the unit-event provider: `crushing_blow`,
  `open_wounds`; `element_hit_class`, `no_get_hit` / `get_hit_divisor` for
  the reaction provider.
- Skills: `skill_level`, `highest_entry`, `special`, `miss_special`,
  `eval_skill`, `eval_missile`, `to_hit` (the hit-test bonus),
  `elem_min/max/len`, `phys_min/max`, `miss_*`, `roll_physical`,
  `roll_elemental` (returns `(EType, v, len)`), `mana_cost`,
  `can_afford`, `consume_mana`, `weapon_mastery`.
- Tables: `SkillTables::from_bin(&BinSet, LEVEL_CAP_114D)`,
  `CombatTables::from_bin(&BinSet)`.

All functions take the world first (`&W` or `&mut W`) and the tables by
reference; no state is kept in the modules.

## 4. Open questions (each has a `TODO` in code naming it)

1. **Spec vector**: `damage.md` Test vectors list `pct(0x200000, 50,
   0x30000)` = 34; §0's rule gives 533 (`d = 0x30000 > v >> 4 =
   0x20000` → 64-bit `v × p / d` = 104,857,600 / 196,608). The code
   follows §0; a spec session should fix the row or the rule.
2. `levels.md` OQ2: level cap (`LEVEL_CAP_114D` = 99, caller-supplied).
3. `levels.md` OQ4: `roll_elemental` returns `(EType, v, len)`; the
   placement `0x0056C8E0` is unspecified, the caller places them.
4. `levels.md` §3.1: getter of the elemental mastery stat (unit getter
   used); whether a gated minimum still evaluates `EDmgSymPerCalc` (d2rs
   evaluates, then gates).
5. `levels.md` OQ5, OQ8: 0x3B result codes to the client; blood-mana
   payment (`ManaUnits::pay_life`).
6. `calc-expressions.md` §3.5: skills `rand` with a context but no unit
   (no draw); missile `rand` (OQ1, returns 0, no draw).
7. `hit.md` §6.4: weapon block with no matching entry (0).
8. `damage.md` §3.1 step 6: getter of the bypass stats 103/104/106 (unit
   getter); step 13: the "high nibble free" test of `0x00554650`
   (`hit_class & 0xF0 = 0`).
9. `damage.md` §3.2: `item_normaldamage` through the item/skill getter.
10. `damage.md` §4.5: leech rows have no resist stat yet take the
    difficulty penalty for non-monster defenders under a literal reading
    (monster drain on a Hell player is then ×2). Confirm.
11. `damage.md` §5.1 step 6: thorns condition read literally.
12. `damage.md` §5.3: missing attacker (leech stops after the shift).
13. `damage.md` §5.7: a new freeze list also switches state 1 on.
14. `damage.md` §8: crushing blow player-count term applied to
    non-hireling monsters only.
15. `damage.md` §9: "defender with an inventory" taken as every player.
16. `damage.md` §7 (reaction, kill) is a seam (`CombatWorld::reaction`):
    checked only at the call level in the spec (OQ3).

## 5. Checks to queue (local run queue, `docs/HANDOFF.md` §5)

1. Game-file tests (need `mpq-tool extract` output under
   `game/extracted/patch_d2/data/global/excel/`):
   `cargo test -p d2-sim -- --ignored`. Expect 3 passes:
   `codes_match_game_tables` (skillcalc / misscalc `code` columns equal
   `SKILLCALC_CODES` / `MISSCALC_CODES`; if the `.txt` has an
   `Expansion` separator or extra rows, the test filters `Expansion`),
   `real_skill_vectors` (Fire Bolt 36, Fire Ball 47, Frozen Orb 64,
   Teleport 54: `levels.md` Test vectors), `real_table_constants`
   (`charstats` ToHitFactor / BlockFactor, `difficultylevels` values). A
   failure naming a missing file means the table lives in another
   archive's folder (`d2exp`, `d2data`); adjust the path, not the
   numbers.
2. The trace checks the specs already request: `hit.md` OQ1–2,
   `damage.md` OQ1–2, `levels.md` OQ1. Replay recorded entry/exit states
   through `hit_test` / `block_or_dodge` / `start_combat` / `apply` /
   `special` with a recording-backed `CombatWorld` once the recordings
   exist (conformance session).
