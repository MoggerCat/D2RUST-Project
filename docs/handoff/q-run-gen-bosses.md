# q-run-gen-bosses: verdicts for the generated su / boss / umod checks

Session q-run-gen-bosses, branch claude/q-run-gen-bosses. State channel only
(the generated checks name `channels state`). Ledger part:
`docs/handoff/ledger/q-run-gen-bosses.tsv` (133 rows, `ledger.py --check` 0 errors).

## Done

133 checks run against 1.14d under Wine (3 workers), after merging
`claude/q-fix-quest-load` (without it every check was DIVERGED@2 on the player's `q` list).

| Family | Checks | EQUAL | PARTIAL | DIVERGED |
|---|---|---|---|---|
| su | 66 | 0 | 7 | 59 |
| boss | 25 | 0 | 4 | 21 |
| umod | 42 | 0 | 39 | 3 |
| total | 133 | 0 | 50 | 83 |

PARTIAL = all 150 frames equal; only the 1.14d-only unit-owner field `own` is not
compared (state-snapshot.md §2, open question 1). The ledger rows are NO-CHECK
(PARTIAL) per ledger.py's convention, not EQUAL.

Top shared first-divergence causes (grouped by routed owner; `-` = route.py named none):

### (no owner routed)  (57 checks)

- 13 x first divergence at player 0:2 class N, field fc (e.g. gen-boss-708, gen-su-11)
- 12 x first divergence at monster 1:8 class N, field m (e.g. gen-boss-229, gen-boss-267)
- 10 x first divergence at monster 1:8 class N, field fr (e.g. gen-boss-243, gen-boss-256)
- 5 x first divergence at monster 1:9 class N, field fr (e.g. gen-su-26, gen-su-27)
- 4 x first divergence at monster 1:9 class N, field m (e.g. gen-su-2, gen-su-48)
- 3 x first divergence at player 0:1 class N, field m (e.g. gen-boss-211, gen-su-65)
- 2 x first divergence at monster 1:10 class N, field m (e.g. gen-su-23, gen-su-34)
- 1 x first divergence at monster 1:11 class N, field m (e.g. gen-su-12)
- 1 x first divergence at monster 1:16 class N, field m (e.g. gen-su-24)
- 1 x first divergence at monster 1:8 class N, field sp (e.g. gen-su-37)
- 1 x first divergence at monster 1:11 class N, field fr (e.g. gen-su-38)
- 1 x first divergence at monster 1:10 class N, field fr (e.g. gen-su-46)
- 1 x first divergence at monster 1:12 class N, field m (e.g. gen-su-53)
- 1 x first divergence at monster 1:13 class N, field m (e.g. gen-su-59)
- 1 x first divergence at player 0:1 class N, field sp (e.g. gen-umod-27)

### claude/q-fix-seed-order  (13 checks)

- 6 x first divergence at monster 1:8 class N, field s (e.g. gen-boss-242, gen-su-0)
- 5 x first divergence at game, field seed (e.g. gen-boss-250, gen-boss-707)
- 1 x first divergence at monster 1:15 class N, field s (e.g. gen-su-32)
- 1 x first divergence at monster 1:11 class N, field s (e.g. gen-su-7)

### claude/coord-resume-3  (11 checks)

- 2 x first divergence at monster 1:8 class N, field tx (e.g. gen-boss-526, gen-su-51)
- 2 x first divergence at missile 3:1 class N, field yf (e.g. gen-su-15, gen-umod-23)
- 2 x first divergence at monster 1:11 class N, field tx (e.g. gen-su-36, gen-su-8)
- 2 x first divergence at monster 1:9 class N, field tx (e.g. gen-su-47, gen-su-52)
- 1 x first divergence at missile 3:1 class N, field ty (e.g. gen-boss-156)
- 1 x first divergence at monster 1:12 class N, field tx (e.g. gen-su-22)
- 1 x first divergence at monster 1:15 class N, field tx (e.g. gen-su-64)

### claude/q-fix-join-items  (2 checks)

- 1 x first divergence at player 0:1 class N, field hp (e.g. gen-su-13)
- 1 x first divergence at missile 3:1 class N, field lvl (e.g. gen-su-58)

Tool change: `tools/scenario-diff/suite.py --checks-dir` (the suite only globbed
`traces/checks/*.check`).

## Open

- The ledger rows' `checks` column is `-` and the check name is in `note`:
  `ledger.py` only accepts names in `traces/checks/*.check`, not `traces/checks/gen/`.
  The coordinator should make it glob `gen/` (not my area), then the column can name them.
- `monsters.tsv` rows for the same areas win until ledger.py is newest-part-wins.
- `traces/orig-cache` recordings were not committed (coordinator's call: too large, regenerable).
- Every size is M, an estimate; no fix was attempted.

## Repro

One check:
`python3 tools/scenario-diff/scenario_diff.py traces/checks/gen/gen-su-0.check --orig-cache traces/orig-cache --fill-cache`

Whole family (boss / umod / su), 3 workers:
`python3 tools/scenario-diff/suite.py --checks-dir traces/checks/gen --filter 'gen-boss-*' --workers 3 --orig-cache --fill-cache --no-playthrough`

Route a divergence: `python3 tools/coord/route.py --json traces/raw/suite/<name>/result.json`
