# rc-walk-y1 hand-back

Task: rc-queue row "player/hireling y differs by 1 subpixel step at first walk step" (44 checks on r10).

Re-measured on r18 (specs-staging-7 + integ-r18 merge), `suite.py --filter 'a1-quest-*,a1-warp-*'` with orig-cache:
- 38 checks: MATCH 38 channel-wise on rng/packets, state PARTIAL 36, DIVERGED 2. 10697/10977 ticks equal (97.4%).
- No player or hireling y difference remains. The row is STALE (fixed upstream of r18); no code change.
- The 2 DIVERGED are a different cause: monster y at frame 21 (first):
  - a1-warp-l31-jail-3-ama: monster 1:32 class 39 y 1.14d 8159 vs d2rs 8156
  - a1-warp-l33-cathedral-ama: monster 1:11 class 22 y 1.14d 4947 vs d2rs 4948
  Both start at frame 21 (monster movement/walk step), 280 / 1400 diffs. Belongs to d2-sim monster movement; size S-M. Not investigated here.
- Playability: act 1 stuck at den-of-evil-done (frame 1000), act 5 at nihlathak-killed; not this task.

Changed: ledger part docs/handoff/ledger/rc-walk-y1.tsv (1 row), this file.
Open: monster y divergence above (needs its own owner).
