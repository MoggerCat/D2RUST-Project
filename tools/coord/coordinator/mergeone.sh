#!/bin/bash
# usage: mergeone.sh <branch>; returns 0 merged, 2 stop (code conflict)
b=$1; S=tools/coord/coordinator
git merge --no-ff --no-edit origin/claude/$b >/dev/null 2>&1 && { python3 tools/coord/ledger.py >/dev/null; git add docs; git commit -q -m "Ledger: regenerate after $b" 2>/dev/null; echo "$b clean"; exit 0; }
U=$(git diff --name-only --diff-filter=U)
O=$(echo "$U" | grep -v 'fidelity-ledger' | grep -v '^specs/')
[ -n "$O" ] && { echo "$b STOP code: $O"; exit 2; }
SP=$(echo "$U" | grep '^specs/')
[ -n "$SP" ] && { python3 $S/idxres.py $SP || { echo "$b STOP spec body"; exit 2; }; }
git checkout --ours docs/handoff/fidelity-ledger.md docs/handoff/fidelity-ledger.tsv 2>/dev/null
python3 tools/spec_index.py >/dev/null; python3 tools/coord/ledger.py | tail -1
git add -A docs specs && git commit -q --no-edit && echo "$b merged"
