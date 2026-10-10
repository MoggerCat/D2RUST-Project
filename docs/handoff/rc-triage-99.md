# rc-triage-99: causes-99.tsv (work queue for the 99/99 push)

Output: `docs/handoff/causes-99.tsv` (sections: DIVERGED causes sorted by rows, NO-CHECK by why,
NOT-IMPLEMENTED by feature). No code changed, no ledger rows changed.

## v1 (first push)
- Input: `fidelity-ledger.tsv` (1021 DIVERGED, 652 NO-CHECK, 46 NOT-IMPLEMENTED), `checks-status.md`
  (593 checks), the first divergences quoted in ledger notes (rc-run-*, rc-link-checks, q-diff-skills
  ...). A row's cause = its earliest parsed first divergence (status table first, then its note).
- Signature = channel + unit type + field (+ small 1.14d->d2rs values for modes) / packet id or
  byte / rng site / draw op+column. Same signature and same code path = one cause.
- PARTIAL rows: reason read from the check file (ignore line, input/send without packets,
  items channel); the rest ("one-sided field/client gap") needs the comparator log: v2.
- Caveat: notes are from several commits (r16..r23); draws row 173 was fixed by rc-draw-row173
  after its verdict. v2 re-runs every check behind a DIVERGED row on this branch.
- owner: a ledger owner without a hand-back in docs/handoff (else empty).
- est: the rows' majority ledger size; model: opus for M/L and seed/rng causes, else sonnet.

## v2
- Re-ran on this branch at dc7576e57 (integ-r23 merged; `suite.py --orig-cache traces/orig-cache
  --no-playthrough --workers 3`, orig-cache hits): the 190 cached gen checks behind DIVERGED /
  NO-CHECK rows; hand checks (319 cached, 70 + 196 gen uncached with `--fill-cache`) still running.
  A row with fresh results uses only them (its older note is ignored).
- New cause "RE-RUN EQUAL under REC-2055/2056": every channel MATCH, or PARTIAL only from the d2rs
  client gap / no item on either side, with packets MATCH or no input/send. Settle in a ledger part.
- PARTIAL reasons now come from the comparator (state_diff one-sided fields and gaps, items_diff).
- Ledger after merging integ-r23: 838 DIVERGED, 500 NO-CHECK, 46 NOT-IMPLEMENTED.

## v3
- Hand checks re-ran too (319 cached checks: MATCH 86, DIVERGED 195, PARTIAL 235 channel results).
  511 checks have fresh results. Uncached gen (196) and hand (70) checks, plus 20 checks new in
  the ledger, are recording 1.14d now (`--fill-cache`; the cache files are not committed here).
- Ledger after merging integ-r23: 824 DIVERGED, 414 NO-CHECK, 46 NOT-IMPLEMENTED.
- C003 (71 rows) is free: the re-run is EQUAL under REC-2055/2056, so a ledger part settles it.
