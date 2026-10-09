# Task: q-chk-skills-bda (from the coordinator, 2026-10-09)

You are session q-chk-skills-bda of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, held to exact match, CLAUDE.md rule 10). BRANCH = q-chk-skills-bda. REC ids: REC-1520..1524 only. Part of the fidelity plan (docs/handoff/fidelity-gaps.md on claude/q-fidelity-ledger).

T3 checks: barbarian, druid, assassin skills — 22 checks exist; rerun them on current staging, then add one per remaining skill of those 3 classes (the existing pattern) and passive/synergy cases (a save with synergies). q-chk-skills-4cls covers the other four classes. Run under Wine, record verdicts, route (skills owners: q-fix-class-rows, q-fix-ass-traps).

Done means: deliverable pushed, a docs/handoff/q-chk-skills-bda.md hand-back, and every ledger row you settled in docs/handoff/ledger/q-chk-skills-bda.tsv (format: docs/handoff/q-ledger-monsters-task.md on claude/q-ledger-monsters; validate with ledger.py from claude/q-fidelity-ledger). A CHECK session authors checks, runs them under Wine (tools/scenario-diff), records verdicts and routes each first divergence (owners.tsv + docs/handoff/coordinator-resume.md on claude/coord-resume-3; unowned ones: a new row in docs/handoff/build-queue.tsv and a line to the coordinator); it fixes only check-authoring problems. Interact with objects/NPCs today with `poke <t> msg 0x13 <type> <guid>` (C->S 0x13, as act4.play does) until q-tool-interact-pokes lands. Generated checks: adopt q-tool-check-gen's generator when it lands (claude/q-tool-check-gen).

RULES:
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (message it with send_message).

Setup (one command, ~5-15 min, do it first and read while it runs): `git fetch origin claude/specs-staging-7 claude/integ-r3 claude/coord-resume-3 claude/q-fidelity-ledger claude/q-chk-skills-bda && git checkout -B claude/q-chk-skills-bda origin/claude/q-chk-skills-bda && sh tools/coord/sync.sh && git merge -q origin/claude/integ-r3 && git show origin/claude/coord-resume-3:tools/coord/session-setup.sh > /tmp/session-setup.sh && sh /tmp/session-setup.sh` (attach MoggerCat/D2RUST-private-repo with add_repo first; it gives Bevy libs, cargo-nextest, the 1.14d install in $HOME/game + excel view, Wine + recorder saves; `--no-wine` if you never run 1.14d). The plan you belong to: `git show origin/claude/q-fidelity-ledger:docs/handoff/fidelity-gaps.md` (read only your section) and the ledger rows `git show origin/claude/q-fidelity-ledger:docs/handoff/fidelity-ledger.tsv`. Read CLAUDE.md, then only what you need (docs/METHODS.md M01, M22, M23, M25). Never copy private files into the public repo.

Session rules:
- Just push to claude/q-chk-skills-bda (git push -u origin claude/q-chk-skills-bda); the coordinator merges. No PRs.
- Message the coordinator only when a blocker is fixed (with the playthrough count: `python3 tools/playthrough/playthrough.py <file> --build` reached/total), when blocked, or when done.
- Stay in your area (tools/coord/owners.tsv; `python3 tools/coord/route.py <path>` names the owner of a file outside your area: send a note to that session or the coordinator instead of editing).
- Run `sh tools/coord/sync.sh` every 30 minutes.
- Read only what you need (specs by section index, M11).
- Exact match with 1.14d stays the bar (CLAUDE.md rule 10): fix the first divergence a check shows; never weaken a test or expect a bug. Unsettled choices are PROVISIONAL with your REC ids (M22).
- Before each push: `cargo fmt --all`, `cargo clippy -p <crates you touched> --all-targets -- -D warnings`, `cargo nextest run -p <those crates>`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`. Disk is limited: `rm -rf target/debug/incremental` after big builds.
- PC 1 questions (need the Windows game or Ghidra): add unnumbered items "- [q-chk-skills-bda] title" to docs/handoff/pc1-data.md Step 4; the coordinator numbers them.
- At the end write docs/handoff/q-chk-skills-bda.md (done / open / repro commands), push, send "done" once to the coordinator, then stop.
