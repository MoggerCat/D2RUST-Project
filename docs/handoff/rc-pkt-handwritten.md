# rc-pkt-handwritten hand-back
EQUAL 2866 -> 2886 (merged ledger vs origin/claude/integ-r23; 25 rows in my part, some lose to higher-ranked parts).
Changes:
- `packets` added to 66 hand-written input checks (b*..w*) in traces/checks. 17 UI checks with a `key` input
  (ui-draws-*, panel/hotkey keys) did NOT get it: `d2-client state-dump --input` cannot apply panel keys
  headless, packets ends ERROR (seen on ui-draws-belt-ama). They stay as before (draws DIVERGED anyway).
  save-items-ama untouched (channel save).
- All 10 `ignore` lines removed (q in 9 sys-* checks, `seed s fc sp` in walk-click-walk-sor): state_diff
  with no ignore shows 0 q/seed/s/fc/sp differences where the check is PARTIAL; sys-difficulty-hell/nightmare
  stay DIVERGED on monster 1:21 (m, fr, fc, s) and have no `q` differences either, so nothing was weakened.
- suite.py: a check whose channel has no ORIG_OUTPUTS entry (save-*) is reported ERROR up front and the
  suite continues (also KeyError is caught per check); selftest passes. `--filter 'save-*'` -> 5 ERROR.
Run: 73 checks, 150 channel results: packets MATCH 22, DIVERGED 45, PARTIAL 1; items MATCH 20; rng MATCH 6;
state PARTIAL 34 / DIVERGED 12; draws DIVERGED 8.
Open: 45 checks now show a real packets divergence (first differences in checks-status.md): e.g.
sys-pets-skeletons s2c 0x5E vs 0x23 at frame 2, walk-click-walk-sor c2s 0x01 bytes[1] 4 vs 3 at frame 22,
ui-draws-left/right-skill-pick, minipanel; sizes M each, owner d2-server/d2-proto.
Ledger: docs/handoff/ledger/rc-pkt-handwritten.tsv (25 rows EQUAL: sys-tick-idle, sys-units-census,
dru-tornado/twister/arctic-blast); `ledger.py --fix` also touched 8 other parts (contradiction fixes).
checks-status.md: rows of the 73 checks replaced/added.
