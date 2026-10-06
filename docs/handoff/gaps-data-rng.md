# Handoff: gap tests for data specs, rng and the `r0` claim — `claude/gaps-data-rng`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud implementation session, 2026-10-06. Task class: tests from specs plus
a small tool change, medium effort (METHODS M14). Base: `main` at `edad871`.
The session had the repo only, synthetic data and no game files (M09). This
note is for the coordinator to fold into `docs/HANDOFF.md` / `docs/PLAN.md`;
neither file is edited here.

## 1. State

- **Tool, HANDOFF §2 step 6(b):** `tools/coverage.py` accepts `r0` in the
  claim grammar.
  - `RULE_REF` now allows `r0`. `r00` and `r01` stay malformed.
  - The rule IDs are unchanged. A list numbered from `0.` still gives
    `§s r0`, and the `1.` after it still starts list 2 (`§s l2 r1`). So no
    existing claim moved.
  - Selftest: a synthetic spec with a 0-list. `§1 r0` and `§1 r0, §1 l2 r1`
    pass, `r00` / `r01` are malformed, and `§1 l2 r0` is dangling.
  - M08: the selftest fails on the old grammar (checked by hand).
- **The two claims this enables:**
  - `price::rule_zero` (`crates/d2-sim/src/world/vendors/tests/price.rs`)
    now also claims `vendors.md §9.2 r0`. Its assertions check every
    outcome of r0: null item → 0x7FFFFFFF; repair of a non-repairable item
    → 0 (unidentified, ethereal, a potion with no durability); flag 0x20000
    → 1. The "charges not all full" branch of repairable is not exercised
    separately; drop the claim if that counts as part of the outcome.
  - `rare_name_pick` (`crates/d2-sim/src/items/tests/gaps_affixes.rs`) now
    also claims `affixes.md §5 r0`. That "item" is the wrapped line "0. Else
    r := roll(count) …; return the rare id of candidate r. No weights.",
    and the test checks the unweighted roll over the candidates.
- **d2-data gap tests,** 14 tests in two new test modules declared in
  `crates/d2-data/src/lib.rs` (`#[cfg(test)]` only). All use synthetic
  data, are unit tier and pass.
- **Coverage, any tier (before → after):**

  | spec | before | after |
  |---|---|---|
  | `callbacks.md` | 38/41 | 41/41 |
  | `runtime-maps.md` | 24/25 | 25/25 |
  | `fixups.md` | 29/36 | 34/36 |
  | `loading.md` | 32/47 | 33/47 |
  | `affixes.md` | 44/46 | 45/46 |
  | `vendors.md` | 105/108 | 106/108 |

  `py tools/coverage.py --summary` total: 2,413 / 2,704 any tier (89.2%),
  unit 2,359. The verified count is unchanged (216).
- **`sim/rng.md`:** no new claim. The 7 open units can't honestly be
  claimed by a test in the rng module (§4).

## 2. Code map

| File | What |
|---|---|
| `tools/coverage.py` | `RULE_REF` accepts `r0`; selftest case for a 0-numbered list |
| `crates/d2-data/src/gaps_loading_tests.rs` | `loading.md` tests:<br>• `load_applies_post_load_and_count_checks` (§4.2 r3) builds a synthetic `patch_d2.mpq` with every runtime table. One §8 or §10.8 rule broken → `LoadError::Check` on that table; a wrong record size → `SizeMismatch`.<br>• `links_point_to_earlier_steps` and `treasure_class_list_order` check parts of §7.3 and §10 r6 (no claim, §4). |
| `crates/d2-data/src/gaps_patch_fixup_tests.rs` | Claims on `fixups.md` §2 r1, §2 r2, §4, §7, edge cases; `runtime-maps.md` edge cases; `callbacks.md` §7, §9, edge cases.<br>No claim: `patch_versioning_and_determinism` (patch-layers §11) and `stat_op_source_may_be_its_own_target` (fixups §2 text), see §4. |
| `crates/d2-data/src/lib.rs` | The two `#[cfg(test)] mod` lines |
| `crates/d2-sim/src/world/vendors/tests/price.rs`, `crates/d2-sim/src/items/tests/gaps_affixes.rs` | One claim comment each (§1) |

## 3. Signature changes

None. No public item changed; only test modules, test comments and the tool.

## 4. Rules left unclaimed, and why (seams reached)

