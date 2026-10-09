# Task: q-fix-combat-live (from the coordinator, 2026-10-09)

You are session q-fix-combat-live of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, held to exact match; goal: the whole game playable start to finish). BRANCH = q-fix-combat-live. REC ids: REC-1260..REC-1269 only.

Area: combat exactness against 1.14d (the weakest area of the suite: 9.5% of ticks equal over 17 combat-* checks; docs/handoff/checks-status.md). Base: staging + q-fix-player-hit (read docs/handoff/q-fix-player-hit.md).
Tasks: 1) q-fix-player-hit's open items: missile data flags 1/2 (spec §R6.1 step 5) and the 0x4000 soft-hit rule. 2) Set up 1.14d under Wine (tools/cloud-game/README.md) and get a LIVE equal on combat-arrow-quillrat and combat-arrow-kill with tools/scenario-diff (state channel). 3) Then the other combat-* checks in order: fix each first divergence in combat/damage/hit code; route RNG/seed-order divergences (frame 2 monster creation draws) to q-fix-seed-order and monster AI to q-fix-monster-ai. Report per check: first divergent frame before/after.

Other sessions own other areas (tools/coord/owners.tsv and docs/handoff/coordinator-resume.md); route their bugs, don't fix them.

RULES:
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (message it with send_message).

Setup: `git fetch origin claude/specs-staging-7 claude/q-fix-combat-live && git checkout -B claude/q-fix-combat-live origin/claude/q-fix-combat-live && sh tools/coord/sync.sh` (merges the latest staging). Read CLAUDE.md, then only the sections of docs you need (docs/METHODS.md M01, M22, M23, M25 at least). Game files: attach MoggerCat/D2RUST-private-repo (add_repo), sparse-clone it per its README.md (`extracted tools install`), `python3 tools/assemble.py /home/user/game`, then `D2_GAME_DIR=/home/user/game`; build the excel view with `tools/realdata-gate.sh` if a test needs it. Never copy private files into the public repo. Bevy system libs: `apt-get install -y pkg-config libwayland-dev libasound2-dev libudev-dev libxkbcommon-dev`.

Session rules:
- Just push to claude/q-fix-combat-live (git push -u origin claude/q-fix-combat-live); the coordinator merges. No PRs.
- Message the coordinator only when a blocker is fixed (with the playthrough count: `python3 tools/playthrough/playthrough.py <file> --build` reached/total), when blocked, or when done.
- Stay in your area (tools/coord/owners.tsv; `python3 tools/coord/route.py <path>` names the owner of a file outside your area: send a note to that session or the coordinator instead of editing).
- Run `sh tools/coord/sync.sh` every 30 minutes.
- Read only what you need (specs by section index, M11).
- Exact match with 1.14d stays the bar (CLAUDE.md rule 10): fix the first divergence a check shows; never weaken a test or expect a bug. Unsettled choices are PROVISIONAL with your REC ids (M22).
- Before each push: `cargo fmt --all`, `cargo clippy -p <crates you touched> --all-targets -- -D warnings`, `cargo nextest run -p <those crates>`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`. Disk is limited: `rm -rf target/debug/incremental` after big builds.
- PC 1 questions (need the Windows game or Ghidra): add unnumbered items "- [q-fix-combat-live] title" to docs/handoff/pc1-data.md Step 4; the coordinator numbers them.
- At the end write docs/handoff/q-fix-combat-live.md (done / open / repro commands), push, send "done" once to the coordinator, then stop.
