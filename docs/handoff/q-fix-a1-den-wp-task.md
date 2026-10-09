# Task: q-fix-a1-den-wp (from the coordinator, 2026-10-09)

You are session q-fix-a1-den-wp of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, held to exact match; goal: the whole game playable start to finish). BRANCH = q-fix-a1-den-wp. REC ids: REC-1050..REC-1059 only.

Area: Act I quests and waypoints (owners.tsv rows "progression, waypoints" and "quests").

Tasks:
1. Den of Evil quest flag: clearing the Den does not set the quest state the way 1.14d does (quest completion / Akara reward). Find it with `python3 tools/playthrough/playthrough.py traces/playthrough/act1.play --build` (Den milestone) and specs/world/quests*.md (A1Q1); fix and add a test.
2. The Cold Plains waypoint (and the arriving player) is (+15, +5) sub-tiles off 1.14d's (5169, 4659): `traces/checks/warp-cold-plains-ama.check` (docs/handoff/claude-q-fix-pc1-proto-items.md, q-prov-recording.md ~line 291). Fix the preset/waypoint placement so the check goes equal (tools/scenario-diff).

RULES:
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (message it with send_message).

Setup: `git fetch origin claude/specs-staging-7 claude/q-fix-a1-den-wp && git checkout -B claude/q-fix-a1-den-wp origin/claude/q-fix-a1-den-wp && sh tools/coord/sync.sh` (merges the latest staging). Read CLAUDE.md, then only the sections of docs you need (docs/METHODS.md M01, M22, M23, M25 at least). Game files: attach MoggerCat/D2RUST-private-repo (add_repo), sparse-clone it per its README.md (`extracted tools install`), `python3 tools/assemble.py /home/user/game`, then `D2_GAME_DIR=/home/user/game`; build the excel view with `tools/realdata-gate.sh` if a test needs it. Never copy private files into the public repo. Bevy system libs: `apt-get install -y pkg-config libwayland-dev libasound2-dev libudev-dev libxkbcommon-dev`.

Session rules:
- Just push to claude/q-fix-a1-den-wp (git push -u origin claude/q-fix-a1-den-wp); the coordinator merges. No PRs.
- Message the coordinator only when a blocker is fixed (with the playthrough count: `python3 tools/playthrough/playthrough.py <file> --build` reached/total), when blocked, or when done.
- Stay in your area (tools/coord/owners.tsv; `python3 tools/coord/route.py <path>` names the owner of a file outside your area: send a note to that session or the coordinator instead of editing).
- Run `sh tools/coord/sync.sh` every 30 minutes.
- Read only what you need (specs by section index, M11).
- Exact match with 1.14d stays the bar (CLAUDE.md rule 10): fix the first divergence a check shows; never weaken a test or expect a bug. Unsettled choices are PROVISIONAL with your REC ids (M22).
- Before each push: `cargo fmt --all`, `cargo clippy -p <crates you touched> --all-targets -- -D warnings`, `cargo nextest run -p <those crates>`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`. Disk is limited: `rm -rf target/debug/incremental` after big builds.
- PC 1 questions (need the Windows game or Ghidra): add unnumbered items "- [q-fix-a1-den-wp] title" to docs/handoff/pc1-data.md Step 4; the coordinator numbers them.
- At the end write docs/handoff/q-fix-a1-den-wp.md (done / open / repro commands), push, send "done" once to the coordinator, then stop.
