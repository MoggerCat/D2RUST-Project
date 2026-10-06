# Handoff: property tests on d2-data, d2-sim table readers, more d2-formats (branch `claude/fuzz-data`, 2026-10-06)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud implementation session, repo only, no game files. Base: `main` at
`edad871`. Task class: tests + crash fixes, medium effort (M14). Follows
the 2026-10-06 property-test lesson (HANDOFF §8: 16 parser bugs in
`d2-formats`): every size, count, offset or walk driven by input is
bounded by the input.

## 1. State

Three new integration-test files (public API only), one crash fix.
All gates pass (§6). Default case counts keep the new tests under ~2 s
in `cargo test` (debug); `PROPTEST_CASES=N` hunts harder.

| File | Properties | What runs |
|---|---|---|
| `crates/d2-data/tests/prop_data.rs` (+ `prop_common/mod.rs`, shared support: deadline thread, config, seeded generator) | 10 tests | **compile** every called 1.14d field list on random `.txt` (columns dropped / duplicated / unknown, cells at type boundaries, codes, names, formulas, quotes, high bytes) with every linker present; **txt → bin → typed** round trip (records → `.bin` container → `BinTable::parse` keeps the bytes; `decode_by_name` on every runtime table; typed equality bin vs compiled for weapons, skills, uniqueitems, monstats, itemstatcost, levels); **whole-set compile** (`compile_all`, linkers across tables, the hand-built `@uniques` / `@sets` / `@treasureclass`) then fix-ups on its output; **`fixup::apply`** on sets of random records for all 73 runtime tables (with and without tables missing, refusing tables emptied in 2/3 so later fix-ups run, ~75 % of sets reach the end); **each fix-up / runtime-map builder** called directly (`maps::*`, `records::*`, incl. `monstats_speeds` with a random `AnimData`); `qsort` sorts and permutes, and returns with an inconsistent comparator; **link validation** (`LinkerSizes::from_tables`, `validate`) on the fixed tables; **patch layers on the real rules** (`rules()`, bases from generated texts for every patchable table, 1–3 layers of statements drawn from real tables / keys / columns / current values, layer k written against the state after layers 0..k; `apply_stack`, `diff_tables`, the §9 diff round trip on stacks that applied cleanly (~30 %), `compile_patched` against a random live set); regression `regress_equiv_walk_cycle` |
| `crates/d2-formats/tests/prop_more_formats.rs` | 8 tests | `.tbl` built from random key/value pairs (hash + linear probing): every key reads back its value, element `e` is pair `e`; mutated tables parse or refuse and every lookup (`get`, `find_slot`, `element`) takes any key / index; `AnimData.d2` built from random records: every name found in either case with its frames / speed, empty name → default; mutated files + `find` / `record` / `info` on arbitrary names (long, NUL, high bytes); COF built from random layers / frames / directions: `component_at(d, f, s)` = `order[(d·F + f)·L + s]`; mutated COFs + `component_at` with any indices; Storm `decrypt` on any length (tail bytes untouched); `crypto::hash`, `priority`, `animdata::hash` / `uppercase`, `key_hash` on arbitrary names |
| `crates/d2-sim/tests/prop_tables.rs` | 3 tests | on random `.bin` sets: `SkillTables::from_bin` (+ `skill`, `missile`, `skilldesc_of` with any index), `CombatTables::from_bin`, `VitalsTables::from_bin`, `StatTable::from_fixed`, `HireRow::from_table`, `cube::recipes`, `UnitData::new`, `monstats_extra`, `component_counts`, `chain_lengths`, `composits`, `PopTables::from_records(..).with_bins` (+ `mon`), `DrlgData::from_tables` (+ `level`, `lvlwarp_row`, `lvltype_file`, `is_waypoint_object`), `MazeData` / `OutdoorData::from_tables`, `PresetDef::from_record`; then `fixup::apply` and on the fixed set `ItemTables::from_fixed` (+ `item`, `itemtype`, `itype_of`, `is_type`, `stat_valid`, `class_skills`, `first_auto` with any index), `VendorTables::from_fixed` (+ its lookups), `StateTable::new`, and the table readers again; `skills::calc::eval` on any code bytes / offset / function arities (calls get their arity) |

