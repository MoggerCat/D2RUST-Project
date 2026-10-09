# q-ledger-cov-a1a2

Done: coverage-map build (release, `d2-client/coverage-map`, own target dir), playthrough act1+act2+classes, class all, normal, jobs 4 (act1 12-13/17, act2 14/15 on 4 classes, classes 1-4/4-6), report, gap rows.
Output: `docs/handoff/ledger/coverage-a1a2.{tsv,json,md}`, raw counters `docs/handoff/ledger/cov/a1a2/`. TSV rows are gaps only (exercised=no, NO-CHECK); one row per table entry, not grouped.
Open: act2.play ran only for ama/sor/pal/nec (matrix); object/npc-topic counters read 0 (not instrumented in these runs or not reached); sizes all `S` placeholder.
Repro: build with `cargo build --release -p d2-client -p d2s-tool --features d2-client/coverage-map`, run the playthrough with `D2_COVERAGE_DIR=<dir>`, then `tools/coverage-map/report.py --excel <Patch_D2,d2exp,d2data excel dirs> --run a1a2=<dir>`.
