# rc-player-mode (REC-2870..2879)

Assigned: "state player field m", 13 checks. Result: 13 of 13 EQUAL (2 before).
Wine 1.14d recorded, 2026-10-10.

## Causes (neither was player-mode logic)
1. Missile area scan had no provider (area missiles hit nobody). Same fix
   as REC-2660 (b0711a889, already on the base); I dropped my duplicate.
   -> gen-boss-250, gen-mon-436/437/438/441/442/443/466/595/597/598.
2. Just-hit list (state 86, `0x005ADB00`) had no remove callback of its own.
   1.14d sets `0x005ADAF0` = state 86 off; in d2rs the state never left the
   unit, so a NextHit missile's later contact (explosion damage) was refused.
   Fix: `callback::JUSTHIT`, set in `apply_justhit`
   (`wiring/action/missiles.rs`), run in `ActionHooks::list_removed`
   (`wiring/action/units.rs`). -> gen-boss-544, gen-mon-533.
   The same fix landed on integ-r23 while I worked; merged to theirs.

## Checks
- All 13: state 150/150 (PARTIAL only for ignored harness fields), rng MATCH
  where recorded.
- Sample after fix 2: gen-boss-*, gen-mon-4*/5*, gen-ai-s*: 207/213 equal;
  the 6 others (boss-242/267/333/526/707, ai-sandmaggotqueen) were not EQUAL
  before (seed / tx causes), none regressed.
- Gates: fmt, clippy -p d2-sim -p d2-client, nextest -p d2-sim,
  coverage, spec_index, ledger --check.

## Open
Nothing in this cause. Seen, not mine: gen-boss-242 f59, 267 f93, 333 f71,
707 f51 (seed), 526 f69 (tx), gen-ai-sandmaggotqueen f43 (seed).
Ledger: `docs/handoff/ledger/rc-player-mode.tsv` (13 rows EQUAL).

## Next causes (coordinator list)
- C028 player sp 128->64 (8), C050 sp 80->40 (3), C033 player m 4->5 (6):
  all 17 checks (gen-mon-295/652/629/630/576/578/624, gen-umod-27,
  gen-monskill-212/335/210/211/339-343/301) are EQUAL on integ-r23 already
  (fixed by the two causes above); ledger part `rc-player-state.tsv`.
- C026 player q (8 rows): all 17 checks of the 7 rows equal on integ-r23
  (stale rc-run-1 verdict and a stale checks-status line); `rc-player-q.tsv`.
- C057 player s (gen-boss-267, gen-monskill-348): EQUAL now. Cause: the
  missile damage set-up found no weapon for a monster that wields one;
  1.14d takes the base from stats 23/24 for a grip of 2 (Blood Raven's bow:
  2..6, not 21/22 = 4..6), so the draw range and the hit differed.
  Fix: d2-client `weapons::sync` copies monster equipment into the hands
  (PROVISIONAL REC-3270). Spec: `specs/missiles/damage.md` §1 step 6.
  Sample (gen-boss/mon/ai/monskill 2xx-3xx): 605/618 equal, the 13 others
  are monster-field divergences that were not EQUAL before.
