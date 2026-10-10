# rc-pets hand-back

REC-2090 (imp teleport point), REC-2092 (monstats chain position). Branch `claude/rc-pets`.

## Checks (cloud, 1.14d under Wine, `--orig-cache`)
| check | before | after |
|---|---|---|
| sys-pets-skeletons | ledger DIVERGED@102 (stale) | state PARTIAL (no difference), rng MATCH 133/133 |
| gen-mon-492..496, 688, 689, 714 (imp1-8) | DIVERGED@38 (x), rng DIVERGED@70 | state PARTIAL (no difference), rng MATCH 133/133 |

Re-run after the merge with staging: same results. EQUAL rows: 18 (imp1-8, system.sim.pets 1-10) in `docs/handoff/ledger/rc-pets.tsv`.

## Findings
- The 10 pets rows were already fixed on the staging branch: the check
  passes unchanged. The `x` after the 2nd Raise Skeleton was not the pet
  list (0x00575D90/720/850/C70 already match); the rows were stale.
- The imp rows had three causes, all outside the pet list:
  1. Imp Teleport (srvdo 129, no O/no T) took the target point from the
     host seam `Pending::path_target_point`, which is (0, 0) in the real
     host, so `mon_teleport` placed nothing. Now the path record target
     (+0x10/+0x12) is read first (`skill_rooms.rs cast_target_point`).
  2. The frame advance 0x00623E00 (+0x44 7936 -> 0 at the teleport) was a
     no-op seam; staging landed the same fix meanwhile
     (`units::anim::advance_frame`, `refresh_unit_animation`), so mine was
     dropped in the merge and its three test edits with it.
  3. `chain_position` (monstats +0x4B) was the default 0: imp2-8 fired
     class 592 instead of 593+. Now computed from BaseId/NextInClass
     (`ActionHooks::monster_chain_position`, same walk as the fix-up).

## Open
- Not re-run (record 1.14d ~35 s each): the other 330 gen-mon checks.
  Pre-merge sample of 19: 16 PARTIAL/MATCH, 3 DIVERGED (gen-mon-102/104/
  117, already DIVERGED in the ledger). The chain position now feeds every
  monster missile that adds it (srvdo 132, 3.x): run `gen-mon-*` once.
- `prop_walk_motion chase_a_moving_target` failed once on the untouched
  tree (proptest, path re-path counters), passed on the full run later.
- KB/S3 event-0 order (step, refresh, complete) is taken from the spec;
  0x005A74A0/0x005A8630 are not in re/exports (S, ~1 h Ghidra).
- dac-farren / magma-torquer (superunique, imp classes) not re-run.
