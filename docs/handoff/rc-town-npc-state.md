# rc-town-npc-state: hand-back (2026-10-10)

Run with `scenario_diff.py --reuse-orig` on the q-run-gen-wp-shrine 1.14d state recordings
(copied from that branch's `traces/orig-cache/gen-{wp,shrine}-*`; the cache key misses on this
branch because `tools/trace-recorder` changed, and there is no Wine prefix here, so no fresh
recording). Not re-recorded: treat the EQUAL counts as against those older recordings.

## Checks (state channel, 120/460 frames)
| Set | Before | After |
|---|---|---|
| gen-shrine-* (19) | 19 DIVERGED at frame 24 (class 201 m) | 15 PARTIAL through frame 120 (no divergence), 4 DIVERGED at frame 40 `game.seed` (18, 19, 21, 22) |
| gen-wp-30 (class 511) | DIVERGED frame 24 | frame 24 gone; next: frame 430 class 513 `tx` 0 vs 5074 (after travel) |
| gen-wp-18..26 (class 359) | DIVERGED frame 37 seed | unchanged (cause below) |

## What changed
1. Jerhyn (class 201) Npc class case: the seam defaults now give the no-palace-Jerhyn outcome
   (`0x0059F570` = 1, (a, b) = (1, 0), guard not moving; `quests-act2.md` §10), so the first think
   after home goes to the interaction step (mode 2 at frame 24) instead of idle 40
   (`wiring/action/pending.rs`). PROVISIONAL REC-1855: palace-Jerhyn states still unread.
2. `radius_point` (`monsters/ai/tactics.rs`) now implements the 1.14d-confirmed `0x005DE4E0`
   of `ai.md` §7.2 (full-size distance, s, k = min(|d-b|, a), `kx+ky<k` grow-both fix-up, no early
   exit). It replaces the provisional rounding geometry (REC-501); Warriv's three recorded walks
   and Jerhyn's frame-50 walk are in the unit test. Settles the frame 50-54 `tx` off-by-one.
   d2-sim: 4737 tests pass, clippy clean.

## Open
- Class 359 frame 37 (wp 18-26, 9 checks): SpecialState06 `0x005E7C10` has no spec (pc1-data.md item).
- `game.seed` at frame 400 (wp 1-8, 17, 28-38, 21 checks): not a sim bug; the 1.14d side never
  sent the 0x49 (poke `@wp` is a Gap). pc1-data.md item; the d2rs side is untested until re-recorded.
- shrine 18/19/21/22: `game.seed` at frame 40 (shrine effect draw; REC-1330 area), size S-M.
- wp 27 (class 405 tx, frame 427) and wp 30 (class 513 tx, frame 430): post-travel NPC walk target; M.
- Ledger part not written: `tools/coord/ledger.py` counts only `traces/checks/*.check`, so rows for
  `gen-*` checks fail `--check` (q-run-gen-wp-shrine's note). REC-1856..1859 unused.
