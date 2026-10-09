# Task: q-fix-client-crash (from the coordinator, 2026-10-09)

You are session q-fix-client-crash of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, held to exact match; goal: the whole game playable start to finish). BRANCH = q-fix-client-crash. REC ids: REC-1010..REC-1019 only.

Area: client tracks and local movement (d2-client bridge::motion, bridge::predict, d2-sim path::walk geometry as called by the client).

Tasks, in order:
1. Client crash: `crates/d2-sim/src/path/walk/geom.rs:197` direction_vector index out of bounds (negative index, len 128), called from `d2_client::bridge::motion::MonsterMotion::frame` in the Cold Plains (autoplay probe route, ~frame 3265). Notes: docs/handoff/q-tool-autoplay.md row monster-motion-crash, q-smoke-quests.md item 1, q-smoke-travel.md (the earlier `127 * ly` overflow). Find the input the 1.14d function receives in that case per specs/sim/pathing.md / specs/client/model.md §19 and make the caller or the function behave as 1.14d does (not a clamp that hides it unless the spec says so). Repro with `d2-client autoplay-host` and the fixed probe route; add a regression test.
2. Player position desync / walk re-target on a click while walking: reproduce with tools/soak and tools/sidebyside scene a1-walk-s (docs/handoff/q-tool-soak.md, q-tool-side-by-side.md; side-by-side pages go to the PRIVATE repo only). Fix the client walk re-target per specs/seams/movement-prediction.md and client-messages 0x02/0x03; prove with tools/scenario-diff on a walk-click-walk .check against 1.14d.

RULES:
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (message it with send_message).

Setup: `git fetch origin claude/specs-staging-7 claude/q-fix-client-crash && git checkout -B claude/q-fix-client-crash origin/claude/q-fix-client-crash && sh tools/coord/sync.sh` (merges the latest staging). Read CLAUDE.md, then only the sections of docs you need (docs/METHODS.md M01, M22, M23, M25 at least). Game files: attach MoggerCat/D2RUST-private-repo (add_repo), sparse-clone it per its README.md (`extracted tools install`), `python3 tools/assemble.py /home/user/game`, then `D2_GAME_DIR=/home/user/game`; build the excel view with `tools/realdata-gate.sh` if a test needs it. Never copy private files into the public repo. Bevy system libs: `apt-get install -y pkg-config libwayland-dev libasound2-dev libudev-dev libxkbcommon-dev`.

Session rules:
- Just push to claude/q-fix-client-crash (git push -u origin claude/q-fix-client-crash); the coordinator merges. No PRs.
- Message the coordinator only when a blocker is fixed (with the playthrough count: `python3 tools/playthrough/playthrough.py <file> --build` reached/total), when blocked, or when done.
- Stay in your area (tools/coord/owners.tsv; `python3 tools/coord/route.py <path>` names the owner of a file outside your area: send a note to that session or the coordinator instead of editing).
- Run `sh tools/coord/sync.sh` every 30 minutes.
- Read only what you need (specs by section index, M11).
- Exact match with 1.14d stays the bar (CLAUDE.md rule 10): fix the first divergence a check shows; never weaken a test or expect a bug. Unsettled choices are PROVISIONAL with your REC ids (M22).
- Before each push: `cargo fmt --all`, `cargo clippy -p <crates you touched> --all-targets -- -D warnings`, `cargo nextest run -p <those crates>`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`. Disk is limited: `rm -rf target/debug/incremental` after big builds.
- PC 1 questions (need the Windows game or Ghidra): add unnumbered items "- [q-fix-client-crash] title" to docs/handoff/pc1-data.md Step 4; the coordinator numbers them.
- At the end write docs/handoff/q-fix-client-crash.md (done / open / repro commands), push, send "done" once to the coordinator, then stop.
