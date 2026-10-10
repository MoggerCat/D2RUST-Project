# rc-mon-missile hand-back (2026-10-10)

Branch `claude/rc-mon-missile` (from integ-r10 + staging-7). No REC ids used.
No Ghidra exports in the private repo (`re/` absent), so no 1.14d function read:
the cause was found from the recordings and the written spec.

## Finding
"Monster missile 320 never fired" was wrong: d2rs fires the Gloam (willowisp3)
bolt on the same frames and the three bolts match 1.14d to frame 38. The
divergence is that d2rs removes a bolt one frame early, when it reaches the aim
point. Cause: `path/walk/step.rs` one-step rule 3 (pathing.md §9.6 r3,
PROVISIONAL REC-1391) snapped missiles onto the current path point and took
index += 1, so a type-4 straight missile hit index == count and the next step
reset the path (`Step::Stopped`, missile freed). 1.14d keeps the direction past
the aim point until its frames run out or it hits (missiles.md §R4.3 step 5).

## Fix
Path type 4 missiles no longer snap (`step.rs`); types 10 and 14 keep the
REC-1391 snap (dru-tornado still 70/70). Spec note in `specs/sim/pathing.md`
§9.6 r3; test `a_straight_missile_keeps_its_direction_past_the_aim_point`
(fails without the fix). Ledger part: `ledger/rc-mon-missile.tsv`. PC 1 read
queued: pc1-data.md Step 4 "[rc-mon-missile]".

## Checks (state channel, orig-cache reused)
- diff-a4-* (12): EQUAL 0 -> 0 (2 PARTIAL 400/400 before and after). Missile-320
  first divergence gone in all 10 others; they now first-diverge at frame
  53-81: `-bm` ones monster `m` 2/7 vs 1 (class 120, mode/AI timing), the rest
  player `fc` 256 vs 0 after the Fire Bolt volley (frame 61/63/81).
- gen missile/tx group (9 run): gen-umod-23 DIVERGED -> PARTIAL 150/150;
  gen-su-15 missile yf -> frame 43 game seed. Others unchanged.
- Regression sweep (14 missile/skill checks: fire/ice/charged bolt, fire ball,
  lightning, chain lightning, holy bolt, magic/fire arrow, tornado, twister,
  frozen orb, quill rat, monster missiles, fire blast): all still PARTIAL, all
  frames equal. ass-blade-fury first divergence moved 39 -> 40.
- d2-sim: 4738 tests pass, clippy clean.

## Open
1. gen tx group (su-47, -52, -51, -36, -8, -22; M): monster in walk mode keeps
   path target = requested player point (1.14d, path result 0 / not moving) but
   d2rs sets target = last A* point 1 step away (5148,4269 vs 5143,4263).
   pathing.md §3 step 10 / §4 area; owner: monster walk path (q-fix-seed-order
   / path owner). Not the missile.
2. Player `fc` 256 vs 0 after a hit (a4 unique/champion; M): hit-reaction
   animation, REC-592 area.
3. Monster `m` 2/7 vs 1 timing in -bm (M): monster AI.
4. gen-boss-156 / -526 have no check file on this tree.
