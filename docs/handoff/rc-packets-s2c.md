# rc-packets-s2c hand-back

Check `sys-intents-moves` (packets): 136/140 equal -> 139/140 equal; state 140/140 (q ignored).

## Changed
- `Pending::play_sound` (`0x00553380`) now queues the sound slot, so the NPC greeting (event 18, `0x005E68F0`) sends S->C 0x2C at the monster update (frame 68, was 0x6D vs 0x2C).
- The client vitals sync (0x95/0x96/0x18, `0x0052D980`) runs inside the forced flush (`Tick::pre_flush`), so the trace marks it as flush phase like 1.14d (frame 68, extra 0x96).
- 0x3A stat spend: the `0x0064C040` refresh queues the unit for update (`CombatView::refresh`), so the player update sends the single stat before the mod flush (frame 70, `1d 02` twice).
- Specs: triggers-2.md §14a, vitals.md §2. Ledger part `ledger/rc-packets-s2c.tsv` (9 rows, still DIVERGED, new cause).

## Open
- sys-intents-moves frame 99: 1.14d sends S2C 0x8F (pong) in the input phase of frame 98 for the original client's wall-clock ping. d2rs never pings, so the message has no source. Size S; needs a decision on whether the d2rs client pings at a fixed tick (client/transport owner).
- gen-missile-149 (missile.shout): the first divergence is no longer frame 12 0xA8 but frame 2 (join burst: d2rs sends 0x59 ... 0x7E and 0xAA of 39 bytes in the tick, 1.14d has them in the earlier frame). Same result on the base commit, so not from this work. Size M, join sequence (`intents-events.md` §8).
- net.c2s.0x06 (combat-melee-fallen): not re-run.
