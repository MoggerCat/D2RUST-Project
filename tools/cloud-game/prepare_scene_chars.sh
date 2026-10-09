#!/bin/bash
# The fixed characters of the rendering-fact scenes (docs/handoff/q-facts-scenes.md),
# made by the project's own d2s-tool in the Wine prefix's save folder, like
# prepare_saves.sh. Same command, same bytes: --map-seed and --time are fixed.
#   tools/cloud-game/prepare_scene_chars.sh          (needs D2_GAME_DIR)
set -euo pipefail
repo="$(cd "$(dirname "$0")/../.." && pwd)"
export WINEPREFIX="${WINEPREFIX:-$HOME/.wine-d2}"
export D2_GAME_DIR="${D2_GAME_DIR:-$HOME/game}"
saves="$WINEPREFIX/drive_c/users/$(whoami)/Saved Games/Diablo II"
mkdir -p "$saves"
cd "$repo"
cargo build --release -q -p d2s-tool
tool="$repo/target/release/d2s-tool"
common=(--expansion --map-seed 1 --time 1700000000)
# SceSor: level 1 sorceress with Fire Bolt, 400 life, gold, a Horadric Cube, a Sash, a few
# normal items in the inventory, the cube grid and the stash; every Act I waypoint.
"$tool" new --name SceSor --class sor "${common[@]}" --skill 0=1 --gold 500 --waypoints all \
  --stat 6=102400 --stat 7=102400 \
  --item box@0,0 --item cap@4,0 --item lgl@6,0 --item lbl@8,0 --item cap@0,0:4 --item lgl@2,0:4 \
  --item cap@0,0:3 -o "$saves/SceSor.d2s"
# SceAct2..SceAct5: the same sorceress standing in the town of Act II..V (quests of the earlier acts done)
for a in 2 3 4 5; do
  "$tool" new --name SceAct$a --class sor "${common[@]}" --skill 0=1 --gold 500 --waypoints all \
    --quests acts=$((a-1)) --act $((a-1)) -o "$saves/SceAct$a.d2s"
done
for f in "$saves"/Sce*.d2s; do "$tool" check "$f" --expansion; done
