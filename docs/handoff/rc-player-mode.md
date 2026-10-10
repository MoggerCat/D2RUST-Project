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
   Spec: `specs/missiles/missiles.md` R5 step 6.1 and R9.6.

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
