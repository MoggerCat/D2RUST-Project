# q-run-objects: hand-back (2026-10-09, branch `claude/q-run-objects`)

## Done
- `check-gen` family `obj`: one check per objects.txt Id (571), `traces/checks/gen/gen-obj-<Id>.check`: `poke object <Id> @x @y`
  at frame 10 (at the player: inside every operate box, `world/objects.md` §7.1; 3 sub-tiles out, 1.14d walks the player over and
  d2rs does not, which hides the object), `poke operate @2:<Id>` at 20, Rogue Encampment, ScnAma seed 1234, channels `state items rng`, 80 ticks.
- `suite.py --checks-dir DIR` (generated checks are not in `traces/checks`). `tools/check-gen/obj_verdicts.py` collates work dirs to the ledger part.
- Ran all 431 ledger object rows on 1.14d (Wine) and d2rs; recordings in `traces/orig-cache/gen-obj-*`. Ran the 136 `gen-lvl-*` checks with
  channel `state` only (cache `gen-lvl-*`) for the populate rows; classes per level in `q-run-objects-levels.json`.
- Ledger part `ledger/q-run-objects.tsv`: 431 `object.<id>-*` rows, 73 `object.operate.*` / `object.init.functions` aggregates, 9 `object.populate.*`.

## Verdicts (state compared with `--ignore q`)
- Object rows: 408 PARTIAL (state, items, rng equal where compared; state is never MATCH while one side lacks fields), 23 DIVERGED, 0 MATCH.
- Aggregates: 16 operate functions + init.functions DIVERGED; populate: 3 DIVERGED (1, 2, 3), 4 PARTIAL, 1 without a level (Crate).
- Not verdicted: `object.preset.574-582` (ids are not objects.txt Ids; presets come from DS1 files, need a level that holds one),
  `object.operate.35-38,60` (no operate function, nothing to operate), `object.operate.*` of rows 569+ not in the ledger.
- Weak spot: chests operated in town drop nothing on either side, so loot is tested only for objects that drop in a town (armor stands, racks, corpse, altars).

## Shared causes (first divergence)
1. **`q` at frame 2 in every check** (REC-1625): d2rs does not carry the save's quest records. Hidden by `--ignore q`; owner: quests / d2s load.
2. **Operate drops items that d2rs lacks, then game seed + rng draw #0 at frame 19-20** (16 rows): objects 104, 105, 550, 551 (armor stand, qui),
   106, 107, 548, 549 (weapon rack, ssd), 179, 180 (bookshelf, tsc), 269 (corpse, gld), 149/354/355/356/405/406/407 and 397 (altar/chests: item bytes differ,
   x position bit / flags). 1.14d site `0x552e31` draws; d2rs none. Owner: unrouted (items from objects; `specs/world/objects.md`).
3. **Walk before operate**: 267 (bank), 385 (urn): 1.14d player mode 3 vs d2rs 5; d2rs treats range as always 1 (objects.md §7.1).
4. 194 (stair): monster mode 2 vs 1 at frame 69; 298 (Arcane Sanctuary portal): object mode 1 vs 0 and an rng draw (site `0x624563`) d2rs lacks.
5. **Population** (levels): objects present on one side only in 8 of 136 levels (36 pairs incl. 79 in L37, 144 in L4, 94/95/148 in L55, 185/246 in L92,
   402 waypoint in L134 d2rs only); 34 levels diverge on the game seed from frame 20 (D1, q-fix-seed-order).

## Repro
`python3 tools/scenario-diff/scenario_diff.py traces/checks/gen/gen-obj-104.check --orig-cache` (then `state_diff.py ... --ignore q`);
whole set: copy the files to a dir and `suite.py --checks-dir DIR --workers 3 --orig-cache --fill-cache`; then
`python3 tools/check-gen/obj_verdicts.py --levels docs/handoff/q-run-objects-levels.json --out docs/handoff/ledger/q-run-objects.tsv`.

## Open
- `ledger.py` only resolves `traces/checks/*.check`: the part writes `checks -` and names `gen-obj-N` in the note (coordinator: let it read `traces/checks/gen`).
- Operate with loot needs a non-town level (poke warp first) to exercise treasure classes; not done.
- Preset objects (574-582) and Crate need a level with a preset/crate.
