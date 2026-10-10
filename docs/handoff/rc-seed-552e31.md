# Hand-back: rc-seed-552e31

Branch `claude/rc-seed-552e31` (specs-staging-7 + integ-r23 + staging-7 tools). No code change.

## Result: the 0x552e31 cause no longer reproduces

Cause row: "1.14d game-seed draw d2rs lacks, site 0x552e31" (6 checks, example
gen-mon-190). 0x552e31 is inside `FUN_00552DF0` (new-unit seed from the game
seed, called by unit allocation `0x00555230`). The draw is now made on this base.

Checks, run on this tree (1.14d re-recorded, rng / state channel):
- gen-mon-190..194, 681 (maggot eggs): rng MATCH 100%, state PARTIAL 150/150.
- gen-mon-69/70/71 (sandmaggot2-4): rng MATCH, state PARTIAL 150/150
  (rc-sandmaggot had them DIVERGED@100).
- Whole gen-mon-[67][018]* glob (42 checks): every row rng MATCH, state
  PARTIAL 150/150. No rng or state divergence left in it.

EQUAL before -> after: 0 -> 0 (state is PARTIAL by design: ignored fields,
`ignore q seed`); rng-diverged rows 6 -> 0.

Fixed upstream by: rc-maggot-seed (hatch/KillBy/DeadFootprint/skill level),
rc-sandmaggot (`dir64` for MagottLay `0x005CB3C0`), rc-seed-order (ancient
equipment items, gen-mon-540..542).

## Open
- No ledger rows flipped (no row reaches EQUAL); no ledger part written.
- The cause row in `rc-gen-mon-causes.tsv` can be marked FIXED.
- Not run here: gen-ai-sandmaggot @112 and gen-ai-sandmaggotqueen @43 (separate, M).
