# Task: q-fix-pt-sweep (from the coordinator, 2026-10-09)

You are session q-fix-pt-sweep of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, held to exact match; goal: the whole game playable start to finish). BRANCH = q-fix-pt-sweep. REC ids: REC-1080..REC-1089 only.

Area: the playthrough tool and goto/checkpoints (owners.tsv row "scenario-diff, state-dump, recorders, input" for tools; coordinate with that owner on d2-sim poke goto).

Tasks:
1. Playthrough sweep: limit hops to ≤16 sub-tiles with a fallback ring of candidate cells (docs/handoff/q-tool-state-diff.md item 1, q-tool-checkpoints.md "Missile-passable cells"); `pos`/`goto` must land on a free, missile-passable cell (specs/tools/poke.md §6, specs/tools/playthrough.md). Keep the 1.14d side (`tools/trace-recorder/poke.py`) in step with any rule change and its selftest green.
2. Then run the class × difficulty matrix for Acts III–V: `python3 tools/playthrough/playthrough.py traces/playthrough/classes.play --build --class all --difficulty all` for the Act III–V milestones and `--all --json out.json` + `python3 tools/coord/playtable.py`; update docs/handoff/playability-matrix.md. For each failing cell, record the first blocker and route it (`python3 tools/coord/route.py`) in your report; don't fix other areas' bugs.

RULES:
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (message it with send_message).

Setup: `git fetch origin claude/specs-staging-7 claude/q-fix-pt-sweep && git checkout -B claude/q-fix-pt-sweep origin/claude/q-fix-pt-sweep && sh tools/coord/sync.sh` (merges the latest staging). Read CLAUDE.md, then only the sections of docs you need (docs/METHODS.md M01, M22, M23, M25 at least). Game files: attach MoggerCat/D2RUST-private-repo (add_repo), sparse-clone it per its README.md (`extracted tools install`), `python3 tools/assemble.py /home/user/game`, then `D2_GAME_DIR=/home/user/game`; build the excel view with `tools/realdata-gate.sh` if a test needs it. Never copy private files into the public repo. Bevy system libs: `apt-get install -y pkg-config libwayland-dev libasound2-dev libudev-dev libxkbcommon-dev`.

Session rules:
- Just push to claude/q-fix-pt-sweep (git push -u origin claude/q-fix-pt-sweep); the coordinator merges. No PRs.
- Message the coordinator only when a blocker is fixed (with the playthrough count: `python3 tools/playthrough/playthrough.py <file> --build` reached/total), when blocked, or when done.
- Stay in your area (tools/coord/owners.tsv; `python3 tools/coord/route.py <path>` names the owner of a file outside your area: send a note to that session or the coordinator instead of editing).
- Run `sh tools/coord/sync.sh` every 30 minutes.
- Read only what you need (specs by section index, M11).
- Exact match with 1.14d stays the bar (CLAUDE.md rule 10): fix the first divergence a check shows; never weaken a test or expect a bug. Unsettled choices are PROVISIONAL with your REC ids (M22).
- Before each push: `cargo fmt --all`, `cargo clippy -p <crates you touched> --all-targets -- -D warnings`, `cargo nextest run -p <those crates>`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`. Disk is limited: `rm -rf target/debug/incremental` after big builds.
- PC 1 questions (need the Windows game or Ghidra): add unnumbered items "- [q-fix-pt-sweep] title" to docs/handoff/pc1-data.md Step 4; the coordinator numbers them.
- At the end write docs/handoff/q-fix-pt-sweep.md (done / open / repro commands), push, send "done" once to the coordinator, then stop.
