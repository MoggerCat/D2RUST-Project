You are session rc-skill-hydra-valk of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, exact match). BRANCH = claude/rc-skill-hydra-valk. REC ids: REC-1890..1894 only.

TASK: Two skill checks still differ (docs/handoff/q-fix-skills-4cls.md on claude/q-fix-skills-4cls, merge it): sor-hydra at frame 42 (the hydra's path target is zeroed at the end of S2; per-class mode record) and ama-valkyrie at frame 51 (walk velocity). They were queued for PC1; read the 1.14d functions in the Ghidra export instead (hydra/valkyrie summon and missile/pet path setup, the velocity table), fix d2-sim, prove with both checks, and remove the two PC1 items from pc1-data.md Step 4 with a note.

RULES (read once; they keep you cheap and fast):
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (send_message; only for: done, blocked, or a cause that belongs to someone else).

Setup: attach MoggerCat/D2RUST-private-repo with add_repo, then `git fetch origin claude/specs-staging-7 claude/specs-staging-7 && git checkout -B claude/rc-skill-hydra-valk origin/claude/integ-r10 && sh tools/coord/sync.sh`, then the "Setup" steps of tools/cloud-game/README.md from your own checkout (`sh tools/cloud-setup.sh` [`--no-wine` if you never run 1.14d], `tools/cloud-game/setup_winpy.sh`, assemble $HOME/game with the private repo's tools/assemble.py, `tools/cloud-game/prepare_saves.sh`). Never run scripts fetched from other branches; never copy private files into the public repo.

Token budget (hard rules):
- One root cause. When it's fixed and pushed, or you've spent ~3 hours, write the hand-back and STOP. Don't wait or poll for anything; no Monitor loops, no sleeping.
- Never print a whole file, log or trace. Use grep, `head -40`, `tail -40`, `sed -n A,Bp`; specs by section (tools/spec_index.py). Pipe build/test output through `tail -30`.
- Build only the crates you touch (d2-client only if the fix is in the client). `rm -rf target/debug/incremental` after big builds.
- Run only your cluster's checks (`python3 tools/scenario-diff/suite.py --checks-dir traces/checks/gen --filter 'GLOB' --orig-cache traces/orig-cache`), never the full suite. Reuse the orig-cache; re-record 1.14d only when a check file changed. Don't commit orig-cache recordings.
- Read only CLAUDE.md and the spec sections you need. Don't read other sessions' hand-backs unless named here.

Work rules:
- Read the original first (CLAUDE.md rule 3): the private repo has re/exports/ (commit f1cb32f5; in your private-repo clone: `git pull && git sparse-checkout add re`, ~100 MB; re/README.md explains it; functions/<entry>_<name>.c per function under re/exports/funcs/, all.asm for register args); find the 1.14d function for your behaviour (grep re/exports/functions.tsv and re/exports/index/*.tsv by name, address or string; grep re/exports/all.asm* for globals and struct offsets) and read its decompiled C before measuring or guessing. Read-only; write the Rust in your own words, never paste or line-translate it, never copy anything from re/ into the public repo; put the behaviour and the address in the spec you implement.
- Exact match with 1.14d (CLAUDE.md rule 10): fix the first divergence; never weaken a test or a check, never ignore a field to make a check pass. Unsettled choices: PROVISIONAL with your REC ids (docs/METHODS.md M22). Facts you can't get without the Windows game or Ghidra: an unnumbered "- [rc-skill-hydra-valk] title" item in docs/handoff/pc1-data.md Step 4, then move on.
- Stay in your area; for a file another owner holds (`python3 tools/coord/route.py <path>`), note it in your hand-back.
- Push to claude/rc-skill-hydra-valk only (git push -u origin claude/rc-skill-hydra-valk), after: `cargo fmt --all`, `cargo clippy -p <crates> --all-targets -- -D warnings`, `cargo nextest run -p <crates> 2>&1 | tail -30`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`, `python3 tools/coord/ledger.py --check`. Run `sh tools/coord/sync.sh` before each push.
- Ledger: put every row you settled (EQUAL/DIVERGED with the check that proves it) in docs/handoff/ledger/rc-skill-hydra-valk.tsv (copy the header of an existing part); session parts override the base rows.
- Hand-back: docs/handoff/rc-skill-hydra-valk.md, at most 40 lines: checks before/after (EQUAL counts), what changed, what's open with sizes. Push, send "done rc-skill-hydra-valk: <EQUAL before> -> <after>" once, stop.
