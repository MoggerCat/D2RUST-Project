# Task: q-tool-items-channel (from the coordinator, 2026-10-09)

You are session q-tool-items-channel of the D2RUST-Project cloud build loop (Rust reimplementation of Diablo II LoD 1.14d, held to exact match, CLAUDE.md rule 10). BRANCH = q-tool-items-channel. REC ids: REC-1310..1319 only. Part of the fidelity plan (docs/handoff/fidelity-gaps.md on claude/q-fidelity-ledger).

Build the scenario-diff `items` channel (gaps §4): for every item created during a check (drop, store fill, gamble, cube, quest), both sides write its creating event + the item bitstream / decoded fields (items/bitstream.md; d2rs d2_proto::item_bits; 1.14d: a recorder hook at item creation 0x00573B20-family or after the S->C 0x9C/0x9D item messages — choose what specs/ already pin), compared item by item in creation order, bytes. Spec: specs/tools/scenario-diff.md new channel section. Then author 3 checks: monster kill drops (poke spawn + kill), vendor stock (talk to Charsi needs interact-pokes: if not ready, use the store-fill on town arrival), gold/potion drop. Record verdicts. Opus.

Done means: the deliverable pushed, its own selftest/check green, a docs/handoff/q-tool-items-channel.md hand-back, and the ledger rows you settled updated in a part file docs/handoff/ledger/q-tool-items-channel.tsv (format: docs/handoff/q-ledger-monsters-task.md on claude/q-ledger-monsters; validate with the ledger.py from claude/q-fidelity-ledger). Other sessions own other areas (tools/coord/owners.tsv, docs/handoff/coordinator-resume.md on claude/coord-resume-3): route their bugs, don't fix them.

RULES:
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (message it with send_message).

Setup (one command, ~5-15 min, do it first and read while it runs): `git fetch origin claude/specs-staging-7 claude/integ-r3 claude/coord-resume-3 claude/q-fidelity-ledger claude/q-tool-items-channel && git checkout -B claude/q-tool-items-channel origin/claude/q-tool-items-channel && sh tools/coord/sync.sh && git merge -q origin/claude/integ-r3 && git show origin/claude/coord-resume-3:tools/coord/session-setup.sh > /tmp/session-setup.sh && sh /tmp/session-setup.sh` (attach MoggerCat/D2RUST-private-repo with add_repo first; it gives Bevy libs, cargo-nextest, the 1.14d install in $HOME/game + excel view, Wine + recorder saves; `--no-wine` if you never run 1.14d). The plan you belong to: `git show origin/claude/q-fidelity-ledger:docs/handoff/fidelity-gaps.md` (read only your section) and the ledger rows `git show origin/claude/q-fidelity-ledger:docs/handoff/fidelity-ledger.tsv`. Read CLAUDE.md, then only what you need (docs/METHODS.md M01, M22, M23, M25). Never copy private files into the public repo.

Session rules:
- Just push to claude/q-tool-items-channel (git push -u origin claude/q-tool-items-channel); the coordinator merges. No PRs.
- Message the coordinator only when a blocker is fixed (with the playthrough count: `python3 tools/playthrough/playthrough.py <file> --build` reached/total), when blocked, or when done.
- Stay in your area (tools/coord/owners.tsv; `python3 tools/coord/route.py <path>` names the owner of a file outside your area: send a note to that session or the coordinator instead of editing).
- Run `sh tools/coord/sync.sh` every 30 minutes.
- Read only what you need (specs by section index, M11).
- Exact match with 1.14d stays the bar (CLAUDE.md rule 10): fix the first divergence a check shows; never weaken a test or expect a bug. Unsettled choices are PROVISIONAL with your REC ids (M22).
- Before each push: `cargo fmt --all`, `cargo clippy -p <crates you touched> --all-targets -- -D warnings`, `cargo nextest run -p <those crates>`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`. Disk is limited: `rm -rf target/debug/incremental` after big builds.
- PC 1 questions (need the Windows game or Ghidra): add unnumbered items "- [q-tool-items-channel] title" to docs/handoff/pc1-data.md Step 4; the coordinator numbers them.
- At the end write docs/handoff/q-tool-items-channel.md (done / open / repro commands), push, send "done" once to the coordinator, then stop.
