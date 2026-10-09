# q-fix-realdata-baseline

## Done
- `tools/coord/realdata-baseline.tsv` built from a full run
  (`python3 tools/coord/realdata.py`, 383 tests, 204 s tests): **354 pass, 29 fail**.
- Script fix (`tools/coord/realdata.py`): nextest 0.9.148 writes its JUnit under
  `<workspace>/target/nextest/coord/`, not under `CARGO_TARGET_DIR`; the script
  reported "wrote no JUnit" and refused to write a baseline. It now looks in both.
- `realdata.py` can join the gate: exit 1 only on a pass in the baseline that fails now.
  Use `D2_GAME_DIR=/home/user/game` (+ `data-tool excel-dir`, see `tools/realdata-gate.sh`).

## Open: the 29 baseline failures, by cause and owner
None is a test-rig bug (all assert real behavior; none was weakened).

| Cause | Tests | Owner area |
|---|---|---|
| UI draw mode 2 ("needs the act's blend tables, not loaded", `world_view/ui_bind.rs` `ui_cel_ops`: front-end / pre-game UI rectangle has no `ShadeTables`) | app_input_pass x4, smoke_frontend x5 | rendering / client UI (q-scenes-compare, q-fix-pc1-client-ui) |
| Play-path frame not drawn / HUD panel draw lists | app_play_640, app_play_e2e, app_play_visibility, app_hud_e2e, app_levelup_ui, app_frame_loop | client UI/HUD, rendering (same blend-table cause likely; recheck after the above) |
| Visibility predicate 0x004DBF20 unresolved (`model.md` open question 7) | seam_drlg_coords | rendering/seams (q-scenes-compare) |
| Run walk ends at x 4899 vs 4901 | d2-server game_town_run | pathing/walk-run (q-scenes-compare; q-fix-client-crash walk desync) |
| Cain not found / red portal flow | app_cain_quest | quests (q-fix-pc1-day3-a-r2) |
| Act 3 NPC 253 unreachable, 0x13 on NPC, monster 20 not dying | app_play_act3 x3 | act3 (q-play-act3), NPC menus (q-fix-npc-menus) |
| Act 5 quest chain stops (monster 90/480 not killed, rescue/betrayal/Rite of Passage) | play_act5 x5 | act5 (q-play-act5), boss damage (q-fix-boss-damage) |
| Waypoint menu not reached; item 0x6D dropped; NPC UI 4 divergences | play_smoke x3 | q-fix-a1-den-wp, q-fix-npc-menus, q-fix-pc1-client-ui |

## Repro
```
D2_GAME_DIR=/home/user/game python3 tools/coord/realdata.py            # check vs baseline
D2_GAME_DIR=/home/user/game python3 tools/coord/realdata.py -E 'test(/smoke_frontend/)'
python3 tools/coord/realdata.py --update-baseline                        # after fixes
```
Full build ~25 min cold; disk ~5 GB free after: `rm -rf target/debug/incremental`.
