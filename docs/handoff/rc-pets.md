# rc-pets hand-back

REC-2090 (imp teleport point), REC-2091 (monster frame advance), REC-2092
(monstats chain position). Branch `claude/rc-pets`.

## Checks (cloud, 1.14d under Wine, `--orig-cache`)
| check | before | after |
|---|---|---|
| sys-pets-skeletons | ledger DIVERGED@102 (stale) | state PARTIAL (no difference), rng MATCH 133/133 |
| gen-mon-492..496, 688, 689, 714 (imp1-8) | DIVERGED@38 (x), rng DIVERGED@70 | state PARTIAL (no difference), rng MATCH 133/133 |

EQUAL rows: 18 (imp1-8, system.sim.pets 1-10) in `docs/handoff/ledger/rc-pets.tsv`.

## Findings
- The 10 pets rows were already fixed on the staging branch: the check
  passes unchanged. The `x` after the 2nd Raise Skeleton was not the pet
  list (0x00575D90/720/850/C70 already match); the rows were stale.
- The imp rows had three causes, all outside the pet list:
  1. Imp Teleport (srvdo 129, no O/no T) took the target point from the
     host seam `Pending::path_target_point`, which is (0, 0) in the real
     host, so `mon_teleport` placed nothing. Now the path record target
     (+0x10/+0x12) is read first (`skill_rooms.rs cast_target_point`).
  2. `Pending::refresh_animation` (frame advance 0x00623E00) was a no-op
     in every host. Now `units::anim::advance_plain` (spec units.md §4.2)
     runs through `ActionHooks::refresh_animation`; this changes +0x44 and
     +0x4E of every monster event 0 that refreshes (attack family, KB, S3,
     SQ fallback, stepping death). Three d2-sim tests assumed the timer's
     code in +0x4E / a frame at the count; updated to the spec rule.
  3. `chain_position` (monstats +0x4B) was the default 0: imp2-8 fired
     class 592 instead of 593+. Now computed from BaseId/NextInClass
     (`ActionHooks::monster_chain_position`, same walk as the fix-up).

## Open
- Not re-run (record 1.14d ~35 s each): the other 330 gen-mon checks.
  Sample of 19 run: 16 PARTIAL/MATCH, 3 DIVERGED (gen-mon-102/104/117,
  already DIVERGED in the ledger; first-divergence frames not compared).
  Coordinator: run `gen-mon-*` once to catch any regression of REC-2091.
- `prop_walk_motion chase_a_moving_target` failed once on the untouched
  tree (proptest, path re-path counters), passed on the full run later.
- KB/S3 event-0 order (step, refresh, complete) is taken from the spec;
  0x005A74A0/0x005A8630 are not in re/exports (S, ~1 h Ghidra).
- dac-farren / magma-torquer (superunique, imp classes) not re-run.
