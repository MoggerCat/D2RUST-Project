# props-deep-hunt — deep property hunt (partial, stopped by the coordinator)

Branch `claude/props-deep-hunt`, cloud session, 2026-10-06. Stopped early on
the coordinator's budget wrap-up. **No code changed. No counterexample found.**

## Runs

| Group | Seed (`PROPTEST_RNG_SEED`) | Cases | Result | Time |
|---|---|---|---|---|
| `sim` (`sh tools/props-deep.sh sim`) | 1001 | 20000 | 55 / 57 PASS, 0 FAIL; 2 still running when the run was stopped: `prop_path_place::floor_drop_never_blocked`, `prop_path_place::nearest_with_field_matches_reference` | about 15 min of test time on 4 cores before the stop |

Slowest sim tests at 20000 cases (4 cores): `prop_timer timer_queue_matches_the_model`
212 s, `prop_lists unit_lists_match_the_model` 156 s, `prop_path_core
footprints_match_the_model` 103 s, `stats::prop_tests::stat_lists_match_the_model`
91 s, `stat_lists_with_ops_and_callbacks` 90 s, `free_point_iff_within_radius` 79 s.

## Setup notes

- `cargo install cargo-nextest --locked` (v0.9.146) and `sh tools/cloud-setup.sh`
  (Bevy libraries for `worldsim`) both succeeded.
- Proptest 1.11 reads `PROPTEST_RNG_SEED` (a u64) through `ProptestConfig::default()`.
  Every config in the workspace is built from `..ProptestConfig::default()`, so a
  fixed seed makes a run reproducible. Without the variable, each run uses a random seed.

## Not done (next steps)

1. Finish `sim` at seed 1001 (only the two `prop_path_place` tests above are left):
   `PROPTEST_RNG_SEED=1001 PROPTEST_CASES=20000 cargo nextest run -p d2-sim --test prop_path_place`.
2. `sh tools/props-deep.sh wire` and `worldsim`, then `all` again with two more seeds (for example 2002 and 3003).
3. Run every proptest in the workspace at `PROPTEST_CASES=5000`, several seeds. The
   `props-deep.sh` filters leave out some tests: the d2-formats in-module properties
   (animdata, cof, dc6, dcc, ds1, dt1, font, palette, tbl, mpq huffman/writer),
   `d2-server/src/tests/gaps.rs`, and the d2-client `prop_*` tests other than `prop_worldsim`.
   Run them by crate, for example `PROPTEST_RNG_SEED=<s> PROPTEST_CASES=5000 cargo nextest run -p <crate>`.
4. For each counterexample, follow HANDOFF §8: list every error the spec allows
   before blaming the code, add a deterministic regression test, and fix the wrong side.
