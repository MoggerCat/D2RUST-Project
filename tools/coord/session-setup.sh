#!/bin/sh
# One-shot setup for a cloud session of the build loop (coordinator, 2026-10-09).
# Idempotent; each step is skipped when already done. Also suitable as the
# cloud environment's setup script, so new containers start ready.
#   sh tools/coord/session-setup.sh [--no-wine] [--no-game]
# Installs: Bevy system libs, cargo-nextest, the pinned toolchain (once, not
# concurrently: parallel rustup installs corrupt it), the 1.14d install from the
# private data repo into $HOME/game (+ the live excel view), Wine + Windows
# Python + the recorder saves (tools/cloud-game/). Never copies private files
# into this repository.
set -u
WINE=1; GAME=1
for a in "$@"; do case $a in --no-wine) WINE=0;; --no-game) GAME=0;; esac; done
ROOT=$(cd "$(dirname "$0")/../.." && pwd); cd "$ROOT" || exit 2
step() { echo "== session-setup: $*"; }
if ! pkg-config --exists wayland-client alsa libudev 2>/dev/null; then
  step "apt: Bevy libs"
  apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq pkg-config \
    libwayland-dev libasound2-dev libudev-dev libxkbcommon-dev libx11-dev libxcursor-dev \
    libxrandr-dev libxi-dev >/dev/null
fi
step "toolchain"; rustup show active-toolchain >/dev/null 2>&1 || rustup toolchain install "$(sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml)"
command -v cargo-nextest >/dev/null 2>&1 || { step "cargo-nextest"; cargo install cargo-nextest --locked -q; }
export D2_GAME_DIR=${D2_GAME_DIR:-$HOME/game}
if [ $GAME = 1 ] && [ ! -f "$D2_GAME_DIR/Game.exe" ]; then
  step "1.14d install -> $D2_GAME_DIR"
  P=$HOME/d2data
  [ -d "$P/.git" ] || git clone -q --depth 1 --filter=blob:none --sparse https://github.com/MoggerCat/D2RUST-private-repo "$P"
  (cd "$P" && git sparse-checkout set tools install >/dev/null && python3 tools/assemble.py "$D2_GAME_DIR") \
    || echo "session-setup: install step failed (private repo not attached? attach MoggerCat/D2RUST-private-repo with add_repo; or disk full: df -h) — rerun after fixing"
  rm -rf "$P/install"   # the assembled copy is enough; keeps the disk free
fi
if [ $GAME = 1 ] && [ -f "$D2_GAME_DIR/Game.exe" ] && [ ! -d "$D2_GAME_DIR/extracted/patch_d2/data/global/excel" ]; then
  step "live excel view"; mkdir -p "$D2_GAME_DIR/extracted/patch_d2/data/global/excel"
  CARGO_INCREMENTAL=0 cargo run -q --release -p data-tool -- excel-dir "$D2_GAME_DIR/extracted/patch_d2/data/global/excel" "$D2_GAME_DIR"
fi
if [ $WINE = 1 ] && [ $GAME = 1 ]; then
  command -v wine >/dev/null 2>&1 || { step "wine"; sh tools/cloud-setup.sh >/dev/null; }
  [ -d "$HOME/winpy" ] || { step "windows python"; tools/cloud-game/setup_winpy.sh >/dev/null; }
  [ -f "$HOME/.d2-saves-ready" ] || { step "recorder saves"; tools/cloud-game/prepare_saves.sh >/dev/null && touch "$HOME/.d2-saves-ready"; }
fi
step "done: export D2_GAME_DIR=$D2_GAME_DIR CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0"
