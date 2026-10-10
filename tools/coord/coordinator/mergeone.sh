#!/bin/bash
# usage: mergeone.sh <branch>; returns 0 merged, 2 stop (code conflict)
b=$1; S=tools/coord/coordinator
eq() { python3 tools/coord/ledger.py | grep -o 'EQUAL [0-9]*;' | grep -o '[0-9]*'; }
E0=$(eq)
# a merge that loses EQUAL rows (a stale part overriding newer verdicts) is reported, not hidden
chk() { E1=$(eq); [ "$E1" -lt "$E0" ] && echo "$b WARN: EQUAL $E0 -> $E1 (check the part for stale rows)"; }
git merge --no-ff --no-edit origin/claude/$b >/dev/null 2>&1 && { python3 tools/coord/ledger.py >/dev/null; git add docs; git commit -q -m "Ledger: regenerate after $b" 2>/dev/null; echo "$b clean"; chk; exit 0; }
U=$(git diff --name-only --diff-filter=U)
echo "$U" | grep -q 'checks-status.md' && python3 $S/statusres.py && git add docs/handoff/checks-status.md
O=$(echo "$U" | grep -v 'fidelity-ledger' | grep -v '^specs/' | grep -v 'checks-status.md')
[ -n "$O" ] && { echo "$b STOP code: $O"; exit 2; }
SP=$(echo "$U" | grep '^specs/')
[ -n "$SP" ] && { python3 $S/idxres.py $SP || { echo "$b STOP spec body"; exit 2; }; }
git checkout --ours docs/handoff/fidelity-ledger.md docs/handoff/fidelity-ledger.tsv 2>/dev/null
python3 tools/spec_index.py >/dev/null; python3 tools/coord/ledger.py | tail -1
git add -A docs specs && git commit -q --no-edit && echo "$b merged"; chk
