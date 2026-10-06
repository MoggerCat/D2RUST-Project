# Handoff: stat-list property failure in CI — `claude/fix-statlist-prop`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here); the coordinator folds it.

Cloud fix session, 2026-10-06. Task class: CI fix, high effort (METHODS
M14). Base: `claude/tender-meitner-mphas3` at `002b244` (head of PR #24).
Inputs read: `specs/sim/{stat-lists,stats}.md`, `docs/`,
`docs/handoff/prop-sim-core.md`, `crates/d2-sim/src/stats/`. Repo only.

## 1. Failure

CI (nextest, random seed): `stats::prop_tests::stat_lists_match_the_model`,
"full of stat 19 layer 0 on ListId { index: 0, generation: 0 }: left -1,
right 0". Proptest's minimal input, now the deterministic test
`regress_full_after_free_and_toggle` in
`crates/d2-sim/src/stats/prop_tests.rs` (the crate keeps no persisted
regressions file, `failure_persistence: None`; minimized failures become
`regress_*` tests). It failed before the fix, at the same assertion.

Decoded (start: extended lists P = player 1 = `ListId{0,0}`, monster 2,
items 10 and 11):

1. Free item 11's list. Set stat 19 (damage-related) = −1 on item 10's
   list I. Attach I to player 1, reset: P.full[19] = −1.
2. Two plain lists, one freed (noise).
3. Toggle static of I on unit 11: I's unit is player 1, so §8.6 runs
   equip(11, I, 1); §8.4 attach is refused (unit 11 has no list) and
   equip sets I's unit to 11. I stays in P's active chain.
4. New extended list R for item 11; two plain lists (noise).
5. Equip(11, I, reset 0): I's unit is 11 and I is not DYNAMIC, so §8.6
   make-dynamic: I gets DYNAMIC and propagate(R, 19, +1).

After step 5, P.full[19] = −1 (code). The model wanted 0: I is DYNAMIC in
P's active chain, so the §6.1 sum skips it, and the model had no drift for
P.

## 2. Rule and which side was wrong

`stat-lists.md` §8.6: "Else, with U's list extended: static: if I's list
is DYNAMIC, clear it and propagate(U's list, key, +value, I) for each
`damagerelated` entry of its full (or base) array; dynamic: if not
DYNAMIC, set it and propagate −value the same way."

The values move into U's list only. When I sits in the active chain of
another list P (its unit set by a refused attach, §8.4), the flag flip
changes how I counts in P's `stats.md` §6.1 sum ("a child c is skipped
when the stat is `damagerelated` and c has the DYNAMIC flag") while P's
full array is not touched. `StatLists::toggle_dynamic` does exactly this.
**The model was wrong**: `toggle_drift` drifted U's list (when I does not
count there) but not I's parent.

## 3. Fix

Model only (`Machine::toggle_drift`): besides the existing drift on U's
list R when I is not in R's active chain, when I's parent P ≠ R is live
and holds I in its active chain, P drifts by the opposite amount (dynamic:
+v, static: −v per damage-related entry). The module docs name the case.
No code change in `stats/lists.rs`; the property is not loosened (every
assertion as before, the drift is the amount the spec rule leaves in P).

## 4. Runs

- `cargo test -p d2-sim --lib prop_tests`: 8 passed (with the new
  regression test).
- `PROPTEST_CASES=20000 cargo test -p d2-sim --lib stat_lists_`, 5 runs
  (both stat-list properties, random seeds): all pass (200,000 cases).
- `PROPTEST_CASES=20000 cargo nextest run -p d2-sim
  stat_lists_match_the_model`, 3 runs: all pass.
- No further counterexample.
- `sh tools/gate.sh all`: see the commit (§5).

## 5. Gate

`sh tools/gate.sh all` on this branch: all PASS (2026-10-06).
