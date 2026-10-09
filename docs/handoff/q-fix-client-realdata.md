# q-fix-client-realdata

Branch `claude/q-fix-client-realdata` (merged `q-fix-ui-blend` first). Real-data run of d2-client, debug build, one test binary at a time (`D2_GAME_DIR=/root/game`; the install assembled with the private repo's `tools/assemble.py`, excel view by `data-tool excel-dir`).

## Done (test expectations fixed, none weakened; PROVISIONAL REC-1705, REC-1706)
| Test | Cause | Fix |
|---|---|---|
| app_hud_e2e | the stamina bar is a rectangle draw (`control-panel.md` §4 r2, q-scenes-compare), not a `hudfill` tile | asserts the `Rect` (273,573,+51,+18, mode 2) |
| app_frame_loop | the test still staged the synthetic town: join is now the recorded REC-530 (126 messages = handled 108 + queued 18); the new character knows only the town waypoint and stands far from it, so the staged travel never ran | counts and rooms from the recording; travel removed (covered by `app_waypoint_warp`); units_hidden 32 and queued = drained over 300 ticks are d2rs-measured (REC-1705, REC-1706) |
| app_cast `a_right_skill_at_a_point...` | the local player's cast mode comes from the client's click (`model.md` §20; the server sends its own client nothing, pc1-day4 item 55); the rig sends raw 0x0C | asserts the server player's cast mode |
| app_single_player `a_save_from_the_command_line_joins` | environment: needs `D2_SAVE` | passes with `d2s-tool new --name ScnSor --class sor --expansion` as `D2_SAVE` |
| save_roundtrip `random_play_round_trips` | passes (483 s for the binary, 3 slow tests): was a timeout, not a failure | none |
| seam_drlg_coords | headless bridge had no visibility predicate (`model.md` OQ7) | `set_visibility(always true)`: the test is about rooms |
| app_cain_quest | probe spawned Cain with spread 0 (ring search fails with no draws, `population.md` §9.3); class 59 portal's `InteractType` is the town level (`objects.md` §5.5 init 11) | spread 5 (the quest's first try); expects town level 1 |
| app_assassin_gaps (2) | the target stood 1 from the sentry: in its melee range, never targeted (`ai.md` §5.3, REC-1270, 1.14d-measured) | monster 4 from the trap |

## Open (cause, owner); all the baseline's families, unchanged by this branch
| Test | Cause (as far as seen) | Owner |
|---|---|---|
| d2-server `game_town_run::town_run_moves_at_the_run_velocity` | **d2-sim movement**, not the test: the run (mode 3, velocity 0x900, smooth 0.56 sub-tile/tick) leaves the move at x 4899 after 18 ticks and switches to mode 5, 2 short of the target 4901 (start 4889, +12). No wall. Looks like an arrival tolerance after `654bd47a` (path preparation keeps the target). Routed, not edited | `crates/d2-sim/src/path/walk` (q-fix-client-crash) / pathing |
| app_play_act3 `the_blade_...` | Hratli's chat gives message 466, test expects 571 (Blade start) | act3 / quests (q-fix-npc-interact) |
| app_play_act3 `the_golden_bird_...` | `j34` not picked up (`app_play_act3.rs:585`) | act3 |
| app_play_act3 `kurast_docks_arrival_...` | `unit 1/[245] not reached` in `app_support::operate...` | act3 / NPC menus |
| play_act5 x4 (`baal_falls`, `shenk_dies` kill monster 90/479 not killed; `ancients_fall` chain 35 (3,3); `caged_barbarians` chain 32 (2,2)) | quest chains stop after the kill | act5, boss damage |
| play_smoke `the_live_run` | S→C 0x6D (MonsterStop) for a unit not in the client model is dropped (`model.md` §4 r1) | server unit visibility / client |
| play_smoke `the_spec_npc_ui_on_the_install` | 4 run-leg divergences drawn vs server (e.g. drawn 4891 vs server 4884) | client path (q-fix-client-crash) |
| play_smoke `the_scripted_play_run` | waypoint menu not reached in 400 frames | q-fix-a1-den-wp |
| 7 GPU tests | no GPU | skipped |

## Repro
```
export D2_GAME_DIR=/root/game
cargo run -q -p data-tool -- excel-dir $D2_GAME_DIR/extracted/patch_d2/data/global/excel $D2_GAME_DIR
cargo nextest run -p d2-client --run-ignored only --test <binary>      # one binary at a time; rm target/debug/incremental
cargo nextest run -p d2-server --run-ignored only --test game_town_run
```
Disk: the full `-p d2-client` run links ~110 test binaries (>15 GB); run per binary and delete `target/debug/deps/<binary>-*` after.
