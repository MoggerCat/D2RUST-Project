# q-ledger-cov-a3a5-r1

## Done
- **R1**: coverage-map release build, 6 cells (act3 / act4 / act5 .play x sor, pal x normal, one
  `timeout 2400` job each, 4 parallel), all finished in minutes (none near the 40 min cap):
  act3 15/15 (sor, pal), act5 13/13 (sor, pal), act4 10/12 (sor, pal; blocker `diablo-present`:
  `pos 1/146 @x+3 @y` unresolved for sor, failed for pal, Diablo never present).
  133,648 events; raw counters in `docs/handoff/ledger/cov/a3a5-r1/` (the first run's 40 files stay in `cov/a3a5/`).
- Report: `docs/handoff/ledger/coverage-a3a5.{json,md,tsv}` (replaces the partial first run). New
  `tools/coverage-map/ledger_tsv.py` turns the report json into the part rows (2,357 rows:
  290 yes, 1,844 no, 223 `?`). `?` = a level of act I-II or a skill of ama/nec/bar/dru/ass, which
  no finished cell could reach (those cells were not run). `ledger.py --fix` then `--check`: 0 errors.
- Quest slots, objects: this run exercised 10 quest slots and 5 objects (Seals); NPC topics 0.

## Open
- **R2 (rng channel in the coverage-checks run): not run, open.** It needs the d2rs side of the 87
  checks built with `rng-trace` and `coverage-map` together (`scenario_diff.py --d2rs-only
  --channels rng`, `D2RS_BIN_DIR` with a `d2-client-rng` build) plus the harness of the coverage-checks
  run, which is not documented in the repo (no hand-back for q-ledger-cov-checks). Not judged cheap.
- Not covered by R1 (by the task's choice): classes ama/nec/bar/dru/ass, nightmare/hell,
  act4-blockers/act4-forge/milestones-a3-baal. Act4 stops before Diablo (playthrough blocker, not a coverage gap).
- `no` rows are valid only for Acts III-V content and the sor/pal skill sets; act I-II `no` rows
  from this file say nothing (the ledger merge takes the `yes` rows only, per fidelity-gaps.md §6).

## Repro
```
cargo build --release -p d2-client -p d2s-tool --features d2-client/coverage-map
export D2_GAME_DIR=$HOME/game D2_COVERAGE_DIR=docs/handoff/ledger/cov/a3a5-r1
for a in act3 act4 act5; do for c in sor pal; do   # xargs -P4 in the real run
  timeout 2400 python3 tools/playthrough/playthrough.py traces/playthrough/$a.play --class $c --difficulty normal; done; done
cargo run -q --release -p data-tool -- excel-dir $HOME/game/extracted/patch_d2/data/global/excel $HOME/game
E=$HOME/game/extracted/patch_d2/data/global/excel
python3 tools/coverage-map/report.py --excel $E --run a3a5-r1=docs/handoff/ledger/cov/a3a5-r1 --json docs/handoff/ledger/coverage-a3a5.json --md docs/handoff/ledger/coverage-a3a5.md
python3 tools/coverage-map/ledger_tsv.py docs/handoff/ledger/coverage-a3a5.json docs/handoff/ledger/coverage-a3a5.tsv --scope "the Acts III-V run (act3/act4/act5.play, sor+pal, normal, 6 cells finished; act4 stops at diablo-present)" --unrun-classes ama,nec,bar,dru,ass --unrun-acts 1,2
python3 tools/coord/ledger.py --fix && python3 tools/coord/ledger.py --check
```
