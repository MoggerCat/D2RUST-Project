# q-fix-d9-arcane — hand-back

## Done
- D9 root cause: the "missing monster class 201" is Jerhyn (start Jerhyn, 1:1). d2rs removed him at the warp
  frame (A2Q4 event 3, old level 40: `start_jerhyn_check` removes the start Jerhyn). 1.14d keeps him to
  frame 143 when the new level is 74, but removes him on every other warp (a2-warp-maggot-lair-ama equal).
- Fix: `crates/d2-sim/src/world/quests/act2/q4.rs` `changed_level` skips the old-level-40 block when
  `b == SANCTUARY`. PROVISIONAL REC-1405 (the branch structure is unread; PC 1 item queued in pc1-data.md Step 4).
- Test: `tests_q4.rs` `jerhyn_leaving_town` (town -> 74 case).
- `a2-warp-arcane-ama` state channel: DIVERGED@20 -> PARTIAL, 160/160 frames, 4525 unit records equal.

## Open
- rng channel of this check still diverges at frame 2 (unit-seed order, q-fix-real-unit-seed-order, not mine).
- Spec text (quests-act2.md §6.6, quests-act2-2.md §2 item 3) not edited: needs the binary read.

## Repro
`D2_GAME_DIR=$HOME/game python3 tools/scenario-diff/scenario_diff.py traces/checks/a2-warp-arcane-ama.check --channels state`
