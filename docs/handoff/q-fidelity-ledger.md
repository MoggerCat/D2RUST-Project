# q-fidelity-ledger (integrator) — done / open / repro

Session q-fidelity-ledger, 2026-10-09. The coordinator's plan change at 18:17 UTC made
this session the integrator of 8 part sessions instead of the sole author.

## Done
- `tools/coord/ledger.py` (`--check`, `--fix`, `--selftest`): validates the part files
  `docs/handoff/ledger/<part>.tsv` (`#ledger 1` format), reconciles them with
  `checks-status.md` (`--fix`), merges them into `docs/handoff/fidelity-ledger.tsv` and `.md`,
  and lists the spec files, checks and message ids that no row names.
- Merged parts: systems, world, skills, monsters, items, plus `integrator.tsv` (30 rows,
  hand-written by this session for the specs and checks no part named). The result has
  2,346 rows and EQUAL 0; nothing is left unnamed.
- `docs/handoff/fidelity-gaps.md`: first divergences D1–D11 with owners, PARTIAL results,
  missing checks per family, tools to build, tranches T0–T6.

## Open
- Coverage parts (q-ledger-cov-a1a2, -a3a5, -checks) have not landed yet. They fill the
  `exercised` column (second pass: pull, `--fix`, merge).
- Four first divergences have no owner (D2, D3, D10, D11 in fidelity-gaps.md).
- Part-quality caveats are in fidelity-gaps.md §6: `needs_pc1` comes from the settling
  kind, the game-join NOT-IMPLEMENTED rows come from a heuristic, and the monsters part
  matched checks by monster name.

## Repro
    # pull the parts (each branch's docs/handoff/ledger/*.tsv) into docs/handoff/ledger/, then
    python3 tools/coord/ledger.py --fix     # reconcile parts with checks-status, merge
    python3 tools/coord/ledger.py --check   # 0 errors, outputs current
    python3 tools/coord/ledger.py --selftest
Verdicts come from `docs/handoff/checks-status.md` if present, otherwise from
`origin/claude/q-fix-check-triage` (fetch it first).
