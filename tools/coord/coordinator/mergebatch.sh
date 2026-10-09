#!/bin/bash
# merge branches onto integ-local from staging; doc conflicts unioned; any leftover marker aborts that branch
cd /home/user/D2RUST-Project
git fetch -q origin claude/specs-staging-7; for b in "$@"; do git fetch -q origin claude/$b 2>/dev/null || echo "$b: no branch"; done
[ -n "$NORESET" ] || git checkout -q -B integ-local origin/claude/specs-staging-7
for b in "$@"; do
  git rev-parse -q --verify origin/claude/$b >/dev/null || continue
  git merge-base --is-ancestor origin/claude/$b HEAD && { echo "$b: already in"; continue; }
  git merge --no-ff --no-commit origin/claude/$b >/dev/null 2>&1
  for f in $(git diff --name-only --diff-filter=U); do case $f in
    docs/handoff/provisional-index*) git checkout --theirs $f; git add $f;;
    specs/*.md) git checkout -m $f; python3 -c "import re,sys;p=sys.argv[1];s=open(p).read();s=re.sub(r'^<<<<<<< [^\\n]*\\n(\\|.*?)^=======\\n(\\|.*?)^>>>>>>> [^\\n]*\\n',r'\\1',s,flags=re.S|re.M);open(p,'w').write(s)" $f; python3 tools/spec_index.py >/dev/null; grep -qE '^(<<<<<<< |>>>>>>> |=======$)' $f || git add $f;;
    docs/*|tools/*/README.md) python3 tools/coord/union.py $f >/dev/null; grep -qE '^(<<<<<<< |>>>>>>> |=======$)' $f || git add $f;;
  esac; done
  L=$(git diff --name-only --diff-filter=U)
  if [ -n "$L" ]; then echo "$b: UNRESOLVED $(echo $L)"; git merge --abort
  else git commit -q -m "Merge $b into staging

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01QeN5r8PoLZsUhwAH2iDzLJ" && echo "$b: merged"; fi
done
python3 tools/provisional_index.py >/dev/null; python3 tools/spec_index.py >/dev/null; git add -A docs/handoff/provisional-index*.tsv specs
git diff --cached --quiet || git commit -q -m "Regenerate the provisional and spec indexes

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01QeN5r8PoLZsUhwAH2iDzLJ"
git grep -nE "^(<<<<<<< |>>>>>>> )" -- docs specs crates tools facts traces | head -3
