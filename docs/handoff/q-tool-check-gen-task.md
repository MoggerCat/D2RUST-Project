# Task: q-tool-check-gen (from the coordinator, 2026-10-09)

You are session q-tool-check-gen of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, held to exact match, CLAUDE.md rule 10). BRANCH = q-tool-check-gen. REC ids: REC-1330..1339 only. Part of the fidelity plan (docs/handoff/fidelity-gaps.md on claude/q-fidelity-ledger).

Build `check-gen` (gaps §4): a generator that writes .check files from the 1.14d tables + existing patterns: one warp check per level (the a*-warp-* pattern, poke warp N), one per waypoint, one spawn check per monster AI type and per boss/superunique (poke spawn / superunique, traces/pokes/boss-kinds.poke), one cast check per class skill (the dru-*/bar-* pattern with a save having all skills 20), one per shrine type. Generated files go under traces/checks/gen/ with a header naming the generator + row. Selftest. Then generate the full set, run the d2rs side of a sample of each family to prove they run, and hand the families to the T3 check sessions (q-chk-*) via docs/handoff/q-tool-check-gen.md. Sonnet. Deliver the generator within ~2 hours: the q-chk-* sessions wait on it.

Done means: the deliverable pushed, its own selftest/check green, a docs/handoff/q-tool-check-gen.md hand-back, and the ledger rows you settled updated in a part file docs/handoff/ledger/q-tool-check-gen.tsv (format: docs/handoff/q-ledger-monsters-task.md on claude/q-ledger-monsters; validate with the ledger.py from claude/q-fidelity-ledger). Other sessions own other areas (tools/coord/owners.tsv, docs/handoff/coordinator-resume.md on claude/coord-resume-3): route their bugs, don't fix them.

RULES:
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (message it with send_message).

Setup (one command, ~5-15 min, do it first and read while it runs): `git fetch origin claude/specs-staging-7 claude/integ-r3 claude/coord-resume-3 claude/q-fidelity-ledger claude/q-tool-check-gen && git checkout -B claude/q-tool-check-gen origin/claude/q-tool-check-gen && sh tools/coord/sync.sh && git merge -q origin/claude/integ-r3 && git show origin/claude/coord-resume-3:tools/coord/session-setup.sh > /tmp/session-setup.sh && sh /tmp/session-setup.sh` (attach MoggerCat/D2RUST-private-repo with add_repo first; it gives Bevy libs, cargo-nextest, the 1.14d install in $HOME/game + excel view, Wine + recorder saves; `--no-wine` if you never run 1.14d). The plan you belong to: `git show origin/claude/q-fidelity-ledger:docs/handoff/fidelity-gaps.md` (read only your section) and the ledger rows `git show origin/claude/q-fidelity-ledger:docs/handoff/fidelity-ledger.tsv`. Read CLAUDE.md, then only what you need (docs/METHODS.md M01, M22, M23, M25). Never copy private files into the public repo.

Session rules:
- Just push to claude/q-tool-check-gen (git push -u origin claude/q-tool-check-gen); the coordinator merges. No PRs.
- Message the coordinator only when a blocker is fixed (with the playthrough count: `python3 tools/playthrough/playthrough.py <file> --build` reached/total), when blocked, or when done.
- Stay in your area (tools/coord/owners.tsv; `python3 tools/coord/route.py <path>` names the owner of a file outside your area: send a note to that session or the coordinator instead of editing).
- Run `sh tools/coord/sync.sh` every 30 minutes.
- Read only what you need (specs by section index, M11).
- Exact match with 1.14d stays the bar (CLAUDE.md rule 10): fix the first divergence a check shows; never weaken a test or expect a bug. Unsettled choices are PROVISIONAL with your REC ids (M22).
- Before each push: `cargo fmt --all`, `cargo clippy -p <crates you touched> --all-targets -- -D warnings`, `cargo nextest run -p <those crates>`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`. Disk is limited: `rm -rf target/debug/incremental` after big builds.
- PC 1 questions (need the Windows game or Ghidra): add unnumbered items "- [q-tool-check-gen] title" to docs/handoff/pc1-data.md Step 4; the coordinator numbers them.
- At the end write docs/handoff/q-tool-check-gen.md (done / open / repro commands), push, send "done" once to the coordinator, then stop.
