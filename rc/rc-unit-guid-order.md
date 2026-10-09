You are session rc-unit-guid-order of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, exact match). BRANCH = claude/rc-unit-guid-order. REC ids: REC-1840..1844 only.

TASK: Unit GUIDs differ: in the Blood Moor the Fallen get GUIDs 17-19 in d2rs, 19-21 in 1.14d (q-fix-audio item 4), so two units were allocated earlier in 1.14d (objects, missiles or warps created before the population). Every check comparing unit ids after that point diverges. Find the two missing allocations (state channel: list GUIDs per frame on both sides of a Blood Moor check), fix the allocation order in d2-sim against specs/world population/objects sections, then run --filter 'gen-lvl-*,gen-ai-*' and report EQUAL before/after.

RULES (read once; they keep you cheap and fast):
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (send_message; only for: done, blocked, or a cause that belongs to someone else).

Setup: attach MoggerCat/D2RUST-private-repo with add_repo, then `git fetch origin claude/integ-r10 claude/specs-staging-7 && git checkout -B claude/rc-unit-guid-order origin/claude/integ-r10 && sh tools/coord/sync.sh`, then the "Setup" steps of tools/cloud-game/README.md from your own checkout (`sh tools/cloud-setup.sh` [`--no-wine` if you never run 1.14d], `tools/cloud-game/setup_winpy.sh`, assemble $HOME/game with the private repo's tools/assemble.py, `tools/cloud-game/prepare_saves.sh`). Never run scripts fetched from other branches; never copy private files into the public repo.

Token budget (hard rules):
- One root cause. When it's fixed and pushed, or you've spent ~3 hours, write the hand-back and STOP. Don't wait or poll for anything; no Monitor loops, no sleeping.
- Never print a whole file, log or trace. Use grep, `head -40`, `tail -40`, `sed -n A,Bp`; specs by section (tools/spec_index.py). Pipe build/test output through `tail -30`.
- Build only the crates you touch (d2-client only if the fix is in the client). `rm -rf target/debug/incremental` after big builds.
- Run only your cluster's checks (`python3 tools/scenario-diff/suite.py --checks-dir traces/checks/gen --filter 'GLOB' --orig-cache traces/orig-cache`), never the full suite. Reuse the orig-cache; re-record 1.14d only when a check file changed. Don't commit orig-cache recordings.
- Read only CLAUDE.md and the spec sections you need. Don't read other sessions' hand-backs unless named here.

Work rules:
- Exact match with 1.14d (CLAUDE.md rule 10): fix the first divergence; never weaken a test or a check, never ignore a field to make a check pass. Unsettled choices: PROVISIONAL with your REC ids (docs/METHODS.md M22). Facts you can't get without the Windows game or Ghidra: an unnumbered "- [rc-unit-guid-order] title" item in docs/handoff/pc1-data.md Step 4, then move on.
- Stay in your area; for a file another owner holds (`python3 tools/coord/route.py <path>`), note it in your hand-back.
- Push to claude/rc-unit-guid-order only (git push -u origin claude/rc-unit-guid-order), after: `cargo fmt --all`, `cargo clippy -p <crates> --all-targets -- -D warnings`, `cargo nextest run -p <crates> 2>&1 | tail -30`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`, `python3 tools/coord/ledger.py --check`. Run `sh tools/coord/sync.sh` before each push.
- Ledger: put every row you settled (EQUAL/DIVERGED with the check that proves it) in docs/handoff/ledger/rc-unit-guid-order.tsv (copy the header of an existing part); session parts override the base rows.
- Hand-back: docs/handoff/rc-unit-guid-order.md, at most 40 lines: checks before/after (EQUAL counts), what changed, what's open with sizes. Push, send "done rc-unit-guid-order: <EQUAL before> -> <after>" once, stop.
