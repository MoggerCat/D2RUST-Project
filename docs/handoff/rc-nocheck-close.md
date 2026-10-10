# rc-nocheck-close — NO-CHECK / NOT-IMPLEMENTED rows (owner goal: 100% checked)

Branch claude/rc-nocheck-close (from integ-r23). Status: batch 1 of ~7 (started; context-limited hand-back below).

## Done
| rows | check | verdict (1.14d under Wine vs d2rs) |
|---|---|---|
| quest.a1q1-den-of-evil | gen-qflow-a1q1-den (new qflow row) | packets MATCH 64/64; state PARTIAL (64/64 frames equal, 0 diffs) |
| quest.a1q6-sisters-to-the-slaughter, quest.done6-... | gen-qflow-a1q6-sisters-slaughter (new) | packets MATCH 64/64; state PARTIAL (64/64 equal) |

Rows stay NO-CHECK in the ledger: ledger.py keeps a row NO-CHECK while the state channel reports PARTIAL
(not all state fields compared). Open question for the owner: what does the state channel leave
uncompared in PARTIAL (it reports 0 differences)? Fixing that would close every PARTIAL row at once.

## Tool fixes
- suite.py: `save` and `cstate` channels added to ORIG_OUTPUTS (save-corpse-ama / save-fresh-ama / save-merc-bar no longer error); selftest passes; not yet re-run on those 3 checks.
- check_gen.py: `ignore q` removed from gen-fmt-anim-walk (regenerated with --family fmt only); strict re-run pending.

## Not started (survey of the 258 + 16)
- ui/system.ui frontend (~61 + 4): need frontend-channel 1.14d scenes (PC 1 facts) — try under Wine.
- system (49): mostly tooling/capture rows, d2s-load variants (3/4/6 need d2s-tool save variants), game-join, d2s-legacy (16 NOT-IMPLEMENTED: spec "draft", no crate cites it).
- message (31): S->C packet ids no scenario triggers.
- entity object/shrine/npc (~25): preset objects 574-582 (DS1 has none), shrine rows never picked by object init.
- other quest (a1q5, a1q7, a3q2/3/6/7, a4q1/4, a5q5, respec): need NPC/object flows per spec.

## Cannot apply (for the owner to rule on; NOT marked NOT-APPLICABLE)
None ruled yet; candidates after attempts: tooling rows "1.14d capture hooks" (9), object.preset 574-579 (no DS1 data).

## Helper tally (Haiku trial)
helper tasks given: 0; accepted as-is: 0; needed fixes: 0; context saved: n/a (none used yet; first batch was a pipeline proof).
