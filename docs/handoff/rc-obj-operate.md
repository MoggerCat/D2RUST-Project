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

## Update after merging integ-r23
Rerun of the 9 cause-1 checks: 8 now packets MATCH (rows 26, 39, 40, 41, 57, 58, 59 EQUAL). EQUAL now 0 -> 13 of 17. Left: gen-obj-149 (row 24), the player's PlaySound 0x2C ahead of the object's 0x0E (cause 1, size M, row in causes tsv). Cause 2 is rc-run-3b's (REC-3160).

## Player mark (REC-3450)
Player update placed in the room update walk: `PLAYER_ITEMS_MARK` (after the 0x15) and `PLAYER_SOUND_MARK` (after the mode messages) are host marks like GROUND_ITEM_MARK; the update pass skips what went out there. gen-obj-149 packets MATCH. object.operate rows EQUAL 13 -> 14 of 17 (3 left: urn and corpse by rc-run-3b's early-exit cause; row 60 has no objects.txt row, null table entry, nothing to check).
Re-run after the change (cached 1.14d where possible): 181 operate checks + gen-item*, gen-itemq*, gen-npc*, gen-wp*, gen-shrine*, gen-netc2s* (320 checks): 22 DIVERGED runs, none new: gen-wp-28 (state, level seed), gen-shrine-7/17 (packets: 0xA8 size; player 0x2C event 0x18 missing and 0x26 content), gen-npc-drehya/natalya/tyrael1/tyrael3 (known), 12 gen-item/gen-itemq items checks (stream byte after the head; the same two checked on the baseline build without this change diverge identically, so they are not caused by it).
New open items: gen-shrine-17 d2rs sends no player 0x2C (1.14d event 0x18) at frame 40; gen-item-05/10, gen-itemq-* item bit-stream byte difference after the head (ledger had them MATCH; not investigated).
