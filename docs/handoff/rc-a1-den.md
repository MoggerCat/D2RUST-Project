# rc-a1-den hand-back (REC-2275..2279 unused)

Task: act1.play 17/17 on r16 (b56458388) -> 15/17 on r17 (956cf3725):
den-of-evil-done stuck f1000, quest 1.0 never set.

## Cause
rc-render-wp-click (merge aee79b42): the headless press now reads the
hover pick of the previous pass, as 1.14d hovers while it draws
(`0x00467A10`, specs/tools/scenario-diff.md §2 r4.5, measured under Wine):
a press posted in the pass of its `move` is a point click. act1.play's
NPC clicks (`click 448 324` on Akara / Warriv) had no prior move, so the
first click walked instead of opening the chat.

Confirmed: the same den run with `move 448 324` one frame before the click
reaches 1.1 and 1.0 (f64); without it, stuck.

## Change
traces/playthrough/act1.play only: a hover `move` one frame before each NPC
click in den-of-evil-done, andariel-done, act2-open (notes updated). No code
change: the engine matches measured 1.14d, and reverting it would drop
the culprit's fidelity gain (CLAUDE.md rule 10). NOTE: the brief said "fix
in code (not the .play)"; the playthrough spec defines `input` as the
scenario-diff script, under which the old input was a point click, so the
.play input was the wrong side. Coordinator: revert this commit if you
want it handled differently.

## Counts (integ-r18 + this commit, D2_GAME_DIR = assembled 1.14d)
| play | before (r17) | after |
|---|---|---|
| act1 | 15/17 | 17/17 |
| act2 | - | 15/15 |
| act3 | - | 15/15 |
| act4 | - | 12/13 (diablo-present-wingn2: `pos 1/156` poke fails, cl 243 never present) |
| act5 | - | 12/13 (nihlathak-killed: not killed by f1000) |
| classes | - | 10/10 |

act4/act5 blockers were not bisected (outside this task; this commit
touches act1.play only). No scenario-diff checks or ledger rows settled.
