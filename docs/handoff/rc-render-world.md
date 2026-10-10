# rc-render-world: waypoint arrival pose (REC-1850)

Branch `claude/rc-render-world`. Scenes re-run with `tools/sidebyside/build.py` (act4out, act5out; pages stay local, none pushed).

## Checks before / after
- EQUAL scenes: 0 -> 0 (all 10 stay DIVERGED).
- a5-outdoor-frigid-highlands: first difference moved from row 120 (player frame: walk cel) to row 121 (player dir 3 frame 2 vs 1.14d dir 0 frame 0); 78.49% unchanged.
- a4-outdoor-city-of-the-damned: not re-run; its first difference (row 114) is an extra object, not the pose.

## Changed
- `bridge/predict.rs` `Predict::stood_mode`, `world_view/walk.rs`: after a waypoint travel the model keeps the walk mode from the arrival 0x0D code 1 forever (the walk is dropped, REC-288); the view now draws neutral (1, town 5). PROVISIONAL (REC-1850): view only, the model mode is unchanged; settle with a `record_state.py` `m` recording across an arrival.

## Open
- Neutral pose restarts at arrival in 1.14d (dir 0 frame 0); d2rs keeps dir 3 and a running frame count (S, same area).
- a4 outdoor: d2rs also has the class 380 TrappedSoul at (5456,4473), screen (448,68); 1.14d has no client unit for it (3 of them near the player, only (5482,4506) is drawn). Likely room population/activation timing in the server (`world/object-population.md` §7.9; d2-sim owner), M.
- a5: 'Quest Log' hint missing and area banner timing (client UI owner), S each.
- Wine prefix in the cloud container was broken (no kernel32); rebuilt with `wineboot -i`, `tools/cloud-game/setup_winpy.sh` leaves it so on a fresh container.
- Not run: nextest / coverage / spec_index / ledger checks beyond clippy (see below).
