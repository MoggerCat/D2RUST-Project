# rc-link-checks hand-back

## Checks before -> after
- EQUAL ledger rows: 1488 -> 1960 (merged ledger; +545 EQUAL, 14 DIVERGED among the linked rows).
- NO-CHECK rows with `checks -`: 1450; 559 now linked (note rule 382, INDEX `ledger_area` rule 177).
- Ran 554 distinct gen checks (1434 runs, 75814 ticks, 98.4% frames equal) with suite.py, orig-cache,
  --fill-cache, 3 workers. Recordings not committed.

## What changed
- tools/coord/link_checks.py (+ --selftest): links NO-CHECK / `checks -` rows to existing gen checks.
- docs/handoff/ledger/rc-link-checks.tsv: 559 rows with verdicts. Rule (PROVISIONAL REC-2055/2056):
  EQUAL = every channel of every linked check MATCH, or PARTIAL with 0 differences (the d2rs
  client gap RUN_GAPS), nothing ignored. DIVERGED = any channel with a first difference
  (first frame and field in the row note).

## Open
- 14 DIVERGED rows, all gen-ai-* state checks (baalcrab, bloodraven, clawviper, desertturret,
  gargoyletrap, genericspawner, mephisto, navi, nihlathak, npcoutoftown, sandmaggotqueen,
  siegebeast, suicideminion, summoner): each needs its own cause (size S each); not started.
- ~890 NO-CHECK rows have no matching gen check: the area rule (object.<n>-*, missile.<name>)
  found nothing the note/INDEX rules missed; they need new checks.
- Skill rows (skill.<cls>.*) were not linked by area (no id in the area name).
