#!/bin/bash
# Continuous merge loop: every round merges every listed branch that is ahead of staging, gates, pushes on green.
S=$(dirname "$0")
cd /home/user/D2RUST-Project
n=${1:-100}
while true; do
  git fetch -q origin 2>/dev/null
  B=$(cat $S/branches.txt)
  ahead=""
  for b in $B; do git rev-parse -q --verify origin/claude/$b >/dev/null || continue
    [ "$(git rev-list --count origin/claude/specs-staging-7..origin/claude/$b)" -gt 0 ] && ahead="$ahead $b"; done
  if [ -z "$ahead" ]; then sleep 120; continue; fi
  echo "=== round b$n $(date -u +%H:%M):$ahead" >> $S/autoloop.log
  $S/mergebatch.sh $ahead 2>&1 | grep -E "merged|UNRESOLVED" >> $S/autoloop.log
  if git diff --quiet origin/claude/specs-staging-7 HEAD; then echo "nothing merged" >> $S/autoloop.log; sleep 300; continue; fi
  $S/gatepush.sh b$n 2>&1 | tail -3 >> $S/autoloop.log
  n=$((n+1))
done
