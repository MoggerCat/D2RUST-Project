#!/bin/sh
# Selftest of tools/coord/sync.sh on toy repositories (run: sh tools/coord/sync.sh --selftest).
# Each case builds an origin with a staging branch and a session branch that
# conflict in one way, runs sync.sh, and checks its exit code and the result.
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
T=$(mktemp -d "${TMPDIR:-/tmp}/sync-selftest.XXXXXX")
FAILS=0
export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
export SYNC_PROV_INDEX="echo regenerated > docs/handoff/provisional-index.tsv"
LT=$(printf '<%.0s' 1 2 3 4 5 6 7)   # a marker built at run time, so this file holds none

spec() { # $1: extra section text after §1, $2: extra section at the end
  printf '# Toy\n\n- **Status:** draft\n\n## 1. One\n\nbody one\n%s\n## 2. Two\n\nbody two\n%s\n' "$1" "$2"
}

setup() { # fresh origin + clone at $T/$1/w on branch "mine"; staging = claude/specs-staging-7
  d=$T/$1; mkdir -p "$d/w"; cd "$d/w" || exit 2
  git init -q -b main . && mkdir -p tools/coord specs docs/handoff crates
  cp "$HERE/sync.sh" "$HERE/union.py" tools/coord/ && cp "$REPO/tools/spec_index.py" tools/
  spec "" "" > specs/toy.md
  printf '<!-- index -->\n<!-- /index -->\n' >> specs/toy.md  # opt the small spec into indexing
  python3 tools/spec_index.py >/dev/null
  printf '# Handoff\n\n- one\n' > docs/HANDOFF.md
  printf 'a\t1\n' > docs/handoff/provisional-index.tsv
  printf 'fn a() {}\n' > crates/x.rs
  git add -A && git commit -q -m base
  git init -q --bare "$d/origin.git" && git remote add origin "$d/origin.git"
  git -c push.negotiate=false push -q origin main:claude/specs-staging-7 && git checkout -q -b mine
}
staging() { # run "$@" as a commit on staging, then return to mine
  git checkout -q -b tmp origin/claude/specs-staging-7 && sh -c "$1" && git add -A && git commit -q -m staging \
    && git -c push.negotiate=false push -q origin tmp:claude/specs-staging-7 && git checkout -q mine && git branch -q -D tmp
}
mine() { sh -c "$1" && git add -A && git commit -q -m mine; }
run() { sh tools/coord/sync.sh > ../out.txt 2>&1; echo $?; }
check() { # name, condition
  if eval "$2"; then echo "ok   $1"; else echo "FAIL $1"; sed 's/^/     /' ../out.txt; FAILS=$((FAILS + 1)); fi
}
merged() { [ -z "$(git rev-parse -q --verify MERGE_HEAD)" ] && git merge-base --is-ancestor origin/claude/specs-staging-7 HEAD; }
nomarkers() { ! git grep -q -E '^(<<<<<<<|=======$|>>>>>>>)' -- docs specs crates tools; }

setup clean
staging "echo 'fn b() {}' >> crates/x.rs"; mine "echo '- two' >> docs/HANDOFF.md"
rc=$(run); check "clean merge" "[ $rc = 0 ] && merged && grep -q 'fn b' crates/x.rs"
rc=$(run); check "already up to date" "[ $rc = 0 ] && grep -q 'already contains' ../out.txt"

setup append
staging "echo '- from staging' >> docs/HANDOFF.md"; mine "echo '- from mine' >> docs/HANDOFF.md"
rc=$(run); check "append-only union" "[ $rc = 0 ] && merged && nomarkers && grep -q 'from staging' docs/HANDOFF.md && grep -q 'from mine' docs/HANDOFF.md"

setup appendsame
staging "echo '- same' >> docs/HANDOFF.md; echo '- s' >> docs/HANDOFF.md"; mine "echo '- same' >> docs/HANDOFF.md; echo '- m' >> docs/HANDOFF.md"
rc=$(run); check "append-only union drops a line both sides added" "[ $rc = 0 ] && [ \$(grep -c -- '- same' docs/HANDOFF.md) = 1 ]"

setup index
staging "python3 -c \"import sys; s=open('specs/toy.md').read(); open('specs/toy.md','w').write(s.replace('body one\n','body one\nmore\nmore\n'))\" && python3 tools/spec_index.py >/dev/null"
mine "printf '\n## 3. Three\n\nbody three\n' >> specs/toy.md && python3 tools/spec_index.py >/dev/null"
rc=$(run); check "spec index conflict regenerated" "[ $rc = 0 ] && merged && nomarkers && python3 tools/spec_index.py --check >/dev/null && grep -q '3. Three' specs/toy.md && grep -q '^more' specs/toy.md"

setup prov
staging "printf 'b\t2\n' > docs/handoff/provisional-index.tsv"; mine "printf 'c\t3\n' > docs/handoff/provisional-index.tsv"
rc=$(run); check "provisional index regenerated" "[ $rc = 0 ] && merged && grep -q regenerated docs/handoff/provisional-index.tsv"

setup other
staging "echo 'fn s() {}' > crates/x.rs"; mine "echo 'fn m() {}' > crates/x.rs"
rc=$(run); check "other conflict stops and lists the file" "[ $rc = 1 ] && grep -q 'crates/x.rs' ../out.txt && [ -n \"\$(git rev-parse -q --verify MERGE_HEAD)\" ]"
git merge --abort

setup specbody
staging "sed -i 's/body two/body two staging/' specs/toy.md"; mine "sed -i 's/body two/body two mine/' specs/toy.md"
rc=$(run); check "spec body conflict stops" "[ $rc = 1 ] && grep -q 'specs/toy.md' ../out.txt"
git merge --abort

setup mixed
staging "echo '- s' >> docs/HANDOFF.md; echo 'fn s() {}' > crates/x.rs"; mine "echo '- m' >> docs/HANDOFF.md; echo 'fn m() {}' > crates/x.rs"
rc=$(run); check "mixed: mechanical staged, other listed" "[ $rc = 1 ] && grep -q 'crates/x.rs' ../out.txt && ! grep -q '  docs/HANDOFF.md' ../out.txt && git diff --cached --name-only | grep -q docs/HANDOFF.md"
git merge --abort

setup marker
staging "printf '%s staging\nx\n' '$LT' > docs/stray.md"; mine "echo '- m' >> docs/HANDOFF.md"
rc=$(run); check "marker left: no commit" "[ $rc = 1 ] && grep -q 'docs/stray.md' ../out.txt && [ -n \"\$(git rev-parse -q --verify MERGE_HEAD)\" ]"
git merge --abort

setup dirty
staging "echo x > docs/a.md"; echo y >> crates/x.rs
rc=$(run); check "dirty tree refused" "[ $rc = 2 ]"

rm -rf "$T"
if [ $FAILS = 0 ]; then echo "sync selftest: ok"; else echo "sync selftest: $FAILS failed"; exit 1; fi
