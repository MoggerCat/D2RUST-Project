# Task: q-fidelity-ledger (from the coordinator, 2026-10-09)

You are session q-fidelity-ledger of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d). BRANCH = q-fidelity-ledger. REC ids: REC-1210..REC-1219 only. Model: Opus. This is planning/measurement work, not bug fixing: do not change game logic.

Goal (from the user): an exact, complete map of what is left before the d2rs build matches 1.14d 100% (CLAUDE.md rule 10), so the user can plan which tools to build and which work to schedule. No unknowns left unnamed: every area of 1.14d is either proven equal by a passing check, or is a row with what is missing and its size.

Deliverables, in this order (push after each; message the coordinator with a short summary after each):

1. FIRST DRAFT (target: 3 hours after start). docs/handoff/fidelity-ledger.tsv (machine-readable, with a format version line) + docs/handoff/fidelity-ledger.md (generated from the tsv by a small script tools/coord/ledger.py with --check and --selftest). One row per behaviour area. Build the area list from the ORIGINAL, not from our code:
   - every spec in specs/ (its status and open questions),
   - every table/entity class in the 1.14d data (monsters incl. superuniques/bosses, skills per class, missiles, items by type/quality, uniques/sets/runewords, cube recipes, levels/presets, objects/shrines, NPCs/vendors/quests per act, states/auras, sounds, UI panels/menus, difficulties),
   - systems: RNG/seeds, movement/pathing, combat/damage, AI per AI type, drops/treasure classes, save/load, network messages (each C->S/S->C id), rendering (draw order, lighting, palettes per act), audio, timing, front end, mercenaries, death/corpse, waypoints/portals/act travel.
   Columns at least: area, kind, 1.14d source of truth, spec(s) + status, checks against 1.14d (traces/checks/*.check names), last verdict (from docs/handoff/checks-status.md on claude/q-fix-check-triage: merge or read it), playthrough/matrix coverage, exercised? (coverage-map), PROVISIONAL / REC count (docs/handoff/provisional-index*.tsv), needs PC 1 (recording/RE) y/n, owner session (tools/coord/owners.tsv), state (EQUAL / DIVERGED / NO-CHECK / NOT-IMPLEMENTED / UNKNOWN), size of remaining work (S/M/L with a one-line reason).
2. COVERAGE. Build d2-client/d2-sim with the coverage-map feature (tools/coverage-map, see its README / tools/perf notes: binaries WITHOUT the feature for perf), run the full playthrough + classes matrix + the checks' d2rs side with D2_COVERAGE_DIR, and fill the "exercised?" column from `python3 tools/coverage-map/report.py --excel DIR[,DIR...] --run NAME=COVDIR ...` (excel: build the live excel view with `cargo run -p data-tool -- excel-dir <out> $D2_GAME_DIR`, as tools/realdata-gate.sh does). Every table row never exercised is a NO-CHECK row or part of one.
3. GAPS AND TOOLS. docs/handoff/fidelity-gaps.md: for every non-EQUAL row group: what check is missing, which existing tool can produce it (scenario-diff channel, playthrough, soak, side-by-side, perf, facts-compare, trace-recorder), and where NO existing tool can (that is the list of tools to build: name, what it measures, comparison, size). Then the remaining work as an ordered plan: tranches with sizes in session-hours, what can run in parallel, what is bounded by PC 1 time, with stated uncertainty (M24: no percentages; remaining work with sizes).
4. Keep it current: re-run ledger.py after each staging sync (every 30 min); the coordinator reruns it after merges.

Inputs you can trust: docs/handoff/checks-status.md (88 checks, run 2026-10-09 17:57 on staging 0a297048), docs/handoff/playability.md and playability-matrix.md, docs/handoff/provisional-index*.tsv, specs/README.md, docs/PLAN.md, tools/coord/owners.tsv. Set up 1.14d under Wine only if you need to record something (tools/cloud-game/README.md); mostly you won't.

Other sessions are fixing bugs in parallel (owners.tsv); don't fix bugs, list them with their owner.

RULES:
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (message it with send_message).

Setup: `git fetch origin claude/specs-staging-7 claude/q-fidelity-ledger && git checkout -B claude/q-fidelity-ledger origin/claude/q-fidelity-ledger && sh tools/coord/sync.sh` (merges the latest staging). Read CLAUDE.md, then only the sections of docs you need (docs/METHODS.md M01, M22, M23, M25 at least). Game files: attach MoggerCat/D2RUST-private-repo (add_repo), sparse-clone it per its README.md (`extracted tools install`), `python3 tools/assemble.py /home/user/game`, then `D2_GAME_DIR=/home/user/game`; build the excel view with `tools/realdata-gate.sh` if a test needs it. Never copy private files into the public repo. Bevy system libs: `apt-get install -y pkg-config libwayland-dev libasound2-dev libudev-dev libxkbcommon-dev`.

Session rules:
- Just push to claude/q-fidelity-ledger (git push -u origin claude/q-fidelity-ledger); the coordinator merges. No PRs.
- Message the coordinator only when a blocker is fixed (with the playthrough count: `python3 tools/playthrough/playthrough.py <file> --build` reached/total), when blocked, or when done.
- Stay in your area (tools/coord/owners.tsv; `python3 tools/coord/route.py <path>` names the owner of a file outside your area: send a note to that session or the coordinator instead of editing).
- Run `sh tools/coord/sync.sh` every 30 minutes.
- Read only what you need (specs by section index, M11).
- Exact match with 1.14d stays the bar (CLAUDE.md rule 10): fix the first divergence a check shows; never weaken a test or expect a bug. Unsettled choices are PROVISIONAL with your REC ids (M22).
- Before each push: `cargo fmt --all`, `cargo clippy -p <crates you touched> --all-targets -- -D warnings`, `cargo nextest run -p <those crates>`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`. Disk is limited: `rm -rf target/debug/incremental` after big builds.
- PC 1 questions (need the Windows game or Ghidra): add unnumbered items "- [q-fidelity-ledger] title" to docs/handoff/pc1-data.md Step 4; the coordinator numbers them.
- At the end write docs/handoff/q-fidelity-ledger.md (done / open / repro commands), push, send "done" once to the coordinator, then stop.
