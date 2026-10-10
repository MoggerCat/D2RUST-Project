# rc-run-4 hand-back (runner only, no code changes)

Suite: gen-mon/umod/su/boss, 468 checks, 3 workers, 1.14d cache filled.
Whole suite: MATCH 316, DIVERGED 74, PARTIAL 413 (channel verdicts).

My half (first 234 names sorted; rc-run-4b has the rest):
- Checks EQUAL (REC-2055/2056 rules): 208; DIVERGED: 26.
- Ledger part: docs/handoff/ledger/rc-run-4.tsv (238 rows).
- Causes: docs/handoff/rc-run-4-causes.tsv (8 distinct first differences,
  largest: player field m, 13 checks). Crate/size columns are guesses
  from the field name, not investigated (runner task).
- Not investigated; no code changed. No files held by other owners touched.
