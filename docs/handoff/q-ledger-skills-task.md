# Task: q-ledger-skills (from the coordinator, 2026-10-09)

You are session q-ledger-skills, one part of the fidelity ledger (coordinator plan: the whole ledger done by 20:15 UTC 2026-10-09). BRANCH = q-ledger-skills. REC ids: REC-1223..1225 only (you should need none). Model: Sonnet. Measurement and inventory only: do not change game logic.

HARD DEADLINE: push docs/handoff/ledger/skills.tsv by 19:40 UTC whatever its state (push early drafts every 20-30 min too). Rows you could not finish: state UNKNOWN with the reason in note. Then message the coordinator (row count, state counts) and the integrator session q-fidelity-ledger (session_01ENnbtJPdyvJmmtjyKpQDjE), say done, stop. Do NOT run the full setup in the RULES below unless your part needs it: skip game files and Bevy libs unless stated; no sync.sh needed (short task).

Your part: player skills and their missiles/states/auras. Rows: every player skill (skills.txt rows with charclass set: 7 classes x 30), plus hireling skills, item-granted skills (oskills, charges), states/auras (states.txt) grouped by state, missiles grouped by the skill that makes them. Truth: skills.txt / missiles.txt / states.txt (from the private repo MoggerCat/D2RUST-private-repo extracted/ excel .txt files: sparse clone `extracted`), specs/skills/, specs/missiles/. Checks: traces/checks per class prefix (ama-, sor-, nec-, pal-, bar-, dru-, ass-), classes.play milestones (docs/handoff/playability-matrix.md).

LEDGER ROW FORMAT (shared by every ledger session; fixed by the coordinator so nobody waits):
File: docs/handoff/ledger/<part>.tsv, UTF-8, tab-separated. Line 1 exactly: `#ledger 1`. Line 2 the header:
area	kind	group	source_1.14d	specs	spec_status	checks	last_verdict	exercised	provisional	needs_pc1	owner	state	size	note
- area: unique id, lowercase, dotted: e.g. `monster.andariel`, `skill.sor.frost-nova`, `item.unique.shako`, `net.s2c.0x4d`, `system.rng.unit-seed`.
- kind: entity | system | message | ui | content.
- group: the part name (monsters, skills, items, world, systems, coverage).
- source_1.14d: the table+row or function/address that is the truth (e.g. `monstats.txt Andariel (156)`, `0x00573B20`).
- specs: spec paths (comma-separated) or `-`; spec_status: as written in the spec header, or `none`.
- checks: traces/checks/*.check names covering it, or `-`; last_verdict: from checks-status.md (`git show origin/claude/q-fix-check-triage:docs/handoff/checks-status.md`): MATCH / DIVERGED@frame / PARTIAL / -.
- exercised: yes / no / ? (coverage sessions fill this; others write ?).
- provisional: count of PROVISIONAL/REC points (docs/handoff/provisional-index.tsv, grep) or 0.
- needs_pc1: y (needs a Windows recording or RE) / n.
- owner: session branch from tools/coord/owners.tsv or `-`.
- state: EQUAL (a passing 1.14d check) | DIVERGED | NO-CHECK (implemented, nothing compares it) | NOT-IMPLEMENTED | UNKNOWN (say why in note).
- size: S (<2 session-hours) | M (2-8) | L (>8) of work left to reach EQUAL; `-` for EQUAL.
- note: one line: what is missing / which tool would check it.
Granularity: one row per table row for bosses, superuniques, quests, NPCs, skills, uniques/sets/runewords may be grouped by type when they share one code path (say so in note); one row per message id; one row per system rule group (spec section).

RULES:
Coordinator: session_01KcnkwCTXbuv5ZbToEUpBSj (message it with send_message).

Setup: `git fetch origin claude/specs-staging-7 claude/q-ledger-skills && git checkout -B claude/q-ledger-skills origin/claude/q-ledger-skills && sh tools/coord/sync.sh` (merges the latest staging). Read CLAUDE.md, then only the sections of docs you need (docs/METHODS.md M01, M22, M23, M25 at least). Game files: attach MoggerCat/D2RUST-private-repo (add_repo), sparse-clone it per its README.md (`extracted tools install`), `python3 tools/assemble.py /home/user/game`, then `D2_GAME_DIR=/home/user/game`; build the excel view with `tools/realdata-gate.sh` if a test needs it. Never copy private files into the public repo. Bevy system libs: `apt-get install -y pkg-config libwayland-dev libasound2-dev libudev-dev libxkbcommon-dev`.

Session rules:
- Just push to claude/q-ledger-skills (git push -u origin claude/q-ledger-skills); the coordinator merges. No PRs.
- Message the coordinator only when a blocker is fixed (with the playthrough count: `python3 tools/playthrough/playthrough.py <file> --build` reached/total), when blocked, or when done.
- Stay in your area (tools/coord/owners.tsv; `python3 tools/coord/route.py <path>` names the owner of a file outside your area: send a note to that session or the coordinator instead of editing).
- Run `sh tools/coord/sync.sh` every 30 minutes.
- Read only what you need (specs by section index, M11).
- Exact match with 1.14d stays the bar (CLAUDE.md rule 10): fix the first divergence a check shows; never weaken a test or expect a bug. Unsettled choices are PROVISIONAL with your REC ids (M22).
- Before each push: `cargo fmt --all`, `cargo clippy -p <crates you touched> --all-targets -- -D warnings`, `cargo nextest run -p <those crates>`, `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check`. Disk is limited: `rm -rf target/debug/incremental` after big builds.
- PC 1 questions (need the Windows game or Ghidra): add unnumbered items "- [q-ledger-skills] title" to docs/handoff/pc1-data.md Step 4; the coordinator numbers them.
- At the end write docs/handoff/q-ledger-skills.md (done / open / repro commands), push, send "done" once to the coordinator, then stop.
