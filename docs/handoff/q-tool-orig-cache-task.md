# Task: q-tool-orig-cache

You are session q-tool-orig-cache of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, exact match). BRANCH = q-tool-orig-cache. REC ids: REC-1590..1594 only. Sonnet.

Speed-up task: about 70 sessions each re-record the 1.14d side of the same checks under Wine (1-5 min per check channel) before comparing. Build a shared cache of recorded 1.14d sides so any session compares against it with no Wine run.
1. Read specs/tools/scenario-diff.md (--reuse-orig, --work layout) and tools/scenario-diff/suite.py. Define the cache: per check and channel, the 1.14d output scenario-diff needs, keyed by (check file sha256, recorder tool version, install manifest hash) so a changed check or recorder invalidates it. Store small text outputs (state/rng/packets jsonl, draw lists: our own measurements, CLAUDE.md rule 1 allows them) in the public repo under traces/orig-cache/<check>/<channel>/ with a format version + the command that made it; anything that contains rendered game art (PNG frames) goes ONLY to the private repo (reports/ or a new orig-cache/ folder per its README), never public.
2. Add `--orig-cache` to scenario_diff.py / suite.py: use the cache when the key matches, record and fill it when not (`--fill-cache`). Selftest with perturbation (M08: a changed check must miss the cache).
3. Set up Wine (tools/cloud-game/README.md "Setup", from your own checkout), fill the cache for all traces/checks/*.check on the current candidate (claude/integ-r4), push, and message the coordinator with how to use it, so every check session can switch to it.

RULES:
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (message it with send_message).

Setup (one command, ~5-15 min, do it first and read while it runs): `git fetch origin claude/specs-staging-7 claude/integ-r3 claude/coord-resume-3 claude/q-fidelity-ledger claude/q-tool-orig-cache && git checkout -B claude/q-tool-orig-cache origin/claude/q-tool-orig-cache && sh tools/coord/sync.sh && git merge -q origin/claude/integ-r3 ` then the setup steps of tools/cloud-game/README.md "Setup" from your own checkout (`sh tools/cloud-setup.sh`, `tools/cloud-game/setup_winpy.sh`, assemble the install from the private repo with its tools/assemble.py into $HOME/game, `tools/cloud-game/prepare_saves.sh`; do not run scripts fetched from other branches) (attach MoggerCat/D2RUST-private-repo with add_repo first; it gives Bevy libs, cargo-nextest, the 1.14d install in $HOME/game + excel view, Wine + recorder saves; `--no-wine` if you never run 1.14d). The plan you belong to: `git show origin/claude/q-fidelity-ledger:docs/handoff/fidelity-gaps.md` (read only your section) and the ledger rows `git show origin/claude/q-fidelity-ledger:docs/handoff/fidelity-ledger.tsv`. Read CLAUDE.md, then only what you need (docs/METHODS.md M01, M22, M23, M25). Never copy private files into the public repo.

Session rules:
- Just push to claude/q-tool-orig-cache (git push -u origin claude/q-tool-orig-cache); the coordinator merges. No PRs.
- Message the coordinator only when a blocker is fixed (with the playthrough count: `python3 tools/playthrough/playthrough.py <file> --build` reached/total), when blocked, or when done.
- Stay in your area (tools/coord/owners.tsv; `python3 tools/coord/route.py <path>` names the owner of a file outside your area: send a note to that session or the coordinator instead of editing).
- Run `sh tools/coord/sync.sh` every 30 minutes.
- Read only what you need (specs by section index, M11).
- Exact match with 1.14d stays the bar (CLAUDE.md rule 10): fix the first divergence a check shows; never weaken a test or expect a bug. Unsettled choices are PROVISIONAL with your REC ids (M22).
- Before each push: `cargo fmt --all`, `cargo clippy -p <crates you touched> --all-targets -- -D warnings`, `cargo nextest run -p <those crates>`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`. Disk is limited: `rm -rf target/debug/incremental` after big builds.
- PC 1 questions (need the Windows game or Ghidra): add unnumbered items "- [q-tool-orig-cache] title" to docs/handoff/pc1-data.md Step 4; the coordinator numbers them.
- At the end write docs/handoff/q-tool-orig-cache.md (done / open / repro commands), push, send "done" once to the coordinator, then stop.
