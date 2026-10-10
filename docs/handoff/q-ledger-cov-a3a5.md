# q-ledger-cov-a3a5

Done: coverage-map release build; partial run; report (coverage-a3a5.{json,md,tsv}); raw counters in ledger/cov/a3a5/ (40 files, 96,077 events).
Open: the matrix (6 .play files x 7 classes x normal,hell, --jobs 4) was far too slow (~45 min, only the first Act III cells finished) and was stopped at 19:15 UTC for the deadline. "exercised: no" rows are NOT evidence; rerun with more time or fewer cells (e.g. --class sor,pal --difficulty normal). Row state is UNKNOWN throughout (coverage measures execution; check verdicts belong to the owner parts). Item gaps list only reachable base items, unreachable table rows are not rows.
Repro: cargo build --release -p d2-client -p d2s-tool --features d2-client/coverage-map; D2_COVERAGE_DIR=docs/handoff/ledger/cov/a3a5 python3 tools/playthrough/playthrough.py <6 play files> --class all --difficulty normal,hell --jobs 4; python3 tools/coverage-map/report.py --excel <excel view> --run a3a5=DIR --json --md.
