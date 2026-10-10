# rc-drops-order hand-back (2026-10-10)

EQUAL on integ-r23: 3456 -> 3457 (C067), 3454 -> 3456 (C010q, after merge).

## Done
- C067 quest kill order (items-drops-nor-09 category 16 vs 5, nor-12):
  1.14d runs the kill parse 0x00543A30 inside the kill, after the TC drop,
  so a quest drop (ass, hfh) is queued and announced in that tick's client
  pass. d2rs ran all quest events after the tick. Link + Kill events now
  run at the end of tick step 4 (`WiredWorld::run_kill_events`); the rest
  stay after the tick (REC-129). Spec note in specs/world/quests.md §4.4.
- C010q quest TC (items-drops-nor-07, Andariel dropped from "Andariel"
  instead of "Andarielq"): `Pending::quest_tc_open` had no host answer.
  The quest control publishes the players' quest flags once per tick
  (`publish_quest_flags`), the client host answers §3.3 from them; the
  caller resolves P (player, else minion owner, else R).
- items-drops-*: nor-07, nor-09, nor-12, hel-04, hel-05 MATCH; 0 regressions.
  Quest-boss checks: a2-quest-radament state 83 -> 135/138.

## Open (sizes)
- items-drops-nor-10 (S-M, combat, not drops): monsters 0x16/0x17/0x1b die
  at frames 183/187 in 1.14d and never die in d2rs, so gld/qui are missing.
- items-drops-nig-06 (S): ops placed at 5141,4266 vs 1.14d 5142,4267; one
  item, probably the monster's position at death (movement), not the drop.
- treasure.md §3.3 stat-list 0x800 owner (0x0063A690/0x00552F60) is not
  read (TODO in wiring/economy/death.rs); minion_owner has no host answer.
- items-drops-* frame-timing rows (cha-01/03, nor-04, rbo-*, uni-02/03/05)
  belong to C017 (claude/rc-drop-nor-timing).
- milestone-izual/-hephasto, items-vendor-nihlathak-stock: their orig-cache
  1.14d recordings report "fatal exit after 4 ticks"; re-record them.

Not run: the d2-client nextest (disk filled twice on its debug build);
clippy --all-targets on d2-client passed.
