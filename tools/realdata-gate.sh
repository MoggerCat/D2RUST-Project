#!/bin/sh
# Real-data gate (M23): run every #[ignore] game-file test and the window-free
# data-tool / mpq-tool checks (docs/LOCAL-RUN.md Batches 1-2) on the real
# 1.14d install. Notes: docs/handoff/q-realdata-gate.md.
#
#   sh tools/realdata-gate.sh [--no-client] [--debug] [--no-fetch] [--list] [--help]
#
# Data: $D2_GAME_DIR if set; else $HOME/game, assembled from the private repo
# (MoggerCat/D2RUST-private-repo, see its README). Exit 0 all pass, 1 any
# failure, 2 usage error or no/incomplete data.
cd "$(dirname "$0")/.." || exit 2
ROOT=$(pwd)

NO_CLIENT=0; RELEASE=--release; FETCH=1; LIST=0
for a in "$@"; do
  case $a in
    --no-client) NO_CLIENT=1 ;;
    --debug)     RELEASE= ;;
    --no-fetch)  FETCH=0 ;;
    --list)      LIST=1 ;;
    -h|--help)
      sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
      echo "  --no-client  skip d2-client (builds Bevy)    --debug  debug builds (sweeps are slow)"
      echo "  --no-fetch   never touch the network         --list   print the plan, run nothing"
      exit 0 ;;
    *) echo "realdata-gate: unknown argument '$a' (try --help)" >&2; exit 2 ;;
  esac
done

