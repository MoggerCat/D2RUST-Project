#!/bin/sh
# Ghidra 12.1.4 headless on a cloud session (Linux), for the 1.14d Game.exe.
#
#   sh tools/ghidra/cloud_setup.sh install            Ghidra into $HOME/ghidra (~790 MB)
#   sh tools/ghidra/cloud_setup.sh project            import + analyze + names + types (~15 min)
#   sh tools/ghidra/cloud_setup.sh types              re-apply names + types to the project
#   sh tools/ghidra/cloud_setup.sh decompile <0xADDR|name-part> [outdir]
#                                                     decompile with current names and types
#   sh tools/ghidra/cloud_setup.sh export <outdir>    full re-export (funcs/, functions.tsv, index/)
#
# Needs: a JDK 21 on PATH (cloud images have one), python3, and the private
# repo cloned at $PRIV (default /home/user/d2rust-private-repo) with the
# install (install/Game.exe) and re/ (git sparse-checkout add re).
#
# Install source: the GitHub release download is refused by the cloud
# session's GitHub scope (github.com/NationalSecurityAgency/ghidra is not an
# attached repository), so `install` takes the unmodified official
# distribution out of the Docker Hub image blacktop/ghidra:12.1.4 (its
# /ghidra layer), straight from the registry API (no Docker daemon needed),
# and checks Ghidra/application.properties says 12.1.4.
# Everything it writes is outside the repo ($HOME/ghidra, $HOME/ghidra-proj,
# $HOME/typed). Never copy the output into the public repo.
set -eu

REPO=$(cd "$(dirname "$0")/../.." && pwd)
PRIV=${PRIV:-/home/user/d2rust-private-repo}
GH=${GHIDRA_HOME:-$HOME/ghidra}
PROJ=${GHIDRA_PROJ:-$HOME/ghidra-proj}
TYPED=$HOME/typed
SCRIPTS=$REPO/tools/ghidra
HEADLESS="$GH/support/analyzeHeadless"

run() { # analyzeHeadless on the existing project, quiet but for script output
    "$HEADLESS" "$PROJ" D2_114d -process Game.exe -noanalysis -scriptPath "$SCRIPTS" "$@" 2>&1 \
        | grep -E '\.java> |ERROR|Exception' | grep -v 'DecompileCallback' || true
}

harvest() {
    (cd "$REPO" && python3 "$SCRIPTS/spec_harvest.py" "$PRIV/re/exports/functions.tsv" "$TYPED")
}

case "${1:-}" in
install)
    if grep -q 'application.version=12.1.4' "$GH/Ghidra/application.properties" 2>/dev/null; then
        echo "Ghidra 12.1.4 already in $GH"; exit 0
    fi
    tmp=$(mktemp -d)
    python3 - "$tmp" <<'EOF'
import json, sys, tarfile, urllib.request
out = sys.argv[1]
repo, tag = 'blacktop/ghidra', '12.1.4'
tok = json.load(urllib.request.urlopen(
    f'https://auth.docker.io/token?service=registry.docker.io&scope=repository:{repo}:pull'))['token']
acc = ('application/vnd.oci.image.index.v1+json,application/vnd.docker.distribution.manifest.list.v2+json,'
       'application/vnd.oci.image.manifest.v1+json,application/vnd.docker.distribution.manifest.v2+json')
def get(path, accept=acc):
    return urllib.request.urlopen(urllib.request.Request(
        f'https://registry-1.docker.io/v2/{repo}/{path}',
        headers={'Authorization': 'Bearer ' + tok, 'Accept': accept}))
m = json.load(get('manifests/' + tag))
if 'manifests' in m:
    d = [x for x in m['manifests'] if x.get('platform', {}).get('architecture') == 'amd64'][0]['digest']
    m = json.load(get('manifests/' + d))
# The Ghidra distribution is the largest layer (~500 MB).
layer = max(m['layers'], key=lambda l: l['size'])
with get('blobs/' + layer['digest'], '*/*') as r, open(out + '/ghidra.tgz', 'wb') as f:
    while b := r.read(1 << 20):
        f.write(b)
EOF
    mkdir -p "$(dirname "$GH")"
    tar xzf "$tmp/ghidra.tgz" -C "$tmp" ghidra/
    rm -rf "$GH" && mv "$tmp/ghidra" "$GH" && rm -rf "$tmp"
    grep -q 'application.version=12.1.4' "$GH/Ghidra/application.properties"
    echo "Ghidra 12.1.4 installed in $GH"
    ;;
project)
    [ -e "$PROJ/D2_114d.gpr" ] && { echo "$PROJ/D2_114d.gpr exists; remove it to rebuild"; exit 1; }
    mkdir -p "$PROJ"
    "$HEADLESS" "$PROJ" D2_114d -import "$PRIV/install/Game.exe" -scriptPath "$SCRIPTS" \
        -postScript ImportCommunityLabels.java "$PRIV/re/labels/LoD 1.14D.txt" 2>&1 \
        | grep -E '\.java> |REPORT|ERROR' || true
    "$0" types
    ;;
types)
    harvest
    run -postScript ApplyNames.java "$PRIV/re/exports/functions.tsv" "$TYPED/spec-names.tsv" \
        $( [ -f "$PRIV/re/exports/names.tsv" ] && echo "$PRIV/re/exports/names.tsv" ) \
        -postScript ApplyTypes.java "$SCRIPTS/d2_114d_types.h" "$TYPED/spec-signatures.tsv"
    ;;
decompile)
    out=${3:-$(mktemp -d)}
    q=$2
    case $q in 0x*|0X*) q=$(printf '0x%08x' "$q") ;; esac
    "$HEADLESS" "$PROJ" D2_114d -process Game.exe -noanalysis -readOnly -scriptPath "$SCRIPTS" \
        -postScript ExportDecompiled.java "$out" "$q" >/dev/null 2>&1
    cat "$out"/funcs/*.c
    ;;
export)
    out=$2
    "$HEADLESS" "$PROJ" D2_114d -process Game.exe -noanalysis -readOnly -scriptPath "$SCRIPTS" \
        -postScript ExportDecompiled.java "$out" \
        -postScript ExportIndex.java "$out/index" 2>&1 | grep -E '\.java> |ERROR' || true
    ;;
*)
    sed -n 2,10p "$0"; exit 1
    ;;
esac
