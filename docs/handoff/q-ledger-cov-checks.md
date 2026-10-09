# q-ledger-cov-checks — done

Coverage of the 1.14d check suite (d2rs side) and a 20-min soak, from a `coverage-map` release build.

- Done: all 88 checks run `--d2rs-only` (state channel; 87 produced counters, `draws-town-arrival-ama` and `rng-town-arrival-ama` none: draws/rng-only, rng needs a d2-client-rng binary not built). Soak: 26 runs, 78k frames.
- Rows: `ledger/coverage-checks.tsv` (308 rows: 298 exercised entries state UNKNOWN because coverage is not equality, 10 `never-exercised` per category NO-CHECK). `coverage-checks.md/json`: report with runs `checks` and `soak`; raw counters in `ledger/cov/checks/<check>/` and `ledger/cov/soak/`.
- Open: rng channel not run; objects are exercised only by soak (checks: 0); only 21 skills, 1 NPC (akara) in checks.
- Repro: build `cargo build --release -p d2-client -p d2s-tool --features d2-client/coverage-map`; copy the binaries to a dir as D2RS_BIN_DIR; per check `D2_COVERAGE_DIR=<dir> python3 tools/scenario-diff/scenario_diff.py <check> --d2rs-only --channels state`; soak `D2_COVERAGE_DIR=<dir> python3 tools/soak/soak.py campaign --minutes 20 --out <out> --no-reduce`; `tools/coverage-map/report.py --excel ... --run checks=<flat> --run soak=<dir>`.
