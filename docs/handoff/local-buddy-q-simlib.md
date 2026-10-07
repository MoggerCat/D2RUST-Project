# local-buddy-q-simlib

Branch `claude/local-buddy-q-simlib-2026-10-07` from origin/main 674996d. No code changed.
rustc 1.99.0 (b940084d7 2026-09-28), x86_64-pc-windows-msvc, LLVM 23.1.1. Env: RUSTC_WRAPPER empty (no sccache), CARGO_BUILD_JOBS=4, fresh CARGO_TARGET_DIR `targets\buddy-simlib-clean`, D2_GAME_DIR = main checkout `game\`.

## Result: the crash is not reproducible in debug; it is environmental/flaky
| Run | Outcome |
|---|---|
| 1. `cargo test -p d2-sim --lib -- --ignored --nocapture` (debug, incremental) | Process exit -1 with no rustc error text; last log line `Compiling criterion v0.5.1` (d2-sim lib test unit was in flight). Likely the same silent crash. |
| 2. `cargo test -p d2-sim --lib --no-run -vv` (debug, same dir) | Built OK (47.7 s) |
| 3. same with `CARGO_INCREMENTAL=0` | Built OK (54.5 s) |
| 4. `cargo test -p d2-sim --lib --release --no-run` | FAILED: `could not compile d2-sim (lib test)`, rustc exit 0xc0000005 STATUS_ACCESS_VIOLATION (reproducible unit: d2-sim lib test, opt-level=3) |
| 5. debug `--lib -- --ignored --nocapture` after build | exit 0 |

Conclusion: rustc 1.99.0 crashes intermittently on the `d2-sim` lib test unit (`--test`, incremental) in debug and consistently in release on this PC. Without sccache a retry succeeds (run 2), so it is flaky rather than a code defect; the release crash is the only deterministic one (rustc bug or toolchain/AV issue; nightly-ish 1.99 version is the prime suspect). Workaround: retry, or `CARGO_INCREMENTAL=0`; avoid `--release` for this unit.

## Ignored lib tests (debug): 8 passed, 0 failed
combat::tests::real_table_constants, skills::special::tests::codes_match_game_tables, skills::use_::bodies::tests::body_tables_from_the_1_14d_tables, monsters::init::tests::real_level_stats, world::npc::tests::live_monstats_records, skills::tests::real_skill_vectors, path::search::tests::expfield_live, items::inventory::tests::real_grid_belt_and_type_tables: all PASS. Matches tri-sim step 1 (8/8 lib ignored pass).

## Claims
No "Intended claim"/"Claim once" lines exist in the lib sources (only in `tests/game_*.rs`), so nothing was converted to `// Covers:`.
