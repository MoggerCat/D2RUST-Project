# rc-gen-client hand-back (REC-2550)

**Task:** generator for ledger rows system.client (85) and system.sim / seams / flows (37), NO-CHECK.

## Checks, before / after
- Before: 166 rows NO-CHECK, 2 EQUAL (outside the new checks), 47 DIVERGED, 4 NOT-IMPLEMENTED in these prefixes.
- After: new family `sysc` in `tools/check-gen/check_gen.py` (`SYSC_T` scenario templates, `SYSC_RULES` row -> template), 124 checks `traces/checks/gen/gen-sysc-*`; ran 123 (suite, `--orig-cache --fill-cache --workers 3`).
- Channel runs: 169 MATCH, 19 DIVERGED, 115 PARTIAL. Checks with no channel DIVERGED: all carry a PARTIAL `state` channel, so **EQUAL: 0** (a check is EQUAL only with every channel MATCH).
- Ledger part `docs/handoff/ledger/rc-gen-client.tsv`: 16 rows DIVERGED with first divergence, 150 NO-CHECK (43 with the reason no scenario reaches them in `note`; the rest have a check but `ledger.py --fix` resets the state to NO-CHECK until `checks-status.md` carries the verdict).

## What changed
- Generator family, 166 area ids added to `ledger-areas.tsv`, merged `claude/integ-r23` (kept all families).

## Open causes (game code untouched)
1. **draws** (5 checks: assets.b, render-pipeline.b, ui.b, world-screen 1-2): tick 72-73 draw row 173, 1.14d `CelDraw` vs d2rs `unit` (d2-client draw list, size M).
2. **talk / waypoint scenarios** (msg-ui 2, 5, 9, 10, 11, 16, 17, 18; 8 checks): frame 4 first S->C message is 0x07 in 1.14d vs 0x15 in d2rs (S->C order after poke pos, likely the warp 0x07 frame already in `system.seams.messages.warp-0x07-frame`, size S).
3. **act scenario** (msg-ui.20, act-change 1 and 3): frame 9 first S->C 0x5d in 1.14d vs 0x08 in d2rs, state also DIVERGED (act change, size M).
4. **state channel PARTIAL** in 115 channel runs: the `state` recorder compares only part of the layers; those checks cannot reach EQUAL until it is complete (size M, owner of state-snapshot).
5. `gen-sysc-flows-save-exit-5-load` produced no result (1.14d save channel run did not finish): rerun.
6. Rows with no reachable behaviour (43): pets (needs summon), hireling, chat/hire/event text, audio, d2rs-internal bridge/Bevy seams, exit/periodic save.
