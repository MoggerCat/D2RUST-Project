# q-fidelity-ledger (integrator) — done / open / repro

Session q-fidelity-ledger, 2026-10-09. The coordinator's plan change at 18:17 UTC made
this session the integrator of 8 part sessions instead of the sole author.

## Done
- `tools/coord/ledger.py` (`--check`, `--fix`, `--selftest`): validates the part files
  `docs/handoff/ledger/<part>.tsv` (`#ledger 1` format), reconciles them with
  `checks-status.md` (`--fix`), merges them into `docs/handoff/fidelity-ledger.tsv` and `.md`,
  and lists the spec files, checks and message ids that no row names.
- Merged all 8 parts plus `integrator.tsv` (30 rows, hand-written by this session for the
  specs and checks no part named): 4,148 rows, DIVERGED 289, NOT-IMPLEMENTED 59,
  NO-CHECK 3,671, UNKNOWN 129, EQUAL 0; exercised yes 553 / no 1,962 / ? 1,633. Nothing is
  left unnamed. Coverage ids are matched to entity rows through aliases in `ledger.py`
  (level id, quest slot, AI name, table row id); `yes` comes from the coverage report
  JSONs' seen lists.
- Integrator edits to the part copies (`tools/coord/ledger_fixups.py`, run by `tools/coord/ledger-pull.sh` after every pull,
  so a re-pulled part needs them again, or the part fixes itself): sizes for unsized
  DIVERGED rows, `specs/world/hirelings*` expanded, truncated `...(+N)` check lists cut,
  coverage-a3a5 reduced to its `yes` rows and lowercased.
- `docs/handoff/fidelity-gaps.md`: first divergences D1–D11 with owners, PARTIAL results,
  missing checks per family, tools to build, tranches T0–T6.

## Open
- Reruns R1 (coverage of Acts III–V: the a3a5 run stopped early) and R2 (the checks'
  rng channel) in fidelity-gaps.md §6.
- Four first divergences have no owner (D2, D3, D10, D11 in fidelity-gaps.md).
- Part-quality caveats are in fidelity-gaps.md §6: `needs_pc1` comes from the settling
  kind, the game-join NOT-IMPLEMENTED rows come from a heuristic, and the monsters part
  matched checks by monster name.

## Repro
    sh tools/coord/ledger-pull.sh            # pull every part, apply the integrator fixups, merge
    python3 tools/coord/ledger.py --fix     # reconcile parts with checks-status, merge
    python3 tools/coord/ledger.py --check   # 0 errors, outputs current
    python3 tools/coord/ledger.py --selftest
Verdicts come from `docs/handoff/checks-status.md` if present, otherwise from
`origin/claude/q-fix-check-triage` (fetch it first).
