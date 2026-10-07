# local-buddy-q-client (2026-10-07)

Base origin/main 674996d; Windows 11, release/debug builds of `d2-client`. Adapter: `AMD Radeon RX 9070 XT (Vulkan, DiscreteGpu, driver AMD proprietary driver 26.8.1)`. `D2_GAME_DIR` = the 1.14d install `game\`. Logs not committed.

## Summary

| Entry | Result |
|---|---|
| 71 `game_panels` | FAIL (1 of 2 tests): finding |
| 76 `app_original_ui` | PASS |
| 77 `play --frames 500` + I/C/T | PARTIAL: UI toggles PASS, no UI/sound errors; run FAILS at the server panic (finding); capture cases ui-0001/0002 NOT RUN |
| 81 `the_recorded_join_on_the_install` | PASS |
| 82 `play` act palette | PARTIAL: no `present_act_palette` / 0x07 / 0xAC / 0x15 refusal logged; run FAILS at the server panic; pixel colour not judged |
| 86 `the_session_join_on_the_install` | PASS |

No "Intended claim" lines exist in game_panels / app_original_ui / app_client_drlg (those carry `// Covers:` already; the only "Intended claim" lines are in `game_assets.rs`, not in this task's entries), so no test source was changed and no coverage edit was needed.

## 71 game_panels (`--ignored`)
- Expected: both tests pass; every quad set frames f..f+3 of 256x256, 64x256, 256x176, 64x176, offsets 0; `ui/panels.md` Constants counts/sizes.
- Actual: `panel_quads_have_the_stated_sizes_and_zero_offsets` ok. `panel_files_frame_counts_and_sizes` FAILED at `game_panels.rs:99`: `menu\horadric frame 1`: left (offset_x, offset_y) = (-205, 17), right (0, 0). All earlier cases in the list (invchar ... waygateicons, including buysellbtn 23, buyselltabs 8 x 79x31) passed, and `menu\horadric` frame 0 and its frame count (31) passed. The test stops at the first failure, so the 7 `skltree_?_back` frame-count checks (16) after it were NOT evaluated by this run.
- FINDING (spec, not test): `menu\horadric.dc6` has non-zero frame offsets (frame 1 = (-205, 17)); `ui/panels.md` Constants / `client/assets.md` state zero offsets for it. Not changed.

## 76 app_original_ui (`--ignored`)
- Expected pass; actual: `wired_panels_load_from_the_install` ok (0.51 s). PASS.

## 81 / 86 app_client_drlg (`--ignored`)
- `the_recorded_join_on_the_install` ok, `the_session_join_on_the_install` ok (11.17 s together). PASS both.
- Printed by 86: `player at (4873, 4228), 10 rooms in sight`: 10 RoomSight entries, all `show: true, level: 1`, positions (968,840) [first, spawn room], (960,832), (960,840), (968,832), (976,832), (960,848), (968,840), (976,840), (968,848), (976,848). Count 10 = 1 from game entry + 9 adjacency, matching the ten 0x07 of the recorded join (C84 count). No message refused, no `game entry failed`.
- 81's 0x07 level-1 room origin (928, 904) and the five act `pal.pl2` loads asserted inside the test: passed.

## 77 / 82 play --frames 500 (release, RUST_BACKTRACE=1)
- Exit code 101 both times (cargo "process didn't exit successfully", plain run 82; the 77 run was started directly, exit code not captured by Start-Process but the identical panic and log occurred). Window closed; no d2-client process left (checked).
- FINDING (recurs; same as tri-client note, line moved 144 -> 152): thread `d2-server` panicked at `crates\d2-sim\src\drlg\room.rs:152:44`, "live DRLG room", after the frame-250 line (106 server ticks, 1 unit in the model, items 6 (82) / 19 (77, with UI), audio presented 106, decodes 9, load_errors 0, engine_errors 0). Backtrace (innermost first): `Drlg::level_rooms` <- `Outdoor::reset_level` <- `WorldTypes::reset_level` / `SharedTypes::reset_level` (wiring/worldgen/levels) <- `Drlg::free_level_rooms` <- `Drlg::free_inactive_levels` <- `DrlgWorld::free_inactive_rooms` (wiring/action) <- `LocalLink` (d2_client bridge/local) <- `server_thread::serve`. Then `server: server thread stopped: no answer from the server thread`. So 500 frames are never reached.
- 77 UI toggling (SendKeys i, c, t, i, t, c to the window, RUST_LOG=info,d2_client=debug): log showed `Opened(1)` (I), `Opened(2)` (C), then T: `Closed(1)` + `Opened(4)` (I then T closes the inventory, as expected), I: `Closed(4)`, `Opened(1)`, T: `Closed(1)`, `Opened(4)`, C: `Closed(2)`. Matches the conflict table. No `UI image file`, `sound world ... (pending`, or `SetUIState` error; `ui_unhandled` and `load_errors`/`engine_errors` 0 at frame 250.
- 82: no `present_act_palette` error and no 0x07 / 0xAC / 0x15 refusal, no `fatal assert 0x13C/0x168`, no `TODO(spec: model.md §9`. Whether the window is non-black was not judged (no screenshot taken).
- Capture cases `ui-0001` / `ui-0002`: NOT RUN (no palette-presented capture path exercised in this lane; play dies at ~frame 260 anyway).
