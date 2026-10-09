# Task: q-fix-npc-menus (from the coordinator, 2026-10-09)

You are session q-fix-npc-menus of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, held to exact match; goal: the whole game playable start to finish). BRANCH = q-fix-npc-menus. REC ids: REC-1040..REC-1049 only.

Area: NPC interaction, NPC menus, client UI panels (owners.tsv rows "quests, NPC interact" and "client UI panels").

Tasks:
1. Natalya (Act III, class 297) and Halbu (Act IV, 257; also Jamella 405) open no menu: the click sends C→S 0x13 but no S→C NPC menu comes (docs/handoff/q-tool-autoplay.md row npc-no-menu; all have monstats `interact` 1). Fix per specs/world/npc.md and specs/ui/npc-menus.tsv.
2. Nihlathak (Act V, Harrogath) never interacts.
3. Esc while the quest log is open also opens the Esc menu; in 1.14d Esc closes the panel only (specs/ui/panels.md, control-panel.md). Also confirm Esc in an NPC dialog sends 0x30 and later talks still work.
Prove each with a d2-client test on the real install and, where 1.14d can be driven, a tools/scenario-diff .check.

RULES:
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (message it with send_message).

Setup: `git fetch origin claude/specs-staging-7 claude/q-fix-npc-menus && git checkout -B claude/q-fix-npc-menus origin/claude/q-fix-npc-menus && sh tools/coord/sync.sh` (merges the latest staging). Read CLAUDE.md, then only the sections of docs you need (docs/METHODS.md M01, M22, M23, M25 at least). Game files: attach MoggerCat/D2RUST-private-repo (add_repo), sparse-clone it per its README.md (`extracted tools install`), `python3 tools/assemble.py /home/user/game`, then `D2_GAME_DIR=/home/user/game`; build the excel view with `tools/realdata-gate.sh` if a test needs it. Never copy private files into the public repo. Bevy system libs: `apt-get install -y pkg-config libwayland-dev libasound2-dev libudev-dev libxkbcommon-dev`.

Session rules:
- Just push to claude/q-fix-npc-menus (git push -u origin claude/q-fix-npc-menus); the coordinator merges. No PRs.
- Message the coordinator only when a blocker is fixed (with the playthrough count: `python3 tools/playthrough/playthrough.py <file> --build` reached/total), when blocked, or when done.
- Stay in your area (tools/coord/owners.tsv; `python3 tools/coord/route.py <path>` names the owner of a file outside your area: send a note to that session or the coordinator instead of editing).
- Run `sh tools/coord/sync.sh` every 30 minutes.
- Read only what you need (specs by section index, M11).
- Exact match with 1.14d stays the bar (CLAUDE.md rule 10): fix the first divergence a check shows; never weaken a test or expect a bug. Unsettled choices are PROVISIONAL with your REC ids (M22).
- Before each push: `cargo fmt --all`, `cargo clippy -p <crates you touched> --all-targets -- -D warnings`, `cargo nextest run -p <those crates>`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`. Disk is limited: `rm -rf target/debug/incremental` after big builds.
- PC 1 questions (need the Windows game or Ghidra): add unnumbered items "- [q-fix-npc-menus] title" to docs/handoff/pc1-data.md Step 4; the coordinator numbers them.
- At the end write docs/handoff/q-fix-npc-menus.md (done / open / repro commands), push, send "done" once to the coordinator, then stop.
