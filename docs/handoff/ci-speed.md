# Handoff: CI speed and the local gate — `claude/ci-speed`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here); the coordinator folds it.

Cloud tooling session, 2026-10-06, base `claude/tender-meitner-mphas3`. Repo only.

## Job layout (`.github/workflows/ci.yml`)

Five independent jobs run in parallel; each has its own `Swatinem/rust-cache` `shared-key`. `concurrency` cancels a superseded run of the same ref. `CARGO_INCREMENTAL=0`.

| Job | Runs | Bevy libs |
|---|---|---|
| `tools` | `spec_index --check`, `methods check`, `coverage --check` + `--selftest`, all trace-checker selftests + `convert_tick --check` + `check_rooms`, `tools/hooks/selftest.sh` (pre-commit patterns in a scratch repo + every tracked file), `cargo fmt --check`, `cargo run -p depcheck` (dependency rules **and** the d2-sim determinism lint) | no |
| `clippy-core` | `clippy --workspace --exclude d2-client --all-targets -D warnings` (compiles tests, benches, examples) | no |
| `test-sim` | nextest `-p d2-sim -p conformance` (includes `tick_replay`), then `cargo test --doc` | no |
| `test-rest` | nextest of everything but d2-sim / conformance / d2-client (includes `d2-proto` `generated_file_is_current`), then `cargo test --doc` | no |
| `client` | apt libs, `clippy -p d2-client --all-targets`, nextest `-p d2-client`, doc-tests | yes |

Union of the clippy and test jobs = the old `--workspace` run; nothing dropped. Only `client` pays the apt install, and only it compiles Bevy.

- **nextest**: same selection as `cargo test` (`#[ignore]` stays skipped); `--no-tests=pass` for crates with no tests; doc-tests, which nextest does not run, run via `cargo test --doc`.
- **Proptest**: robust tests read `PROPTEST_CASES` and *override* their in-source defaults, so CI leaves it unset (defaults are <1 s). Only `PROPTEST_MAX_SHRINK_TIME=10000` is set, bounding a failing property.
- **Blizzard-file check**: the pre-commit hook only sees staged files, so `tools/hooks/selftest.sh` tests its patterns on synthetic files and scans `git ls-files`.
- Command-map items needing `game/`, a GPU or Windows stay out of CI (HANDOFF §4 "Needs" column).

## `tools/gate.sh`

`sh tools/gate.sh` runs the same steps in CI order and prints a PASS/FAIL + seconds summary; every step runs, exit 1 if any fails. `--no-client` skips the Bevy crate (clippy excludes d2-client). Uses nextest when installed, else `cargo test`.

## Timings

Workflow logs are not reachable from this session, so these are **estimates**, not measurements.

- Before (one serial job): ~40 min cold, ~3 min cached (HANDOFF §6).
- After: wall time = the slowest job. Cold: `client` (Bevy compile + test build), still ≈ the old cold figure minus the ~3 min of non-Bevy work; cold runs are rare. Cached: the longest job's incremental build, ~1–2 min, since the jobs overlap instead of adding up (estimate). Non-Bevy jobs finish in minutes even cold.
- Measured locally (cold target dir, `cargo test` fallback, this container): see the gate summary. Bevy compile dominates: clippy 266 s, d2-client test 693 s (mostly compiling).

## Gate summary (`sh tools/gate.sh`, after `sh tools/cloud-setup.sh`)

```
spec_index --check                 PASS  0s
methods check                      PASS  0s
coverage --check/--selftest        PASS  1s
trace checkers                     PASS  3s
pre-commit hook selftest           PASS  1s
fmt                                PASS  2s
depcheck (+determinism)            PASS  8s
clippy workspace                   PASS  266s
test d2-sim + conformance          PASS  59s
test rest (no client)              PASS  27s
test d2-client                     PASS  693s
doc-tests (no client)              PASS  2s
doc-tests d2-client                PASS  0s
GATE: PASS
```

Not exercised here: the workflow itself (YAML parses; first run on GitHub is the real check) and the nextest paths (not installed locally).
