# Coordinator scripts

The merge gate and loop the coordinator used on 2026-10-09 (see
`docs/handoff/coordinator-resume.md`). Copy this folder to a scratch
directory before use (logs are written next to the scripts). They assume
the checkout at /home/user/D2RUST-Project.

- `gate.sh`: fmt, clippy -D warnings (d2-sim, d2-server, d2-client,
  test-fixtures), nextest, coverage, spec_index, conflict markers.
- `gatepush.sh <tag>`: gate `integ-local`; push to claude/specs-staging-7
  only on `GATE fail=0` and a fast-forward.
- `mergebatch.sh b1 b2 …`: reset integ-local to staging (unless
  NORESET=1), merge each `claude/<b>`, union docs, resolve spec index
  tables, regenerate indexes; a branch with any other conflict is skipped.
- `autoloop.sh [n]`: forever: merge every branch in `branches.txt` that is
  ahead of staging, gate, push.
