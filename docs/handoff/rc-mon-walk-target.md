# rc-mon-walk-target — hand-back

Task as briefed: "d2rs sets a walking monster's target to the last A* path
point while 1.14d keeps the requested point". **That premise is wrong**:
`0x00649970` step 10 does overwrite path +0x10/+0x12 with the last point
(flag 0x10 clear, no target unit), as `sim/pathing.md` §3 says; units 11/12
of gen-su-47 show both sides doing it. The tx differences were two
upstream causes, found by hooking 1.14d (scratch debugger on `0x005A62F5`
dumping the path after the mode-request compute).

## Checks (`--filter 'gen-su-*,gen-boss-*'`, 91 checks, state channel)
- before: EQUAL 0, DIVERGED 78, PARTIAL 13
- after:  EQUAL 0, DIVERGED 77, PARTIAL 14; no check moved earlier
- moved: gen-su-47 first divergence 31 -> 128; gen-su-52 31 -> 83;
  gen-su-51 51 -> all 150 frames equal (PARTIAL).
  The new first divergences in 47/52 are player 0:2 field `fc` (dead player
  frame count 256 vs 0), not minion walks.

## What changed (d2-sim)
1. **`0x005DC640` "can reach directly"** was a `Pending` stub returning
   false. ReanimatedHorde (`0x005E1540`) gates its Skill2 on it and the gate
   consumes one RNG draw, so with false d2rs took "walk in radius" where
   1.14d took "walk to target unit" (target unit set, so tx = player pos).
   Implemented (`monsters/ai/tactics.rs reach_directly_probes`,
   `wiring/action/ai.rs`, `wiring/path/units.rs unit_point_line_blocked`):
   three probes, `0x006229F0` mask 0x805, point size 2. Spec: `ai.md` §6.
2. **Walk-in-radius geometry `0x005DE4E0`** still had the old PROVISIONAL
   rounded no-size reading although `ai.md` §7.2 documents the 1.14d one
   (full-size distance, truncate then fix-up, sign s, no early exit).
   `radius_point` now takes the unit size and always returns a point.
   Tests: Warriv's 4 recorded walks, plus gen-su-47 unit 10 (size 2).

## Open (sizes)
- gen-su-12, gen-su-48: tx differs with equal seeds and positions after
  other causes (toward path point count, 1 vs 3 points at frame 31 in
  gen-su-12 style cases) — M.
- gen-su-47/52: dead-player `fc` 256 vs 0 (frame 128 / 83) — S.
- Remaining first divergences over the cluster, by field: fr 22, fc 19,
  m 13, s 9, seed 5, other 8 — each its own cause, none a path-target rule.
- gen-boss-526 "tx" at 69 is an RNG divergence (seeds differ), not pathing.
- Tooling note: running the suite with the default worker count raced the
  `blood-moor-empty` variant build ("hash table extends past end of file");
  build the variant once, then `--workers 3`.
