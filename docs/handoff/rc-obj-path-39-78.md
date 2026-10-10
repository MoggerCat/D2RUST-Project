# rc-obj-path-39-78 hand-back (2026-10-10)

Task: gen-obj-39 and gen-obj-78 (second path compute "returns 0 in 1.14d").

## Checks (state / rng, single runs, orig-cache)
- gen-obj-39: state DIVERGED -> PARTIAL (80/80), rng DIVERGED -> MATCH.
- gen-obj-78: state DIVERGED -> PARTIAL (80/80), rng MATCH.
- gen-obj-*: family re-run, see the coordinator's number below if listed.
EQUAL rows: 2 of 2 clean (the six gen-obj rows are all clean now).

## Cause (premise was wrong)
1.14d's second compute does NOT return 0: it returns 4 points, as d2rs does.
Breakpoints under Wine (0x00649970 entry/exit, 0x0064EDA0 try move) show: the
player's step is refused (an NPC stands on the next cell, mark 0x1000), step
result 2, then the queued interaction in 0x00580C20 (0x00548B00 case 2 ->
0x00548A50) starts a new run (the compute), and the ENDANIM neutral start
(step result 2, units.md 4.5) ends it the same frame: mode 5, queue cleared.
d2rs ran the retry after that neutral start, so its new run survived.

## What changed
- d2-server object_approach.rs: the retry's run is followed by a neutral
  start, not re-queued. d2-sim PathCtx::neutral_start (new).
- specs/world/objects.md 7.3 rule 4.4: the ENDANIM neutral start.
- Collision map near the fire (6x6 wall block) checked equal to 1.14d.

## Open
- The in-range retry (operate in the stop frame) and the NPC / item arrival
  retries (npc_approach, item_approach) probably need the same second neutral
  start; no check covers them yet (size S).
