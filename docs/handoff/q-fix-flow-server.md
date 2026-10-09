# q-fix-flow-server: server and client flow fixes (2026-10-08/09)

Branch `claude/q-fix-flow-server` (from `claude/q-tick-flow`), merged
with `claude/specs-staging-7` before every push. Rows:
`q-fix-flow-client-pass`, `q-fix-flow-after-tick`, `q-fix-flow-join-load`
(the join-sequence half named in the task), `q-fix-flow-act-change`,
`q-fix-flow-client-order` (findings in `docs/handoff/q-tick-flow.md`).
Saving is `q-fix-flow-save`'s; message bytes `q-proto-audit`'s.

## Done, in push order

| # | Findings | Change | Play-path test |
|---|---|---|---|
| 1 | T4 | Stat messages: the per-client update's changed-stat array flush (`0x006258D0`, `stat-lists.md` §11 r2) in `ActionSim::client_update_messages`, between the unit updates and the room switch; the flush-time cache diff is gone (the vitals sync, `vitals.md` §5.1, stays at the flush: the spec puts it there). Flag-ex bit 21 (set by the join's item messages, §8.2 r3.5) sends the inventory refresh's 0x48. | `app_single_player::the_first_tick_sends_the_stats_in_the_client_pass_before_0x04` (recorded frame 2: stats, 0x48, 0x04) |
| 2 | T2, T3, A5 | `d2_sim::tick` split at step 4 / step 5 (`tick_through_timers`, `tick_from_client_pass`); the host's unit work (approach and item arrivals, death starts, corpse items, pet deaths, approaches, hireling calls with NPC act changes, pet follows, hireling drive) runs at the end of step 4, before the client pass; the NPC handler's own work runs when the handler returns (drain). A corpse announced by the death pass is marked announced. | `app_single_player::a_death_reaches_the_client_pass_of_its_tick`; the arrival test `app_play_npc_approach::the_approach_arrival_talks_in_the_tick_the_run_stops` went with that file when q-fixture-migrate removed the synthetic world (see below) |
| 3 | T5, T6, J3 | State-3 / 5 inventory refresh (0x48 after 0x04); the join sequence (q-fix-proto's 0x5B, 0x65, 0x5A plus the spec's 0x8D, merged); quest event 3 CHANGEDLEVEL in the per-client level change on the lent quest control (`QuestObjectHost::changed_level`), the after-tick level detection removed. | the session test's tail `04 48 5B 65 8D 5A` (the recorded frame-2 order); `rooms::quest_event_3_runs_in_the_level_change` |
| 4 | A1–A4 | `act_change::run` in `waypoints.md` §11 order: state 5, spawn + free point (0x1C89, 5), leave O, enter R, switch to none, 0x05, unit act, 0x03, 0x53, switch to R, 0x15 via flag-ex 0x10000 and room-change messages, pets follow; no direct 0x04, no 0x59 / part B / 0x0B re-add. | `app_single_player::an_act_change_goes_through_state_5_and_the_client_pass_sends_0x04` |
| 5 | C2, C4 | Paused pass: `Bridge::set_paused` (set each frame by `pause_frame` from UI state 9 / 11): no pump, no receive, only the skill fallback; not while the local player has no room or after Save and Exit (`save-exit.md` §1 r3). `PreviewOrder` pins `deliver_outputs` → `preview_walk_frame` → `monster_walk_frame` between `bridge_frame` and `mirror_units`. | `smoke_frontend::the_esc_menu_pauses_the_single_player_game`; `bridge::tests::a_paused_frame_runs_no_pump_and_only_the_skill_fallback`, `present::order_tests::*` |

After q-fixture-migrate (staging `b463cda4`) the play path runs only on
the real install: this branch's `app_single_player` play-path tests and
`smoke_frontend::the_esc_menu_pauses_the_single_player_game` carry the
real-data ignore mark (`tools/realdata-gate.sh`; results below). The
removed files `app_play_npc_approach.rs` (with the arrival test of item
2) and `smoke_town.rs` (with this branch's rig fix: a ground click must
pick no unit) are in `q-fixture-migrate-removed.tsv` for rewrite on the
install; the arrival test belongs in that rewrite (an NPC 7–8 sub-tiles
from the player, C→S 0x13, the talk starts in the tick the server's run
stops).

PROVISIONAL: REC-291 (the per-client inventory refresh sends its 0x48
without the pass's item steps 1–7), REC-292 (the join's 0x8D party word).

## Left open

- The town-leave refresh `0x00537340` after quest event 3: the vendor
  records are not lent to the tick (TODO in `client_level_change`).
- Act change steps 1, 6, 7 and step 10's removal to other clients
  (TODO in `act_change.rs`); NPC act changes still need the paths.
- A paused pass draws in 1.14d; the d2rs world view draws only on a new
  server tick (`present.rs`), so a paused frame shows the last picture.
- C5 (Esc on a dead player read by three systems), C7 (doubled
  registrations): not in this task's list; still open
  (`q-fix-flow-client-order` row).
- T7 (expired items, arena sync hooks), J1, J2 (loader messages, refused
  load's 0xB4): `q-fix-flow-join-load` row's other half, not done here.
- The host unit work runs after the whole timer queue, not inside the
  event that raised it (d2rs-own order within step 4).

## Real-data run (M23)

Install from the private repo (`9710830`, `tools/cloud-game/fetch.sh`,
23 files, 0 mismatches), `D2_GAME_DIR=/root/game`, debug build:
`cargo nextest run -p d2-client --test app_single_player --test
smoke_frontend --run-ignored ignored-only` on this branch's tests:

| Test | Result |
|---|---|
| `app_single_player::the_first_tick_sends_the_stats_in_the_client_pass_before_0x04` | pass |
| `app_single_player::a_death_reaches_the_client_pass_of_its_tick` | pass |
| `app_single_player::an_act_change_goes_through_state_5_and_the_client_pass_sends_0x04` | pass |
| `smoke_frontend::the_esc_menu_pauses_the_single_player_game` | pass |
| `smoke_frontend::esc_options_save_and_exit_then_reload_the_character` (Save and Exit from the Esc menu with the pause in) | pass |
| `app_single_player::the_session_flow_creates_the_game_then_loads_the_character_at_the_join` | fails on its 0x23 count (3: the real `StartSkill` 0x23; the test expects the synthetic 2): q-fixture-migrate's known G4, not this branch. Its tail check passes: the real join's frame 2 ends `… 1D 1D 1D 1D 1E 1E 1E 1D 48 04 48 5B 65 8D 5A`, the recorded order of `intents-events.md` §8.3 |
