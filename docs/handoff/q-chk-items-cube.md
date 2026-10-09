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

## Update (after fixing the checks)
First version opened the cube with the wrong GUID (@4) and x=y=0 (the 0x20 handler needs the point within 50 subtiles of the player), so nothing transmuted on either side. Fixed in gen_cube.py (`item=@1 x=@x y=@y`).
- 16 representative recipes (quest, portal, potions, crafting, convert, misc, gem and rune upgrades, socket, reroll, repair) run: **items channel MATCH on all 16** (1.14d recording in traces/orig-cache, d2rs live).
- Packets channel still diverges at join frame 2 (0x9C vs 0x23 order), independent of the cube; the state channel is PARTIAL (state-field parity).
- Not run: the other 124 generated recipes (traces/checks-gen/cube), 6 rows needing unique/set inputs, the 0x2A put-in message, op-gated recipes (day-of-month, ops 28), Cow level portal use, drops, vendors, gamble, imbue.
- Repro: `python3 tools/scenario-diff/scenario_diff.py traces/checks/<name>.check --orig-cache --channels items`.
