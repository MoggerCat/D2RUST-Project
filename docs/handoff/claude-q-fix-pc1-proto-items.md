# Hand-back: claude/q-fix-pc1-proto-items (2026-10-09)

## Done (all pushed, gated)
- The 22 PC 1 rows: join-multiclient, join-load-item-msgs, proto-0x20-sender, 0x73-fields,
  0x92-all-items, 0xab-gate, player-mode-rows, game-id-counter, load-taken-cell,
  quest-reward-no-spot, quest-item-delete-modes, code-drop-20-7, progression-sites,
  trade-lock-test, legacy-bytime-param, mercitem-room-order, merc-swap-update-list,
  pet-resync, pet-palette, dead-body-path-settings, and the closures vitals-dx-sign,
  state-param-sign, seam-stamina-scale.
- q-fix-real-item-replay-belt-use (REC-730), q-fix-p5-potion-entry3 + q-fix-real-potion-effect
  (REC-102 settled, REC-731 new).
- Area H (progression, diff-driven): the join packets of `packets-town-arrival-ama` match
  1.14d for all S→C messages (player 0xAA state 105 = REC-732, loaded-save 0x5F + 0x23,
  NPC alignment lists, the player's own 0xA8 / 0x1D, stat-lists §11 r4 corrected to the
  recording); act travel to Lut Gholein spawns start Jerhyn; quest spawns run population's
  creation (REC-733); act-1 town NPCs freeze after the player leaves; 0 life alone does not
  kill a player.
- Playability blocker: level-1 objects (waypoint 119, stash) lost after a waypoint round trip
  — preset objects and warp tiles lacked unit flags 0x3000000 (`83bd5887`).

## In progress
Nothing half-done is in the tree.
- Lethal-hit death check: a poked Fallen in the Rogue Encampment never attacks (town AI?).
  Next step: `warp 2` (Blood Moor), poke life to 1, then spawn the Fallen; write
  `traces/checks/death-lethal-ama.check`.
- Level-up / stat / skill-point click script and quest-flag joins for acts 3–5: not started.
- The full app path of the town round trip (`d2-client` play) was not re-run after `83bd5887`;
  only `crates/test-fixtures/tests/town_round_trip.rs` (ignored, live install).

## Open rows / RECs / PC 1 items
- REC-734: Jerhyn's Npc-AI class case `0x0059F570` has no spec; first divergence in
  `act-travel-lut-ama` (frame 32) and `join-act2-quests-ama` (frame 24). pc1-data Step 4.
- REC-732: the player's alignment call site is not in any spec (provisional).
- REC-731: remove callback `0x0056E900` also queues the unit for update (provisional).
- pc1-data Step 4: 0x8E flag byte at the join (sent as 1), `0x00625870` mod-array test,
  item-use entry 3 question, the stat in the changed list but missing from the base array.
- `warp-cold-plains-ama.check`: the Cold Plains waypoint and player land (+15, +5) subtiles off
  1.14d (5169, 4659); monsters match. DRLG owner (waypoint tile pattern pasted at another
  spot; needs the room-seed draws).
- join-load-item-msgs 0x22/0x21 generation on load is untested against 1.14d (needs the
  Wine join recording of -022633, or `scenario_diff` with a d2s-tool character that has items).
- Real-install ignored d2-client sweep (187 passed / 16 failed at 1097b98): 13 failures
  predate this branch (list in the coordinator's mail); GPU tests cannot run in the cloud.
  Disk: each d2-client test link is ~10 GB; run targets one at a time and delete the binary.

## Repro
```
export D2_GAME_DIR=/root/game CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
# one-time in a cloud container: sh tools/cloud-setup.sh; tools/cloud-game/setup_winpy.sh;
# tools/cloud-game/fetch.sh; tools/cloud-game/prepare_saves.sh
python3 tools/scenario-diff/scenario_diff.py traces/checks/<name>.check --work <dir>   # add --reuse; delete <dir>/d2rs.* after code changes
# checks: packets-town-arrival-ama (match), death-town-ama (match), act-travel-lut-ama,
# join-act2-quests-ama (REC-734), warp-cold-plains-ama (DRLG)
cargo test -p test-fixtures --test town_round_trip -- --ignored
```
