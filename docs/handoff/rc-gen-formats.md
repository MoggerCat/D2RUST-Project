# rc-gen-formats hand-back

Area system.formats (55 NO-CHECK rows). EQUAL before: 0 of the 55 (the d2s 2-7 rows were already EQUAL, untouched) -> after: 0 of the 55 (no new EQUAL).

## Changed
- tools/check-gen/check_gen.py: family `fmt` (3 checks), FMT_ROWS maps check -> row prefixes; resolve_area case.
- tools/check-gen/ledger-areas.tsv: the 74 system.formats areas added.
- traces/checks/gen: gen-fmt-draws-town, gen-fmt-load-ama, gen-fmt-anim-walk (+INDEX.tsv).
- docs/handoff/ledger/rc-gen-formats.tsv: 55 rows.

## Verdicts (suite.py, orig-cache filled)
- gen-fmt-draws-town: draws DIVERGED, tick 72 draw row 173 (CelDraw) column op: 1.14d CelDraw vs d2rs unit. Rows: dcc (10), cof (3), dt1 (5), palette (2), dc6 (4), animdata 1-3, d2s-appearance 1,2,3,6 -> DIVERGED (cause is draw-list content, not the format readers; needs a draws session).
- gen-fmt-load-ama: packets MATCH 20/20, state PARTIAL. Rows d2s 1, 9, d2s-load 1,2,7,8 -> NO-CHECK (PARTIAL).
- gen-fmt-anim-walk: packets MATCH 80/80, state PARTIAL. Rows animdata 4-6 -> NO-CHECK (PARTIAL).
Why state is PARTIAL was not investigated.

## Open (NO-CHECK, reason in note)
- wav 1-5 (5): no audio channel.
- font-tbl (2): no channel compares text.
- d2s-load 3-6, d2s.10 (5): need Necromancer/hotkey/runeword/corrupt-save variants.
- d2s-appearance 4,5 (2): need equipped items.
- d2s-legacy (12): NOT-IMPLEMENTED, unchanged (not in the 55).
Not done: game code fixes, Ghidra reading, the pre-push cargo fmt/clippy/nextest (only Python/check files changed).
Pre-existing: `check_gen.py --selftest` fails (gen-netc2s-02.check missing); the netc2s family needs docs/handoff/packet-census.tsv, absent here, so INDEX.tsv fmt lines were added by hand.
