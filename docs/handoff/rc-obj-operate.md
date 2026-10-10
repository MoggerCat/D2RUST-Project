# rc-obj-operate hand-back

Command: `suite.py --checks-dir traces/checks/gen --filter <the 181 gen-obj checks of the 17 operate functions> --orig-cache traces/orig-cache --fill-cache --workers 3 --no-playthrough` (1.14d re-recorded under Wine for all: the cache key did not match; recordings not committed).

Finding: the gen-obj checks already carry `packets` in their channels (check_gen.py unchanged); no input/send line exists, the operate is `poke operate` (dispatcher call at the tick end). rc-packets-chan had only left the rows unrefreshed.

Checks: object.operate rows EQUAL 0 -> 6 (4 chest, 19, 20, 32, 34, 47); DIVERGED 0 -> 8 (24, 26, 39, 40, 41, 57, 58, 59); NO-CHECK 3 (3 urn, 14 corpse: 15 checks where 1.14d exits early; 60: no check). 181 checks run: 160+21; packets MATCH 154, DIVERGED 9, PARTIAL 18 (early exit).
State is PARTIAL on all (RUN_GAPS client gap), counted EQUAL under DECIDED REC-2055/2056.

Packets causes by count (`rc-obj-operate-causes.tsv`):
1. 9 checks: item 0x9C before object 0x0E at frame 20 (order of the client pass; 1.14d walks the room update list, items newest first, d2rs sends 0x0E in the tick and ground items in the server update_pass). Size M. Not fixed.
2. 15 checks: 1.14d exits during the operate (not comparable). Size S (recorder).
3. OperateFn 60: no check. Size S.

Also: object.populate/preset rows are level-generation rows (gen-lvl), not operate; not touched. Ledger part: `docs/handoff/ledger/rc-obj-operate.tsv` (17 rows).
