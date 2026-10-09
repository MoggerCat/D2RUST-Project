# q-fix-d3-player-mode — hand-back (2026-10-09, branch claude/q-fix-d3-player-mode)

Task: ledger first divergence D3, player mode 1.14d 5 vs d2rs 1 (frames 61–67)
and 5 vs 6 (frame 93): `milestone-anya`, `-hellforge`, `-hephasto`,
`combat-cold-plains-wp`.

## Done

- **Cause (measured on 1.14d under Wine).** Not a mode rule. The 1.14d client
  part keeps its local player at the server player's point; d2rs's headless
  client did not. After a server-side move (the `pos` pokes of the three
  `milestone-*` checks; the run sent by `poke msg 0x03` in
  `combat-cold-plains-wp`) the first S→C 0x96 (stamina, with the player's
  point, tick 66 / 92) made the d2rs position check (`client/model.md` §6
  rule 8) send C→S 0x5F with the stale client point; the server's resync
  (`sim/pathing.md` §1.6) then walked the player back (mode 1, or 6 in
  town). 1.14d sends no 0x5F in either check. Recorded with a scratch copy of
  `record_state.py` that logged the client unit's path every tick end and the
  arguments of the check `0x004804E0`: anya, client player at the poked point
  from the poke's tick, check at tick 66 called with the same point; cold
  plains, the client runs from tick 11 (mode 3) and stops at tick 34 with the
  server player. (An earlier reading, "the 0x96 arrives a tick later on
  1.14d", was a recorder-order artefact: the flush record sits before the
  sync's messages on 1.14d, after them on d2rs.)
- **Fix.** `d2-client state-dump` now always runs the play preview's walk
  prediction (an empty input script when there is no `--input`) and, after
  each frame, hands it to the model as `play` does
  (`Headless::sync_local` = `preview_walk_room`: room recache at the
  predicted sub-tile + `set_local_walk`), so rule 8 takes the server's point
  and sends nothing. Files: `crates/d2-client/src/app/state_dump.rs`,
  `crates/d2-client/src/world_view/input_script.rs` (+ test
  `sync_local_records_the_prediction_for_the_check`). Spec:
  `specs/tools/state-snapshot.md` §3 rule 4. PROVISIONAL **REC-1385**
  (`docs/HANDOFF.md`): positions reproduced, mechanism not found.
- **Result (scenario-diff, state, Wine, 2026-10-09).** The player is equal past
  the old frames in all four checks. First differences now:

  | check | before | after |
  |---|---|---|
  | milestone-anya | frame 67 player m 5 vs 1 | frame 94 monster 1:36 class 449 field s (unit seed) |
  | milestone-hellforge / -hephasto | frame 61 player m 5 vs 1 | frame 83 monster 1:20 class 311 field fr (150 vs 16384) |
  | combat-cold-plains-wp | frame 93 player m 5 vs 6 | frame 444 monster 1:7 class 155 field m (1 vs 8) |

  No check is EQUAL yet; the next differences are other areas (below).
- Regression sample after the change (`walk-town-ama`, `death-town-ama`,
  `merc-rogue-town-bar`, `ass-fade`, `a4-warp-river-ama`): see the end of this
  file for the verdicts.
- Ledger rows updated in `docs/handoff/ledger/q-fix-d3-player-mode.tsv`
  (`quest.a4q3-hell-s-forge`, `quest.a5q4-betrayal-of-harrogath`,
  `waypoint.1.cold-plains`): still DIVERGED, with the new first frame and
  owner note. Validated with `ledger.py` (0 format errors).

## Open (route, not fixed here)

- Monster 1:36 class 449 seed at frame 94 (anya): unit seed order → the
  `q-fix-real-unit-seed-order` area. Monster 1:20 class 311 `fr` at frame 83
  (hellforge/hephasto): monster animation frame field. Monster 1:7 class 155
  mode 8 vs 1 at frame 444 (cold plains): monster combat
  (`q-fix-b-monster-combat`).
- The 1.14d mechanism that moves the client player after a `pos` poke without
  any S→C message and without a call of `0x00650BE0` / `0x00650910` /
  `0x004654C0` is not found (a Dr0 write watch under Wine raised no hit):
  PC 1 item "[q-fix-d3-player-mode]" in `docs/handoff/pc1-data.md` Step 4.

## Repro

```sh
export D2_GAME_DIR=$HOME/game
cargo build --release -p d2-client
python3 tools/scenario-diff/scenario_diff.py traces/checks/milestone-anya.check --next 5
python3 tools/scenario-diff/scenario_diff.py traces/checks/combat-cold-plains-wp.check --next 5
python3 tools/scenario-diff/scenario_diff.py traces/checks/milestone-hellforge.check --next 5
```

## Regression sample (state channel, Wine, after the change)

`walk-town-ama`, `death-town-ama`, `merc-rogue-town-bar`, `ass-fade`,
`a4-warp-river-ama`: state PARTIAL (no difference in what was compared), as
in `checks-status.md` before the change.
