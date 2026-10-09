#!/bin/sh
# Setup script for cloud Claude Code sessions (Linux). Installs what CI
# installs: Bevy's native libraries and the Rust toolchain pinned in
# rust-toolchain.toml, and Wine + Xvfb for running the original 1.14d
# Game.exe headless (tools/cloud-game/README.md; D2_NO_WINE=1 skips it).
# Game files come from the private data repo (CLAUDE.md "Where work runs").
set -e

SUDO=""
if [ "$(id -u)" != "0" ] && command -v sudo >/dev/null 2>&1; then SUDO="sudo"; fi

$SUDO apt-get update
$SUDO apt-get install -y --no-install-recommends \
  pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev

# Original game under Wine (tools/cloud-game). Measured 2026-10-08 on Ubuntu
# 24.04: wine 9.0~repack-4build3 (wine32:i386, wine64), xvfb
# 2:21.1.12-1ubuntu1.8, mingw-w64 11.0.1-3build1 (stand-in build),
# imagemagick (screenshots), x11-utils (xdpyinfo, xwininfo), xdotool.
if [ "${D2_NO_WINE:-0}" != "1" ]; then
  $SUDO dpkg --add-architecture i386
  $SUDO apt-get update
  DEBIAN_FRONTEND=noninteractive $SUDO apt-get install -y --no-install-recommends \
    wine wine32:i386 wine64 xvfb x11-apps x11-utils imagemagick xdotool mingw-w64 bc
  # d2rs' window under Xvfb without a GPU (scenario-diff draws channel,
  # specs/tools/scenario-diff.md §3 rule 7): Vulkan on Mesa lavapipe.
  # Measured 2026-10-09: mesa-vulkan-drivers 25.2.8-0ubuntu0.24.04.4,
  # libxkbcommon-x11-0 1.6.0-1build1.
  DEBIAN_FRONTEND=noninteractive $SUDO apt-get install -y --no-install-recommends \
    mesa-vulkan-drivers libvulkan1 libxkbcommon-x11-0 libxcursor1 libxrandr2 libxi6 libx11-xcb1
fi

if ! command -v rustup >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  . "$HOME/.cargo/env"
fi

# Run from the repo root so rustup reads rust-toolchain.toml.
cd "$(dirname "$0")/.."
rustup show active-toolchain || rustup toolchain install
cargo --version
