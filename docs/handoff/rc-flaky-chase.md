# rc-flaky-chase: `prop_walk_motion::chase_a_moving_target` flake

Base: `claude/specs-staging-7` + `origin/claude/integ-r17` (merged, no conflicts).

## Result
Root cause found; no code change needed on r17. Checks (scenario-diff EQUAL counts): not run, d2-sim test only; before -> after unchanged.

## Cause
Not nondeterminism in the sim: proptest draws a fresh random seed each run
(`failure_persistence: None`), so a model/code disagreement that needs a
specific (distance table cell, unit size) pair shows up as "passes on rerun".
`585fea57` (rc-object-approach, REC-1930) made `walk/geom.rs::unit_distance`
return 0 at once for a negative `dist8_unit` entry (0x00641634). The test's
model `ref_unit_distance` still clamped to 0 and then applied the size
adjustment, so the arrival check (`d <= stop`, pathing.md §9.5 r3) disagreed
with the model: the model expected a re-path, the code stopped, and the
assertion at `prop_walk_motion.rs:352` failed with `(0,0)` vs `(1,1)`
(`(flagged, queued)`), the failure rc-ai-special saw (~1 in 3 runs).
`c86111a3` (in r17) changed the model to the same early return.

## Proof
- Re-adding the old model line (`d = 0` instead of `return 0`): 3 of 3 runs of
  `PROPTEST_CASES=3000` FAIL at line 352 `(0,0)` vs `(1,1)`. Reverted.
- Current tree: debug 30 x 100,000 cases, release 4 x 40 x 200,000 cases
  (32 M): 0 failures.
- Earlier (type 15 after re-path) flake: fixed in `4c391149`-era change, not
  seen again.

## Open
- Branches that took `585fea57` without `c86111a3` fail this test ~1 in 3;
  merging r17 fixes it. Nothing else open. No ledger rows (no 1.14d check
  exercised); no spec change.
