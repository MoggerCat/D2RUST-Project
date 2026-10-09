# Hand-back: q-tool-items-channel (2026-10-09)

Session q-tool-items-channel, branch `claude/q-tool-items-channel`. Task:
`docs/handoff/q-tool-items-channel-task.md` (fidelity-gaps §4, the `items`
channel). REC ids used: REC-1310.

## Done

- **`items` channel** of `scenario_diff.py` (`specs/tools/scenario-diff.md`
  §3 rule 13; `tools/scenario-diff/items_channel.py`, comparator
  `items_diff.py`). It reads the packets channel's two recordings
  (`record_packets.py`, `d2-client state-dump --packets`; recorded once
  when a check asks for both `packets` and `items`). An item is created
  by the first S→C 0x9C / 0x9D carrying its GUID; the two lists are
  compared item by item in creation order: frame, id, action (the
  creating event), category, owner, then the bit stream byte for byte,
  with the head field of the first differing bit named (flags, version,
  mode, location, code). GUIDs are left to the packets channel.
  PROVISIONAL REC-1310 (open question 5): the hook is the S→C queue both
  sides already pin, not the 1.14d item-creation function, so items never
  sent to the client are not seen (edge case 4).
- `packets_channel.py`: recording split into `record()` (shared, once per
  run); `suite.py`: `items` in `ORIG_OUTPUTS`.
- Selftests: `items_diff.py --selftest` 10, `scenario_diff.py --selftest`
  146 (dry runs of the channel; every committed check parses),
  `suite.py --selftest` 19.
- **Three checks**, verdicts (1.14d under Wine, d2rs at 0fa047ce + this
  branch):

| Check | Items 1.14d / d2rs | Equal | Verdict | First difference |
|---|---|---|---|---|
| `items-vendor-akara-stock` | 41 / 41 | 39 | DIVERGED@20 | items #7, #12 (wands): charged Teeth (stat 204, param 4289) current charges 67 (= max) on 1.14d, 65 / 64 on d2rs |
| `items-drop-gold-potion` | 6 / 6 | 0 | DIVERGED@4 | flag 0x10 (identified) missing on every d2rs item; all other bits (positions, gold amounts, compact records) equal |
| `items-drop-monster-kill` | 1 / 1 | 0 | DIVERGED@36 | 1.14d drops gold at frame 36; d2rs a stamina potion (`vps`) at frame 37 |

  Charsi: `interact-pokes` is not built and Charsi's position is not
  measured, so the store check uses Akara's proven scripted flow
  (`items-vendor-akara-buy` without the buy).
- Ledger part `docs/handoff/ledger/q-tool-items-channel.tsv` (8 rows:
  drop.monster, drop.gold, drop.placement, item.treasure-tables,
  item.property-functions, vendor.store-gen, vendor.akara, net.s2c.0x9c),
  validated with `ledger.py` from `claude/q-fidelity-ledger` (0 format
  errors).

## Routed (not fixed here; owner q-fix-server-store-fill, items)

1. Store items: charged-skill current charges below max (1.14d gives
   max on a store wand; `items/properties.md` charged-skill function).
2. Ground items created by the poke `item` path (`ItemUnits::create_item`
   + `ground_place`) lack item flag 0x10; the d2rs drop path sets it
   (`items/create.rs`), 1.14d's `0x00558D90` path does too.
3. Monster drop: one frame late and a different treasure pick (potion
   where 1.14d drops gold) with the seeds poked at frame 30.

## Open

- `items_diff.py --selftest` is not in CI (`scenario_diff.py --selftest`
  is not either); `.github/workflows/ci.yml` is shared.
- A creation-hook variant (REC-1310) would cover items never sent.
- Gamble, cube and quest-item checks need `interact-pokes` / panel sends.

## Repro

```
sh tools/coord/session-setup.sh           # from coord-resume-3; game + Wine
export D2_GAME_DIR=$HOME/game
python3 tools/scenario-diff/items_diff.py --selftest
python3 tools/scenario-diff/scenario_diff.py traces/checks/items-vendor-akara-stock.check
python3 tools/scenario-diff/scenario_diff.py traces/checks/items-drop-gold-potion.check
python3 tools/scenario-diff/scenario_diff.py traces/checks/items-drop-monster-kill.check
```
