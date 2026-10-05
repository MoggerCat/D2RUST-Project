#!/bin/sh
# Setup script for cloud Claude Code sessions (Linux). Installs what CI
# installs: Bevy's native libraries and the Rust toolchain pinned in
# rust-toolchain.toml. Cloud sessions have no game files; tests that need
# them are #[ignore] (see CLAUDE.md "Where work runs").
set -e

SUDO=""
if [ "$(id -u)" != "0" ] && command -v sudo >/dev/null 2>&1; then SUDO="sudo"; fi

$SUDO apt-get update
$SUDO apt-get install -y --no-install-recommends \
  pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev

if ! command -v rustup >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  . "$HOME/.cargo/env"
fi

# Run from the repo root so rustup reads rust-toolchain.toml.
cd "$(dirname "$0")/.."
rustup show active-toolchain || rustup toolchain install
cargo --version
