# Hand-back: q-chk-items-vendors (2026-10-09)

Branch `claude/q-chk-items-vendors`. Checks run under Wine with the `items`
channel (merged: integ-r4, q-tool-items-channel, integ-r6). REC ids: none used.

## Done (verdicts, 1.14d under Wine vs d2rs)

| Check | Items | Equal | Verdict |
|---|---|---|---|
| `items-vendor-charsi-stock` | 43 / 43 | 43 | MATCH |
| `items-vendor-gheed-stock` | 30 / 30 | 30 | MATCH |
| `items-vendor-gheed-gamble` (0x38 action 2) | 14 / 14 | 0 | DIVERGED@20: item #0 byte 5, head mode/location bits (1.14d `00 12`, d2rs `10 00`) |
| `items-vendor-akara-stock` (re-run) | 41 / 41 | 39 | DIVERGED@20: two wands' charges (known, routed earlier) |

Both Gheed checks first recorded nothing on 1.14d (player never in reach);
fixed in the check by moving the last two pokes to (4842,4262), (4837,4274).
Ledger rows: `docs/handoff/ledger/q-chk-items-vendors.tsv` (4 rows; validates,
the remaining ledger.py errors are the stale generated files, not this part).

## Routed

- Gamble list item head (mode/location) and Akara wand charges:
  `claude/q-fix-server-store-fill` (items owner). The gamble first item is a
  ring; the whole 14-item list differs at the same bits, so one fix likely.

## Open (not done in this session)

Not authored or run: Elzix, Alkor, Jamella, Anya gamble; other vendors'
stock; vendor refresh; sell, repair, prices; imbue; monster/chest/quest/
champion/unique drops over many seeds; every cube recipe kind. Reasons:
each Wine run is ~12 min, and interact-pokes/check-gen did not land (their
branches hold only task briefs). Needs: panel/item-move sends for sell,
repair and cube; NPC positions for acts 2-5 (measure with a state dump).
Ledger rows for those stay NO-CHECK as in the main ledger.

## Repro

```
export D2_GAME_DIR=$HOME/game
python3 tools/scenario-diff/scenario_diff.py traces/checks/items-vendor-charsi-stock.check
python3 tools/scenario-diff/scenario_diff.py traces/checks/items-vendor-gheed-stock.check
python3 tools/scenario-diff/scenario_diff.py traces/checks/items-vendor-gheed-gamble.check
```
