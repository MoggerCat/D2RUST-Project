# rc-run-1 hand-back (runner, no code changes)

Command: suite.py --filter 'a*' --no-playthrough --workers 3 --fill-cache (1.14d recorded under Wine where the cache was empty).
Result: 262 checks. Comparator: MATCH 133 / PARTIAL 222 / DIVERGED 67 channel verdicts over 422 channel runs.
Per check under the ledger rules (REC-2055/2056): EQUAL 147, DIVERGED 115.
- docs/handoff/ledger/rc-run-1.tsv: 287 ledger rows touched by these checks (180 EQUAL, 107 DIVERGED).
- docs/handoff/rc-run-1-causes.tsv: 12 distinct first differences, largest first.

Largest causes:
1. 62 checks: input/send lines but no packets channel, so EQUAL cannot be settled (state itself has 0 differences). Size S: add `packets` to the channels line of those checks (or relax the rule).
2. 20 checks (a5-su-ancient*, ...): monster class 522 field tx 4324 vs 4321 at frame 7. Size S, d2-sim monsters.
3. 17 packets: s2c id differs (a2-quest-radament: 0x9c vs 0x51 at frame 61). Size M.
4. 6 state monster field m (a3 temple warps frame 43), 2 rng site mismatches (a2 tomb), rest single checks. See causes file.

Notes:
- `ledger.py --check` still reports contradictions on my part: its reconcile reads the old checks-status file, not this run. The status file needs regenerating from /tmp/rc-run-1.json; then `ledger.py --fix`. Also the ledger's EQUAL-with-PARTIAL case (REC-2055) is not known to reconcile(). Integrator's call.
- traces/orig-cache was refilled locally by --fill-cache; not committed (brief).
- Not run: coverage.py / spec_index.py checks (no code or spec change).
