#!/bin/sh
# Integrator pull: copy every ledger part branch's docs/handoff/ledger/*.{tsv,json,md}
# into this checkout, apply tools/coord/ledger_fixups.py, then run
# `python3 tools/coord/ledger.py --fix`. Our own code. Usage: sh tools/coord/ledger-pull.sh
cd "$(git rev-parse --show-toplevel)" || exit 2
for b in monsters skills items world systems cov-a1a2 cov-a3a5 cov-checks; do
  git fetch -q origin claude/q-ledger-$b 2>/dev/null || continue
  for f in $(git ls-tree --name-only origin/claude/q-ledger-$b docs/handoff/ledger/ 2>/dev/null | grep -E "\.(tsv|json|md)$"); do
    git show origin/claude/q-ledger-$b:$f > $f
  done
done
python3 tools/coord/ledger_fixups.py
python3 tools/coord/ledger.py --fix
