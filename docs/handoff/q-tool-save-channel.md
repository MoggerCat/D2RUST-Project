# q-tool-save-channel: hand-back (2026-10-09)

## Done
- `save` channel in scenario-diff (`specs/tools/scenario-diff.md` §3 rule 13; `tools/scenario-diff/save_channel.py`): Save and Exit (C->S `0x69`, injected as `at <ticks+1> send hex 69`) on both sides, then the written `.d2s` files are compared byte for byte (save time +0x30 and the checksum normalised; each file's checksum verified first).
- 1.14d side: `record_state.py --write-save --save-watch FILE` (no `-nosave`; ends 2 s after the file changed). d2rs side: `d2-client state-dump --save-out FILE`. The start save is restored in the 1.14d folder afterwards.
- Checks (run 2026-10-09, cloud Wine, release build): 
  | check | verdict |
  |---|---|
  | save-fresh-ama | MATCH (848 bytes) |
  | save-levelup-ama | MATCH (857 bytes, level 2 on both) |
  | save-merc-bar | MATCH (861 bytes, hireling block equal) |
  | save-corpse-ama | MATCH (844 bytes) but see Open 2 |
  | save-items-ama | DIVERGED: 1.14d 866 bytes (potion on the cursor, 1 player item), d2rs 848 bytes (none) |
- Format checks recorded as EQUAL rows (ledger part): `mpq-tool formats` (2372 ds1, 21717 dcc, 1653 dc6 ... 0 errors), `data-tool tables` (73 tables: 72 identical, 1 explained, 4/4 code buffers identical).
- Ledger part: `docs/handoff/ledger/q-tool-save-channel.tsv` (10 rows, validated with ledger.py `--check` on the part alone; areas overlap the systems/integrator parts, which must yield to these).

## Open
1. save-items-ama: d2rs does not pick the potion/gold up under the same click input (items stay on the ground at tick 99 on d2rs? not confirmed: only the save was compared); route to the items/pickup owner and q-fix-save-input. Repro: `python3 tools/scenario-diff/scenario_diff.py traces/checks/save-items-ama.check`. Run the check's `state` channel for the first cause.
2. save-corpse-ama: on 1.14d the dead character is saved with 0 corpses (death in town, life poked to 0); the check proves equality of that, not of a corpse record. A corpse in the field (a kill outside town) needs a further check.
3. Several checks make no quest/waypoint/NPC change, so the d2s quest/waypoint/NPC rows are EQUAL for starting values only.
4. Not run: clippy and nextest for d2-client, full-workspace tests (only d2-client `cargo check` and the python selftests were run).

## Repro
`export D2_GAME_DIR=$HOME/game; python3 tools/scenario-diff/scenario_diff.py traces/checks/save-<name>.check`; `python3 tools/scenario-diff/save_channel.py --selftest`; compare two files: `python3 tools/scenario-diff/save_channel.py ORIG.d2s D2RS.d2s`.
