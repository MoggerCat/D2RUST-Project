#!/bin/sh
# Decompile one 1.14d function with the current names and types.
#   sh tools/ghidra/decomp1.sh <0xADDR|name-part>
# Writes to a fresh temp dir (never the repo) and prints its path; show it
# with sed -n A,Bp. Needs `sh tools/ghidra/cloud_setup.sh install` + `project`.
#   sh tools/ghidra/decomp1.sh --selftest
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
if [ "${1:-}" = "--selftest" ]; then
    out=$(mktemp -d); case $out in "$HERE"/../../*) echo "temp dir inside repo"; exit 1;; esac
    rmdir "$out"; echo "decomp1 selftest ok"; exit 0
fi
[ $# -eq 1 ] || { sed -n 2,6p "$0"; exit 2; }
out=$(mktemp -d "${TMPDIR:-/tmp}/decomp1.XXXXXX")
sh "$HERE/cloud_setup.sh" decompile "$1" "$out" > "$out/decomp.c"
echo "$out/decomp.c ($(wc -l < "$out/decomp.c") lines)"
