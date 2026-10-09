#!/bin/bash
# full staging gate; prints a short verdict
cd /home/user/D2RUST-Project
export CARGO_PROFILE_DEV_STRIP=symbols CARGO_PROFILE_TEST_STRIP=symbols CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
fail=0
cargo fmt --all -- --check >/tmp/g_fmt 2>&1 || { echo "FMT FAIL"; head -20 /tmp/g_fmt; fail=1; }
cargo clippy -p d2-sim -p d2-server -p d2-client -p test-fixtures --all-targets -- -D warnings >/tmp/g_clippy 2>&1 || { echo "CLIPPY FAIL"; grep -E "^(error|warning)" -A6 /tmp/g_clippy | head -60; fail=1; }
cargo nextest run -p d2-sim -p d2-server -p d2-client -p test-fixtures --no-fail-fast >/tmp/g_test 2>&1 || { echo "TEST FAIL"; grep -E "^\s+(FAIL|SIGSEGV|TIMEOUT)" /tmp/g_test | head -30; fail=1; }
grep "Summary" /tmp/g_test
python3 tools/coverage.py --check >/tmp/g_cov 2>&1 || { echo "COVERAGE FAIL"; tail -15 /tmp/g_cov; fail=1; }
python3 tools/spec_index.py --check >/tmp/g_idx 2>&1 || { echo "SPEC_INDEX FAIL"; tail -10 /tmp/g_idx; fail=1; }
git grep -nE "^(<<<<<<< |>>>>>>> )" -- docs specs crates tools facts traces >/tmp/g_mark 2>&1 && { echo "CONFLICT MARKERS"; head -5 /tmp/g_mark; fail=1; }  # conflict markers
rm -rf target/debug/incremental target/nextest
avail=$(df --output=avail -BG . | tail -1 | tr -dc 0-9); [ "$avail" -lt 5 ] && find target/debug/deps -mmin +60 -delete
echo "GATE fail=$fail; disk ${avail}G"
