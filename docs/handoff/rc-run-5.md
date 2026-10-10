# rc-run-5 hand-back (runner, no code changes)

Branch claude/rc-run-5. Command: suite.py --checks-dir traces/checks/gen
--filter 'gen-missile-*,gen-item-*,gen-itemq-*,gen-wp-*,gen-shrine-*'
--orig-cache traces/orig-cache --fill-cache --workers 3 --no-playthrough.
Whole filter ran (380 checks); per coordinator split, this part reports the
FIRST half by sorted name (190 checks, gen-item-* .. gen-missile-419).
The second half is rc-run-5b's.

- First half: 176 EQUAL (REC-2055/2056 rules: state PARTIAL with 0 differences
  counted equal), 14 DIVERGED.
- Ledger part: docs/handoff/ledger/rc-run-5.tsv (rows whose checks/note name a
  first-half check; EQUAL only when every named check of the row is EQUAL).
- Causes: docs/handoff/rc-run-5-causes.tsv. 10 distinct first differences:
  11 gen-itemq-*/gen-item-* item-stream divergences after the head (by bit
  offset: 105 x3, 44 x2, 34 x2, 33, 45, 95, n/a; likely d2-sim items,
  specs/items/generation.md, size M overall), 3 single missile ones
  (gen-missile-149 missing S->C 0xa8 at f12; -407 extra rng draw f36; -411
  missing draw), size S each.
- Open: verdict is by my script (/tmp, not committed) from result.json;
  EQUAL is approximated as every channel MATCH or PARTIAL with 0 differences
  and equal frames. orig-cache rewrites left uncommitted per brief.
- Not run (docs only): fmt/clippy/nextest/coverage/spec_index checks.
