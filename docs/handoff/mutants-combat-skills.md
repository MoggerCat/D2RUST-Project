# Mutation testing: combat and skills (`d2-sim`)

> To be folded into `docs/HANDOFF.md` (§1, §3, §7) and `docs/PLAN.md` by the docs session; this file stays as the detailed record.

Branch `claude/mutants-combat-skills`, from `claude/tender-meitner-mphas3`
at `4b5b0bf`. Cloud session (repo only, no game files). Task class:
tests from specs, medium effort (M14), method M08 (prove the check can
fail). It adds tests and test-fixture changes only; no non-test code
changes, and no spec, `docs/HANDOFF.md` or `docs/PLAN.md` edits.

## 1. Run and counts

Tool: `cargo-mutants` 27.1.0, toolchain 1.99.0. Command (`mutants.out`
kept in the session scratchpad, not committed):

```
cargo mutants -p d2-sim --file 'crates/d2-sim/src/combat/**' \
  --file 'crates/d2-sim/src/skills/**' -j 3 --timeout 60 \
  --copy-target=true --no-shuffle -- --lib
```

Then the same command with `--iterate`, run twice, to re-test only the
survivors against the new tests. The first run took 2 h for 1940
mutants. The two `--iterate` passes took 28 min and 8 min.

| | Mutants | Caught | Missed | Timeout | Unviable |
|---|---|---|---|---|---|
| Before (base `4b5b0bf`) | 1940 | 1544 | 358 | 5 | 33 |
| After (this branch) | 1940 | 1855 | 47 | 5 | 33 |

A timeout is an infinite loop that the tests detect, so it counts as
caught. The 5 are:
- `calc.rs` `run`: 3 position-step mutants;
- `level_from_exp`: `i += 1` → `*=`;
- `durability`: `(i + 1) % 7` → `*`.

The missed rate among viable mutants went from 18.8% (358 / 1907) to
2.5% (47 / 1907). All 47 remaining are equivalent or unobservable (§3).

Missed by file:

| File | Before | After |
|---|---|---|
| `combat/damage.rs` | 201 | 28 |
| `combat/hit.rs` | 29 | 6 |
| `combat/mod.rs` | 4 | 0 |
| `combat/vitals/mod.rs` | 19 | 8 |
| `skills/calc.rs` | 2 | 1 |
| `skills/levels.rs` | 65 | 1 |
| `skills/use_/mod.rs` | 34 | 3 |
| `skills/use_/table.rs` | 4 | 0 |

## 2. Tests (disposition a)

Each surviving mutant that the spec decides got a test asserting the
spec's outcome at the boundary or branch that the mutant changed. 105
tests:
- `crates/d2-sim/src/combat/mutant_tests.rs`: 64 tests, including the
  `vitals_mutants` submodule with its own `VitalsUnits` fake;
- `crates/d2-sim/src/skills/mutant_tests.rs`: 41 tests, including the
  submodules `use_mutants` and `table_check_mutants`.

Areas covered:
- **damage.md**:
  - `pct` thresholds (§0);
  - `roll_in_range`, `bonuses` (§3.2–§3.3);
  - `fill`: the crit draws, drains and presets, bypass flags, poison
    mastery and override, burn length, event 3, conversion including
    `rand`, monster crit (§3.1);
  - `start_combat`: the avoid mask and the will-die compare (§3);
  - `totals`: the damage-percent rows, DR pierce, resist rules, lengths,
    `no_absorb`, a negative field (§4);
  - leech: the drain column, the hireling rule, overlays, the monster
    rule (§5.3);
  - cold and freeze (§5.6–§5.7);
  - `apply` room rules and subtraction bounds (§5.2);
  - `apply_melee` overlay and thorns (§5.1);
  - hit class and get-hit (§6);
  - crushing blow and open wounds (§8);
  - the durability slot walk (§9).
- **hit.md**:
  - Holy Shield gating (§2);
  - negative hit terms (§3.3);
  - fractional AC and the demon / undead to-hit (§3.2);
  - prevent-heal targets (§3.5);
  - `melee_result` gates, range and miss (§4);
  - dodge for moving monsters and the weapon-block draw (§6.2).
- **vitals.md**:
  - the class columns of `experience`, `level_from_exp`, the target
    level (§1, §4.1);
  - message 0x3A count 100, a negative energy, vitality stamina (§2).
