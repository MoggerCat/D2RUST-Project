# Task: q-fix-walk-desync (from the coordinator, 2026-10-09)

You are session q-fix-walk-desync of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, held to exact match; goal: the whole game playable start to finish). BRANCH = q-fix-walk-desync. REC ids: REC-1250..REC-1259 only.

Area: local player movement on the client: walk prediction, re-target on a click while walking, the client position check (Opus; exactness work). Base: staging now contains the input-lock fix (bridge/player_anim.rs) — read docs/handoff/q-fix-input-lock.md first.
Task: the player position desync / walk re-target on a click while walking. Evidence: `python3 tools/soak/soak.py run --start town1 --seed 7 --steps 1500 --out OUT` → desync:player-position at step 261 / frame 284, client (5661,5797) vs server (5664,5812), 15 sub-tiles; side-by-side scene a1-walk-s (docs/handoff/q-tool-side-by-side.md). Specs: specs/seams/movement-prediction.md, specs/sim/client-messages.tsv 0x01-0x04, specs/client/model.md §6 (position check 0x004804E0). Set up 1.14d under Wine (tools/cloud-game/README.md) and prove with a walk-click-walk .check equal in tools/scenario-diff, plus a soak run without a desync finding. q-fix-client-crash keeps the geom.rs crash only.

Other sessions own other areas (tools/coord/owners.tsv and docs/handoff/coordinator-resume.md); route their bugs, don't fix them.

RULES:
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (message it with send_message).

Setup: `git fetch origin claude/specs-staging-7 claude/q-fix-walk-desync && git checkout -B claude/q-fix-walk-desync origin/claude/q-fix-walk-desync && sh tools/coord/sync.sh` (merges the latest staging). Read CLAUDE.md, then only the sections of docs you need (docs/METHODS.md M01, M22, M23, M25 at least). Game files: attach MoggerCat/D2RUST-private-repo (add_repo), sparse-clone it per its README.md (`extracted tools install`), `python3 tools/assemble.py /home/user/game`, then `D2_GAME_DIR=/home/user/game`; build the excel view with `tools/realdata-gate.sh` if a test needs it. Never copy private files into the public repo. Bevy system libs: `apt-get install -y pkg-config libwayland-dev libasound2-dev libudev-dev libxkbcommon-dev`.

Session rules:
- Just push to claude/q-fix-walk-desync (git push -u origin claude/q-fix-walk-desync); the coordinator merges. No PRs.
- Message the coordinator only when a blocker is fixed (with the playthrough count: `python3 tools/playthrough/playthrough.py <file> --build` reached/total), when blocked, or when done.
- Stay in your area (tools/coord/owners.tsv; `python3 tools/coord/route.py <path>` names the owner of a file outside your area: send a note to that session or the coordinator instead of editing).
- Run `sh tools/coord/sync.sh` every 30 minutes.
- Read only what you need (specs by section index, M11).
- Exact match with 1.14d stays the bar (CLAUDE.md rule 10): fix the first divergence a check shows; never weaken a test or expect a bug. Unsettled choices are PROVISIONAL with your REC ids (M22).
- Before each push: `cargo fmt --all`, `cargo clippy -p <crates you touched> --all-targets -- -D warnings`, `cargo nextest run -p <those crates>`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`. Disk is limited: `rm -rf target/debug/incremental` after big builds.
- PC 1 questions (need the Windows game or Ghidra): add unnumbered items "- [q-fix-walk-desync] title" to docs/handoff/pc1-data.md Step 4; the coordinator numbers them.
- At the end write docs/handoff/q-fix-walk-desync.md (done / open / repro commands), push, send "done" once to the coordinator, then stop.
