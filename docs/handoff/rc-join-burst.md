# rc-join-burst hand-back (2026-10-10)
Branch `claude/rc-join-burst` = `claude/specs-staging-7` + `claude/integ-r23` + this ledger part.
No code change: the cause was already fixed on this base when the session started.

## Cause (census layer 1, `rc-packets-census.tsv`)
Frame 2 S->C #1: d2rs sent the player's join 0xAA with 39 bytes where 1.14d sends 12
(only state 105 alignment, stat 172 = 2), and the join burst 0x59..0x7E was not where
1.14d has it. The census was taken on staging-7 + integ-r17; integ-r23 carries the fixes:
- `e19366ab8` (PROVISIONAL REC-2105): the load's passive states set only the
  state-changed bit (`0x00646D60` -> `0x00639E30`), so the join 0xAA no longer lists them
  (39 -> 12 bytes); the unit bits go on at the end of the join and the first update
  sends them as 0xA8.
- `dfceb6d4d` / `4886262d2`: every post-tick poke runs at the tick-end hook, and the tap
  records the sends made before and after it.
Verified 1.14d frame 2 (gen-state-1, recorded under Wine): `0x59/26 0xAA/12 0x76/6 0x94 ...
0x7E`, then the town population (0xAC/0xAA 12/0x6D, 0x51/0x0E); d2rs now equals it record by record.

## Checks before -> after (1.14d re-recorded: poke.py moved the recorder hash; not committed)
- gen-state (all 184): packets MATCH 0 -> 184 (before: all DIVERGED@2 at the join 0xAA);
  state 184 PARTIAL, 0 differences, every field compared, no ignore line (client gap only).
- gen-missile sample (10-18, 149): packets MATCH 8/9; gen-missile-149 stays DIVERGED@12
  (S->C 0xA8 missing at the missile hit, census layer 3, already open, not this cause).

## Ledger
`docs/handoff/ledger/rc-join-burst.tsv`: 184 `state.*` rows -> EQUAL (REC-2055 rule:
packets MATCH and state PARTIAL only for RUN_GAPS). Merged ledger EQUAL 1488 -> 1672,
DIVERGED 940 -> 756.

## Open (sizes)
- M: gen-missile (264): not re-run in full; with the join fixed, the census expects 262
  MATCH (`packets_diff --from 3`: 522/524). Run
  `suite.py --checks-dir traces/checks/gen --filter 'gen-missile-*' --orig-cache traces/orig-cache`
  and write the rows.
- S: gen-missile-149 0xA8 frame 12; gen-netc2s-60 0x23 vs 0x47 frame 20 (census layer 3).
- M, not mine: the coordinator's ~203 gen-skill rows (and gen-obj) cannot be promoted
  by packets: gen-skill checks are `channels state` only (210/210) and gen-obj
  `state items rng` (571/571). They need a packets channel from `tools/check-gen/check_gen.py`
  (route.py owner) and a fresh 1.14d recording.
- S: `net.s2c.0xaa` and `tools.poke.tick-end` rows still read DIVERGED from hand-written
  checks (a1-*, a2-npc-fara-heal) that I did not re-run.
