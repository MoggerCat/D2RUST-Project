You are session rc-player-hit of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, exact match). BRANCH = claude/rc-player-hit. REC ids: REC-1835..1839 only.

TASK: In the monster-hit audio check and combat checks, monsters never hit the player in d2rs; 1.14d plays the impact and get-hit sounds at T 75, 96, 122 (docs/handoff/q-fix-audio.md item 3 on claude/q-fix-audio). Find why monster melee/missile attacks on the player miss or are never resolved (to-hit roll, attack resolution, owner/target flags: q-fix-player-hit's 4ec8eb00 is merged) against specs/combat, fix it, prove with the combat-monster-* and monster-hit checks.

RULES (read once; they keep you cheap and fast):
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (send_message; only for: done, blocked, or a cause that belongs to someone else).

Setup: attach MoggerCat/D2RUST-private-repo with add_repo, then `git fetch origin claude/integ-r10 claude/specs-staging-7 && git checkout -B claude/rc-player-hit origin/claude/integ-r10 && sh tools/coord/sync.sh`, then the "Setup" steps of tools/cloud-game/README.md from your own checkout (`sh tools/cloud-setup.sh` [`--no-wine` if you never run 1.14d], `tools/cloud-game/setup_winpy.sh`, assemble $HOME/game with the private repo's tools/assemble.py, `tools/cloud-game/prepare_saves.sh`). Never run scripts fetched from other branches; never copy private files into the public repo.

Token budget (hard rules):
- One root cause. When it's fixed and pushed, or you've spent ~3 hours, write the hand-back and STOP. Don't wait or poll for anything; no Monitor loops, no sleeping.
- Never print a whole file, log or trace. Use grep, `head -40`, `tail -40`, `sed -n A,Bp`; specs by section (tools/spec_index.py). Pipe build/test output through `tail -30`.
- Build only the crates you touch (d2-client only if the fix is in the client). `rm -rf target/debug/incremental` after big builds.
- Run only your cluster's checks (`python3 tools/scenario-diff/suite.py --checks-dir traces/checks/gen --filter 'GLOB' --orig-cache traces/orig-cache`), never the full suite. Reuse the orig-cache; re-record 1.14d only when a check file changed. Don't commit orig-cache recordings.
- Read only CLAUDE.md and the spec sections you need. Don't read other sessions' hand-backs unless named here.

Work rules:
- Exact match with 1.14d (CLAUDE.md rule 10): fix the first divergence; never weaken a test or a check, never ignore a field to make a check pass. Unsettled choices: PROVISIONAL with your REC ids (docs/METHODS.md M22). Facts you can't get without the Windows game or Ghidra: an unnumbered "- [rc-player-hit] title" item in docs/handoff/pc1-data.md Step 4, then move on.
- Stay in your area; for a file another owner holds (`python3 tools/coord/route.py <path>`), note it in your hand-back.
- Push to claude/rc-player-hit only (git push -u origin claude/rc-player-hit), after: `cargo fmt --all`, `cargo clippy -p <crates> --all-targets -- -D warnings`, `cargo nextest run -p <crates> 2>&1 | tail -30`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`, `python3 tools/coord/ledger.py --check`. Run `sh tools/coord/sync.sh` before each push.
- Ledger: put every row you settled (EQUAL/DIVERGED with the check that proves it) in docs/handoff/ledger/rc-player-hit.tsv (copy the header of an existing part); session parts override the base rows.
- Hand-back: docs/handoff/rc-player-hit.md, at most 40 lines: checks before/after (EQUAL counts), what changed, what's open with sizes. Push, send "done rc-player-hit: <EQUAL before> -> <after>" once, stop.
