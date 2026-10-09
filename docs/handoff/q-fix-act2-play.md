# q-fix-act2-play handoff

Branch `claude/q-fix-act2-play` (merged `claude/q-fix-boss-damage` @ ffbd7886).

## Done
- Act II playthrough before: reached 14/15, furthest consecutive 5/15 (first blocker
  `radament-killed`: "class 229 never present").
- Cause was not Radament's spawn (class 229 is present in level 49 from f1697): the
  harness `find_probe` ran only the 1000-tick deadline, not the whole sweep.
  Fixed on `q-fix-boss-damage`; merged here.
- Found while tracing: with `pos @1:229 @x+3 @y` the Fire Bolt died at its first step
  because the sweep's last player cell sits between missile-blocking cells (collision
  bit 0x4, `cell_walk` blocked). `goto unit 1:229` plus bolts in several directions
  kills him. The merged `act2.play` (boss-damage's version) already reaches; my own
  variant is in commit 14e9f6ed.
- After: reached 15/15, furthest 15/15 (merged tree).

## Open
- Act II quest flag sequences (Radament, Horadric Staff, Tainted Sun, Summoner, Seven
  Tombs) are not checked with `quest` predicates in `act2.play`; milestones only test
  kills and levels.
- Unverified: whether 1.14d also blocks missiles at the sweep's end cell (map collision
  bit 0x4 around (7687,8120) in level 49); no 1.14d side in the playthrough tool.

## Repro
`D2_GAME_DIR=/home/user/game python3 tools/playthrough/playthrough.py traces/playthrough/act2.play --build`
