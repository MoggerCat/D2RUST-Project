# Handoff: nightly deep property runs — `claude/ci-nightly-props`

> Folded into `docs/HANDOFF.md` (§1–§5, §7, §8) and `docs/PLAN.md` as of the eighth fold (`claude/docs-fold-8`); this file stays as the detailed record.

Cloud tooling session, 2026-10-06, base `main` (`a9115fd`). Repo only.
Answers HANDOFF §8 (two counterexamples found only on CI's random seed).

## 1. What was added

- `.github/workflows/nightly-props.yml`: daily 03:17 UTC + `workflow_dispatch`
  (input `cases` overrides every group). Not in `ci.yml`'s `check`
  (`ci.yml` changed by a comment only). Matrix of three jobs (rust-cache key
  per group, nextest, `fail-fast: false`, 90 min timeout); only `worldsim`
  installs the Bevy apt libraries.
- `tools/props-deep.sh [sim|wire|worldsim|all] [cases]`: the single place
  the filters live; CI calls it, so local == CI. Logs go to
  `target/props-deep/<group>.log`. On failure it prints and appends to
  `$GITHUB_STEP_SUMMARY` the failing tests and each `minimal failing input`
  block (proptest keeps no regression files here: `failure_persistence:
  None`, so the output is the only record), and the workflow uploads
  `target/props-deep/` as artifact `proptest-<group>`. `tools/gate.sh` help
  names it.

## 2. Filters, cases, time

`PROPTEST_MAX_SHRINK_TIME=60000` (ci.yml uses 10000).

| Group | nextest selection | PROPTEST_CASES | Time |
|---|---|---|---|
| `sim` | `-p d2-sim -E 'binary(/^prop_/) \| test(/prop_tests::/)'`: prop_inventory, prop_lists, prop_path_place, prop_rng, prop_tables, prop_timer, prop_units, `stats::prop_tests` (incl. `stat_lists_match_the_model`), `items::moves::prop_tests` | 20000 | **measured 670 s wall**: floor_drop_never_blocked 630 s, nearest_with_field_matches_reference 597 s, timer_queue_matches_the_model 177 s, unit_lists_match_the_model 114 s, stat_lists_with_ops_and_callbacks 62 s, free_point_iff_within_radius 60 s, stat_lists_match_the_model 56 s (failed), inventory_matches_the_model 20 s |
| `wire` | `-p d2-proto -p d2-server -p d2-data -p d2-formats -E 'binary(/^prop_/) \| test(/robust/) \| test(/_property$/)'` (118 tests) | 20000 | **estimate** ≈ 10–20 min: 118 tests took 10 s at 200 cases; not run at 20000 |
| `worldsim` | `-p d2-client --test prop_worldsim` | 500 | **estimate** ≈ 12 min: measured 13.4 s for 10 cases of `same_seed_same_messages_same_game` (≈1.3 s/case, debug) → ≈ 670 s, plus ≈ 1 min for `other_seed_other_digest` |

Debug build (CI's test profile; overflow checks on), single local machine;
GitHub runners may be 1.5–2× slower.

## 3. Findings from the first deep run (`sim`, 20000 cases)

1. `stats::prop_tests::stat_lists_match_the_model` fails: "full of stat 19
   layer 0 on ListId{index 2, gen 0}: left 0, right 2147483647" (a long
   AllocPlain / Free / AllocExt / Attach op sequence; full shrunk input in
   the run's log). This is the `fix-statlist-prop` bug of HANDOFF §8, not
   fixed on `main`. The nightly will be red until it lands.
2. `prop_units::anim_schedule_matches_the_closed_form` aborts with
   "Too many global rejects" (1024 rejections of the `prop_assume!` at
   `prop_units.rs:223`, after 12459 successes). That is a test-generator
   issue (the assumption rejects too often at high case counts), not a
   sim bug; fix by constraining the strategy or raising
   `max_global_rejects`. Not changed here (test code out of scope).

Wire and worldsim were not run at full counts here.

## 4. Gate

`sh tools/gate.sh`: GATE: PASS (nextest installed, `tools/cloud-setup.sh` run).
