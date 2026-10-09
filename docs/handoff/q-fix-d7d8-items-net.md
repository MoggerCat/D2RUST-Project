# Hand-back: q-fix-d7d8-items-net (2026-10-09)

## Done
- **D7** (ground item flags `if` 524304 vs 524288): `items::create::create_item` now sets
  item flag 0x10 (identified) at allocation, together with 0x80000, as `generation.md` §1.4
  (staging r6: the item kind's init sets it; only a successful quality routine clears it).
  First fix was in the normal routine only (REC-1400); moved to allocation after the merge,
  so qualities 1, 3 and 9 are covered too. No longer provisional. `items-ground-many` and
  `items-ground-pokes` were PARTIAL with no difference (30/30, 40/40) with the first version.
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

## Pick-up blocker (C->S 0x16 not applied; coordinator request after D7/D8)
Check `traces/checks/items-pickup-ama.check` (potion on the ground, scripted `send PickItem`):
state PARTIAL (no difference), packets MATCH against 1.14d (was DIVERGED at frame 10).
Causes, all fixed:
- A player loaded from an item-less save had no inventory in the model (`load_list` and
  `join_player_items` returned early on an empty list), so `can_pick` failed and the pick-up
  did nothing (REC-1402; the original makes it at unit allocation `0x0063ABD0`).
- One S->C 0x0A per pick-up: `leave_room` announces only while `in_room` (the grid / belt
  placement already left the room), REC-1403.
- The pickup sound (S->C 0x2C, event 1) was never queued: `InvDesk::pickup_sound` queues it
  in the player's sound slot. REC-1404 PROVISIONAL: event 1 is the one recorded value (a
  healing potion); what picks the event is not written. PC 1 question for gold and other items.
- State dump: an item that left the ground reports its inventory place and no `lv`.
Not fixed (not mine): `save-items-ama` still diverges at c2s frame 11 because d2rs' headless
client never sends the Walk / PickItem for `clickunit 4 *` (input owner, q-tool-state-diff).
Gold pick-up (0x19 at f43) is not covered by the new check.
