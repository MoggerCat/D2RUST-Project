# rc-npc-goto6 hand-back

Result: no code change. The remaining divergences are fixed by claude/q-fix-npc-interact (not merged).

## Checks (packets channel, staging f6fc17e3, 1.14d re-recorded fresh under Wine)
- Before (rc-goto-settle hand-back): 6 DIVERGED (cain, elzix-gamble, fara-heal, jerhyn, meshif, warriv).
- Now on staging: cain, jerhyn, meshif = MATCH (their divergences were already gone; likely stale
  orig-cache). Still DIVERGED: 3.
  - fara-heal f7: 1.14d sends the 13-byte 0x95 vitals sync inside the frame-7 flush (after the poke);
    d2rs sends it in the frame-8 tick (buffer 66 vs 53). q-fix-npc-interact 82d8a567 moves the sync
    to Tick::flush_sync (combat/vitals.md 5.1 r1).
  - warriv f36: 0x2c unit sound missing in d2rs; same branch routes NPC sounds through the unit sound queue.
  - elzix-gamble f18: 0x9c item byte 13 (and state item 4:1 class 522 field m); same branch adds
    gamble node inventories.
- EQUAL 3/6 -> 3/6 here; expected 6/6 after the merge (unverified).

## Why not verified
- Trial merge of q-fix-npc-interact into staging conflicts in ~14 files (d2-sim ai/objects/quest_objects,
  d2-client state_dump + e2e_full_loop, 6 specs, pc1-data.md). That is the branch's merge job; aborted.
- Running its tip alone is not comparable: it predates the goto-settle recorder fix, so all a2-npc
  checks diverge at the landing (0x07).

## Open
- Merge q-fix-npc-interact, then rerun the three: suite.py --filter 'a2-npc-fara-heal' etc. with --fresh.
- Ledger rows not written (no area ids for these NPC services settled; nothing newly settled).
