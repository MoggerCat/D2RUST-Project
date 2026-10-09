# q-ledger-items — items and economy ledger part

Done: `docs/handoff/ledger/items.tsv`, 135 rows (128 NO-CHECK, 7 DIVERGED, 0 EQUAL, 0 UNKNOWN).
Groups: item generation/quality/affix/property rules per spec section, base items by type,
uniques/sets/runewords/cube recipes (grouped, counts from the private tables), drops,
inventory + 25 C->S item messages, vendors per trader NPC (17), tooltip, identify, bitstream.

Method: spec headings, `specs/world/vendors.tsv`, `cube-ops.tsv`, `item-actions.tsv`,
counts from `Patch_D2.mpq` excel tables, verdicts from `origin/claude/q-fix-check-triage:docs/handoff/checks-status.md`.

Open / caveats:
- Provisional counts are approximate (grepped from provisional-index.tsv by path), `exercised` is `?` for the coverage sessions.
- Only 4 checks touch items (items-ground-many, items-ground-pokes, items-load-mixed, items-vendor-akara-buy); none compares dropped/generated item contents, so almost every row is NO-CHECK.
- Not run: fmt/clippy/tests (only a TSV was added).

Repro: `python3 tools/coverage.py --check` is unaffected; rows were produced by a throwaway script (not committed).
