# Task: q-fix-items-shop (from the coordinator, 2026-10-09)

You are session q-fix-items-shop of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, held to exact match; goal: the whole game playable start to finish). BRANCH = q-fix-items-shop. REC ids: REC-1030..REC-1039 only.

Area: items, stores, inventory (owners.tsv row "items, stores, vendors").

Tasks, in order:
1. Shop buy answers "no room" when the inventory has room (vendor buy C→S 0x32 → server free-spot search). Repro on a checkpoint save in town with a vendor (tools/checkpoints) and state-dump `--poke`/`--input`; fix per specs/items/inventory-moves.md and specs/world/vendors*.
2. Belt potions in mode 4 after a save load, and a new character's 4 belt potions never reach the client model (queue rows q-fix-soak-belt-model, and the save item-load placement in formats/d2s.md §8 0x00531210; docs/handoff/q-tool-soak.md).
3. Monster item creation `0x00573B20` (monequip, monsters/init.md §12; summon equipment skills/bodies.md §6.5 step 9): the `InitHost::create_equip_item` seam is a no-op. Implement it; the check `traces/checks/ass-shadow-master.check` (tools/scenario-diff) must go equal past frame 28 (docs/handoff/q-diff-skills-2.md "Open").

RULES:
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (message it with send_message).

Setup: `git fetch origin claude/specs-staging-7 claude/q-fix-items-shop && git checkout -B claude/q-fix-items-shop origin/claude/q-fix-items-shop && sh tools/coord/sync.sh` (merges the latest staging). Read CLAUDE.md, then only the sections of docs you need (docs/METHODS.md M01, M22, M23, M25 at least). Game files: attach MoggerCat/D2RUST-private-repo (add_repo), sparse-clone it per its README.md (`extracted tools install`), `python3 tools/assemble.py /home/user/game`, then `D2_GAME_DIR=/home/user/game`; build the excel view with `tools/realdata-gate.sh` if a test needs it. Never copy private files into the public repo. Bevy system libs: `apt-get install -y pkg-config libwayland-dev libasound2-dev libudev-dev libxkbcommon-dev`.

Session rules:
- Just push to claude/q-fix-items-shop (git push -u origin claude/q-fix-items-shop); the coordinator merges. No PRs.
- Message the coordinator only when a blocker is fixed (with the playthrough count: `python3 tools/playthrough/playthrough.py <file> --build` reached/total), when blocked, or when done.
- Stay in your area (tools/coord/owners.tsv; `python3 tools/coord/route.py <path>` names the owner of a file outside your area: send a note to that session or the coordinator instead of editing).
- Run `sh tools/coord/sync.sh` every 30 minutes.
- Read only what you need (specs by section index, M11).
- Exact match with 1.14d stays the bar (CLAUDE.md rule 10): fix the first divergence a check shows; never weaken a test or expect a bug. Unsettled choices are PROVISIONAL with your REC ids (M22).
- Before each push: `cargo fmt --all`, `cargo clippy -p <crates you touched> --all-targets -- -D warnings`, `cargo nextest run -p <those crates>`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`. Disk is limited: `rm -rf target/debug/incremental` after big builds.
- PC 1 questions (need the Windows game or Ghidra): add unnumbered items "- [q-fix-items-shop] title" to docs/handoff/pc1-data.md Step 4; the coordinator numbers them.
- At the end write docs/handoff/q-fix-items-shop.md (done / open / repro commands), push, send "done" once to the coordinator, then stop.
