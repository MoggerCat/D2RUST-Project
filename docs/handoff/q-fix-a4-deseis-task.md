# Task: q-fix-a4-deseis

You are session q-fix-a4-deseis of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, exact match). BRANCH = q-fix-a4-deseis. REC ids: REC-1440..1444 only.

Task: in the Chaos Sanctuary (level 108) the De Seis seal (object 394) never opens when the level seed picks the WingN1 arm file (seeds 1-4, 6-9, 11): its boss spot (-39,+33) lies 6 sub-tiles outside the DS1 and the free-spot search ends on a spot with no room, so the seal stays shut and Diablo can never be reached in real play. Found by q-fix-act4-play (docs/handoff/q-fix-act4-play.md on claude/q-fix-act4-play; the act4.play Diablo milestone runs on seed 5 only because of this). Find what 1.14d does with that spot (specs: drlg/preset.md, sim/path-placement.md free-point search, world/quests-act4*.md seal bosses); fix; prove with the Diablo milestone on seeds 1-11 (playthrough with --seed or a copy of the milestone) and, under Wine, a scenario-diff check that warps to 108 with seed 1 and compares the seal boss spawn. If a spec fact is missing, PROVISIONAL (M22) + pc1-data Step 4 item (q-fix-act4-play already added one: extend it, don't duplicate).

RULES:
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (message it with send_message).

Setup (one command, ~5-15 min, do it first and read while it runs): `git fetch origin claude/specs-staging-7 claude/integ-r3 claude/coord-resume-3 claude/q-fidelity-ledger claude/q-fix-a4-deseis && git checkout -B claude/q-fix-a4-deseis origin/claude/q-fix-a4-deseis && sh tools/coord/sync.sh && git merge -q origin/claude/integ-r3 && git show origin/claude/coord-resume-3:tools/coord/session-setup.sh > /tmp/session-setup.sh && sh /tmp/session-setup.sh` (attach MoggerCat/D2RUST-private-repo with add_repo first; it gives Bevy libs, cargo-nextest, the 1.14d install in $HOME/game + excel view, Wine + recorder saves; `--no-wine` if you never run 1.14d). The plan you belong to: `git show origin/claude/q-fidelity-ledger:docs/handoff/fidelity-gaps.md` (read only your section) and the ledger rows `git show origin/claude/q-fidelity-ledger:docs/handoff/fidelity-ledger.tsv`. Read CLAUDE.md, then only what you need (docs/METHODS.md M01, M22, M23, M25). Never copy private files into the public repo.

Session rules:
- Just push to claude/q-fix-a4-deseis (git push -u origin claude/q-fix-a4-deseis); the coordinator merges. No PRs.
- Message the coordinator only when a blocker is fixed (with the playthrough count: `python3 tools/playthrough/playthrough.py <file> --build` reached/total), when blocked, or when done.
- Stay in your area (tools/coord/owners.tsv; `python3 tools/coord/route.py <path>` names the owner of a file outside your area: send a note to that session or the coordinator instead of editing).
- Run `sh tools/coord/sync.sh` every 30 minutes.
- Read only what you need (specs by section index, M11).
- Exact match with 1.14d stays the bar (CLAUDE.md rule 10): fix the first divergence a check shows; never weaken a test or expect a bug. Unsettled choices are PROVISIONAL with your REC ids (M22).
- Before each push: `cargo fmt --all`, `cargo clippy -p <crates you touched> --all-targets -- -D warnings`, `cargo nextest run -p <those crates>`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`. Disk is limited: `rm -rf target/debug/incremental` after big builds.
- PC 1 questions (need the Windows game or Ghidra): add unnumbered items "- [q-fix-a4-deseis] title" to docs/handoff/pc1-data.md Step 4; the coordinator numbers them.
- At the end write docs/handoff/q-fix-a4-deseis.md (done / open / repro commands), push, send "done" once to the coordinator, then stop.
