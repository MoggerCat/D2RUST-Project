You are session rc-town-npc-state of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, exact match). BRANCH = claude/rc-town-npc-state. REC ids: REC-1855..1859 only.

TASK: Town NPC spawn state differs from 1.14d at frames 24-37: class 201 mode in Lut Gholein (first divergence of all 19 gen-shrine-* checks), class 511 mode (gen-wp 30-38), class 359 state (gen-wp 18-26) (docs/handoff/q-run-gen-wp-shrine.md on claude/q-run-gen-wp-shrine). q-fix-d2-town-mode fixed Larzuk's arrival path step only. Fix the town NPC spawn/arrival state machine in d2-sim against specs (monsters/town NPC sections), re-run --filter 'gen-shrine-*,gen-wp-*' and report EQUAL before/after. Then the next shared cause in those checks: game.seed after waypoint travel at frame 400 (wp1-8,28,29).

RULES (read once; they keep you cheap and fast):
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (send_message; only for: done, blocked, or a cause that belongs to someone else).

Setup: attach MoggerCat/D2RUST-private-repo with add_repo, then `git fetch origin claude/specs-staging-7 claude/specs-staging-7 && git checkout -B claude/rc-town-npc-state origin/claude/specs-staging-7 && sh tools/coord/sync.sh`, then the "Setup" steps of tools/cloud-game/README.md from your own checkout (`sh tools/cloud-setup.sh` [`--no-wine` if you never run 1.14d], `tools/cloud-game/setup_winpy.sh`, assemble $HOME/game with the private repo's tools/assemble.py, `tools/cloud-game/prepare_saves.sh`). Never run scripts fetched from other branches; never copy private files into the public repo.

Token budget (hard rules):
- One root cause. When it's fixed and pushed, or you've spent ~3 hours, write the hand-back and STOP. Don't wait or poll for anything; no Monitor loops, no sleeping.
- Never print a whole file, log or trace. Use grep, `head -40`, `tail -40`, `sed -n A,Bp`; specs by section (tools/spec_index.py). Pipe build/test output through `tail -30`.
- Build only the crates you touch (d2-client only if the fix is in the client). `rm -rf target/debug/incremental` after big builds.
- Run only your cluster's checks (`python3 tools/scenario-diff/suite.py --checks-dir traces/checks/gen --filter 'GLOB' --orig-cache traces/orig-cache`), never the full suite. Reuse the orig-cache; re-record 1.14d only when a check file changed. Don't commit orig-cache recordings.
- Read only CLAUDE.md and the spec sections you need. Don't read other sessions' hand-backs unless named here.

Work rules:
- Exact match with 1.14d (CLAUDE.md rule 10): fix the first divergence; never weaken a test or a check, never ignore a field to make a check pass. Unsettled choices: PROVISIONAL with your REC ids (docs/METHODS.md M22). Facts you can't get without the Windows game or Ghidra: an unnumbered "- [rc-town-npc-state] title" item in docs/handoff/pc1-data.md Step 4, then move on.
- Stay in your area; for a file another owner holds (`python3 tools/coord/route.py <path>`), note it in your hand-back.
- Push to claude/rc-town-npc-state only (git push -u origin claude/rc-town-npc-state), after: `cargo fmt --all`, `cargo clippy -p <crates> --all-targets -- -D warnings`, `cargo nextest run -p <crates> 2>&1 | tail -30`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`, `python3 tools/coord/ledger.py --check`. Run `sh tools/coord/sync.sh` before each push.
- Ledger: put every row you settled (EQUAL/DIVERGED with the check that proves it) in docs/handoff/ledger/rc-town-npc-state.tsv (copy the header of an existing part); session parts override the base rows.
- Hand-back: docs/handoff/rc-town-npc-state.md, at most 40 lines: checks before/after (EQUAL counts), what changed, what's open with sizes. Push, send "done rc-town-npc-state: <EQUAL before> -> <after>" once, stop.
