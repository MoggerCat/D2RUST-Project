# rc-link-2 hand-back

EQUAL 2759 -> 2866, NO-CHECK 653 -> 581 (checked rows: 2759 -> 2866).

- Cause of most of it: `tools/coord/ledger.py` merge. Rows of group `coverage` that appear in several coverage parts kept the first part by name (coverage-a1a2); the rc-* re-measurements (rc-link-checks etc.) never replaced them. An rc-* part now supersedes a plain coverage row (exercised keeps the max). 68 rows left NO-CHECK, EQUAL +103 in total.
- `tools/coord/link_checks.py`: INDEX `ledger_area` is matched as a comma list.
- 4 rows settled by running on integ-r23 (Wine): monster.quest.{ancients,cow-king,griswold,hellforge-hephasto} -> gen-qkill-* state 180/180 equal, PARTIAL only for the d2rs client gap; EQUAL (part rc-link-2.tsv).

Not linkable (checked, left NO-CHECK):
- net.s2c 78 / net.c2s 14: a scan of 1457 cached 1.14d packets recordings: 1.14d never carries those ids in any check (only s2c 0x60 appears, in gen-obj-59/60, which have no packets channel). Strict rule (REC-2055/2056): no MATCHing packets channel carrying the id -> no link. The 4 gen-nets2c-* notes (0x14, 0x58, 0x7b, 0x8f) say 1.14d sends nothing; stay NO-CHECK.
- system.ui / render / formats / client: the remaining notes are windows-only facts awaiting a d2rs draw-list comparison, d2rs-internal seams, tooling or audio rows; no existing check exercises them (INDEX covers all the rest already).
