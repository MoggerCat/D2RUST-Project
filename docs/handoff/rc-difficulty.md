# rc-difficulty hand-back (2026-10-09)

Branch `claude/rc-difficulty` (from integ-r10 824d5aa8). No REC ids used. No code changed.

## Checks (36 diff-a{3,4,5}-{nm,hell}-*, current tree, orig-cache reused)
- EQUAL before: 0 of 36. EQUAL after: 0 of 36 (nothing fixed). 2 are PARTIAL 400/400 (a4 normal non-bm, no drop on either side).
- Earlier "first divergences" (frame 4 seed / spawn x; hp 593221 vs 605952) are gone on this tree: stats, hp scaling, seed and spawn position now equal.

## Finding: there is no shared difficulty divergence left
Stat scaling, levels, seeds are EQUAL until frames 39-90. First divergences now:
- a4 (all 12 except the PARTIAL ones): frame 39-59, 1.14d has monster missile class 320 that d2rs lacks. Monster attack/skill use. Owner q-fix-b-monster-combat.
- a5 and a3 -bm: frame 60-90, monster `m` 2 (or 5) vs 1, or player `fc` 256 vs 0 (player facing/hit reaction after the Fire Bolt volley). Owners: monster AI (q-fix-b-monster-combat), animation/hit recovery (REC-592 area).
- a3 non-bm (6 checks): frame 5 game seed after `poke warp 76`. Not difficulty specific: Normal `a3-warp-l76-jungle-1-ama` diverges the same way (frame 21 seed). Owner: DRLG / q-prov-recording.

## Open (sizes)
1. Monster missile 320 not fired in d2rs (a4, 12 checks): M.
2. Monster mode/AI timing + player fc after hit (a3/a5, ~20 checks): M.
3. Warp level-gen seed (a3 non-bm, 6 checks, also Normal): L.
Nothing here is fixable by a difficulty-table change; the 36 checks will move only when the above land.

Repro: `D2_GAME_DIR=$HOME/game python3 tools/scenario-diff/suite.py --filter 'diff-a*' --no-playthrough --orig-cache traces/orig-cache`
