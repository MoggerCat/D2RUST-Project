# q-fix-player-hit (done)

## Done
- **q-fix-c6-player-flags**: `wiring/action/units.rs` `init_kind` sets
  unit flags `|= 0x0E` for a player (`0x005348C0`, units.md §1 row 0), so
  missiles accept the player as a target. `apply_missile_record`
  (`wiring/action/missiles.rs`) now follows `0x005AD730`: block/dodge
  (avoid 1, block = physical != 0) → hit-class merge → monster crit →
  apply only while the hit bit stands → reaction.
- **q-fix-c7-owner-flags**: `set_owner_data` (`worldgen/population_init.rs`)
  ORs f2 → 0x2, f1 → 0x1 into the AI control flags (independent of the
  GUID lookup); REC-892 PROVISIONAL comment dropped. All callers
  (`party`, Nihlathak, bosses) pass their f1/f2 through it.
- Checks (d2rs side, matches the recorded values): combat-arrow-quillrat
  rat seed {1069704589,1707661690} at f46, quill gone, player hp 12800;
  combat-arrow-kill rat seed {1151986076,1319447910} at f46, hp 12415.
- Tests: player flags asserted in `action/tests/missiles.rs` `shot()`;
  `owner_data_stores_the_f1_f2_control_flags` in `worldgen/tests/population.rs`.

## Open
- The 1.14d side was not run (no Wine here): the checks were compared with
  the values the queue rows record, not a live `scenario-diff` equal.
- missiles.md §R6.1 step 5 (missile data flags 1/2 → hit flags 0x20/0x80)
  and the 0x4000 soft-hit rule remain unapplied (TODO in missiles.rs).
- No unit test forcing a monster-owned ToHit miss / Crit-5 seed stepping
  yet (the scenario checks cover the hit path end to end).

## Repro
`D2_GAME_DIR=/home/user/game python3 tools/scenario-diff/scenario_diff.py traces/checks/combat-arrow-quillrat.check --d2rs-only --work DIR`
