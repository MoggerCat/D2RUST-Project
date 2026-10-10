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
