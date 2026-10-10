# Hand-back: q-run-items (2026-10-09, branch `claude/q-run-items`)

Task: `docs/handoff/q-run-items-task.md`. Scope: item.* (coverage 581 + items), inv.*, drop.*, cube.*.
No crates changed. REC ids: none used (no unsettled choice).

## Done

- **check-gen families** (`tools/check-gen/check_gen.py`, `--check` and `--selftest` green):
  - `item`: 33 census checks `gen-item-NN` (20 bases each, 659 bases in the combined index), every base created at normal quality by poke `item`, seed fixed, items channel. `ear` is skipped (`poke item` refuses it).
  - `itemq`: 24 checks `gen-itemq-<quality>-<seedset>` (8 qualities x 3 seed sets, 20 spread bases at ilvl 85).
  - ledger-areas.tsv extended with the 581 `item.<id>-<code>` areas.
- **Hand-authored**: `inv-pick-store|belt|equip|drop` (ground item, 0x16 pickup, then 0x18 / 0x23 / 0x1A / 0x17).
- **Ran** (1.14d under Wine via `--orig-cache`, entries committed): 33 + 24 + 4 checks, and the 140 generated cube recipes (`traces/checks-gen/cube`, items channel).
- Ledger part `docs/handoff/ledger/q-run-items.tsv`, `ledger.py --check` 0 errors.

## Verdicts

| Group | Result |
|---|---|
| 581 base items (item.<id>-<code>) | 489 EQUAL, 92 DIVERGED |
| Quality census (60 items per quality and seed set) | normal 60/60 EQUAL; low and superior DIVERGED (16 of 20 per check); set, unique, magic, rare, crafted DIVERGED in 1-2 of 20 items per check, equal elsewhere |
| Cube recipes (140 generated) | 137 MATCH, 2 PARTIAL (cube-083, -084: recordings end at different frames), 1 no verdict (cube-017: d2s-tool cannot place the inputs on the cube page) |
| inv.cursor / belt / equip / grid | DIVERGED at the pickup |
| drop.* (chest, gold, quality, tc.*, walk, quest-helper), inv.gold / stash / corpse / mercenary / town-portal, item.props.*, item.runeword, item.desc.tooltip | no verdict: see "Cannot get a verdict" |

## Divergences, grouped by owner (the coordinator forwards)

**claude/q-fix-d7d8-items-net** (items, `crates/d2-sim/src/item`):
1. Flag 0x10 (identified) missing on d2rs for 92 of 659 bases at normal quality, and for low / superior armor and weapons (16 of 20 per quality check). 1.14d stream is longer (e.g. dr4 21 vs 20 bytes; superior dr4 28 vs 20). Repro: `python3 tools/scenario-diff/scenario_diff.py traces/checks/gen/gen-item-00.check --orig-cache traces/orig-cache` (also gen-itemq-low-0, gen-itemq-superior-0).
2. Property list tail of magic / rare / set / unique / crafted items: first differing byte inside the last property bytes, 1.14d one byte longer. Items: dr6 (unique-0, rare-0, crafted-0), 7gd (rare-1, crafted-1), 9kr (magic-1, set-1), ucl (set-0..2). Repro: `gen-itemq-unique-0`, `gen-itemq-rare-1`, `gen-itemq-magic-1`, `gen-itemq-set-0`, `gen-itemq-crafted-1`.
3. Cube: cube-083 / -084 only PARTIAL (frame count), no content difference.

**claude/q-fix-items-shop** (inventory moves): PickItem (0x16) of a ground item: 1.14d sends S->C 0x9C at frame 10, d2rs nothing, so 0x18 / 0x23 / 0x1A / 0x17 are never reached. Repro: `python3 tools/scenario-diff/scenario_diff.py traces/checks/inv-pick-store.check --orig-cache traces/orig-cache`. The state channel also differs from frame 2 (player field `q`, 1.14d 9 entries, d2rs none): join-stream owner.

## Cannot get a verdict (needs a tool feature)

- **drop.***: no poke that rolls a treasure class or kills a unit directly, so drops stay confounded by combat timing (see `q-chk-items-drops.md`). Needs `poke drop <tc> <ilvl> <x> <y>` (1.14d TC walker) plus quality / magic-find setup.
- **Unique / set / rune word row picks** (item.unique 403 rows, item.runeword 169, set items): `poke item` takes a quality, not a row; needs `idx=ROW`-like selection on the poke (the save tool has `idx=ROW`).
- **inv.gold / stash / corpse / mercenary / town-portal / item-use**: flows need sends with a stash or merc setup; not authored.
- **Cube**: ingredient removal (0x9D), 0x2A put-in, cow-level portal use, ops-gated recipes; `gen_cube.py` needs a larger-cube placement for rows like 017.
- **Ledger merge conflict**: my rows for item.quality.*, item.affix.magic / rare / crafted and item.gen.normal-quality lose to `items.tsv` (first part kept). Coordinator: let `q-run-items.tsv` win for those areas, or copy them into `items.tsv`. The same holds for cube.recipe.* / inv.* if another part keeps NO-CHECK rows.
- **ledger.py** only resolves `checks` against `traces/checks/*.check`, so the generated checks are named in the `note` (`[gen-item-NN]`), not in `checks`.

## Repro

```
sh tools/coord/session-setup.sh; export D2_GAME_DIR=$HOME/game
python3 tools/check-gen/check_gen.py --check
python3 tools/scenario-diff/scenario_diff.py traces/checks/gen/gen-item-00.check --orig-cache traces/orig-cache
python3 tools/scenario-diff/scenario_diff.py traces/checks-gen/cube/cube-083-*.check --channels items --orig-cache traces/orig-cache
```