PY=python3; command -v python3 >/dev/null 2>&1 || PY=py
PRIV_URL=${D2_PRIVATE_URL:-https://github.com/MoggerCat/D2RUST-private-repo}
PRIV=${D2_PRIVATE_DIR:-$HOME/d2rust-private-repo}
GAME_DEFAULT=$HOME/game

CRATES="d2-formats d2-data d2-sim d2-server conformance d2s-tool scenario-run seed-finder"
[ $NO_CLIENT = 1 ] || CRATES="$CRATES d2-client"
SKIP_NAMES=$($PY tools/realdata_inventory.py --skip-names)

if [ $LIST = 1 ]; then
  echo "crates (nextest --run-ignored only, else cargo test -- --ignored): $CRATES"
  echo "tool checks: data-tool tables | links ; mpq-tool check | formats"
  echo "skipped (needs-window): GPU tests and dump-recording tests:"; echo "$SKIP_NAMES" | sed 's/^/  /'
  echo "skipped (needs-window): d2-client play / verify / examples (LOCAL-RUN Batches 3-6)"
  exit 0
fi

# ---- 1. find or build the install
no_data() { echo "realdata-gate: no usable 1.14d install: $1" >&2; echo "  Set D2_GAME_DIR to an install, or wait for 'install: complete' in $PRIV_URL" >&2; exit 2; }
have_install() { [ -f "$1/d2data.mpq" ] && [ -f "$1/d2exp.mpq" ] && [ -f "$1/Game.exe" ]; }

if [ -n "$D2_GAME_DIR" ]; then
  [ -d "$D2_GAME_DIR" ] || no_data "D2_GAME_DIR=$D2_GAME_DIR is not a directory"
  have_install "$D2_GAME_DIR" || no_data "D2_GAME_DIR=$D2_GAME_DIR lacks d2data.mpq / d2exp.mpq / Game.exe"
  GAME=$D2_GAME_DIR
else
  GAME=$GAME_DEFAULT
  if ! have_install "$GAME"; then
    [ $FETCH = 1 ] || no_data "D2_GAME_DIR unset, $GAME empty, and --no-fetch given"
    command -v git >/dev/null 2>&1 || no_data "git missing"
    if [ ! -d "$PRIV/.git" ]; then
      git clone --depth 1 --filter=blob:none --sparse "$PRIV_URL" "$PRIV" >&2 || no_data "cannot clone $PRIV_URL"
    else
      git -C "$PRIV" fetch --depth 1 origin >&2 && git -C "$PRIV" reset -q --hard FETCH_HEAD >&2
    fi
    git -C "$PRIV" sparse-checkout set tools >&2
    # completeness first (cheap): every piece the manifest names must be in the tree
    MISSING=$(git -C "$PRIV" show HEAD:install/manifest.json 2>/dev/null | $PY -c '
import json, subprocess, sys
try:
    m = json.load(sys.stdin)
except Exception:
    print("install/manifest.json"); sys.exit()
have = set(subprocess.check_output(["git", "-C", sys.argv[1], "ls-tree", "-r", "--name-only", "HEAD", "install"], text=True).split())
for f in m["files"]:
    for piece in f["parts"] or [f["path"]]:
        if "install/" + piece not in have:
            print(f["path"])
            break
' "$PRIV")
    if [ -n "$MISSING" ]; then
      echo "realdata-gate: private repo install is incomplete; missing: $(echo $MISSING | tr '\n' ' ')" >&2
      no_data "install not fully uploaded yet"
    fi
    # install/ only: the repo's extracted/ (3 GB, per-archive folders, Patch_D2.mpq
    # partly named) is not what the tests read; step 1b builds their view.
    git -C "$PRIV" sparse-checkout set tools install >&2
    $PY "$PRIV/tools/assemble.py" "$GAME" >&2 || no_data "assemble.py reported mismatches"
    have_install "$GAME" || no_data "assembled install lacks d2data.mpq / d2exp.mpq / Game.exe"
  fi
  export D2_GAME_DIR=$GAME
fi

# ---- 1b. the excel view the table tests read ($D2_GAME_DIR/extracted/patch_d2/data/global/excel/):
# the live excel set, each file from the highest-priority archive, lowercase names
EXCEL=$GAME/extracted/patch_d2/data/global/excel
if [ ! -f "$EXCEL/skills.bin" ]; then
  cargo run -q $RELEASE -p data-tool -- excel-dir "$EXCEL" "$GAME" || echo "realdata-gate: warning: excel-dir failed; tests reading extracted/ will fail" >&2
fi
echo "realdata-gate: install at $GAME"

# ---- 2. run
SUMMARY=""; FAILED=0; NPASS=0; NFAIL=0
add() { SUMMARY="$SUMMARY$(printf '%-30s %-5s %s' "$1" "$2" "$3")
"; }
LOGDIR=$(mktemp -d)

tool_step() { # name, command...
  name=$1; shift
  echo "=== $name"
  if "$@" >"$LOGDIR/$name.log" 2>&1; then add "$name" PASS ""; else FAILED=1; add "$name" FAIL "see $LOGDIR/$name.log"; tail -n 15 "$LOGDIR/$name.log"; fi
}
tool_step data-tool-tables cargo run -q $RELEASE -p data-tool -- tables
tool_step data-tool-links  cargo run -q $RELEASE -p data-tool -- links
tool_step mpq-tool-check   cargo run -q $RELEASE -p mpq-tool -- check
tool_step mpq-tool-formats cargo run -q $RELEASE -p mpq-tool -- formats

if cargo nextest --version >/dev/null 2>&1; then
  FILTER=""
  for n in $SKIP_NAMES; do FILTER="$FILTER${FILTER:+ | }test(=$n)"; done
  for c in $CRATES; do
    echo "=== $c (nextest)"
    log=$LOGDIR/$c.log
    cargo nextest run -p "$c" $RELEASE --run-ignored only --no-fail-fast --no-tests=pass ${FILTER:+-E "not ($FILTER)"} >"$log" 2>&1
    rc=$?
    p=$(sed -n 's/.*Summary.* \([0-9]*\) passed.*/\1/p' "$log" | tail -1); f=$(sed -n 's/.* \([0-9]*\) failed.*/\1/p' "$log" | tail -1)
    p=${p:-0}; f=${f:-0}
    [ $rc = 0 ] || [ "$f" != 0 ] || f=1   # build error etc. counts as a failure
    NPASS=$((NPASS + p)); NFAIL=$((NFAIL + f))
    if [ $rc = 0 ]; then add "$c" PASS "$p passed"; else FAILED=1; add "$c" FAIL "$p passed, $f failed; $log"; grep -E "^\s+(FAIL|SIGABRT|TIMEOUT)" "$log" | sort -u | head -n 20; fi
  done
else
  SKIPARGS=""; for n in $SKIP_NAMES; do SKIPARGS="$SKIPARGS --skip $n"; done
  for c in $CRATES; do
    echo "=== $c (cargo test)"
    log=$LOGDIR/$c.log
    # shellcheck disable=SC2086
    cargo test -p "$c" $RELEASE --no-fail-fast -- --ignored $SKIPARGS >"$log" 2>&1
    rc=$?
    p=$(sed -n 's/^test result: .* \([0-9]*\) passed; \([0-9]*\) failed.*/\1/p' "$log" | awk '{s+=$1} END{print s+0}')
    f=$(sed -n 's/^test result: .* \([0-9]*\) passed; \([0-9]*\) failed.*/\2/p' "$log" | awk '{s+=$1} END{print s+0}')
    [ $rc = 0 ] || [ "$f" != 0 ] || f=1
    NPASS=$((NPASS + p)); NFAIL=$((NFAIL + f))
    if [ $rc = 0 ]; then add "$c" PASS "$p passed"; else FAILED=1; add "$c" FAIL "$p passed, $f failed; $log"; grep -E "^test .* FAILED" "$log" | head -n 20; fi
  done
fi

echo
echo "=========== realdata-gate summary (install: $GAME)"
printf '%s' "$SUMMARY"
echo "ignored tests: $NPASS passed, $NFAIL failed"
echo "needs-window (not run): $(echo $SKIP_NAMES | tr ' ' ',')"
echo "needs-window (not run): d2-client play / verify / gpu_compare (docs/LOCAL-RUN.md Batches 3-6), trace recordings"
[ $NO_CLIENT = 1 ] && echo "skipped on request: d2-client ignored tests (--no-client)"
echo "logs: $LOGDIR"
[ $FAILED = 0 ] && exit 0 || exit 1
