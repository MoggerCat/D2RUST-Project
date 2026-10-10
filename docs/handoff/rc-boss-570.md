# rc-boss-570: hand-back (2026-10-10, branch `claude/rc-boss-570`)

Base: `claude/specs-staging-7` + `claude/integ-r17`. REC-2250 used.

## Checks

| Set | Before | After |
|---|---|---|
| `gen-boss-570` | frame 31 (m: 1.14d mode 0, d2rs 1) | EQUAL (150/150, PARTIAL) |
| other gen-boss / gen-su | not re-run before | see the final line below |

## Cause (one)

`BaalCrabClone` (`0x005FD210`) with a missing owner kills itself on its first think
(`0x0057CCB0(game, unit, 0, 1)`). d2rs' body was right (`bodies5::baal_crab_clone`), but
`AiActs::kill` in `wiring/action/ai.rs` called the `Pending::ai_kill` default, which is empty. Now
it runs `reaction::kill_by` on the view's parts (death mode 0). Spec note in
`ai-bodies-5.md` §22.

## Open

- Other AI bodies that call `world.kill` (Baal crab pair, minion-owner deaths) now really kill;
  their checks were not re-run beyond gen-boss / gen-su.
- Ledger: `ledger/rc-boss-570.tsv` (1 row). d2-sim: clippy clean, nextest 4768 pass.
