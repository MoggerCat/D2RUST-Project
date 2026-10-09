# Task: q-fix-ui-blend

You are session q-fix-ui-blend of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, exact match). BRANCH = q-fix-ui-blend. REC ids: REC-1445..1449 only.

Task: 15 real-data tests fail with one cause found by q-fix-realdata-baseline (docs/handoff/q-fix-realdata-baseline.md on claude/q-fix-realdata-baseline): UI draw mode 2 has no act blend tables on the front-end UI path (crates/d2-client world_view/ui_bind.rs ui_cel_ops). Failing: app_input_pass x4, smoke_frontend x5, and probably app_play_640, app_play_e2e, app_play_visibility, app_hud_e2e, app_levelup_ui, app_frame_loop. Specs: render/blend-modes.md, render/shading.md, client/render-pipeline.md, ui/ (front end palettes). Fix per spec; prove with those tests (`cargo nextest run -p d2-client --run-ignored only -E '<names>'` with D2_GAME_DIR) and, where a side-by-side UI scene exists (tools/sidebyside), pixel-equal frames. Then rerun `python3 tools/coord/realdata.py` (from claude/q-fix-realdata-baseline) and report the new pass count.

RULES:
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (message it with send_message).

Setup (one command, ~5-15 min, do it first and read while it runs): `git fetch origin claude/specs-staging-7 claude/integ-r3 claude/coord-resume-3 claude/q-fidelity-ledger claude/q-fix-ui-blend && git checkout -B claude/q-fix-ui-blend origin/claude/q-fix-ui-blend && sh tools/coord/sync.sh && git merge -q origin/claude/integ-r3 && git show origin/claude/coord-resume-3:tools/coord/session-setup.sh > /tmp/session-setup.sh && sh /tmp/session-setup.sh` (attach MoggerCat/D2RUST-private-repo with add_repo first; it gives Bevy libs, cargo-nextest, the 1.14d install in $HOME/game + excel view, Wine + recorder saves; `--no-wine` if you never run 1.14d). The plan you belong to: `git show origin/claude/q-fidelity-ledger:docs/handoff/fidelity-gaps.md` (read only your section) and the ledger rows `git show origin/claude/q-fidelity-ledger:docs/handoff/fidelity-ledger.tsv`. Read CLAUDE.md, then only what you need (docs/METHODS.md M01, M22, M23, M25). Never copy private files into the public repo.

Session rules:
- Just push to claude/q-fix-ui-blend (git push -u origin claude/q-fix-ui-blend); the coordinator merges. No PRs.
- Message the coordinator only when a blocker is fixed (with the playthrough count: `python3 tools/playthrough/playthrough.py <file> --build` reached/total), when blocked, or when done.
- Stay in your area (tools/coord/owners.tsv; `python3 tools/coord/route.py <path>` names the owner of a file outside your area: send a note to that session or the coordinator instead of editing).
- Run `sh tools/coord/sync.sh` every 30 minutes.
- Read only what you need (specs by section index, M11).
- Exact match with 1.14d stays the bar (CLAUDE.md rule 10): fix the first divergence a check shows; never weaken a test or expect a bug. Unsettled choices are PROVISIONAL with your REC ids (M22).
- Before each push: `cargo fmt --all`, `cargo clippy -p <crates you touched> --all-targets -- -D warnings`, `cargo nextest run -p <those crates>`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`. Disk is limited: `rm -rf target/debug/incremental` after big builds.
- PC 1 questions (need the Windows game or Ghidra): add unnumbered items "- [q-fix-ui-blend] title" to docs/handoff/pc1-data.md Step 4; the coordinator numbers them.
- At the end write docs/handoff/q-fix-ui-blend.md (done / open / repro commands), push, send "done" once to the coordinator, then stop.
