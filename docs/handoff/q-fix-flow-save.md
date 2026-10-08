# q-fix-flow-save: saving on the server's path (2026-10-08)

Branch `claude/q-fix-flow-save` (from `claude/q-tick-flow`, merged with
`claude/specs-staging-7`). Findings T1, E1, E2, E3, C8 of
`docs/handoff/q-tick-flow.md`; flow `specs/flows/save-exit.md`.

## What changed

| Flow step | Where | What |
|---|---|---|
| §3 r1 (`tick.md` §6 r3) | `d2-sim` `tick/mod.rs` `client_pass`, `game.rs` | frame % 8192 = 0 raises `Game::character_save_due` before the per-client loop (single player too; no heartbeat drop modelled); `period::CHARACTER_SAVE` |
| §2 r2, §3 r1 | `d2-server` `adapters/storage.rs` (new), `adapters/sim.rs` | the character storage seam `CharacterStore`; `SimGame::set_storage`, `SimGame::save_characters` (`0x0052CA10`: every client with a player, client-list order); the tick writes the raised save after the sim's steps (REC-291 (1)); failures in `SimGame::save_faults` |
| §2 r2 | `d2-server` `adapters/session_flow.rs` `leave` | saves before 0x05 / 0x06 / 0xB0; `NotSaved` only when no storage is installed; `SaveFailed(String)` when the writer refuses |
| §1 r2 | `d2-client` `bridge/mod.rs` `Bridge::save_and_exit`; `world_view/present.rs`; `app/save.rs` `request_save_and_exit` | Esc → Save and Exit sends C→S 0x69 (system queue) and sets `exit_requested`; the app no longer exits at once |
| §4 r1 | `d2-client` `app/save.rs` `end_of_game` (added by `play::add_game`) | the app ends when `exit_requested` and not `in_game` (after 0x05, 0x06): the C8 reader |
| E3 | `d2-client` `app/save.rs` `FileStore`, `share`; `app/play.rs` | `play` installs a `FileStore` (read_live → apply_live → write_file) as the server's storage on the server thread; the after-`app.run` client save is gone; a window close leaves through 0x69 (`play::leave_game`, REC-291 (2)); the death saves (`hardcore.rs`) ask the server's `save_characters` through `SaveHandle` |

## Tests

- `d2-sim` `tick::tests::the_client_pass_raises_the_character_save_every_8192_frames`
- `test-fixtures` `synthetic_game`: `leave_saves_the_character_before_its_messages`,
  `a_refused_save_is_a_session_fault_and_the_leave_goes_on`,
  `the_tick_saves_every_8192_frames`; `leave_sends_its_messages_and_removes_the_client`
  unchanged (no storage installed → `NotSaved`, comment reworded)
- `d2-client` `bridge::tests::save_and_exit_sends_0x69_and_waits_for_the_server`
- `d2-client` `play_smoke::the_live_run`: the save leg now installs the
  `FileStore`, opens the Esc menu, clicks Save and Exit, waits for the
  app to end on the server's answer, asserts C→S 0x69 was sent, the client
  removed, no save fault and the file written by the leave; then loads it
  and compares level, stats, skills, items, **waypoints and quests**
  (snapshot extended). M08: with the leave's save removed the leg fails at
  "the server's leave wrote the save".

## Open

- The save's inventory flag bit 1 clear (`inventory-moves.md` §6.1 r4).
- The DD-start save (`vitals.md` §4.8 r2) is still asked by the app
  (`hardcore.rs`, REC-126), now written by the server's storage.
- `flows/save-exit.md` OQ1 (which client code sends 0x69 on the Esc path).
- The `.d2s` bytes themselves: q-save-audit.
