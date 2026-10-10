# q-run-systems: hand-back (2026-10-09, branch `claude/q-run-systems`)

## Done
- 17 census checks `traces/checks/sys-*.check` (stats, clamps, states, stat points, vitals x2, level-up, corpse, tick x2, intents, units, pets, town portal, difficulty x2, gold), run against 1.14d under Wine and d2rs with `suite.py --filter 'sys-*' --orig-cache --fill-cache` (orig-cache entries committed, text only). State checks use `ignore q` (see cause 1).
- Result (36 channel runs): 13 MATCH (rng/items), 11 PARTIAL (state, q ignored), 12 DIVERGED, 0 errors.
- Ledger part `docs/handoff/ledger/q-run-systems.tsv`: 94 rows (sim/stats, stat-lists, tick, units, pets, intents-events, combat damage/hit/events/vitals, flows server-tick/act-change/save-exit, seams/messages). `ledger.py --check`: 0 errors.

## Verdicts by owner (divergences are routed, not fixed)
| Owner | First divergence | Repro |
|---|---|---|
| claude/q-fix-npc-interact | state `q` (quest flag record): 1.14d fills it at frame 2, d2rs has none (hidden by `ignore q`; every state check diverges at frame 2 without it) | remove `ignore q` from any `sys-*` check, `scenario_diff.py traces/checks/sys-stats-base.check --orig-cache traces/orig-cache` |
| claude/q-fix-join-items | packets s2c frame 68: 1.14d 0x2c vs d2rs 0x6d | `scenario_diff.py traces/checks/sys-intents-moves.check --orig-cache traces/orig-cache` |
| claude/coord-resume-3 (walk/pets) | sys-pets-skeletons frame 102 player x 5160 vs 5159 | `... sys-pets-skeletons.check` |
| claude/q-fix-seed-order | game seed at frame 5 (combat-*), rng extra draw at frame 5 after a Blood Moor warp in nightmare/hell (population/room.rs:193), units-census frame 61 (ai/functions.rs:488) | `... sys-difficulty-nightmare.check`, `... sys-units-census.check` |
| unrouted (units/event_records) | town NPC mode `m`: 1.14d 2 vs d2rs 1 at frame 24/32 (sys-tick-idle-a2/a5, act-travel-lut-ama); a5 also misses draw 0x5e70a5 | `... sys-tick-idle-a2.check` |

## Open (no verdict; reason)
About 280 NO-CHECK rows of the group stay open: audio (needs audio-diff channel, q-tool-audio-diff), render/* and client/* (draws channel, one scene only), formats/* (no 1.14d byte-compare tool beyond mpq-tool), seams/*, client/bridge and client/model (d2rs-own design, nothing in 1.14d to compare). Not attempted here.
Rows verdicted PARTIAL still need: items stat lists (`is`) in the state channel, client vitals sync, timer-handler coverage; checks with a combat/damage hit are blocked by the seed-order divergence.
## Notes
- `sys-town-portal` ran only 29 ticks on both sides (the interaction ends the recorder early); its PARTIAL verdict covers that window.
- `sys-units-census` needs `umod` ids on `unique`/`champion` spawns (poke spawn rule).
