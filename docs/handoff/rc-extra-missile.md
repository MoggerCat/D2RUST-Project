# rc-extra-missile hand-back

Cause: "d2rs creates a missile (class 144 or 675) that 1.14d does not" and
"missile class off by 1-3".

Root cause: the inferno channel Frames helper (`0x005CC2E0`, spec
`specs/skills/bodies-3.md` §3.2) set the missile's total/left frames only
through a host seam whose default is empty, so the live host never applied
them. The missile lived its full `Range` (30) instead of `Param2 + L - 1`
(14 for fetishshaman2, level 8).

Fix: `crates/d2-sim/src/wiring/interaction/skill_use.rs` `set_missile_frames`
writes total/current into the missile store.

Checks (gen-mon, orig-cache; 1.14d re-recorded for 279 only):
- before: gen-mon-279 state DIVERGED at frame 54 (128/150).
- after: gen-mon-279, 280, 281, 360, 361, 362, 662, 664, 686, 687, 712 and
  gen-mon-60: state 150/150 equal, rng MATCH (verdict PARTIAL = recorder
  gaps, no divergence). 12 of 12 equal.

Not run: a gen-missile sample (the brief's second item); no divergence
seen in these 12 suggests none is needed, but it is unverified.
Open: the "class off by 1-3" cause (gen-mon-60) is now equal; it was the
same lifetime cascade, not a separate class-choice bug.
Others: rc-missile-missing untouched.
