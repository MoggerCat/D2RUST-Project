# Handoff: q-tool-coverage-perf (paused 2026-10-09)

Cloud tools session, branch `claude/q-tool-coverage-perf` from `claude/specs-staging-7`.
Data: private repo install (assembled to `$HOME/game`) and its `extracted/` excel.

## Done

- **tools/coverage-map** (`specs/tools/coverage-map.md`): crate `coverage-map`
  (per-thread counters, `cov-<pid>-<n>.tsv` in `$D2_COVERAGE_DIR`, `coverage-map 1`);
  d2-sim feature `coverage-map` (off by default, `cov!` in `debug::coverage`) at ten
  count sites; `d2-client` feature `coverage-map` and `debug::coverage::flush()` in
  main; `report.py` (merge, compare with tables, rank gaps; `--selftest`).
  Proven: state records byte-identical with and without the feature
  (`combat-arrow-kill`, `milestone-baal-throne`); `cargo test -p coverage-map` writes
  and reads a dump.
- **docs/handoff/coverage-map.md**: playthrough + the d2rs side of the 27 checks +
  the game-file tests of d2-server, conformance, scenario-run, seed-finder and d2s-tool.
  Headline gaps: no object operated, 1 of 47 NPCs talked to, 15 of 357 skills.
- **tools/perf** (`specs/tools/perf.md`): `d2-server::perf` (Host::frame drain /
  tick / flush times), `d2-client::app::perf` (`D2_PERF_OUT`: Bevy pump / update /
  post / render-world marks, headless bridge frames, peak RSS); `perf.py`
  (10 scenes: each act's town + one outdoor level, headless and Xvfb; `--profile`
  runs `perf record`; `--selftest`). The windowed path works under Xvfb + lavapipe
  (smoke: 120 frames, 115 ticks, 636 MiB peak RSS).
- **tools/coverage-map/rec_priority.py** (coordinator item 18): ranks the open
  rows of `provisional-index.tsv` by LLVM line hits (`--selftest`).

## Not done (next steps, in order)

1. **perf.md**: not measured. The CPU was busy with coverage builds the whole
   session, so no clean numbers exist. Run on an idle machine:
   `CARGO_TARGET_DIR=target/perf cargo build --release -p d2-client -p d2s-tool`,
   then `D2_GAME_DIR=... python3 tools/perf/perf.py --profile a5-foothills
   --md docs/handoff/perf.md` (perf is `/usr/lib/linux-tools-*/perf`; install
   `linux-tools-generic`, `xvfb`, `mesa-vulkan-drivers` per `tools/cloud-setup.sh`).
   File a q-fix row per scene in its "Budget misses".
2. **rec-priority.md**: not produced. Build an instrumented client
   (`RUSTFLAGS="-C instrument-coverage" CARGO_TARGET_DIR=target/llvmcov cargo build
   --release -p d2-client`), run the playthrough (copy it to `target/release/`, as
   playthrough.py reads that path) and the checks
   (`D2RS_BIN_DIR=... scenario_diff.py X --d2rs-only --channels state`) with
   `LLVM_PROFILE_FILE=DIR/%p.profraw`, then per run `llvm-profdata merge` +
   `llvm-cov export -format=lcov -instr-profile ... BIN > run.lcov`, then
   `python3 tools/coverage-map/rec_priority.py --run playthrough=... --run checks=...
   --md docs/handoff/rec-priority.md`. Disk: the container ran down to 1.6 GB free;
   delete `target/cov` (12 GB) first.
3. Coverage map additions: d2-client game-file tests (a staging merge landed
   mid-build) and d2-sim game-file tests (rustc SIGSEGV on the release lib-test
   build: retry, or build with `--debug`).

## Findings for other sessions

- `depcheck` is red on staging: `crates/d2-sim/src/debug/rng_trace.rs:88`
  `thread_local!` (global-state rule), not from this branch.
- Real-data red in `d2-server` `game_town_run::town_run_moves_at_the_run_velocity`
  (run reaches (4899, 5634), expected (4901, 5634)). Seen with the coverage
  feature, whose records match the plain build byte for byte, so it is very
  likely red without it too. Not confirmed without the feature.
