# rc-packets-s2c hand-back

Check `sys-intents-moves` (packets): 136/140 equal -> 140/140 equal; state 140/140 (q ignored).

## Changed
- `Pending::play_sound` (`0x00553380`) now queues the sound slot, so the NPC greeting (event 18, `0x005E68F0`) sends S->C 0x2C at the monster update (frame 68, was 0x6D vs 0x2C).
- The client vitals sync (0x95/0x96/0x18, `0x0052D980`) runs inside the forced flush (`Tick::pre_flush`), so the trace marks it as flush phase like 1.14d (frame 68, extra 0x96).
- 0x3A stat spend: the `0x0064C040` refresh queues the unit for update (`CombatView::refresh`), so the player update sends the single stat before the mod flush (frame 70, `1d 02` twice).
- Specs: triggers-2.md §14a, vitals.md §2. Ledger part `ledger/rc-packets-s2c.tsv` (9 rows, still DIVERGED, new cause).

## Ping replay
- The packets channel replays each recorded C->S 0x6D (system queue) at its recorded frame + 1 (`packets_channel.recorded_pings`); `PendingSession` answers 0x6D with S->C 0x8F (33 bytes, `0x0053E020`). The ping bytes are data from the recording; no clock in d2-sim.
- sys-intents-moves packets: 140/140 (MATCH), state 140/140.

## Open
- gen-missile-149 is not a regression of this branch (same on base 97f27b87 and on integ-r17): frame 2, the player AddUnit 0xAA is 39 bytes in d2rs vs 12 in 1.14d (ScnSor level 30, all skills 20). The 1.14d startup ping before frame 1 (frame null) is not replayed. Size M.
- net.c2s.0x06 (combat-melee-fallen): packets 147/150, first divergence frame 3 stream s2c #0 missing 0x07; state diverges at frame 46 (seed). Not touched.
