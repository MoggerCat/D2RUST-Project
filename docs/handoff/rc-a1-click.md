# Handoff: rc-a1-click (2026-10-10)

Branch `claude/rc-a1-click`. REC ids used: none (nothing PROVISIONAL added).

## Result
`python3 tools/playthrough/playthrough.py traces/playthrough/act1.play --build`:
14/17 -> **17/17**. d2rs-only (no 1.14d side), so no ledger rows (header-only
part). Other .play files, before -> after (same results, no regression):
act2 14/15, act3 15/15, act4 13/13, act4-forge 6/6, act4-blockers 6/6,
act5 12/13, classes 8/10, milestones-a3-baal 4/4.

## Cause
The hover pick (`bridge/hover.rs`) was fine and on. `poke pos`/`hop` moved the
unit on the server only: the client model kept the old cell (and the camera
the old player cell), so the click hit empty ground and walked.
Fix (`d2-sim/src/poke.rs`, `mark_reassign`): a successful `pos`/`hop` queues
the unit for update and sets flag-ex 0x10000, so the next update sends S->C
0x15 like a placement (`sim/path-placement.md` §6 r4). d2rs-own harness
behaviour.

## act1.play
- First click on Akara/Warriv plays the intro: their text list is
  (0, string 0), (0, string 183/76); m = smallest kind-0 id = 0 (as the
  spec's `0x00661400`), so C->S 0x31 carries m 0. Chat end (C->S 0x30, the
  headless run has no menu to close) via `poke msg 0x30`, then a second click
  gets the quest message.
- act2-open: `poke msg 0x38 0 @1:155 1` stands in for the menu pick
  (`ui/panels/npc.rs` option_intent).
- den-of-evil-done: `need quest 1 1 set` -> `need ever quest 1 1 set` (1.1
  clears once 1.0 is set).

## Open
- Headless state-dump does not route Esc/menu clicks to the NPC UI (size M):
  the 0x30 / 0x38 pokes stand in.
- Pre-existing blockers, unchanged: act2 summoner-killed, act5
  nihlathak-killed, classes summon-follows-wp.
- Not run: scenario-diff checks using `pos` pokes (the extra S->C 0x15 only
  reaches the client model; state fields are unaffected).
