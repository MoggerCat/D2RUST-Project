# q-run-net: hand-back (scope: ledger rows net.*)

## Done
- `tools/check-gen` family `netc2s`: 75 checks `traces/checks/gen/gen-netc2s-NN.check`, one per C->S id no check carried (default field values, `packets` channel; PROVISIONAL REC-1750). `--selftest` and `--check` green.
- Hand-authored S->C trigger checks `traces/checks/net-s2c-{chat,gold-pickup,townportal,kashya-hirelist}.check` (REC-1751..1754, provisional: chat layout, gold ref `@4`, `ScnCube` save for the portal).
- `tools/packet-census`: also reads `traces/checks/gen`, `run --orig-cache` (fills `traces/orig-cache`), `report --seed-tsv` (adds an earlier census).
- Ran both sides (real 1.14d under Wine, cache committed, text only): 75 netc2s, 4 net-s2c, 19 gen-shrine, gen-wp 0/1/9/18/27/30, a2/a5 NPC, hire, merc, items, milestone, cube-00*, save, poke, interact, death, act, join checks.
- Verdicts (`docs/handoff/ledger/q-run-net.tsv`, 294 rows, ledger `--check` 0 errors): EQUAL 104, DIVERGED 66, NO-CHECK 94, NOT-IMPLEMENTED 30. `docs/handoff/packet-census.{md,tsv}` regenerated.

## Open
- 94 NO-CHECK rows: C->S 0x14/0x15 chat (variable size, no table layout), system ids 0x66+ (session level, need a join/leave check), S->C ids needing a kill, party or multiplayer trigger (0x11 ReportKill etc.), ids carried but never paired (13).
- Most C->S EQUAL rows are the refusal path with default fields; a handler success path needs a live target (a follow-up check per id).
- `tools/coord/ledger.py` globs only `traces/checks/*.check`, so gen check names are written in the row `note`, not `checks`. Suggest extending its glob to `traces/checks/gen/`.
- `net-s2c-townportal` diverges at frame 2 on the `ScnCube` item load, before the portal messages: needs a save whose items load equal.

## Divergences by owner (not fixed)
- claude/q-fix-pc1-proto-items (57 rows, S->C): first divergence causes: "missing in d2rs" 31, wrong id 17 (e.g. 0x07/0x08/0x23 sent where 1.14d sends another), "extra" 8, byte diffs 8, size 2.
  Repro: `python3 tools/scenario-diff/scenario_diff.py traces/checks/net-s2c-chat.check --orig-cache traces/orig-cache` (0x26 chat echo missing in d2rs), `.../gen/gen-netc2s-60.check` (0x97 WeaponSwitch missing), `.../gen/gen-netc2s-34.check` (0x2A NpcTransaction size), `.../gen/gen-shrine-7.check` (0xA8 size 15 vs 12), `.../gen/gen-wp-27.check` (0x47 missing).
- claude/q-fix-server-store-fill (1 row): C->S 0x16 PickItem, `save-items-ama@42`.
- claude/q-diff-skills-2 (2 rows) and 6 rows with no owner: see the part file notes (C->S 0x31, 0x4B, 0x67 CreateGame stack bytes at frame 1).

## Repro
```
sh tools/cloud-setup.sh; tools/cloud-game/setup_winpy.sh; tools/cloud-game/prepare_saves.sh   # + install into $HOME/game, excel view via data-tool excel-dir
python3 tools/check-gen/check_gen.py
python3 tools/packet-census/packet_census.py run --dir D --orig-cache --filter 'gen-netc2s-*'
python3 tools/packet-census/packet_census.py report --dir D --seed-tsv docs/handoff/packet-census.tsv --tsv docs/handoff/packet-census.tsv --md docs/handoff/packet-census.md --ledger <fidelity-ledger.tsv> --part docs/handoff/ledger/q-run-net.tsv
```
