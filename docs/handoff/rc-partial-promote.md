# rc-partial-promote hand-back

Branch claude/rc-partial-promote = specs-staging-7 + integ-r16. Ledger EQUAL 887 -> 1242
(`ledger.py`, merged); part `docs/handoff/ledger/rc-partial-promote.tsv` (371 EQUAL, 184 DIVERGED).

## Checks (1.14d re-recorded under Wine: integ-r16's record_state change missed every state cache entry)
| family | rows PARTIAL -> EQUAL | other |
|---|---|---|
| gen-umod (42) | 39 | umod-27 DIVERGED@149 (player sp), umod-30 @85 (rows already DIVERGED) |
| gen-state (184) | 0 | 184 -> DIVERGED@2: state equal with every field, packets differ at the join (S->C 0xAA AddUnit size 12 vs 39; route.py: q-fix-replay-hooks, d2-server packets.rs) |
| gen-obj (388 of 571 run) | 304 | 4 DIVERGED (rows already DIVERGED), 29 items PARTIAL "recordings end at different frames", 2 ERROR (disk full) |
| gen-missile (265) | 0 | not run (out of time) |
| walk-town-ama (coordinator) | 28 | walk-click-walk-town-ama DIVERGED@18: C->S 0x01 x 142 vs 143 at the 2nd click (click point, scenario-diff.md open q. 3, controls-0001) |

## What changed
- check_gen: gen-state / gen-missile no longer `ignore q` (449 checks). One-sided fields: none left
  (d2rs writes `own` and `q`; every run compared all 42 fields).
- orig_cache: key `orig-cache-key-2` hashes the check's recording lines (no comments / `ignore`);
  `--migrate-v1` moved 3520 unchanged entries. Recordings not committed.
- walk-town-ama, walk-click-walk-town-ama: `ignore seed s fc sp` removed, packets channel added.
- PROVISIONAL REC-2055 / REC-2056 (scenario-diff.md open questions 6-7): every d2rs state header
  carries the client gap `RUN_GAPS`, so state_diff can only say PARTIAL. A row is EQUAL when all
  fields of both sides are compared and equal, nothing ignored, and the check is pokes-only or its
  packets MATCH; an items PARTIAL "no item created on either side" counts as equal. These rows
  keep last_verdict PARTIAL (the comparator's word). Coordinator: confirm or reject the rule.
- d2-sim state tests: two malformed `Covers:` claims (coverage --check).

## Open
- gen-missile 265 + gen-obj 183 not run (~1.5 h Wine, 3 workers):
  `suite.py --checks-dir traces/checks/gen --filter 'gen-missile-*,gen-obj-*' --orig-cache DIR --fill-cache`.
  Mind disk: ~8 MB per work dir; delete finished `*.jsonl`.
- gen-state packets@2 (184 rows) belongs to the join/AddUnit owner.
- Pre-existing, not mine: `ledger.py --check` 2 errors (rc-whirlwind.tsv); check_gen `--check` /
  `--selftest` fail on HEAD (deletes 75 gen-netc2s checks it does not generate);
  `d2-sim::prop_walk_motion chase_a_moving_target` failed once, passed on re-run (proptest input
  w=8 h=17 all masks 1, both monsters, budget 3, mvel 1669).
