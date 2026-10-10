# rc-run-6 hand-back (runner, no code changes)

Command: `suite.py --checks-dir traces/checks/gen --filter 'gen-state-*,gen-netc2s-*' --orig-cache traces/orig-cache --fill-cache --workers 3 --no-playthrough`.

- 259 checks, 17720 ticks, 17719 equal. Channels: packets MATCH 258, DIVERGED 1; state PARTIAL 184 (all gen-state-*).
- Every state PARTIAL has one cause: the d2rs header's `RUN_GAPS` client gap. None of those checks has an `ignore`, `input` or `send` line, and each packets channel MATCHes. They count as EQUAL under DECIDED REC-2055/2056.
- Ledger part: `docs/handoff/ledger/rc-run-6.tsv`, 232 rows (fresh verdicts; EQUAL where the checks above hold). Four rows (net.c2s.0x16, net.c2s.0x19, net.s2c.0x2a, tools.poke.tick-end) also list non-gen checks I did not rerun, so their verdict and state are left as base.
- Causes: `docs/handoff/rc-run-6-causes.tsv`, 1 row.
- Open: gen-netc2s-60 (SwapWeapons, 0x60) diverges at frame 20. The first s2c message is id 0x23 in 1.14d and 0x47 in d2rs. Size S. Ledger rows net.c2s.0x60 and net.s2c.0x97 are marked DIVERGED@20 by check-level verdict; the c2s bytes are equal.
- Not investigated (brief: runner only).
