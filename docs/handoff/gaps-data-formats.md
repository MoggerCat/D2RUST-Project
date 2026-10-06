# Handoff: gap tests for data and format specs (branch `claude/gaps-data-formats`, 2026-10-06)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

The branch starts from `claude/bold-ptolemy-jvyvxy` at `ac01471` and is
targeted at PR #15. This was a cloud session with the repo only and no game
files. Task class: tests from specs, medium effort (M14). The work was split
across four agents, each editing its own set of files:
1. calc-expressions
2. loading + schema
3. field-types + txt-format
4. formats + rng

The diff adds tests, one test helper file and this note. The only non-test
code change is in "Code changes" below. No spec, `docs/HANDOFF.md` or
`docs/PLAN.md` was edited.

Gate (all pass on this branch):
- `cargo fmt --all -- --check`
- `cargo clippy -p d2-data -p d2-formats -p d2-sim --all-targets -- -D warnings`
- `cargo test -p d2-data -p d2-formats -p d2-sim`
- `cargo run -p depcheck`
- `tools/spec_index.py --check`
- `tools/methods.py check`
- `tools/coverage.py --check` (2583 claims, 0 errors)

## 1. Coverage before → after (units covered by any tier / game tier)

| spec | units | any before | any after | game before | game after |
|---|---|---|---|---|---|
| data/calc-expressions.md | 50 | 12 | 46 | 1 | 9 |
| data/field-types.md | 20 | 11 | 20 | 2 | 9 |
| data/loading.md | 47 | 11 | 32 | 6 | 13 |
| data/schema.md | 9 | 3 | 6 | 0 | 0 |
| data/txt-format.md | 35 | 24 | 34 | 4 | 6 |
| formats/animdata.md | 14 | 7 | 10 | 0 | 1 |
| formats/dc6.md | 6 | 3 | 6 | 3 | 4 |
| formats/dt1.md | 6 | 4 | 6 | 4 | 5 |
| formats/font-tbl.md | 4 | 2 | 4 | 2 | 2 |
| formats/mpq-tables.md | 4 | 2 | 4 | 0 | 0 |
| formats/mpq.md | 48 | 28 | 48 | 20 | 20 |
| formats/tbl.md | 6 | 4 | 6 | 2 | 3 |
| sim/rng.md | 26 | 9 | 19 | 0 | 0 (trace 4) |
| **repo total** | 2704 | 1815 (67.1%) | 1936 (71.6%) | 158 | 186 |

The total "verified" figure went from 188 (7.0%) to 216 (8.0%). The 28 new
game-tier claims sit on `#[ignore]` tests that have never been run, because
this session had no game files. Under `docs/COVERAGE.md` §3 they count as
verified only once a local run passes (§3 below). Until then, read them as
unverified.

## 2. Code changes (each behavior-neutral)

- `crates/d2-sim/src/rng.rs`, `Seed`: added `#[repr(C)]`, citing
  `specs/sim/rng.md` §1 r1 (`lo` at offset 0, `hi` at 4). Rust's default
  layout does not guarantee field order. Draws are unchanged, and the test
  `seed_layout_lo_then_hi` checks the offsets.
- `crates/d2-data/src/bin.rs`: the inline server-only-file check
  (`loading.md` §3.3) moved out of `load()` into
  `check_server_files(set, table)`. `load()` calls it at the same point, so
  behavior is unchanged. The move lets a unit test reach the check without a
  full install.
- Test-only:
  - `mpq/huffman.rs`: the test module and `Model::decode` became
    `pub(crate)`, so `robust_tests.rs` can reuse the spec model.
  - `tests/formats_game.rs`: `parse_all` now takes `FnMut`.

No test showed code deviating from a spec, so no fix was needed. Neither
change alters output. Still, the queued re-run below confirms nothing moved:
`cargo run --release -p data-tool -- tables` and
`cargo test -p d2-data -p d2-formats -- --ignored`.

## 3. Game-file tests to run locally (for `docs/HANDOFF.md` §5)

```
D2_GAME_DIR=<install> cargo test -p d2-data --test gaps_fields_game -- --ignored
D2_GAME_DIR=<install> cargo test -p d2-data calc::tests::game -- --ignored
D2_GAME_DIR=<install> cargo test -p d2-data --test game_data -- --ignored sound_tables_are_runtime_txt client_composite_tables live_records_zero_outside_fields txt_line_observations compile_only_lookup_tables loading_edge_cases game_truth_set
D2_GAME_DIR=<install> cargo test -p d2-formats --test formats_game -- --ignored
cargo run --release -p data-tool -- tables
```

