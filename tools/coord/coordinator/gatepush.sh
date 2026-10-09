#!/bin/bash
# gate integ-local, then push to staging only on GATE fail=0 and a fast-forward
S=$(dirname "$0")
cd /home/user/D2RUST-Project
avail=$(df -BG --output=avail . | tail -1 | tr -dc 0-9); [ "$avail" -lt 9 ] && { echo "disk ${avail}G: cargo clean"; cargo clean -q 2>/dev/null; }
log=$S/gate-$1.log
bash $S/gate.sh > $log 2>&1; tail -3 $log
git fetch -q origin claude/specs-staging-7
if grep -q "GATE fail=0" $log && git merge-base --is-ancestor origin/claude/specs-staging-7 integ-local; then
  git push -q origin integ-local:claude/specs-staging-7 && echo PUSHED $(git rev-parse --short HEAD)
else echo NOT-PUSHED; fi
