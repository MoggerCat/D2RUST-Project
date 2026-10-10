# rc-gen-misc hand-back

Ledger EQUAL 2080 -> 2137 (part `docs/handoff/ledger/rc-gen-misc.tsv`, 61 rows: 58 EQUAL, 3 DIVERGED).
NO-CHECK rows of my areas: checked (a check exists and ran) 0 -> 61.

## Runs (1.14d recorded under Wine, orig-cache not committed)
| rows | checks | result |
|---|---|---|
| monster.superunique x6, monster.boss x4 | gen-su-1/5/20/21/40/66, gen-boss-559/704/705/709 | 150/150 frames equal, nothing ignored -> EQUAL (REC-2055/2056) |
| level.* x15, waypoint x3 | gen-lvl-*, gen-wp-2/30/37 | state equal all frames, rng MATCH -> EQUAL |
| system.sim.stat-lists/stats x20, system.combat.vitals x5 | sys-stats-base/clamp, sys-states, sys-statpoints, sys-vitals-create-bar/sor, sys-levelup-state, sys-corpse-state | `ignore q` REMOVED from all 8 checks; still all frames equal, rng MATCH -> EQUAL |
| monster.quest x12 | NEW family `qkill` (check_gen.py): 12 gen-qkill-* | cain-rescue-guard, izual, nihlathak, radament, summoner EQUAL (monster dies on both sides); baal DIVERGED@57 (game seed), blood-raven DIVERGED@64 (player mode 5 vs 4), countess DIVERGED@51 (SU member seed) |

## Changed
- tools/check-gen/check_gen.py: family `qkill`; tools/scenario-diff/suite.py: skip channels without a recording (the `save` channel crashed a --fill-cache worker with KeyError).

## Still NO-CHECK (with reasons)
- monster.quest ancients, cow-king, griswold, hellforge-hephasto: gen-qkill checks exist and are equal, but the single missile did not kill (the monster stayed alive at 1 life on both sides), so no kill is compared. Needs a better kill (several missiles / melee). 
- item.* (affix, gen, quality, props, base, runeword, set, unique), item.bitstream: gen-itemq results: 14 MATCH, 9 DIVERGED (crafted/rare/magic-1/set/unique-0 at items #6-#18); no per-row mapping, rows stay (the DIVERGED set belongs to the items owner).
- system.flows.save-exit, items-load-mixed, vendor.drehya (items 0/0), a2-npc-drognan: save-* checks not run (suite lacks a save recording); drognan state equal, packets MATCH but the row note says PARTIAL for other reasons; not promoted.
- system.seams, system.flows.client-frame, system.replay, system.perf, object.preset (574-582), object.populate, shrine.0/4/5/16, cube.*, vendor.*: need interact/preset pokes (poke cannot create presets) or are spec-only seam rows; unreachable by a scenario.
- gen-boss-333 (sim.monster-mode-records) still DIVERGED@71.
- coverage.check-gen.* family rows: not promoted (family rows cover hundreds of checks, not run in full).

## Causes for others
- baal qkill @57: game seed differs before the kill (spawn/AI draws of class 544); blood-raven @64: player mode 5 vs 4; countess @51: member seed s.