- **levels.md**:
  - `highest_entry` and the skill-tab layer (§1);
  - the skill and missile formula contexts (`calc-expressions.md`
    §3.4–§3.5);
  - special values par5/par6, the missile ranges, the masteries and
    the shifts (§2);
  - brackets, elemental length and synergy gate, mastery stats, the
    `SrcDam` weapon source, the missile-unit rules (§3);
  - the throw path of weapon mastery (§3.5);
  - shifted cost and `can_afford` (§4);
  - skill requirements (§6).
- **use.md**:
  - resync after 25 frames, own-inventory items, the 0x07 and hold
    handlers (§1);
  - dual wield (§2);
  - the arrived flag (§5.2);
  - native entries, TargetableOnly with an ally, the were-form mana
    check, town and line of sight 5 in the start core (§5.3);
  - the do core: used-skill level, mana at do, ItemEffect 1, an invalid
    srvmissile, the missile flag and aim (§5.4).
- **functions.tsv check**: a repeated slot and bad index ranges. These
  are M08 perturbations of the check, so they carry no claim.

**Claims.** One new claim: `specs/data/calc-expressions.md §3.1 r2`, on
`special_mastery_types`. Its formulas at offsets 3 and 6 show that
evaluation runs from the offset to the buffer end.

The other tests repeat rules that are already claimed. They add
boundary cases, and a second claim on the same rule adds nothing to the
metric. The rule stays claimed only where the assertions check its
outcome (`docs/handoff/coverage-claims.md` §1).

Coverage: `calc-expressions.md` unit tier 41 → 42 (any tier 46 → 47).
The repository goes from 2459 to 2460 rules covered (any tier, 91.0%).

**Test-fixture changes** (test-only code):
- `skills/fake.rs` (`Fake`), three new fields, each defaulting to the old
  behavior:
  - `range_needed: Option<i32>`: when set, in melee range only for that
    range argument;
  - `shifted: bool`: drives `ManaUnits::shapeshifted`;
  - `revived` / `prime_evil` on `FUnit`: drive `is_revived` and
    `is_prime_evil`.
- `skills/use_/mod.rs`: `mod tests` became `pub(crate) mod tests`.
  - In `use_/tests.rs`, these became `pub(crate)`: the fakes `U` and `F`
    and their fields, `native`, `rec`, `tables`, `Edit`, `mana`,
    `point`, `unit_msg`, `caster`, `dual_wielder`.
  - No test body changed.
- One `#[cfg(test)] mod mutant_tests;` line each in `combat/mod.rs` and
  `skills/mod.rs`.

## 3. Equivalent or unobservable (disposition b), 47 mutants

| Mutant (file:line) | Why no test can tell it apart |
|---|---|
| `damage.rs:206` `max > min` → `>=` (`roll_in_range`) | `roll(0)` returns 0 without a step (`rng.md` §3) |
| `damage.rs:227` `max < 8` → `==`, `<=` (`element`) | `max = stat << 8` is a multiple of 256, never 8; for `max ≤ 0`, `roll_in_range` adds nothing either |
| `damage.rs:306` `max > 0` → `>=` (`bonuses`) | after step 2, `min ≥ 1` and `max > min` (or `min + 256`), so `max` is never 0 |
| `damage.rs:318` `maxT > minT` → `>=` | `roll(0)`, as above |
| `damage.rs:356`, `:368` demon / undead `p > 0` → `>=` | adds 0 |
| `damage.rs:460:74` `(Monster && hireling)` → `\|\|` | reached only by non-monsters and hireling monsters. Differs only for a non-monster that the hireling test `0x0063EE90` calls a hireling, which the spec ("a hireling of any type") does not define. Seam fact, not a d2-sim rule |
| `damage.rs:482` `pmax > 0` → `>=` | with `pmax = 0`, `roll_in_range` adds nothing, whatever the mastery |
| `damage.rs:496` `poison_count > 1` → `>=` | divide by 1 |
| `damage.rs:605` three `\|` → `^` (avoid mask) | dodge, avoid, evade and weapon block are distinct bits |
| `damage.rs:911`, `:915` DR `> 0` → `>=` | `pct(0, …) = 0` |
| `damage.rs:923` scaled field `> 0` → `>=` | `pct(0, …) = 0` |
| `damage.rs:933` cold / freeze length `> 0` → `>=` | only a length ≤ 0 is newly halved or zeroed, and rows 5 and 6 then set any length ≤ 0 to 0 (§4.6 step 1) |
| `damage.rs:942`, `:945` poison / burn length `> 0` → `>=` | sets 0 to 0 |
| `damage.rs:980` `v > 0` → `>=`, `:986` absorb `% > 0` → `>=` | `pct(0, …) = 0`, `pct(v, 0, …) = 0` |
| `damage.rs:1199` cold `effect < 0` → `<=` | effect 0 already returned (§5.6 step 2) |
| `damage.rs:1328`, `:1333` `HIT \| WILL_DIE` → `^` | distinct bits |
| `damage.rs:1473:23` thorns `== Object` → `!=` | the condition differs only for a player or monster in mode 0, which returned at §5.1 step 4.1 when hit |
| `damage.rs:1508` `base \| nibble` → `^` | base is the weapon class (low nibble), the element is the high nibble |
| `damage.rs:1605` crushing `dr > 0` → `>=` | `pct(x, 0, 100) = 0` |
| `hit.rs:111` `base > 0` → `>=` | base 0 gives bonus 0 on both branches |
| `hit.rs:139` `def < 0` → `==`, `<=` | §3.3's first step never changes the factor (see below) |
| `hit.rs:143` `toHit < 0` → `<=`, `:147` `def < 0` → `<=` | at 0 both subtract or set 0 |
| `hit.rs:225` fractional `f > 0` → `>=` | `pct(def, 0, 100) = 0` |
| `vitals/mod.rs:190` `act < 5` → `<=` | `act` is clamped to 4 first |
| `vitals/mod.rs:278`, `:300`, `:309` `n > 0` → `>=` | `n = 0` adds 0 |
| `vitals/mod.rs:282`, `:303`, `:312` clamp `>` → `>=` | clamping at equality writes the same value |
| `vitals/mod.rs:409` `dlvl > 0` → `>=` | the branch needs `dlvl > alvl ≥ 25` |
| `calc.rs:31` `offset < len` → `<=` | running an empty slice returns 0, like the out-of-range path |
| `levels.rs:150` `DM` `v > b` → `>=` | `v = b` returns `b` either way |
| `use_/mod.rs:678` delete arm `DT \| GH \| BL \| DD` | that arm and the fallback `_` both return false |
| `use_/mod.rs:794` delete arm `Player` (`skill_mode`) | the fallback `_ => anim` returns the same value |
| `use_/mod.rs:1087` delay `d > 0` → `>=` | `set_delay` returns at `d = 0` |

