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

## q-save-audit rows taken here

- `q-fix-save-autosave` (F12): the 8192-frame save above; through the
  same `FileStore` (`write_file` keeps the `.bak`), by frame number;
  `synthetic_game::three_periods_of_ticks_save_three_times` counts the
  saves over 3 × 8192 frames (8192, 16384, 24576).
- `q-fix-save-exit-order` (F13) and `q-fix-play-exit-resource`:
  `play::after_run` leaves through the server first (the leave writes the
  file), then tears the automap down; both through `get_resource_mut`, so
  a missing `WorldViewState` or `BridgeResource` no longer panics;
  `smoke_save::the_window_close_leaves_through_the_server_first`.

- `q-fix-save-status` (F7, the dead bit): `d2-sim` `ClientEntry::status_set`
  and `UnitLists::set_player_status` (`0x00538650`); the DT start sets
  0x08 after the penalties (`0x00580F83`), the DD start after the corpse
  (`0x0057FD46`), softcore too; `save::Live::status_set` is ORed into the
  header. `smoke_save::a_corpse_with_its_items_survives_save_and_reload`
  now asserts status 0x0028 after a softcore death and respawn, and after
  the reload's save. Changed to the spec: `app_play_hardcore::
  a_softcore_death_respawns_and_the_save_stays_alive` expected 0x08 clear
  (§2.3 says set). Left: the progression bits 8–12 (`0x00538680` at the
  act-credit sites) and the ladder bit (no ladder in single player).

- `q-fix-save-map-seed` (F6): `save::Live::map_seed` (the act-0 DRLG
  init seed = game +0x7C) is written to +0xAB; `single_player::game_seed`
  picks the seed `play` builds with: `--seed N` (game +0x84 = 1) wins, else
  a save whose town byte for the difficulty has 0x80 restores +0xAB
  (`d2s.md` §2.2 r8), else `DEFAULT_SEED` (REC-291 (4): 1.14d draws a
  fresh one). `main.rs` `--seed` is now optional. Tests:
  `single_player::new_character_tests::the_map_seed_comes_from_the_switch_then_the_save`;
  `play_smoke::the_live_run` reloads with `game_seed` (as `play` does) and
  asserts the saved seed, the town byte and the same rects of levels 1–3
  (M08: without the write the saved seed is 0 and the leg fails).

- `q-fix-save-hotkeys` (F4): the save encodes the client record's 16
  slots (`SimGame::hotkeys`, written by C→S 0x51) with the item GUID as
  its 1-based inventory position (`save_gaps::hotkey_slots`, §2.4 r1–r2,
  r8); the load decodes the save's slots, resolves the indices to GUIDs
  over the loaded items (`save_gaps::loaded_hotkeys`, r4, r6.1) into the
  join entry (so the join's 0x7B go out), and `SessionFlow` writes them to
  the client record (r5). Seam fix found on the way: the 0x51 check
  `ActionPlayer::has_skill` read `Pending::skill_list`, which the play
  host never provides, so every 0x51 in play was refused with 3; it now
  reads `ActionHooks::skill_list_of` (the wired list, else the Pending's).
  Tests: `smoke_save::a_hotkey_bound_in_play_round_trips` (a real 0x51,
  the saved `code | 0x8000`, unbound `FF FF 00 00`, the reloaded slot),
  `app_save_gaps::hotkey_items_are_saved_as_positions_and_loaded_as_guids`.

- `q-fix-save-npc-fields` (F5): `d2-sim` `PlayerQuests::first_talk`
  holds field A (set `0x00572360` = `hear`, test `0x005723C0` = `heard`,
  bit by `d2s::npc_bit`); `intro` stays field B, with `intro_bits` /
  `set_intro_bits` for the writer and reader. The intro chains of acts
  I, III, V and Malah's quest kept their first-talk bits in `intro`
  (field B) — a conflation of the two fields; they now use field A, and
  the `QuestWorld` defaults of `npc_intro_heard` / `set_npc_intro`
  (chain 38, act II, which reported "unhandled") use it too. The save
  writes A and B (`Live::npcs`); the app's loader restores both.
  Changed to the spec (§6 r3, `quests.md` §6.7): `act1_intro_first_talk`,
  act III `intro_bits` / `intro_active_and_status`, act V intro tests and
  `q1_tests` read / seed field A instead of `intro`. Test
  `smoke_save::npc_fields_round_trip` (Kashya heard in Normal → A =
  `08 00 …`, the measured save).

- `q-fix-save-old-version` (F8): `apply_live` upgrades a loaded
  0x5C–0x5E file to 0x60 with the mask stats turned into bit-field
  entries at layer 0 (§1 r6, §7.1 r7); `FileStore::save` refuses to
  overwrite such a file when a loaded item list would pass through as
  loaded (`save::old_items_pass_through`: no inventory model, no
  hireling / golem unit), so the old-layout records never sit under a
  0x60 header; the file and its `.bak` stay. Tests:
  `app_save::an_old_version_save_is_written_as_0x60`,
  `app_save::old_item_records_that_pass_through_are_named`. Local check
  still owed: a real 0x5C–0x5E save loaded and re-saved on PC 1 (row text).

## Open

- The save's inventory flag bit 1 clear (`inventory-moves.md` §6.1 r4).
- The DD-start save (`vitals.md` §4.8 r2) is still asked by the app
  (`hardcore.rs`, REC-126), now written by the server's storage.
- `flows/save-exit.md` OQ1 (which client code sends 0x69 on the Esc path).
- The `.d2s` bytes themselves: q-save-audit.
