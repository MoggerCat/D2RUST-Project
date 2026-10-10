# rc-rerun-r10 hand-back

Re-measure of every check on integ-r10 + `claude/local-pc1-late` (PC1's `q` recorder fix).

## Checks (EQUAL = every channel MATCH)
- Main suite (354 checks, 579 channel runs): before (stale 1.14d recordings, q read from header): MATCH 142 / DIVERGED 358 / PARTIAL 79 runs.
  After (re-recorded): MATCH 141 / DIVERGED 265 / PARTIAL 171 / ERROR 2 runs. The `q` divergence (about 225 checks) is gone;
  first divergences moved later (frame 4+ instead of frame 2).
- gen suite (1600 checks): 1471 measured (the 4h cap hit; ~129 not run, the tail of the list): EQUAL 4, DIVERGED 939, PARTIAL 528.
- Ledger part `ledger/rc-rerun-r10.tsv`: 1161 rows (`chk.<name>`), 28 EQUAL, rest DIVERGED (PARTIAL rows omitted).
  `ledger.py --fix` rewrote 23 rows from checks-status.md (e.g. rng-town-arrival-ama stays DIVERGED@2).

## What changed
- Merged `origin/claude/local-pc1-late`. Conflicts: `record_state.py` taken whole from PC1 (a half-merge left a duplicate buffer
  deref, so 1.14d `q` read "absent"); `specs/monsters/ai.md`, `sim/units.md`, `tools/poke.md`, `world/quests-act3.md` taken from PC1
  (integ-side edits in those four files may be lost: owners please re-check); `state-snapshot.md` kept integ's text with PC1's `q` row;
  `build-queue.tsv` and `pc1-data.md` unioned.
- `docs/handoff/rc-queue.tsv`: 29 first-divergence causes sorted by count (main + gen).
- Gotcha: `suite.py` reuses `traces/raw/suite/<check>/orig.*.jsonl` work-dir files even when the orig-cache key changed;
  delete them (or `--fresh`) after a recorder change. Not fixed here.

## Open (sizes in rc-queue.tsv)
- S->C 0xAA AddUnit 12 vs 39 bytes in all gen state checks (M); missile present in 1.14d only (L); game seed / RNG draws at level
  entry (L); player y off by 1 on first walk (M); monster mode/frame/target (M).
- 2 ERROR runs (sys area: record_state.py wrote no output) need a look.
- Not done: Ghidra-based naming of causes (re/ not fetched), the playthrough, the unrun ~129 gen checks.
- orig-cache recordings were left uncommitted, as the brief says.
