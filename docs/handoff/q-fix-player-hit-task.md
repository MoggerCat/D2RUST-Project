# Task: q-fix-player-hit (from the coordinator, 2026-10-09)

You are session q-fix-player-hit of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, held to exact match; goal: the whole game playable start to finish). BRANCH = q-fix-player-hit. REC ids: REC-1100..REC-1109 only.

Area: monsters hitting the player (missile targets, hit path, AI control flags).
Tasks: queue rows q-fix-c6-player-flags (the player is never a missile target: unit flags 0x0E missing; the missile hit path lacks block/dodge and monster crit) then q-fix-c7-owner-flags (owner data f1/f2 not stored as AI control flags). Rows carry spec pointers and checks (traces/raw/check-combat-arrow-quillrat etc.); make those checks go equal with tools/scenario-diff.

Other sessions running now (don't take their work; tools/coord/owners.tsv): q-fix-input-lock (client cast input lock), q-fix-client-crash (geom crash, walk desync), q-fix-boss-damage (Andariel/Radament), q-fix-items-shop (shop room, belt after load, monster items), q-fix-npc-menus, q-fix-a1-den-wp, q-fix-class-rows (whirlwind, aura, pet follow, hammer), q-fix-pt-sweep (playthrough tool, Act III-V matrix), plus the batch-2 sessions q-fix-monster-death, q-fix-player-hit, q-fix-monster-ai, q-fix-seed-order, q-fix-save-input, q-fix-room-links, q-fix-act2-play, q-fix-act4-play, q-fix-act5-play, q-fix-check-triage, q-fix-realdata-baseline, q-fix-difficulty-a1a2.

RULES:
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (message it with send_message).

Setup: `git fetch origin claude/specs-staging-7 claude/q-fix-player-hit && git checkout -B claude/q-fix-player-hit origin/claude/q-fix-player-hit && sh tools/coord/sync.sh` (merges the latest staging). Read CLAUDE.md, then only the sections of docs you need (docs/METHODS.md M01, M22, M23, M25 at least). Game files: attach MoggerCat/D2RUST-private-repo (add_repo), sparse-clone it per its README.md (`extracted tools install`), `python3 tools/assemble.py /home/user/game`, then `D2_GAME_DIR=/home/user/game`; build the excel view with `tools/realdata-gate.sh` if a test needs it. Never copy private files into the public repo. Bevy system libs: `apt-get install -y pkg-config libwayland-dev libasound2-dev libudev-dev libxkbcommon-dev`.

Session rules:
- Just push to claude/q-fix-player-hit (git push -u origin claude/q-fix-player-hit); the coordinator merges. No PRs.
- Message the coordinator only when a blocker is fixed (with the playthrough count: `python3 tools/playthrough/playthrough.py <file> --build` reached/total), when blocked, or when done.
- Stay in your area (tools/coord/owners.tsv; `python3 tools/coord/route.py <path>` names the owner of a file outside your area: send a note to that session or the coordinator instead of editing).
- Run `sh tools/coord/sync.sh` every 30 minutes.
- Read only what you need (specs by section index, M11).
- Exact match with 1.14d stays the bar (CLAUDE.md rule 10): fix the first divergence a check shows; never weaken a test or expect a bug. Unsettled choices are PROVISIONAL with your REC ids (M22).
- Before each push: `cargo fmt --all`, `cargo clippy -p <crates you touched> --all-targets -- -D warnings`, `cargo nextest run -p <those crates>`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`. Disk is limited: `rm -rf target/debug/incremental` after big builds.
- PC 1 questions (need the Windows game or Ghidra): add unnumbered items "- [q-fix-player-hit] title" to docs/handoff/pc1-data.md Step 4; the coordinator numbers them.
- At the end write docs/handoff/q-fix-player-hit.md (done / open / repro commands), push, send "done" once to the coordinator, then stop.
