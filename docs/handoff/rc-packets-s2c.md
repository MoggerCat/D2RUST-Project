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

## Join burst (gen-missile-149 and the other gen checks)
- 1.14d `0x00646D60` ends with `0x00639E30` (state-changed bit only), so the join's 0xAA lists only state 105 and the first player update sends the passive states as 0xA8 (with lists). d2rs set the unit bits at load, so its 0xAA was 39 bytes. Now: the load sets the changed bit; the unit bits go on at the end of the join (`skill_events::passive_states_on`, PROVISIONAL REC-2105: the 1.14d site that turns them on is not identified).
- gen-missile-149 frame 2 is equal; first divergence is frame 3 (below). gen-missile-101/104/105/106 show the same frame-3 divergence.

## Open
- Frame 3 (all gen checks with `poke warp`, and combat-melee-fallen): 1.14d logs the warp's first S->C 0x07 at frame 3 (the poke runs at tick_end of frame 3, its sends are flushed in frame 3). d2rs runs pokes before host frame 4 (`send.rs` `set_before_pump`) and its sends reach the buffers only at the end of tick 4, so the whole warp lands in frame 4. Fix: run the pokes in the host's tick-end hook (`Host::frame_with`) of frame f-1, or drain `take_sent` right after the pokes. Needs care for the state snapshot order (state-dump snapshots f-1 before pokes). Size M, owner: harness/`send.rs`.
- net.c2s.0x06 (combat-melee-fallen): packets 147/150 (same frame-3 0x07); state diverges at frame 46 (seed). Not touched.
