You are session rc-mon-y-warp of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, exact match). BRANCH = claude/rc-mon-y-warp. REC ids: REC-2315..2319 only.

TASK: After setup run `git fetch origin claude/integ-r19 && git merge origin/claude/integ-r19` (r19 = staging + ~16 newer fixes, gate pending). Monster y differs at frame 21 in a1-warp-l31-jail-3-ama (1.14d 8159 vs d2rs 8156) and a1-warp-l33-cathedral-ama (4947 vs 4948) (docs/handoff/rc-walk-y1.md). Find the monster movement/placement cause from re/exports-typed, fix it in d2-sim, re-run the a1-warp family.

RULES (read once; they keep you cheap and fast):
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (send_message; only for: done, blocked, or a cause that belongs to someone else).

Setup: attach MoggerCat/D2RUST-private-repo with add_repo, then `git fetch origin claude/specs-staging-7 && git checkout -B claude/rc-mon-y-warp origin/claude/specs-staging-7 && sh tools/coord/sync.sh`, then the "Setup" steps of tools/cloud-game/README.md from your own checkout (`sh tools/cloud-setup.sh` [`--no-wine` if you never run 1.14d], `tools/cloud-game/setup_winpy.sh`, assemble $HOME/game with the private repo's tools/assemble.py, `tools/cloud-game/prepare_saves.sh`). Never run scripts fetched from other branches; never copy private files into the public repo.

Token budget (hard rules):
- One root cause. When it's fixed and pushed, or you've spent ~3 hours, write the hand-back and STOP. Don't wait or poll for anything; no Monitor loops, no sleeping.
- Never print a whole file, log or trace. Use grep, `head -40`, `tail -40`, `sed -n A,Bp`; specs by section (tools/spec_index.py). Pipe build/test output through `tail -30`.
- Build only the crates you touch (d2-client only if the fix is in the client). `rm -rf target/debug/incremental` after big builds.
- Run only your cluster's checks (`python3 tools/scenario-diff/suite.py --checks-dir traces/checks/gen --filter 'GLOB' --orig-cache traces/orig-cache`), never the full suite. Reuse the orig-cache; re-record 1.14d only when a check file changed. Don't commit orig-cache recordings.
- Read only CLAUDE.md and the spec sections you need. Don't read other sessions' hand-backs unless named here.

Work rules:
- Read the original first (CLAUDE.md rule 3): the private repo has the Ghidra exports of 1.14d Game.exe (in your private-repo clone: `git pull && git sparse-checkout add re`). Prefer re/exports-typed/ (typed structs, 1,243 named functions; e.g. AITHINK_Fn013_FallenShaman); re/exports/ is the untyped original (functions.tsv, funcs/<entry>_<name>.c, index/*.tsv, all.asm for register args), re/exports/names.tsv and offsets.tsv when present. To decompile a function yourself with current names: `sh tools/ghidra/cloud_setup.sh` (see its header; Ghidra headless from Docker Hub). Find the 1.14d function for your behaviour and read it BEFORE measuring or guessing; most causes are settled by reading. Read-only: write Rust in your own words, never paste or line-translate it, never copy anything from re/ into the public repo; put the behaviour and the address in the spec. If you confirm a function's role, append it to re/exports/names.tsv in the private repo (address, name, source=session, evidence) and push the private repo.
- Exact match with 1.14d (CLAUDE.md rule 10): fix the first divergence; never weaken a test or a check, never ignore a field to make a check pass. Unsettled choices: PROVISIONAL with your REC ids (docs/METHODS.md M22). Facts you can't get without the Windows game or Ghidra: an unnumbered "- [rc-mon-y-warp] title" item in docs/handoff/pc1-data.md Step 4, then move on.
- Stay in your area; for a file another owner holds (`python3 tools/coord/route.py <path>`), note it in your hand-back.
- Push to claude/rc-mon-y-warp only (git push -u origin claude/rc-mon-y-warp), after: `cargo fmt --all`, `cargo clippy -p <crates> --all-targets -- -D warnings`, `cargo nextest run -p <crates> 2>&1 | tail -30`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`, `python3 tools/coord/ledger.py --check`. Run `sh tools/coord/sync.sh` before each push.
- Ledger: put every row you settled (EQUAL/DIVERGED with the check that proves it) in docs/handoff/ledger/rc-mon-y-warp.tsv (copy the header of an existing part); session parts override the base rows.
- Hand-back: docs/handoff/rc-mon-y-warp.md, at most 40 lines: checks before/after (EQUAL counts), what changed, what's open with sizes. Push, send "done rc-mon-y-warp: <EQUAL before> -> <after>" once, stop.
