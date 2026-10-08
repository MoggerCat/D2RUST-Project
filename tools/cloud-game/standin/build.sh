#!/bin/sh
# Build the stand-in Game.exe (32-bit, fixed image base 0x400000) and its
# symbol map. Needs mingw-w64 (tools/cloud-setup.sh). Output in $1
# (default $HOME/standin): standin.exe, standin.syms (nm).
set -e
here="$(cd "$(dirname "$0")" && pwd)"
out="${1:-$HOME/standin}"
mkdir -p "$out"
i686-w64-mingw32-gcc -O1 -masm=intel -Wall -o "$out/standin.exe" "$here/standin.c" \
  -Wl,--image-base=0x400000 -Wl,--disable-dynamicbase -Wl,--subsystem,console -lgdi32 -luser32
i686-w64-mingw32-nm "$out/standin.exe" | grep -i ' t _standin_rng_' > "$out/standin.syms"
cat "$out/standin.syms"
