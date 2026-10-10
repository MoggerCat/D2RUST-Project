# rc-intents-moves hand-back

Branch `claude/rc-intents-moves` (on staging-7 + integ-r16). REC ids unused.

## Check
`sys-intents-moves` (packets): DIVERGED@68 (137/140) -> **MATCH 140/140**.
State channel PARTIAL as before (100% equal ticks). Ledger: 9 rows
DIVERGED -> EQUAL in `ledger/rc-intents-moves.tsv` (row 7 of
intents-events stays with rc-player-hit's own check).
There was no shared move-intent bug: the walk/run/stat intents were fine;
three separate message-order causes, each the first difference in turn.

## What changed
1. Frame 68, S->C 0x2C missing: the local host's monster `play_sound`
   (`Pending`, AI NpcStationary greeting, event 18, `0x005E73A0`) was the
   trait's no-op. `LocalSeams::play_sound` now calls `queue_sound`
   (`0x00553380`), so the monster update sends `2c 01 <guid> 1200`.
2. Frame 68, 0x96 in the wrong phase: the vitals sync (`0x0052D980`) ran
   at the end of the sim tick; 1.14d runs it inside the flush before the
   buffers go. New `Tick::flush_sync` (default no-op), called by
   `Host::flush_inner`; `SimGame` runs `vitals_sync` there.
   Walk-test fixture calls it after `tick`.
3. Frame 70, `1d 02 1b` once instead of twice: AddStatPoint's refresh
   `0x0064C040` did not queue the unit for the update pass, so step 7's
   single stat send (dex) was missing before the mod flush. `CombatView::
   refresh` now queues the unit; the 0x3A handler runs on `CombatView`.
Specs: vitals.md §2 and §5.1, triggers-2.md §14 table note.

## Open
- State channel stays PARTIAL (unrelated to intents; not investigated).
- Only the 0x2C of NPC greeting was exercised; other monster
  `play_sound` callers (event 16 in bodies5) now also queue: unverified.