Why `hit.rs:139` changes nothing (for values without 32-bit overflow):
- with `toHit ≥ 0` and `def < 0`, the factor is 100 whether `def` is
  moved into `toHit` or zeroed at the last step;
- with `toHit < 0`, both paths end with `def = def − toHit` and
  `toHit = 0`.

So §3.3's first step never changes the outcome. A wrap at `i32::MIN` was
not searched. The `def < 0` → `==` mutant at `:147` **is** caught, by AR
= `i32::MIN`.

## 4. Code against the spec (disposition c)

No deviation found: no non-test code changed. Two readings for the spec
owner:
- **`damage.md` §3.1 step 6** says "a hireling of any type". The code
  asks the hireling test only for monsters. If `0x0063EE90` can be true
  for another unit type, that is a difference, otherwise not (§3 row
  `460:74`).
- **`hit.md` §3.3**: the first `def < 0` step is redundant in its
  outcome (§3). This is not wrong; worth a line in the spec's
  edge-case list if confirmed.

## 5. Gate (this branch, cloud)

All passed:
- `cargo fmt --check`;
- `cargo clippy --workspace --all-targets -- -D warnings`, after
  `sh tools/cloud-setup.sh`;
- `cargo test -p d2-sim`: 1337 passed, 5 ignored, plus 1 ignored
  doc-test;
- `cargo run -p depcheck`: 8 crates OK, determinism lint clean;
- `python3 tools/spec_index.py --check`;
- `python3 tools/methods.py check`: 21 methods OK;
- `python3 tools/coverage.py --check`: 3260 claims, 0 errors;
- `python3 tools/coverage.py --selftest`.

## 6. Lessons

- **`pgrep -f` / `pkill -f` match their own shell.** A loop waiting on
  `pgrep -f "cargo install …"` never ended, and a `pkill -f "cargo
  mutants"` killed the session's own shell. Match on the process name
  (`pgrep -x cargo-mutants`) instead.
- **Running `cargo install` beside the toolchain's first install
  corrupted the 1.99.0 toolchain.** rustup lost its `rust-std`; a later
  `rustup` call repaired it. Let `tools/cloud-setup.sh` (or the first
  `cargo` call) finish before starting a second `cargo`.
- **A full mutation run here takes about 2 h for about 1900 mutants**
  with `-j 3` on a 4-core container. A background command is stopped
  after 2 h, so start the run detached (`setsid nohup … &`) and use
  `--iterate` afterwards: it re-tests only the survivors (tens of
  minutes).
