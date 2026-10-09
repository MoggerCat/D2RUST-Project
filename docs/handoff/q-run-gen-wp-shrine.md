# q-run-gen-wp-shrine: hand-back (2026-10-09)

Ledger part: `docs/handoff/ledger/q-run-gen-wp-shrine.tsv` (79 rows). Branch merges
`q-fix-quest-load` (the `q` DIVERGED@2 is gone) and `q-run-gen-bosses` (suite.py `--checks-dir`).
New 1.14d sides are in `traces/orig-cache/gen-{wp,shrine,lvl}-*` (text outputs only).

## Done

| Scope | Rows | Verdict |
|---|---|---|
| `waypoint.*` (gen-wp-0..38) | 39 | 39 DIVERGED |
| `shrine.*` (gen-shrine, 19 rows) | 19 | 19 DIVERGED (frame 24, before the shrine exists) |
| `shrine.0/4/5/16` | 4 | no check possible: need preset objects 574-579, `poke` cannot create them (tool feature: poke preset objects) |
| `level.*` that were NO-CHECK (17, via gen-lvl) | 17 | 13 PARTIAL (state PARTIAL for snapshot gaps, rng MATCH), 4 DIVERGED (21, 100, 128, 132) |

## First-divergence causes (grouped by owner)

1. **Town NPC spawn state, owner `claude/q-fix-real-unit-seed-order`.** Frame 24 monster class 201 (Lut Gholein) field `m` 1.14d 2 vs d2rs 1: all 19 shrine checks. Frame 24 class 511 `m` (Harrogath): wp 30-38. Frame 37 class 359 `s` (Kurast, seed): wp 18-26. Also wp 0, wp 27, gen-lvl-21 (monster seed), gen-lvl-128 (class 545 `m`).
2. **Level-change seed after waypoint travel, owner `claude/q-fix-join-items`.** Frame 400 `game.seed` differs after C->S 0x49: wp 1-8, 28, 29 (act 1 outside, act 4 river/city).
3. **Player `tx` during the walk, act 2: 1.14d 0 vs d2rs 5153 at frame 68 (wp 9-17).** Suspected `claude/q-fix-client-crash` (walk) or the poke `pos` handling; not confirmed.
4. **Level generation seed, owner `claude/coord-resume-3`.** gen-lvl-100 (frame 21) and gen-lvl-132 (frame 97): rng site 0x552e31 missing in d2rs.

Because (1) and (3) end the compare before frame 30 / 400, the shrine effects and waypoint travel behaviour themselves are **unverified**; they need (1) fixed, then a rerun.

## Open

- `tools/coord/ledger.py` only counts `traces/checks/*.check` as checks, so `--check` reports
  75 errors ("matches no traces/checks/*.check") for the `gen-*` names in my part. One-line fix in
  `repo.checks` (line 72) to include `traces/checks/gen`: not my area, sent to the coordinator.
- PROVISIONAL REC-1330 (shrine seed pick) is still unsettled: no shrine check got past frame 24.

## Repro

```
python3 tools/scenario-diff/suite.py --checks-dir traces/checks/gen --filter 'gen-wp-*' --no-playthrough --orig-cache
python3 tools/scenario-diff/suite.py --checks-dir traces/checks/gen --filter 'gen-shrine-*' --no-playthrough --orig-cache
python3 tools/scenario-diff/scenario_diff.py traces/checks/gen/gen-lvl-21.check --orig-cache traces/orig-cache
```
