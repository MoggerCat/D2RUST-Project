# rc-mon-missile144 hand-back

Cause: "d2rs creates a missile (class 144 or 675) that 1.14d does not".

Result: already fixed on the base. rc-extra-missile (merged in integ-r23)
found the root cause: the inferno channel Frames helper (0x005CC2E0, spec
specs/skills/bodies-3.md 3.2) never applied the missile's total/left frames
in the live host, so d2rs missiles lived their full Range. I read the
FetishShaman think (0x005F9A80) and MonInferno srvdo (0x005CC4E0) against
the spec and d2-sim: they match, no further change needed.

Checks (1.14d re-recorded, orig-cache not committed):
- before (base = staging-7 + integ-r23): gen-mon-279 state 150/150 equal,
  rng MATCH (verdict PARTIAL = recorder gaps) -> EQUAL count 1 of 1 run.
- after: unchanged; no code change in this branch.
- The other 10 checks of the cluster (280, 281, 360, 361, 362, 662, 664,
  686, 687, 712) were not re-run here; rc-extra-missile reports them equal.

Open: nothing for this cause. Unrelated: playthrough act 5 stops at
nihlathak-killed (stuck, frame 1000) on this base.
