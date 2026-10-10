# rc-sandmaggot hand-back (2026-10-10)

Task: sand maggot open items after rc-maggot-seed. REC ids unused.

## Checks (EQUAL = rng channel / state channel)
Before: gen-mon-72/679/716 rng MATCH, state DIVERGED@135 (x 5151 vs 5147);
gen-mon-69/70/71 rng DIVERGED@100 (missing `0x552e31`); gen-ai-sandmaggot,
gen-ai-sandmaggotqueen DIVERGED; gen-ai-andariel PARTIAL.
After: gen-mon-72/679/716 rng MATCH, state 149/150 (x equal; first diff now
frame 150, game seed). 69/70/71 unchanged (rng 97%, state @100).
gen-ai-sandmaggot DIVERGED@112 (game seed), queen DIVERGED@43 (game seed),
gen-ai-andariel PARTIAL 150/150 (re-run, nothing diverged). EQUAL count: 0 -> 0.

## Fixed (one root cause)
`UseView::dir64` (BodyWorld) called the host's `body_dir64`, whose default is
0 and which no client overrides. MagottLay (`0x005CB3C0`) therefore always
used k = 0 (egg at -2,-2). Now it uses the path provider's direction vector
(`0x00621DC0`), as the AI wiring already did. Spec note in bodies-3.md §5.11.
Other bodies using `dir64` (Andarial spray, etc.) change too; not re-run.

## Open
- gen-mon-69/70/71 (S-M): in 1.14d frame 100 has an extra unit allocation
  (`0x552e31`, ~81 draws, 5 unit-seed inits) with no new unit visible at
  101..150. d2rs shows a second ring-0 placement test for class 191 at the
  first egg's own cell (coll true: bits 0x1000 and 0x100 set, no 0x8000) and
  fails there. Suspect: the second spawn is not at that cell in 1.14d, or the
  egg's start footprint release differs for a laid egg (mode 8 at creation).
- gen-mon-72/679/716 frame 150 game-seed diff with rng MATCH (S): a draw
  after the rng window or a seed set not logged.
- gen-ai-sandmaggot @112, gen-ai-sandmaggotqueen @43 (M): not examined.
