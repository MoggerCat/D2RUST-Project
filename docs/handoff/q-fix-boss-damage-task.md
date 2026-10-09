# Task: q-fix-boss-damage (from the coordinator, 2026-10-09)

You are session q-fix-boss-damage of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, held to exact match; goal: the whole game playable start to finish). BRANCH = q-fix-boss-damage. REC ids: REC-1020..REC-1029 only.

Area: combat damage to monsters, monster death (d2-sim combat, monster death/vitals; coordinate with owners.tsv rows "act1-combat" and "monster melee/AI combat").

Tasks: Andariel never dies (her hp never reaches the kill), and Radament stays at 256 hp (hp raw 256 = 1 hp shown) under the player's attacks. Find why with the playthrough milestones (`python3 tools/playthrough/playthrough.py traces/playthrough/act1.play --build --only <andariel milestone>`, same for act2.play Radament) and the checkpoints (tools/checkpoints, 14 saves) plus `state-dump`. Likely areas: boss hp scaling / regeneration (stat 6 raw <<8, regen events type 3), damage reduction / resist fields of the superunique/boss rows, the kill test (hp < 256?). Compare with 1.14d using tools/scenario-diff (state channel; a .check that hits the boss) wherever possible. Also check that killed monsters stay dead (mode 12, earlier report "monsters stand back up, hp 0") on the bosses. Add tests on real data (M23).

RULES:
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (message it with send_message).

Setup: `git fetch origin claude/specs-staging-7 claude/q-fix-boss-damage && git checkout -B claude/q-fix-boss-damage origin/claude/q-fix-boss-damage && sh tools/coord/sync.sh` (merges the latest staging). Read CLAUDE.md, then only the sections of docs you need (docs/METHODS.md M01, M22, M23, M25 at least). Game files: attach MoggerCat/D2RUST-private-repo (add_repo), sparse-clone it per its README.md (`extracted tools install`), `python3 tools/assemble.py /home/user/game`, then `D2_GAME_DIR=/home/user/game`; build the excel view with `tools/realdata-gate.sh` if a test needs it. Never copy private files into the public repo. Bevy system libs: `apt-get install -y pkg-config libwayland-dev libasound2-dev libudev-dev libxkbcommon-dev`.

Session rules:
- Just push to claude/q-fix-boss-damage (git push -u origin claude/q-fix-boss-damage); the coordinator merges. No PRs.
- Message the coordinator only when a blocker is fixed (with the playthrough count: `python3 tools/playthrough/playthrough.py <file> --build` reached/total), when blocked, or when done.
- Stay in your area (tools/coord/owners.tsv; `python3 tools/coord/route.py <path>` names the owner of a file outside your area: send a note to that session or the coordinator instead of editing).
- Run `sh tools/coord/sync.sh` every 30 minutes.
- Read only what you need (specs by section index, M11).
- Exact match with 1.14d stays the bar (CLAUDE.md rule 10): fix the first divergence a check shows; never weaken a test or expect a bug. Unsettled choices are PROVISIONAL with your REC ids (M22).
- Before each push: `cargo fmt --all`, `cargo clippy -p <crates you touched> --all-targets -- -D warnings`, `cargo nextest run -p <those crates>`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`. Disk is limited: `rm -rf target/debug/incremental` after big builds.
- PC 1 questions (need the Windows game or Ghidra): add unnumbered items "- [q-fix-boss-damage] title" to docs/handoff/pc1-data.md Step 4; the coordinator numbers them.
- At the end write docs/handoff/q-fix-boss-damage.md (done / open / repro commands), push, send "done" once to the coordinator, then stop.
