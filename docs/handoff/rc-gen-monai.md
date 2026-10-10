# rc-gen-monai hand-back

Base: specs-staging-7 + integ-r23 (merged again before the push; check_gen.py keeps every family).

## Checks before -> after
- EQUAL rows: 2635 -> 2635. No generated monster check is MATCH: every one is PARTIAL (every compared state frame equal, client gap) or DIVERGED.
- monster.ai / monster.quest / monster.superunique NO-CHECK rows with a recorded run: 0 -> 185.
  - 146 rows PARTIAL (state NO-CHECK = partly compared), 39 rows DIVERGED.
- Runs (suite.py, state channel, 150 ticks, 3 workers): gen-ai 146 (18 DIVERGED, 128 PARTIAL), gen-su 66 (19 / 47), gen-quest 12 (5 / 7).

## What changed
- `tools/check-gen/check_gen.py`: family `quest` (12 checks `gen-quest-*`, one per `monster.quest.*` row; class spawn by monstats Id, Countess and the Cain rescue guard through `superunique 6` / `3`). Selftest skips it on the synthetic tables.
- `traces/checks/gen/gen-quest-*.check` (12).
- Ledger part `docs/handoff/ledger/rc-gen-monai.tsv` (188 rows); `ledger.py --check` 0 errors. Orig recordings stay in `traces/orig-cache` (not committed).
- gen-ai and gen-su were already generated; nobody had run them.

## Open
- 39 DIVERGED rows: causes not analysed (game code is not in scope). First divergences sit in `state` frames 2-150; ledger `note` carries each. Examples: gen-quest-baal / gen-ai-baalcrab frame 106 player field m (1.14d 0 vs d2rs 1, 1274 diffs); gen-su-10 frame 30 monster class 229 field s.
- NO-CHECK, unreachable: monster.ai.none (no class), monster.ai.shadowmasternoinit (no class carries it), monster.ai.provisional-points (code list).
- Not touched: `traces/checks/gen/INDEX.tsv` has no quest lines (full regeneration is stale for netc2s upstream); regenerate it on the integrator.
- Checks only spawn one class per AI name next to the player; AI branches that need a target state (e.g. hireling, merc, pets, town NPC think) are covered by their first class only.
