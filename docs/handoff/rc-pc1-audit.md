# rc-pc1-audit hand-back

Audit of the 517 non-EQUAL `needs_pc1=y` ledger rows (518 found).

## Done
- Ran every check named by those rows under Wine against the orig-cache:
  `traces/checks` 95 checks (MATCH 40, PARTIAL 80, DIVERGED 21 by channel),
  `traces/checks/gen` 48 checks (MATCH 17, PARTIAL 47, DIVERGED 1).
- Part `docs/handoff/ledger/rc-pc1-audit.tsv`: 145 rows now `needs_pc1=n`
  (1.14d recorded in the cloud). Row EQUAL only when every channel of
  every check is MATCH: 1 row. Others stay DIVERGED / NO-CHECK with the
  verdicts in the note (PARTIAL is never counted as equal).
- Ledger regenerated; `ledger.py --check` 0 errors. EQUAL 1487 -> 1488.

## Still `needs_pc1=y`: 385 rows
- 217 need Windows (ui 129, client 36, render 28, audio 22, seams 2):
  listed by family in `docs/handoff/pc1-data.md` Step 4.
- 168 are Wine-recordable but have no check yet (items 64: affix, gen,
  quality, props, inv.*; world: object.preset 9, waypoint 9, quest/npc
  gossip ~70, level/drlg ~12; skills/monsters/formats ~20). Flag kept
  because no check exists to run; each needs a recorded scenario + check
  (size M each). Not started: out of this session's ~2 h budget.

## Not run
- Rust fmt/clippy/nextest: no Rust changed. coverage/spec_index checks not run.