- **`sim/rng.md`:**
  - §3 r6 is a calling convention (ECX/EDX fastcall). It can't be
    observed in Rust.
  - §5.2–§5.5, §6 and §7 are each one unit holding a cross-reference
    table of every system's seeds. The facts are tested by their owner
    specs' tests (for example drlg `levels.rs` / `rooms.rs` check every
    row of §5.4 with the recorded vectors), but no single test checks a
    whole table.
  - §5.5 (automap and particle seeds) is client-only and not implemented.
  - Proposal (spec work): number the rows of §5.2–§5.4 and §7 as items.
    The system tests can then claim them one by one. §6 restates what each
    system spec owns ("what they do with `lo'` belongs to each system's
    spec") and could leave the rules sections.
- **`loading.md`:**
  - Not reproduced: §1 r3 (`expfield.d2` is read nowhere) and §3.2 text /
    r2 / r3 (`-txt` mode, d2rs policy r5).
  - Owned elsewhere or one big unit: §7.3 (deviation D1), §7.4 (the
    fix-up rows belong to `fixups.md` / `runtime-maps.md`), §9 (the affix
    and token combined arrays are in d2-sim).
  - Partly implemented: §10 r6 (the automatic TCs' item lists, "level L
    holds items with level in (L−3, L]", exist nowhere) and §10 r7 (DS1
    Expansion-row handling is not in d2-data).
  - Prose, or outside d2-data: §rules text, §d2-data-policy text, r4
    (`Ruleset`), r5 (D3), r6 (survey tools in `tools/`).
- **`schema.md`:** §3 r1–r3 describe how the field lists were extracted
  from Game.exe. Only a binary check could test them.
- **`patch-layers.md`:**
  - Not in d2-data: §1 (pipeline / `Ruleset`), §10 (`tools/data-tool`
    CLI), §12 (deferred; most items don't exist yet).
  - §8 has a deviation (D2).
  - §11: the test checks the version gate (P03, S02), unknown and deferred
    statements → P04, applying twice gives the same result, and all 5,040
    orders of a 7-statement layer. It makes no claim: the section also
    holds an untestable policy bullet, and see D4.
  - Edge cases are 1.14d table facts (a game-file test, not written so
    that nothing counts as verified before a run).
- **`fixups.md`:**
  - §2 text: the test checks +0x13E–0x13F stay 0 and a stat being its own
    op target, but not "after the count check" (load order). No claim.
  - §11 text is a 1.14d survey (game files).
- **`txt-format.md` §1** and **`calc-expressions.md`** §3.1 text, §3.1 r2
  and policy r3 / r5: prose or a game check. The evaluator lives in
  `d2-sim::skills::calc`, which this task's files don't include.
- **`field-types.md`** has no unit without a claim (any tier 20/20), so no
  test was added.

## 5. Deviations found (not fixed: outside the owned files or not unambiguous)

- **D1 `loading.md` §7.3 vs `specs/data/fields.tsv`.**
  - `chartemplate.class` links `playerclass.code`, but §7.3 lists
    chartemplate under "all others — none".
  - `setitems.set` links `sets.index`, but the setitems row doesn't list
    `sets`.
  - Both targets load earlier, so the compile order stays valid. Fix in
    the spec (spec session).
- **D2 `patch-layers.md` §8, report order "B (table)".**
  `patch.rs` `Finding::sort_key` class 2 is
  `(class, 0, 0, 0, code, table, …)`, so B findings sort by code before
  table: B03 `aaa` and B01 `zzz` come out as [B01 zzz, B03 aaa]. A local
  code fix (key on table before code) if the spec's "(table)" means table
  first. Owner: `patch` module.
- **D3 `loading.md` d2-data policy r5 vs `bin::load`.** r5 requires
  `d2exp.mpq`. `bin.rs:244` sets `lod = set.has_archive("d2exp.mpq")` and
  loads a classic set without error, skipping `expansionstring.tbl` and the
  hireling check. `d2_formats::REQUIRED_MPQS` lists d2exp, but `load` never
  enforces it.
- **D4 `patch-layers.md` §11 order independence.** The valid layer
  `set #2 ax2 lvl 5 -> 6` / `set #2 ax2 code ax2 -> ax3` gives A05 when
  reversed (the row label differs after the rename). That follows §4, but
  A05 is not in §11's list of allowed failures. Either add A05 to the list
  or give the label check after an index-selector rename a rule (spec
  session).
- **D5 `loading.md` §8 automap names vs `bin.rs` `post_load_check`.**
  - The spec: a value starting with `0` gives 0 without a compare, and an
    empty value fails.
  - The `.bin` check treats an empty LevelName / TileName as known and has
    no `0` case. `fixup/maps.rs` `automap_name` follows the spec.
  - Ambiguous: the spec text describes the compile-time conversion, while
    `post_load_check` reads `.bin` records. Whether a `.bin` can hold an
    empty or `0…` name is a game-file question (local check below).
  - The existing claim of `bin.rs` `automap_names` on §8 doesn't exercise
    either case.

## 6. Questions

1. D1, D4: spec rows or code? (spec session)
2. D2: does "B (table)" mean table first? If yes, a one-line fix in
   `Finding::sort_key`.
3. rng.md: number the cross-reference tables (§4) so system tests can
   claim rows?

## 7. Local checks to queue

- **D5:** `cargo run -p data-tool -- tables`, then look in the 1.14d
  `automap.bin` for any record whose LevelName or TileName is empty or
  starts with `0`. Expected: none, which would make the stricter `.bin`
  check harmless. Any such record shows which behavior is right.

## 8. Gate (this branch)

- `cargo fmt --all -- --check`: pass
- `cargo clippy --workspace --all-targets -- -D warnings` (after
  `sh tools/cloud-setup.sh`): pass
- `cargo test -p d2-data -p d2-sim`: pass
- `cargo run -p depcheck`: pass
- `python3 tools/spec_index.py --check`, `python3 tools/methods.py check`:
  pass
- `python3 tools/coverage.py --check`: 0 errors; `--selftest`: ok
- Non-source files in the diff: this note only (no `__pycache__`, `.pyc`
  or `target`).
