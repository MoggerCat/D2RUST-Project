# Hand-back: q-fix-d7d8-items-net (2026-10-09)

## Done
- **D7** (ground item flags `if` 524304 vs 524288): `items::create::normal` (the
  quality-2 routine, `generation.md` §6.1) now leaves item flag 0x10 (identified)
  set. Evidence: `items-ground-many` recording, poked quality-2 misc items read
  0x80010 while magic / rare / set / unique read 0x10 clear; the items channel's
  `items-drop-gold-potion` agrees. **REC-1400, PROVISIONAL**: superior (3) and low
  (1) quality have no recording, so they were not touched.
  Result: `items-ground-many` and `items-ground-pokes` are PARTIAL with no
  difference in 30/30 and 40/40 frames (was DIVERGED at frame 4).
- **D8** (S→C 0x27 byte 10, 64 vs 11): the text list reached the wire in
  reverse. The quest records add Akara's lines newest → oldest (chain 37 → 11,
  then chain 1 → 64) but 1.14d sends [64, 11]. `encode_text_list`
  (`d2-client/src/app/npc_seams.rs`, d2rs-own) now writes the entries reversed.
  **REC-1401, PROVISIONAL** (only one multi-entry recording; `0x006612F0` is not
  specified: it may prepend). Result: the 0x27 records of `items-vendor-akara-buy`
  are equal; the first s2c divergence moves from frame 16 to frame 20.
- Ledger part: `docs/handoff/ledger/q-fix-d7d8-items-net.tsv` (rows `item.identify`,
  `net.s2c.0x27`). `ledger.py` flags them as contradicting `checks-status.md`
  only because that file still holds the pre-fix verdicts; regenerate it.

## Open (not mine)
- `items-vendor-akara-buy` packets, next first divergences:
  c2s frame 17: 1.14d client sends 0x31 QuestMessage (`31 0c000000 0b000000`) after
  the 0x27; the headless d2rs client sends nothing (client bridge owner).
  s2c frame 20: store item 0x9c byte 37 (161 vs 160): the magic-item property roll
  order question (pc1-data Step 4, q-fix-server-store-fill).
  s2c frame 24/25: SetStatWord 0x1E one frame early in d2rs.
- Superior / low-quality identified flag: needs a recording (poke `quality`
  names for superior and low).

## Repro
```
export D2_GAME_DIR=$HOME/game CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
sh tools/coord/session-setup.sh           # Wine, Windows Python, recorder saves
python3 tools/scenario-diff/scenario_diff.py traces/checks/items-ground-many.check --work /tmp/sd1
python3 tools/scenario-diff/scenario_diff.py traces/checks/items-ground-pokes.check --work /tmp/sd2
python3 tools/scenario-diff/scenario_diff.py traces/checks/items-vendor-akara-buy.check --work /tmp/sd3
```
Note: `tools/coord/session-setup.sh` run from /tmp fails (it derives the repo root from
its own path); run it from `tools/coord/`.
