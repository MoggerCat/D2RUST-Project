#!/bin/sh
# Install a Windows CPython for the trace recorders under Wine.
# python.org is blocked by the cloud network policy; NuGet is not, and
# NuGet's `python` package is the official CPython build for x64 Windows
# (the recorders need 64-bit Python: they debug the 32-bit Game.exe
# through Wow64GetThreadContext, as on PC 1).
#   tools/cloud-game/setup_winpy.sh [DEST]     (default $HOME/winpy)
# Then: wine $DEST/tools/python.exe --version
set -e
ver="${WINPY_VERSION:-3.12.7}"
sha="${WINPY_SHA256:-149dd298e0b7a82250ca019471770fff079874088a4e8501ca20922d7df3a6ac}"
dest="${1:-$HOME/winpy}"
tmp="$(mktemp -d)"
curl -sSfL -o "$tmp/python.nupkg" "https://api.nuget.org/v3-flatcontainer/python/$ver/python.$ver.nupkg"
echo "$sha  $tmp/python.nupkg" | sha256sum -c -
rm -rf "$dest"
mkdir -p "$dest"
python3 -c "import zipfile,sys; zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])" "$tmp/python.nupkg" "$dest"
rm -rf "$tmp"
export WINEPREFIX="${WINEPREFIX:-$HOME/.wine-d2}" WINEDEBUG="${WINEDEBUG:--all}"
export WINEDLLOVERRIDES="${WINEDLLOVERRIDES:-mscoree,mshtml=}"
[ -f "$WINEPREFIX/system.reg" ] || { wineboot -i >/dev/null 2>&1; wineserver -w; }
# A pty: Windows Python under Wine 9.0 cannot start with stdout on a file/pipe.
script -qec "wine $dest/tools/python.exe -c \"import sys, struct; print(sys.version, struct.calcsize('P') * 8, 'bit')\"" /dev/null </dev/null