Coverage of the earlier property session (not repeated here):
`d2-data` `robust_tests.rs` (txt reader, `.bin` container, load checks),
`calc/robust_tests.rs` (formula compiler, constant evaluator, buffer
validator), `patch/robust_tests.rs` (layer / stack parsers, apply and diff
on a synthetic fixture); `d2-formats` `robust_tests.rs` (arbitrary bytes
into every `parse`), per-parser mutation tests, `mpq/robust_tests.rs`.

Hunting runs (debug, all passed after the fix): `prop_data` with
`PROPTEST_CASES=4000` (patch property 3,000), `prop_more_formats` 20,000,
`prop_tables` 3,000.

## 2. Bugs found

| # | Input | Failure | Fix | File |
|---|---|---|---|---|
| 1 | `itemtypes` (or `montype`) with a row whose `equiv1` leads back to itself, directly (row 1 → 1) or through other rows (montype 1 → 2 → 1), and a column outside the cycle (any row not on it) | `fixup::apply` / `maps::equiv_matrix` never returns: the §2 walk pops the row and pushes it again, so the stack never passes the 124 test (hang; found by `fixups_on_random_tables`, seed 12848956610323624669) | the walks of one matrix share a budget of n² × 128 pops; past it, `equiv_matrix` returns a `FixupError` ("equivalence walk (i, j) does not end (link cycle)"). The 1.14d tables (103 / 59 rows, short chains) use a tiny fraction of it, so their matrices are unchanged; regression `regress_equiv_walk_cycle` | `crates/d2-data/src/fixup/maps.rs` |

No other crash, overflow, abort or hang was found in the reachable code.

## 3. Fixes outside my files

- `crates/d2-data/src/fixup/maps.rs` (bug 1): private `equiv` gains a
  step counter; `equiv_matrix` keeps its signature.
- `crates/d2-sim/Cargo.toml`: `[dev-dependencies] proptest.workspace =
  true` (the crate had no dev-dependency section; checked for a duplicate
  section). `Cargo.lock` gains that one line.

## 4. Signature changes

None. Behavior change: `d2_data::fixup::maps::equiv_matrix` (and so
`fixup::apply`) can now return `Err` for an equivalence-link cycle that
never reaches the asked column; before, it did not return.

## 5. Questions (for the spec owners; not decided here)

1. **`runtime-maps.md` §2 / Edge cases:** by §2 as written, a cycle of
   `equiv1` links that does not contain column j never ends (each pop
   pushes one entry, so the "> 124 entries" test never fires); 1.14d
   would hang at load on such a table. d2rs now reports a load error
   after n² × 128 pops. Add it to the "Out of range in 1.14d … d2rs
   reports a load error" list (with the budget) or state another rule.
2. **`patch-layers.md` §9 diff round trip:** D06 checks key uniqueness
   only within the edited table, but A12 checks the key's scope, which
   can span tables (`items.code`: weapons, armor, misc). An edited table
   whose new or renamed key already exists in another table of the scope
   gives a diff that A12 then rejects, so "applying the output after B
   gives rows X exactly" does not hold for that X. Found when the
   property diffed the state left by a stack that stopped on an A12
   (§5 keeps that layer's passing statements). Should D06 check the scope
   (needs the other tables) or should the round-trip claim be limited?
   The property now asserts the round trip only for stacks that applied
   without an error.

## 6. Gate results (this branch, before pushing)

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings` (after
  `sh tools/cloud-setup.sh`): clean.
- `cargo test --workspace`: all pass (0 failed).
- `cargo run -p depcheck`: OK (8 crates).
- `python3 tools/spec_index.py --check`, `python3 tools/methods.py check`
  (21 methods OK), `python3 tools/coverage.py --check` (3,195 claims, 0
  errors) and `--selftest` (ok).

No `Covers:` claims were added: the properties check robustness and the
round trips, not one spec rule's outcome each (coverage-claims §1).

## 7. Local run queue (for `docs/HANDOFF.md` §5)

- `D2_GAME_DIR=<install> cargo test -p d2-data --test game_data -- --ignored fixups_on_live_set`:
  must still pass (the live `itemtypes` / `montype` walks stay far under
  the new budget, so `fixup::apply` returns `Ok`).
- `cargo run --release -p data-tool -- dump-compare traces/raw/20261006-021210-tables`: the
  `itemtypes_equiv` and `montype_equiv` maps still identical to 1.14d
  memory (expected: unchanged, 70/70 and every map identical).
