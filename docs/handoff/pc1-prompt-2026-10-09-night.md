# PC 1 prompt (coordinator session_01KcnkwCTXbuv5ZbToEUpBSj, 2026-10-09 night)

Paste the block below into a new local Claude Code session on PC 1 (Windows,
in the D2RUST-Project checkout, with `re/`, `../refs/` and `game/`).
Follows `pc1-prompt-2026-10-09.md` (evening; its A–C are done, see
`docs/handoff/pc1-eve.md`).

---

You are PC 1 for the D2RUST-Project build loop (Rust reimplementation of Diablo II LoD 1.14d, exact match: CLAUDE.md rule 10). Coordinator: cloud session session_01KcnkwCTXbuv5ZbToEUpBSj (not reachable from PC 1: your hand-back file stands in for messages). About 70 cloud sessions fix and compare against 1.14d under Wine; you do only what the cloud cannot: read the 1.14d binary (Ghidra, `re/`), read `../refs/`, and record on the real Windows game. This is a SPEC-WRITING session (CLAUDE.md hard rule 3): you may read `re/` and `../refs/`; you write specs, facts and traces, never Rust in `crates/`.

Setup:
1. `git fetch origin && git checkout -B claude/local-pc1-night origin/claude/specs-staging-7 && git merge origin/claude/integ-r7 origin/claude/coord-resume-3` (newest fixes incl. your evening work, the coordinator's notes).
2. Read CLAUDE.md, docs/METHODS.md (M01, M03, M04, M22, M25), then `docs/handoff/pc1-data.md` (top: one `Game.exe` at a time, `%TEMP%\d2-game.lock`; "Set up any state for a check"; Step 4) and `docs/handoff/pc1-eve.md`.
3. Collect the open questions cloud sessions left on their own branches: `git fetch origin "+refs/heads/claude/q-*:refs/remotes/origin/claude/q-*"` then `for b in $(git branch -r --list "origin/claude/q-*"); do git show $b:docs/handoff/pc1-data.md 2>/dev/null | grep "^- \[q-" | sed "s|^|$b: |"; done | sort -u` and keep the ones not already in your pc1-data.md.

Work, in this order (highest cloud impact first):
A. Hireling creation (cloud session q-fix-hire-create waits; it blocks all 15 hireling checks): from the binary, what 1.14d does at C->S 0x36 (hire): every RNG draw in order (game seed and unit seed, which function, which site address), and how the hireling's spawn spot is chosen (start point, search order, which collision bits). Write it into `specs/world/hirelings*.md` with provenance. Recording optional (state + rng channels of a hire in Act I).
B. Game-seed divergences (cloud session q-fix-seed-game): the GAME seed differs from about frame 4 in the state checks and at frames 5, 21, 28, 34, 50 in combat-*, ass-fire-blast, dru-volcano, milestone-izual. Your evening read showed monster creation draws are equal (the frame-2 report was attribution). Read which 1.14d callers draw from the game seed (not unit seeds) in the first frames after join and in a melee/missile hit, in order, into the spec that owns each caller (`specs/sim/rng.md` has the list format).
C. Every question collected in setup step 3 and every unnumbered `- [session] title` in Step 4/5: number them from 62 on (61 is the last used), answer each into the spec it names (binary reads first, then recordings). Known so far: [q-fix-d9-arcane] A2Q4 event 3 (`0x0059F0C0`): is the old-level-40 handling skipped when the new level is 74 (REC-1405)?
D. Two tool addresses cloud sessions need (binary reads): (1) the player's quest flag record in 1.14d (address/layout of the quest flags the state recorder must read; cloud session q-tool-quest-flags), into specs/tools/state-trace*.md; (2) the object-interact walk `0x00548A50` (objects.md §7.3 rule 4): when C->S 0x13 targets an object out of range, what it does (path, range, when the operate fires), into world/objects.md §7.3 (cloud session q-fix-npc-interact; interact-operate-stash diverges there).
E. Settle REC-1150 (handler at the tick-return stop vs drained next frame) if a recording can show it; and any remaining PROVISIONAL that a binary read settles (`grep -rn "PROVISIONAL" specs | grep -i "ghidra\|binary\|address"`).

Rules: push to `claude/local-pc1-night` after each answered item (the coordinator merges); commit messages name the item numbers. Mark each answered item in pc1-data.md "answered → <spec §>"; for each answer that changes d2rs behaviour add a build-queue row (`docs/handoff/build-queue.tsv`). Never commit Blizzard files (pre-commit hook). Keep `docs/handoff/pc1-night.md` as the hand-back (same layout as pc1-eve.md; A first, as soon as it is done, pushed).