What to look for: all tests pass. If one fails, either fix the expected
value from the observation, or remove its claim in the same session
(COVERAGE.md §3).

Assertions written blind that are the most likely to need adjusting:
- `gaps_fields_game`:
  - the `txt_source`/`bin_source` strings (lower-case archive names assumed);
  - the table names `levels` and `monstats2`;
  - the lower bound of 69 tables in the zero-bytes check.
- `calc::tests::game::calc_cells`: assumes the parser keeps skilldesc's
  single-space cell as `" "`, and that this cell is the one 1.14d Fail
  (`strictness_counts`).
- `game_data::txt_line_observations`: expects 75 excel `.txt` files in X
  and 56 in D.
- `game_data::loading_edge_cases`: "no empty key" is taken to mean that
  neither `"    "` nor 0 is in the `colors.code` or `properties.code`
  linkers.
- `game_data::live_records_zero_outside_fields`: skips `monstats`,
  `monstats2`, `monpreset` and `cubemain`, because their callbacks write
  outside field footprints.
- `formats_game`:
  - the animdata duplicate set (29 duplicates, 9 differing);
  - `42DTHTH` is the only record over 144 frames;
  - the six version-4 DT1 names;
  - `DEFAULT.TBL` and `FONTER.TBL` are the only text `.tbl` files.

## 4. Left uncovered (untestable here or not owned by these crates)

- **calc-expressions**
  - §3.1 text: callers' fallbacks, which the table specs own.
  - §3.1 r2 and policy r5: the run-time evaluator lives in `d2-sim`
    skills, which is outside this session's scope.
  - Policy r3: `Ruleset::Mod` rebuild is not implemented.
- **loading**
  - §rules text and §d2-data-policy text: definitions only.
  - §1 r3: `expfield.d2` is never read.
  - §3.2 text, r2, r3: `-txt` write-back is not reproduced (policy r5).
  - §4.2 r3: needs a full synthetic install of 73 tables.
  - §7.3: see the spec question in §5.
  - §7.4: a summary; its owner specs carry the claims.
  - §9: the affix and token arrays are not built in d2-data.
  - §10 r6 and r7: these parts live in d2-sim (treasure, DS1 skipping).
  - Policy r4: owned by patch-layers.md.
  - Policy r6: survey tools.
- **schema** §3 r1–r3: these describe extraction from `Game.exe`. Their only
  output is the TSVs, which are already checked.
- **txt-format** §1: 1.14d's own compile-to-`.bin` mode. d2rs has no such
  mode, and its output-equality clause is covered by crosscheck.
- **animdata**
  - §1: load order and the missing-file error live in the d2-data loader.
  - §5: the COF-name composer is `fixups.md` §8.
  - §7: a debug speed setter that is not implemented.
  - §expfield-d2: no parser exists.
- **rng**
  - §3 r6: the fastcall convention, which cannot be tested in Rust.
  - §5.2–§5.5: seeding in game, unit and DRLG code that is not wired yet.
  - §6 and §7: per-system draw use, which the system specs own.

## 5. Spec questions

1. `mpq.md` §11 has two gaps (the implementation returns an error in both
   cases, and the spec model copies it):
   - Input that ends exactly between codes, with no 0x100 code and the
     output not full: is that an error? The spec only covers running out
     mid-code.
   - Increment when `lead` is the root or `n`'s parent: the spec does not
     say what happens. Valid streams never reach this case.
2. The edge-case sections of `dc6.md`, `dt1.md` and `animdata.md` mix
   behavior with claims about 1.14d data, so only game tests can claim them
   as whole sections. Splitting the data claims into their own items would
   let unit tests claim the behavior parts.
3. `calc-expressions.md` §4.5: an 8-bit push needs `len(out) < 1024`, which
   allows 1025 bytes. The code follows the spec, and tests can't observe
   the difference because END fails right after. Confirm against 1.14d.
4. `calc-expressions.md` policy r4 says Fail is "a non-empty cell gives
   0xFFFFFFFF". A whitespace-only cell also reports Fail. Is that
   intended?
5. `loading.md` §7.3 omits `chartemplate`, whose `class` column links to
   `playerclass.code`. The table also mixes formula-derived links
   (misscalc, skillcalc) with field links, so it can't be checked exactly
   against `fields.tsv`.
6. `loading.md` policy r5 says `d2exp.mpq` is required, but `bin::load` has
   no explicit check and only fails later on missing X files. Should a
   classic install be rejected up front?
7. `txt-format.md` §edge-cases says "monstats 255 slots" without splitting
   columns from missing fields, so monstats was left out of the slot-count
   check.
