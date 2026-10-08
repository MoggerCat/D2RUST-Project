#!/bin/bash
# Make the expansion characters the recorders' --auto start needs
# (tools/trace-recorder/README.md: ScnAma, ScnSor, TestSor) in the Wine
# prefix's save folder, with the project's own d2s-tool. 1.14d reads saves
# from the Saved Games known folder + "Diablo II" when the registry has no
# NewSavePath (specs/tools/original-hooks.md §5.3 rule 2a); under Wine that
# is $WINEPREFIX/drive_c/users/$USER/Saved Games/Diablo II.
#   tools/cloud-game/prepare_saves.sh          (needs D2_GAME_DIR: d2s-tool reads the tables)
set -euo pipefail
repo="$(cd "$(dirname "$0")/../.." && pwd)"
export WINEPREFIX="${WINEPREFIX:-$HOME/.wine-d2}"
export D2_GAME_DIR="${D2_GAME_DIR:-$HOME/game}"
saves="$WINEPREFIX/drive_c/users/$(whoami)/Saved Games/Diablo II"
mkdir -p "$saves"
cd "$repo"
cargo build --release -q -p d2s-tool
tool="$repo/target/release/d2s-tool"
"$tool" new --name ScnAma --class ama --expansion -o "$saves/ScnAma.d2s"
"$tool" new --name ScnSor --class sor --expansion -o "$saves/ScnSor.d2s"
"$tool" new --name TestSor --class sor --expansion --waypoints all --quests acts=4 -o "$saves/TestSor.d2s"
for f in "$saves"/*.d2s; do "$tool" check "$f" --expansion; done
ls -l "$saves"
