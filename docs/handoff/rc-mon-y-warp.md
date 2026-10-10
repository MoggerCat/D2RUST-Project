# rc-mon-y-warp hand-back

Task: monster y differs at frame 21 in a1-warp-l31-jail-3-ama (8159 vs 8156) and a1-warp-l33-cathedral-ama (4947 vs 4948).

Tree: specs-staging-7 + merge of integ-r19. Both checks re-recorded against 1.14d (not in orig-cache) and re-run:
- a1-warp-l31-jail-3-ama: state 160/160 equal (PARTIAL verdict, no divergence), rng 133/133 MATCH.
- a1-warp-l33-cathedral-ama: state 160/160 equal (PARTIAL verdict, no divergence), rng 133/133 MATCH.

Before (r18 numbers in rc-walk-y1): 2 DIVERGED, 280 / 1400 diffs. After: 0 DIVERGED, 0 diffs.
Cause: already fixed by changes in staging-7 / integ-r19 (the r18 measurement predates them). I did not bisect which commit.
No code change. Changed: ledger part docs/handoff/ledger/rc-mon-y-warp.tsv (1 row), this file.
Open: the PARTIAL verdicts on these checks come from other state fields, not monster y; not looked at here. The remaining a1-warp family was not re-run (only these two).
