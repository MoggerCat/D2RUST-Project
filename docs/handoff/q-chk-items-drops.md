# Hand-back: q-chk-items-drops (2026-10-09)

Session q-chk-items-drops, branch `claude/q-chk-items-drops` (merged integ-r3, integ-r4,
q-tool-items-channel, integ-r6). REC ids: REC-1525..1528. No game logic changed.

## Done

- **Generators** (our own code, read only the user's tables / the public spec table):
  `tools/scenario-diff/gen_drop_checks.py EXCEL_DIR` (one monster per distinct
  monstats `TreasureClass1`, 5 per check; champion / unique / random-boss variants),
  `gen_vendor_checks.py` (every trading/gambling NPC of `specs/world/vendors.tsv`: warp to
  town, `poke goto unit <npc>`, 0x13 + 0x2F + 0x38 action 1 (trade) and 2 (gamble)),
  `items_content.py orig.packets.jsonl d2rs.packets.jsonl` (items channel compare IGNORING the
  creation frame, so "one frame late" and "different item" are separated).
- **Checks** (normal difficulty): `items-drops-nor-00..13` (14), `-cha-00..05`, `-uni-00..05`,
  `-rbo-00..05` (18), `items-vendor-<npc>-stock` (17). All run under Wine, 1.14d vs d2rs,
  `channels items`. 1.14d recordings are not committed (`traces/orig-cache` is 126 MB; refill
  with `--orig-cache --fill-cache`).
- **Ledger part** `docs/handoff/ledger/q-chk-items-drops.tsv` (0 format errors with ledger.py).

## Verdicts and routing

| Group | Result | First divergence | Owner |
|---|---|---|---|
| Vendor trade stock | 11 of 17 MATCH (akara, charsi, drognan, fara, lysander, ormus, asheara, hratli, halbu, larzuk, malah) | - | - |
| Gamble lists (gheed, alkor, elzix, jamella, nihlathak) | DIVERGED, 14 items each | item mode (3-bit, bit 2) differs on every gamble item | q-fix-server-store-fill (items) |
| Drehya | PARTIAL (no items on a side) | flow incomplete; investigate | q-fix-server-store-fill |
| Normal monster drops (14) | DIVERGED | drop 1 frame late on every item (known); content: nor-00/12/13 equal, others differ in flags bit 4 (identified 0x10), category 5 vs 16, x/y | q-fix-server-store-fill (items) |
| Champion/unique/random-boss (18) | DIVERGED, every item | d2rs kills the boss 40-140 frames after 1.14d (e.g. cha-00: frame 37 vs 84), so drops are late and differ | combat owner (q-fix-b-monster-combat) first, then items |
| nor-01 | PARTIAL | no drop on 1.14d side in the window | check-authoring: more bolts |

## Open

- **Nightmare/hell**: the 1.14d recorder (`autostart.py`) has no difficulty selection, so only
  d2rs changes (nig-00's 1.14d recording is identical to nor-00's). Generated checks were removed
  (`gen_drop_checks.py --all-difficulties` regenerates them). Needs a recorder difficulty option:
  scenario-diff owner (q-tool-state-diff). REC-1526.
- Not authored (need item GUIDs, panel sends or interact pokes): buy/sell/repair prices, gamble
  buys, refresh, imbue, chests, quest drops, every cubemain recipe (0x2A ItemToCube + a cube save),
  quality/seed sweeps ("poke kill N"). Ledger rows are NO-CHECK.
- Drops are confounded by combat timing; a stationary-target or direct-kill poke would isolate TC
  picks from AI.

## Repro

```
sh tools/coord/session-setup.sh; export D2_GAME_DIR=$HOME/game
cargo run -q --release -p data-tool -- excel-dir /tmp/excel $D2_GAME_DIR
python3 tools/scenario-diff/gen_drop_checks.py /tmp/excel; python3 tools/scenario-diff/gen_vendor_checks.py
python3 tools/scenario-diff/scenario_diff.py traces/checks/items-drops-nor-07.check --work /tmp/w
python3 tools/scenario-diff/items_content.py /tmp/w/orig.packets.jsonl /tmp/w/d2rs.packets.jsonl
```
