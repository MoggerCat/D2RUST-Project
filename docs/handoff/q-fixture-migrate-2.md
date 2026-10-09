# q-fixture-migrate-2: G4–G7, F1–F3 and the gate snap-back

Branch `claude/q-fixture-migrate-2` (from staging). Data: the private repo's `install/`
assembled by `tools/cloud-game/fetch.sh` to `$HOME/game`. REC ids used: REC-530 (join
recording). Real-data run = `D2_GAME_DIR=$HOME/game cargo nextest run -p d2-client --release
--run-ignored only -E 'kind(test) & not test(/gpu_/)'`.

Real-data pass count (d2-client ignored set): baseline 182 / 207 (staging 87736d7);
211 / 220 after the first push; 214 / 222 at the last full run (a796c2ae minus two later
fixes, app_server_core and app_move_anims, which pass in their own run).
Gate (fmt, clippy -D warnings, coverage, spec_index, nextest d2-sim + d2-server in debug,
d2-client + test-fixtures in release): green at a796c2ae. rustc segfaults on d2-sim's test
build at release opt-level 3: run d2-sim and d2-server in debug.

## Done

| Item | What | Where |
|---|---|---|
| G5 smoke_save | real item codes (hp1, mp1, rin, amu, tsc; ring on body loc 6, merc cap on 1); counts relative to the start kit; stamina fixture sets the maximum before the current value (`vitals.md` §7.2) | `tests/smoke_save.rs` |
| G5 smoke_frontend | staging's version kept (`q-fix-real-front-save` moved to q-fix-server-store-fill) | |
| F3 belt | start items placed in the belt, and belt items on a d2s load, now get mode BELT (the slot placement leaves the mode to its caller); the load fell back to the inventory | `d2-server` `handlers/world.rs` `place_belt`, `d2-sim` `wiring/inventory/load.rs` |
| REC-530 | Wine recording of a loaded expansion sorceress's join (`traces/sim/join/sim-0530.json`, `tools/trace-recorder/convert_join.py`); `app_client_drlg` compares the join's 0x07 rooms with it | |
| G4 | app_server_skills (natives from the `charstats` rule + the staff's start skill), app_single_player (0x23 count = 2 + valid StartSkill, 3 on the install), app_client_drlg (init seed = game seed, rooms = recorded 0x07s, BlankScreen from the levels row), app_frame_loop (join count from `facts/join/a1-new-sor.tsv`: red, see below) | tests |
| G6 | app_town_gaps (Gheed, Charsi, Elzix, Jamella placed by the presets), app_cain_quest (joined game), app_level_border (route out of town), app_play_preview (`add_live_client`), app_server_core (real zombie) | tests |
| app_levelup | on the install's rows; stat/skill points from `charstats.StatPerLevel` and a learnable class skill from `skills` | `tests/app_levelup.rs` |
| Class skill tests | app_necro, paladin, sorc, cast, druid, amazon, assassin on `real_rig` with the install's skill rows (their invented anim data no longer finished a cast after the player fc/sp change); 0x4D checks kept through `Rig::s2c_mark` / `s2c_contains_since` | tests, `real_rig` |
| Gate snap-back | root cause: the Rig never wired `add_walk`, so the client stayed at the spawn point and its C→S 0x5F told the server so; the server (`pathing.md` §1.6, walk branch for d > 45) walked the player back. Rig now calls `add_walk`; `leave_town` retry limit back to 12 (3 rounds on seed 9) | `real_rig/mod.rs` |

## Findings still red on the install (one line each)

- `app_frame_loop`: the client applies 116 of the 127 server messages the original sends up to the first tick (`q-fix-real-join-missing-msgs`; suspect the 0x0E object states). Later invented numbers in that test are behind this assert.
- `play_smoke` `the_live_run`: stops at "run leg: dropped {6D: 01}" (a 0x6D for a unit the client model lacks; `q-fix-real-6d-dropped`, monster-walk session). F2 (equip-2h) behind it.
- `play_smoke` `the_scripted_play_run`, `the_spec_npc_ui_on_the_install`: F1 client-path drift (`q-fix-client-path-exact`: the original's client path equals the server's 16.16 position on every tick).
- `seam_drlg_coords` border walk: the client player has no room for one frame at the level border (`q-fix-real-border-room-lag`).
- `app_single_player` `a_save_from_the_command_line_joins`: needs `D2_SAVE` (the gate skips it).
- `app_single_player` load steps: the load logs "has skill" as an unapplied step on the install (`q-fix-real-load-steps`).
- `app_assassin_gaps` `a_lightning_sentry_fires_its_missile`: missile damage setup (`q-fix-real-missile-damage`).

## In progress at the pause (kept out of the branch)

- F1 `q-fix-client-path-exact` and F2 `q-fix-real-equip-2h` were with subagents, stopped half done. Their
  uncommitted production edits are NOT in the branch; the diff is saved as
  `wip-f1-f2.patch` in the session scratchpad only (lost with the container), so redo from the notes:
  - F1: root cause not yet confirmed. Lead: the client path's grids lack other units' (objects')
    footprints, `client_path.rs` module doc; a fire (objects class 37) stands near the run stop
    point (4925,4210); measure client vs server position per tick on `the_scripted_play_run`
    (legs to (4931,4207), (4898,4227)), then stamp the model objects' footprints into the
    client path's private grids with `d2-sim` `path/footprint.rs` (objects table rows:
    `client_object_rows`, `bridge/objects`). Target: `play_smoke` tolerance 2 -> 0.
  - F2: reproduce in a new ignored test on `Rig::new("sorceress", &[])`: C->S 0x1C bodyloc 4, then
    0x1A equip of the staff to 4; find the failing step in `d2-sim` `items/inventory/equip.rs`
    `equip_check` (body_location_allowed / requirements_met / hands_compatible). Not yet found.

## Not done

G7 (the live run in release already; blocked behind the 6D finding), F1, F2, corpse-regen
(`stat-lists.md` §10.1 has no dead-mode stop; needs the original's behavior), the
`app_frame_loop` numbers after the join count, `test_fixtures::install` users outside the client.
