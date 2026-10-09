#!/bin/bash
# Run a Windows program (by default the 1.14d Game.exe, `-w -ns`) headless
# under Wine on a virtual X display, with a time limit; capture its
# stdout/stderr, exit code and screenshots of the display.
#
#   tools/cloud-game/run.sh [options] [-- PROGRAM ARGS]
#
# Options:
#   --exe PATH        program (default $D2_GAME_DIR/Game.exe; D2_GAME_DIR
#                     defaults to $HOME/game)
#   --seconds N       kill it after N seconds (default 30)
#   --shot-at S[,S..] screenshot times in seconds after launch (default N-2)
#   --out DIR         output folder (default $HOME/cloud-game-runs/<time>)
#   --python          run tools/cloud-game Windows Python instead: PROGRAM
#                     ARGS are a .py script and its arguments (wine python.exe)
# Default program args: -w -ns.
#
# Output in DIR: stdout.txt (--python: also stdout.clean.txt), stderr.txt (Wine's own messages, WINEDEBUG),
# exit.txt (exit code; 124 = time limit), shot-<S>.png, windows.txt (X
# windows at the first shot), run.json (command, versions, times).
# Nothing is written to the game folder by this script (the game itself
# may write its own files there, as on Windows).
set -u
here="$(cd "$(dirname "$0")" && pwd)"
seconds=30
shots=""
out=""
exe=""
use_python=0
while [ $# -gt 0 ]; do
  case "$1" in
    --exe) exe="$2"; shift 2 ;;
    --seconds) seconds="$2"; shift 2 ;;
    --shot-at) shots="$2"; shift 2 ;;
    --out) out="$2"; shift 2 ;;
    --python) use_python=1; shift ;;
    --) shift; break ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
done
args=("$@")
game_dir="${D2_GAME_DIR:-$HOME/game}"
if [ "$use_python" = 1 ]; then
  pyexe="${D2_WINPY:-$HOME/winpy/tools/python.exe}"
  [ -f "$pyexe" ] || { echo "no Windows Python at $pyexe (run setup_winpy.sh)" >&2; exit 2; }
  exe="$pyexe"
else
  [ -n "$exe" ] || exe="$game_dir/Game.exe"
  [ ${#args[@]} -gt 0 ] || args=(-w -ns)
fi
[ -f "$exe" ] || { echo "no program at $exe" >&2; exit 2; }
[ -n "$shots" ] || shots=$(( seconds > 3 ? seconds - 2 : 1 ))
[ -n "$out" ] || out="$HOME/cloud-game-runs/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$out"

export WINEPREFIX="${WINEPREFIX:-$HOME/.wine-d2}"
export WINEDEBUG="${WINEDEBUG:--all}"
export WINEDLLOVERRIDES="${WINEDLLOVERRIDES:-mscoree,mshtml=}"   # no Mono/Gecko prompts
display="${D2_DISPLAY:-:99}"

# One run per Wine prefix at a time: every run ends with `wineserver -k`,
# which kills any other run sharing the prefix (and its display). Wait for
# the prefix lock (fd 9 stays open until this script exits).
mkdir -p "$WINEPREFIX"
exec 9>"$WINEPREFIX/.run.lock"
if ! flock -n 9; then
  echo "run.sh: waiting for another run on $WINEPREFIX" >&2
  flock 9
fi
export DISPLAY="$display"

# Virtual display. 1024x768x24: the game's -w window (800x600) fits.
xvfb_pid=""
if ! xdpyinfo >/dev/null 2>&1; then
  Xvfb "$display" -screen 0 1024x768x24 -nolisten tcp >"$out/xvfb.txt" 2>&1 9>&- &
  xvfb_pid=$!
  for _ in $(seq 50); do xdpyinfo >/dev/null 2>&1 && break; sleep 0.1; done
fi
cleanup() {
  wineserver -k >/dev/null 2>&1
  [ -n "$xvfb_pid" ] && kill "$xvfb_pid" 2>/dev/null
}
trap cleanup EXIT

# First run: create the prefix (a few seconds; quiet).
if [ ! -f "$WINEPREFIX/system.reg" ]; then
  wineboot -i >"$out/wineboot.txt" 2>&1
  wineserver -w
fi

# Screenshots in the background at the requested times. Neither this
# subshell nor its `sleep` keeps the prefix lock (fd 9): a sleep left
# running after the run held the lock for up to --seconds (seen 2026-10-09).
t0=$(date +%s.%N)
(
  exec 9>&-
  IFS=, ; first=1
  for s in $shots; do
    now=$(date +%s.%N)
    wait_s=$(echo "$t0 + $s - $now" | bc)
    case "$wait_s" in -*|0) ;; *) sleep "$wait_s" ;; esac
    import -display "$display" -window root "$out/shot-$s.png" 2>>"$out/shot-errors.txt"
    if [ "$first" = 1 ]; then
      xwininfo -root -tree -display "$display" >"$out/windows.txt" 2>&1
      first=0
    fi
  done
) &
shot_pid=$!

if [ "$use_python" = 1 ]; then
  # Windows CPython under Wine 9.0 fails at start-up ("init_sys_streams:
  # Invalid handle") when its stdout or stderr is a file or pipe; a pty works.
  # `script` gives it one and keeps stdout and stderr together (with terminal
  # escapes) in stdout.txt.
  cmd="$(printf '%q ' timeout -k 5 "$seconds" wine "$exe" "${args[@]}")"
  script -qec "$cmd" "$out/stdout.txt" </dev/null >/dev/null
  code=$?
  : >"$out/stderr.txt"
  # readable copy: cursor-forward escapes are spaces, the rest is dropped
  sed 's/\x1b\[[0-9]*C/ /g; s/\x1b\[[0-9;?]*[a-zA-Z]//g; s/\r//g' "$out/stdout.txt" \
    >"$out/stdout.clean.txt"
else
  cd "$(dirname "$exe")"
  timeout -k 5 "$seconds" wine "$exe" "${args[@]}" >"$out/stdout.txt" 2>"$out/stderr.txt"
  code=$?
fi
t1=$(date +%s.%N)
pkill -P "$shot_pid" 2>/dev/null; kill "$shot_pid" 2>/dev/null; wait "$shot_pid" 2>/dev/null
echo "$code" >"$out/exit.txt"

python3 - "$out/run.json" "$exe" "$code" "$t0" "$t1" "${args[@]}" <<'PY'
import json, subprocess, sys
path, exe, code, t0, t1, *args = sys.argv[1:]
def v(cmd):
    try:
        return subprocess.run(cmd, capture_output=True, text=True).stdout.strip().splitlines()[0]
    except Exception as e:
        return f"? ({e})"
json.dump({"format": "cloud-game-run-1", "exe": exe, "args": args, "exit": int(code),
           "seconds": round(float(t1) - float(t0), 2), "wine": v(["wine", "--version"]),
           "xvfb": v(["dpkg-query", "-W", "-f=${Version}", "xvfb"])},
          open(path, "w"), indent=1)
PY
echo "exit $code ($([ "$code" = 124 ] && echo 'time limit' || echo 'program exit')); output in $out"
ls "$out"
