#!/bin/sh
# Full local gate, in the CI order (.github/workflows/ci.yml runs the same
# steps as parallel jobs). One command for sessions and coordinators:
#   sh tools/gate.sh            everything
#   sh tools/gate.sh --no-client  skip the Bevy crate (clippy + tests)
# Prints a summary and exits 1 if any step failed (all steps still run).
cd "$(dirname "$0")/.." || exit 2

NO_CLIENT=0
[ "$1" = "--no-client" ] && NO_CLIENT=1

PY=python3
command -v python3 >/dev/null 2>&1 || PY=py

# nextest keeps the same selection as `cargo test` (ignored tests skipped);
# doc-tests always run through `cargo test --doc`.
if cargo nextest --version >/dev/null 2>&1; then
  TEST="cargo nextest run --no-tests=pass"
else
  TEST="cargo test"
fi

SUMMARY=""
FAILED=0
step() {
  name=$1; shift
  echo "=== $name"
  start=$(date +%s)
  if "$@"; then r=PASS; else r=FAIL; FAILED=1; fi
  SUMMARY="$SUMMARY$(printf '%-34s %s  %ss' "$name" "$r" "$(( $(date +%s) - start ))")
"
}
sh_step() { name=$1; shift; step "$name" sh -c "$*"; }

# ---- tools job
step "spec_index --check"      $PY tools/spec_index.py --check
step "methods check"           $PY tools/methods.py check
sh_step "coverage --check/--selftest" "$PY tools/coverage.py --check && $PY tools/coverage.py --selftest"
sh_step "trace checkers" "
  $PY tools/trace-recorder/check_tick.py --selftest &&
  $PY tools/trace-recorder/convert_tick.py --selftest &&
  $PY tools/trace-recorder/convert_tick.py --check traces/sim/tick/*.json &&
  $PY tools/trace-recorder/check_rooms.py --selftest &&
  $PY tools/trace-recorder/check_rooms.py traces/sim/tick/*.json"
step "pre-commit hook selftest" sh tools/hooks/selftest.sh
step "fmt"                     cargo fmt --all -- --check
step "depcheck (+determinism)" cargo run -q -p depcheck

# ---- clippy jobs
if [ $NO_CLIENT = 1 ]; then
  step "clippy (no client)" cargo clippy --workspace --exclude d2-client --all-targets -- -D warnings
else
  step "clippy workspace" cargo clippy --workspace --all-targets -- -D warnings
fi

# ---- test jobs
step "test d2-sim + conformance" $TEST -p d2-sim -p conformance
step "test rest (no client)" $TEST --workspace --exclude d2-sim --exclude conformance --exclude d2-client
[ $NO_CLIENT = 1 ] || step "test d2-client" $TEST -p d2-client
step "doc-tests (no client)" cargo test --doc --workspace --exclude d2-client
[ $NO_CLIENT = 1 ] || step "doc-tests d2-client" cargo test --doc -p d2-client

echo
echo "=== gate summary"
printf '%s' "$SUMMARY"
if [ $FAILED = 0 ]; then echo "GATE: PASS"; else echo "GATE: FAIL"; exit 1; fi
