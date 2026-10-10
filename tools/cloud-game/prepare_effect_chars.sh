#!/bin/bash
# The characters of the effect scenes (scene_defs.FX): Fx<skill id>, level 30, the one skill at
# level 1 selected on the left or right button, every waypoint, in the Wine prefix's save folder.
#   tools/cloud-game/prepare_effect_chars.sh          (needs D2_GAME_DIR)
set -euo pipefail
repo="$(cd "$(dirname "$0")/../.." && pwd)"
export WINEPREFIX="${WINEPREFIX:-$HOME/.wine-d2}"
export D2_GAME_DIR="${D2_GAME_DIR:-$HOME/game}"
saves="$WINEPREFIX/drive_c/users/$(whoami)/Saved Games/Diablo II"
mkdir -p "$saves"
cd "$repo"
cargo build --release -q -p d2s-tool
tool="$repo/target/release/d2s-tool"
python3 - "$repo/tools/cloud-game" <<'PY' | while read -r cls idx sid btn; do
import sys
sys.path.insert(0, sys.argv[1])
from scene_defs import FX
for g, c, i, s, b, l in FX:
    print(c, i, s, b)
PY
  flag=--left-skill; [ "$btn" = r ] && flag=--right-skill
  "$tool" new --name "Fx$sid" --class "$cls" --expansion --map-seed 1 --time 1700000000 --level 30 \
    --skill "$idx=1" $flag "$sid" --gold 500 --waypoints all \
    --stat 6=102400 --stat 7=102400 --stat 8=102400 --stat 9=102400 -o "$saves/Fx$sid.d2s"
  "$tool" check "$saves/Fx$sid.d2s" --expansion
done
