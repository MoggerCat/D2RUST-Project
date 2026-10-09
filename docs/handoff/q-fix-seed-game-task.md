# Task: q-fix-seed-game

You are session q-fix-seed-game of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, exact match). BRANCH = q-fix-seed-game. REC ids: REC-1580..1589 only. Opus: exactness work.

Ledger first difference D1 (docs/handoff/fidelity-gaps.md §1 on claude/q-fidelity-ledger) is the first divergence of 41 of 108 check results; it is split between two sessions so it clears faster:
- q-fix-seed-order (session_01JhQc4nNShp9YNmxNXpqAdx, Opus) owns the monster creation draw order at frame 2 (draw #0 of unit 1:x: 1.14d site 0x573f8f vs d2rs crates/d2-sim/src/monsters/init/create.rs:265) and the Blood Moor population.
- YOU own every OTHER seed divergence the suite shows: the GAME seed (game.seed) differing from frame ~4 in the state checks, and the unit / game seed steps at frames 5, 21, 28, 34, 50 in combat-*, ass-fire-blast, dru-volcano, milestone-izual, milestone-baal-*, milestone-act5-entry, a1-warp-tower-cellar, a3-warp-durance, a3-warp-kurast-sewers (docs/handoff/checks-status.md on claude/q-fix-check-triage: the rng channel names each draw's 1.14d site). First message q-fix-seed-order with send_message to agree who takes which draw site (avoid editing the same functions); re-check with it before touching monsters/init.
Method: scenario-diff rng + state channels under Wine (record the 1.14d side once, --reuse-orig after), fix the first draw that differs, re-run, report per check the first divergent frame before/after. Specs: sim/rng.md, sim/unit-order.md, the spec of each draw site's caller.

RULES:
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (message it with send_message).

Setup (one command, ~5-15 min, do it first and read while it runs): `git fetch origin claude/specs-staging-7 claude/integ-r3 claude/coord-resume-3 claude/q-fidelity-ledger claude/q-fix-seed-game && git checkout -B claude/q-fix-seed-game origin/claude/q-fix-seed-game && sh tools/coord/sync.sh && git merge -q origin/claude/integ-r3 ` then the setup steps of tools/cloud-game/README.md "Setup" from your own checkout (`sh tools/cloud-setup.sh`, `tools/cloud-game/setup_winpy.sh`, assemble the install from the private repo with its tools/assemble.py into $HOME/game, `tools/cloud-game/prepare_saves.sh`; do not run scripts fetched from other branches) (attach MoggerCat/D2RUST-private-repo with add_repo first; it gives Bevy libs, cargo-nextest, the 1.14d install in $HOME/game + excel view, Wine + recorder saves; `--no-wine` if you never run 1.14d). The plan you belong to: `git show origin/claude/q-fidelity-ledger:docs/handoff/fidelity-gaps.md` (read only your section) and the ledger rows `git show origin/claude/q-fidelity-ledger:docs/handoff/fidelity-ledger.tsv`. Read CLAUDE.md, then only what you need (docs/METHODS.md M01, M22, M23, M25). Never copy private files into the public repo.

Session rules:
- Just push to claude/q-fix-seed-game (git push -u origin claude/q-fix-seed-game); the coordinator merges. No PRs.
- Message the coordinator only when a blocker is fixed (with the playthrough count: `python3 tools/playthrough/playthrough.py <file> --build` reached/total), when blocked, or when done.
- Stay in your area (tools/coord/owners.tsv; `python3 tools/coord/route.py <path>` names the owner of a file outside your area: send a note to that session or the coordinator instead of editing).
- Run `sh tools/coord/sync.sh` every 30 minutes.
- Read only what you need (specs by section index, M11).
- Exact match with 1.14d stays the bar (CLAUDE.md rule 10): fix the first divergence a check shows; never weaken a test or expect a bug. Unsettled choices are PROVISIONAL with your REC ids (M22).
- Before each push: `cargo fmt --all`, `cargo clippy -p <crates you touched> --all-targets -- -D warnings`, `cargo nextest run -p <those crates>`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`. Disk is limited: `rm -rf target/debug/incremental` after big builds.
- PC 1 questions (need the Windows game or Ghidra): add unnumbered items "- [q-fix-seed-game] title" to docs/handoff/pc1-data.md Step 4; the coordinator numbers them.
- At the end write docs/handoff/q-fix-seed-game.md (done / open / repro commands), push, send "done" once to the coordinator, then stop.
