# q-chk-items-cube hand-back (partial)

## Done
- `tools/checks/gen_cube.py`: generates one check per enabled cubemain row (140 of 146; rows 62, 63 need The Stone of Jordan, 129-132 need unique/set rows). Inputs sit in the cube page of save ScnCube, frame 4 `UseGridItem` on the cube (`@4`, confirmed to resolve), frame 8 `ClickButton 0x18`. Channels state, packets, items.
- Merged `claude/q-tool-items-channel`.
- `traces/checks/cube-005-3-small-rejuvs-one-large.check` run under Wine (about 12 min including the build).

## Result
- state PARTIAL, packets DIVERGED at frame 2, s2c #17: 1.14d sends 0x9C (item add) where d2rs sends 0x23 SetSkill. This is the join stream with stored items, before any cube step, so it hides the transmute. Owner: claude/q-fix-server-store-fill (items) / join stream. Items channel not run yet for this check.

## Open
- Run the other recipe kinds (gems, runes, reroll, crafting, socket, Horadric Staff, Khalim's Will) after the frame-2 divergence is fixed or masked; generate with
  `python3 tools/checks/gen_cube.py --excel $D2_GAME_DIR/extracted/patch_d2/data/global/excel --out DIR`.
- Drops, vendors, gamble, imbue from the task are not started.
- Run: `python3 tools/scenario-diff/scenario_diff.py traces/checks/<name>.check --channels state,packets,items`.
