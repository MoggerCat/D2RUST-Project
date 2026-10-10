# rc-wp-poke-travel

Cause: `tools/trace-recorder/poke.py` raised a Gap for every `@wp`, so the 1.14d side of
`poke msg 0x49 @wp <level>` (gen-wp checks) never sent the travel; d2rs travelled, so the
game seed diverged at frame 400 for a tool reason.

Fix (tools only): `@wp[#n]` resolves among objects (type 2) of the 16 waypoint classes
(specs/world/waypoints.md §5 rule 1: 119, 145, 156, 157, 237, 238, 288, 323, 324, 398, 402, 429,
494, 496, 511, 539), as `@2:<class>`. Selftest passes. d2rs side was already right
(`waypoint_classes`, operate fn 23). No check file changed.
Not done: `send.py` has the same `@wp` Gap (line ~325); the gen-wp checks don't use it.

Verdicts (`--filter 'gen-wp-*'`, 39 checks): before = diverge at frame 400 on game seed (per
brief); after = 0 EQUAL, 37 DIVERGED, 2 PARTIAL (wp 0, 9; 460/460 equal). The first
divergence moved to frame 401+, i.e. both sides now travel:
- wp 1,3-8,12-17,29,31-38 (about 25): frame 401, the departure town's waypoint object, field m:
  1.14d 1 vs d2rs 2 (1.14d re-runs init 17 / arrival list §5.1 on the object; d2rs keeps mode 2).
- wp 10, 11, 28: frame 401 game seed / monster class differs after travel.
- wp 2: frame 400 player x 5098 vs 5338 (walk/placement).
- wp 18-26: frame 37 monster 1:6 class 359 seed (before the poke, not travel related).
- wp 27, 30: frame 427/430 monster tx 0 vs 5088/5074.
Open: sim side of travel (arrival list / object init after travel, §5.1, §7.1) is the next
root cause; the biggest cluster is the m 1 vs 2 one. Not investigated for lack of budget.
