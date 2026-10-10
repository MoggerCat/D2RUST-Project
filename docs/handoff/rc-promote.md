# rc-promote hand-back

Ledger EQUAL 2697 -> 2809 (`ledger.py`, 0 errors). Part `docs/handoff/ledger/rc-promote.tsv`; `part_rank` gives it
rank 3 (older rc-* parts such as rc-pc1-audit held the same rows and won by name order). `ledger.py --fix` also
refreshed 4 rows of rc-promote / rc-run-4 / rc-run-7 that the new check-status rows contradicted.

## Rule (REC-2055/2056, scenario-diff.md open questions 6-7), strictly, per check
EQUAL only when: every channel MATCH except a state PARTIAL whose only cause is the d2rs client-gap header
(both state headers read from the work dir: no one-sided field, every gap starts "client: headless bridge");
an items PARTIAL only with 0 items on both sides; no `ignore` line; pokes-only or packets MATCH.
A row is EQUAL when all its checks are. Other rows: DIVERGED if a check diverged, NO-CHECK (reason in note) if PARTIAL.

## Run (fresh, `--orig-cache traces/orig-cache --fill-cache --workers 3`, 1.14d re-recorded under Wine on cache miss)
- 84 checks of traces/checks (the 123 rows' non-gen checks) + 156 of traces/checks/gen (32 of the rows + all 124
  gen-sysc-* from rc-gen-client): 240 checks.
- Per check: 177 qualify, 39 PARTIAL (input/send and no packets channel), 1 ignore line, 22 DIVERGED/ERROR
  (packets DIVERGED 15+3, draws DIVERGED 4; 1 ERROR: gen-sysc-client-ui-b-... d2rs `play` wrote no output).
- Rows: 113 -> EQUAL (78 DIVERGED, 35 NO-CHECK), 41 DIVERGED -> NO-CHECK/DIVERGED per the fresh verdict.
  3 rows stay PARTIAL/DIVERGED (a check diverged; last_verdict is the stale word of a part).
- checks-status.md: rows of the 240 checks replaced in place (totals not recomputed).

## Tool
`tools/coord/promote.py SUITE.json [--ledger F] --out PART.tsv` (`--list` per-check, `--selftest`). Run it on the
ledger as it was before your part (a re-run on the merged ledger sees its own rows). Group `coverage` rows are
written as `cov-promoted` (ledger.py merges group coverage as "exercised" only).

## Open
- 39 checks with input/send and no packets channel: add `packets` to the check (then they qualify).
- 1 ignore line check; 22 DIVERGED are real divergences (packets 18, draws 4).
- orig-cache entries refreshed by --fill-cache are left uncommitted (rule).

## Round 2 (after the coordinator's no-deadline note)
- integ-r23 had added `packets` to the hand-written input checks; re-run of the 40 still PARTIAL: a5-wp (7) and
  gen-skill (7) now show real packets DIVERGED; added `packets` to 10 ass-*, 4+15 gen-shrine-* (check_gen shrine
  family too), 2 ass-lightning-sentry-*; removed `ignore q` from sys-intents-moves (EQUAL now).
- Result: 25 of those checks now run packets; most diverge in C->S (the gap's condition occurred), so their rows are
  DIVERGED by the fresh verdict, not promoted. Part regenerated against the ledger without rc-promote
  (265 checks): 62 rows EQUAL, 23 DIVERGED. Ledger EQUAL 3073 (integ-r23 + this), 0 errors.
- C002/C028 (causes-99): not started; need the packets C->S divergences and the one-sided field list per check.

## Round 3 (packets DIVERGED groups, fixed by count)
- Stale binary: the 36 `ass-*` / `gen-skill-*` packets DIVERGED were a suite-bin built before the post-load 0x23 fix
  (a62bc1547) merged; rebuilt -> MATCH (check `target/suite-bin` age when a first divergence is a known fixed one).
- `pos` / `hop` pokes: d2rs queued its own S->C 0x15 (play aid); `state-dump` now queues none (`poke::Env::reassign`,
  `app::poke::set_compare_run`); 8 gen-sysc-client-msg-ui + interact-* + a2-npc-* first divergence moved frame 4 -> 16.
- Store-item recharge (`0x0055FE80`, `properties.md` §5 r9; pc1-data.md item 27): `vendor_world.rs` recharge now sets
  stat-204 charges to max (was a log-only stub); Akara guid 8 charges 67/67. ~30 NPC talk/trade checks packets MATCH.
- Ledger EQUAL 3134 (incl. integ-r23), 0 errors; part 54 rows against the ledger without rc-promote.
## Open (first divergences, by count)
- 0x5D QuestItemState, 13 checks: (a) 1.14d sends `5d 04 00 0c 00 00` (chain 4) in the frame of a `warp 40` poke from a
  fresh ScnBuy-like save (drognan/lysander stock, act-change x3, ...): d2rs sends none; `send_status` (quests.rs:1979)
  needs the player's room act = record act (0), as 0x00544190, so the sender in 1.14d is not L1 at tick time (caller
  0x53d72c is the 0x0053D710 builder); owner to read the 1.14d sender. (b) d2rs sends extra `5d 1f/20/21 00 01 ..` (act 5
  chains 31-33) on a5 warps that 1.14d does not (a5-wp-31..38 x7, drehya stock).
- 0x9c vendor stock (halbu): frame 66 0x9c vs 0x67 MonsterMove ordering; cube-000..005: 1.14d `0x3f` (targeting reset)
  where d2rs sends `0x47`/`0x48`; assassin sentries (7): 0xA7/0xAC/0x21 summon messages; gen-shrine-7 (0xA8 size),
  -17 (0x2C vs 0x0E); act-video x3.
