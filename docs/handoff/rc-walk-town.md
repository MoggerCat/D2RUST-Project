# rc-walk-town hand-back

Branch claude/rc-walk-town = specs-staging-7 + integ-r16 (merged clean).

## Result: no walk divergence left on r16; nothing to fix

Checks re-run on r16 (`suite.py --filter 'walk-town-ama,walk-click-walk-town-ama' --orig-cache traces/orig-cache`, 1.14d re-recorded, no cache hit):

| check | channel | ticks equal | verdict |
|---|---|---|---|
| walk-town-ama | state | 80/80 | PARTIAL |
| walk-click-walk-town-ama | state | 70/70 | PARTIAL |

Extra probes (scratch copy of walk-town-ama, not committed):
- without the `ignore seed s fc sp` line: state still 80/80 equal.
- with `channels state packets`: packets 80/80 MATCH (the 0x01 Walk messages equal).
- rng channel errored only because I ran with `--no-build` (suite-bin d2-client missing); not run again.

## Why the 28 rows read DIVERGED

The 28 systems rows (specs/sim/pathing.md, path-placement.md, seams/movement-prediction.md) take
their state from the mapped check, PARTIAL. PARTIAL here is not a difference: `state_diff.py`
prints "no difference in what was compared" plus the standing d2rs gap
"client: headless bridge (no UI or visibility art)..." (suite.log line 25 of the run dir).
So the rows score DIVERGED because the check cannot reach EQUAL while that gap
stands, not because walk step, path, mode/frame or packets differ.
The earlier first difference (the @11 / @16 / @2 rows in q-run-net, coverage-checks, world) was
fixed by one of the 20 r16 fixes; I did not bisect which.

## Open
- The PARTIAL gap: closing it is client-bridge/tooling work (route with `tools/coord/route.py`
  on tools/trace-recorder/state_diff.py), not walk logic. Size: unknown, not measured.
- Re-score the 28 rows once the gate or scoring rule treats "PARTIAL with zero differences
  and packets MATCH" as EQUAL, or once the gap closes. I wrote no ledger part (nothing settled
  to EQUAL by the rules).
- Not done: the rng channel on walk-town-ama (needs a build of suite-bin d2-client).
