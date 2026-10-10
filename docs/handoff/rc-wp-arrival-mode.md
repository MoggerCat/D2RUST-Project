# rc-wp-arrival-mode

Cause: d2rs scheduled the destination waypoint object's ENDANIM at frame + 0 instead of
+ 15. `WaypointData::new` expects the objects table after its load fix-up (`FrameCnt1` x 256,
fixups.md 13); `d2-client` passed the raw rows (15), and init 17 (`0x00547210`) reads `>> 8`.
1.14d keeps the object in mode 1 for 15 frames after arrival, d2rs for 1.
(The object is the destination's waypoint, not the departure town's as rc-wp-poke-travel said.)

Fix: `crates/d2-client/src/app/single_player.rs` `fixed_objects()` feeds both
`WaypointData::new` sites. Spec waypoints.md 5.1 notes it. Merged claude/rc-wp-poke-travel first.

`--filter 'gen-wp-*'` (39 checks, state channel):
- before: 0 EQUAL-ticks-complete (poke-travel hand-back: 37 DIVERGED, 2 PARTIAL)
- after: 17 PARTIAL with 460/460 ticks equal (wp 0,1,3-17), 22 DIVERGED.
- ledger: 17 rows in ledger/rc-wp-arrival-mode.tsv (state EQUAL).

Open, next first divergences (not this cause):
- wp 18-26 (9): frame 37 monster 1:6 class 359 seed, before any poke: monster spawn/seed owner.
- wp 27, 29 (2): frame 427 monster 1:1 class 405 tx 0 vs 5088.
- wp 30-38 minus: frame 430 monster 1:7 class 513 tx 0 vs 5074 (9 checks).
- wp 28: frame 401 monster 1:29 class 403 vs 308 (population after travel).
- wp 2: frame 400 player x 5098 vs 5338 (placement).
Not run here: nextest (d2-client change only, build + clippy clean).
