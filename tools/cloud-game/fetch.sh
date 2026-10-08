#!/bin/bash
# Fetch the 1.14d install from the private data repo and rebuild it.
#
#   tools/cloud-game/fetch.sh [--partial] [DEST]     (DEST default $HOME/game)
#
# 1. Sparse, blob-less clone of MoggerCat/D2RUST-private-repo into
#    $D2_PRIVATE_REPO (default /home/user/d2rust-private-repo), or update an
#    existing clone; checks out install/ and tools/ only.
# 2. Checks every piece named by install/manifest.json is present.
# 3. tools/assemble.py rebuilds DEST, checking each file's size and sha256.
#    With --partial, files whose pieces are still missing (an upload in
#    progress) are skipped and listed; the rest are rebuilt and checked the
#    same way. Without it, a missing piece is an error (exit 1).
# Prints the line to set D2_GAME_DIR. Never writes into the public repo.
set -euo pipefail
partial=0
if [ "${1:-}" = "--partial" ]; then partial=1; shift; fi
dest="${1:-$HOME/game}"
repo="${D2_PRIVATE_REPO:-/home/user/d2rust-private-repo}"
url="https://github.com/MoggerCat/D2RUST-private-repo"

if git -C "$repo" rev-parse HEAD >/dev/null 2>&1; then
  git -C "$repo" fetch --depth 1 origin main
  git -C "$repo" reset -q --hard origin/main
else
  git clone --depth 1 --filter=blob:none --sparse "$url" "$repo"
fi
git -C "$repo" sparse-checkout set install tools
echo "private repo at $(git -C "$repo" log -1 --format='%h %ci %s')"

rc=0
python3 - "$repo" "$dest" "$partial" <<'PY' || rc=$?
import hashlib, json, sys
from pathlib import Path
repo, dest, partial = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3] == "1"
src = repo / "install"
m = json.loads((src / "manifest.json").read_text())
missing = {f["path"]: [p for p in (f["parts"] or [f["path"]]) if not (src / p).exists()]
           for f in m["files"]}
missing = {k: v for k, v in missing.items() if v}
for k, v in missing.items():
    print(f"missing pieces of {k}: {', '.join(v)}")
if missing and not partial:
    print(f"{len(missing)} files incomplete: upload not finished (use --partial to rebuild the rest)")
    sys.exit(1)
if not missing:
    sys.exit(0)  # complete: assemble.py does the work below
# --partial: assemble.py's rule (join pieces, check size and sha256) on the complete files
bad = done = 0
for f in m["files"]:
    if f["path"] in missing:
        continue
    out = dest / f["path"]
    out.parent.mkdir(parents=True, exist_ok=True)
    h = hashlib.sha256()
    with out.open("wb") as o:
        for piece in f["parts"] or [f["path"]]:
            data = (src / piece).read_bytes()
            h.update(data)
            o.write(data)
    done += 1
    if out.stat().st_size != f["size"] or h.hexdigest() != f["sha256"]:
        print(f"MISMATCH {f['path']}")
        bad += 1
print(f"PARTIAL: {done} files -> {dest}, {bad} mismatches, {len(missing)} skipped")
sys.exit(1 if bad else 3)
PY
if [ $rc -eq 0 ]; then
  python3 "$repo/tools/assemble.py" --src "$repo/install" "$dest"
elif [ $rc -ne 3 ]; then
  exit $rc
fi
echo "export D2_GAME_DIR=$dest"
