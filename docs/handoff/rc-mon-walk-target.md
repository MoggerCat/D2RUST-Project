# rc-mon-walk-target — hand-back

Brief: "d2rs sets a walking monster's target to the last A* path point while
1.14d keeps the requested point". **That premise is wrong.** `0x00649970`
step 10 does overwrite path +0x10/+0x12 with the last point (flag 0x10
clear, no target unit), as `sim/pathing.md` §3 says; gen-su-47 units 11/12
show both sides doing it. The tx differences came from two causes upstream
of the path, found by hooking 1.14d (scratch debugger break at `0x005A62F5`,
after the mode-request compute, dumping the path record).

## Checks (`--filter 'gen-su-*,gen-boss-*'`, 91 checks, state channel)
- before (staging aa5de032): EQUAL 0, DIVERGED 78, PARTIAL 13
- with my fixes alone: DIVERGED 77, PARTIAL 14 (gen-su-47 31 -> 128,
  gen-su-52 31 -> 83, gen-su-51 51 -> 150 frames equal; none earlier)
- after merging staging f6fc17e3 (which already carries the same two
  fixes, landed independently, plus the player-`fc` fix): DIVERGED 58,
  PARTIAL 33. gen-su-47/51/52 are PARTIAL (150/150 frames).
  EQUAL stays 0 only because the state channel reports PARTIAL for these
  checks (unchecked fields), not because of a difference.

## What was the cause (both already on staging; my duplicates dropped)
1. `0x005DC640` "can reach directly" was a stub returning false.
   ReanimatedHorde `0x005E1540` gates Skill2 on it and the gate consumes
   one RNG draw: false -> "walk in radius", true -> "walk to target unit"
   (target unit set, tx = player position). Seeds stayed equal because both
   paths draw twice, which is why only tx differed.
2. `0x005DE4E0` walk-in-radius geometry: full-size distance, truncate then
   fix-up (`ai.md` §7.2), instead of the provisional rounded reading.
Spec text for both is on staging (`ai.md` §6, §7.2).

## Open (sizes)
- gen-su-12, gen-su-48: tx differs with equal seeds/positions (toward
  path point count: 1 point in 1.14d vs 3 in d2rs at the first compute,
  `info.target` after target preparation, `0x00679C80` step 1-2) — M.
  Method: break `0x005A62F5`, dump path +0x10..0x28 and points from +0x9C.
- Remaining first divergences by field: fr 22, m 13, s 9, seed 5, cl 2,
  x 1, ty 1 — separate causes, none a path-target rule.
- gen-boss-526 "tx" at frame 69 is an RNG divergence (seeds differ).
- Suite tip: the default worker count raced the `blood-moor-empty` variant
  build ("hash table extends past end of file"); build it once, then
  `--workers 3`.
