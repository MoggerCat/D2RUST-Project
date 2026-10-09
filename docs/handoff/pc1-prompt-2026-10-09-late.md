# PC 1 prompt (coordinator session_01KcnkwCTXbuv5ZbToEUpBSj, 2026-10-09 late)

Paste the block below into a new local Claude Code session on PC 1 (Windows,
in the D2RUST-Project checkout, with `re/`, `../refs/` and `game/`).
Follows `pc1-prompt-2026-10-09-night.md` (its A, B, D, E and items 62-64 are
done, see `docs/handoff/pc1-night.md` on `claude/local-pc1-night`). If the
night session is still running, start this one only for binary reads and
wait for `%TEMP%\d2-game.lock` before any recording.

---

You are PC 1 for the D2RUST-Project build loop (Rust reimplementation of Diablo II LoD 1.14d, exact match: CLAUDE.md rule 10). Coordinator: cloud session session_01KcnkwCTXbuv5ZbToEUpBSj (not reachable from PC 1: your hand-back file stands in for messages). About 80 cloud sessions fix and compare against 1.14d under Wine; you do only what the cloud cannot: read the 1.14d binary (Ghidra, `re/`), read `../refs/`, and record on the real Windows game. This is a SPEC-WRITING session (CLAUDE.md hard rule 3): you may read `re/` and `../refs/`; you write specs, facts and traces, never Rust in `crates/`.

Setup:
1. `git fetch origin && git checkout -B claude/local-pc1-late origin/claude/specs-staging-7 && git merge origin/claude/integ-r9 origin/claude/local-pc1-night origin/claude/coord-resume-3` (newest fixes, the night's answers, the coordinator's notes).
2. Read CLAUDE.md, docs/METHODS.md (M01, M03, M04, M22, M25), then `docs/handoff/pc1-data.md` (top: one `Game.exe` at a time, `%TEMP%\d2-game.lock`; "Set up any state for a check"; Step 4) and `docs/handoff/pc1-night.md`.
3. Collect open questions cloud sessions left on their own branches: `git fetch origin "+refs/heads/claude/q-*:refs/remotes/origin/claude/q-*"` then `for b in $(git branch -r --list "origin/claude/q-*"); do git show $b:docs/handoff/pc1-data.md 2>/dev/null | grep "^- \[q-" | sed "s|^|$b: |"; done | sort -u` and keep the ones not already answered in your pc1-data.md.

Work, in this order (highest cloud impact first). Each names the cloud session waiting on it:
A. Act III-V host paths (q-fix-a3a5-hosts): (1) monster secondary target `0x005DDC30`: which units it scans (players, hirelings, pets, monsters), order, range, every RNG draw, and when its result replaces the main target, into `specs/monsters/ai.md` (recorded need: baal-throne / worldstone frame 50); (2) the unnumbered items "monster vision record +0x50/+0x24" and "Compelling Orb spawn spread/flags" in pc1-data.md Step 4 if the night did not answer them; (3) Larzuk's map-AI nodes: where 1.14d builds them (the call that d2rs's `larzuk_map_ai` stands for), into the Act V NPC spec (recorded need: act5-entry frame 26).
B. Monster think draws (q-fix-monster-ai-2): combat-pop-cold-plains frame 7, Fallen Shaman (class 58): 1.14d steps the unit seed 4 times per think, d2rs 3; the missing one is the inline step at `0x005DF7DB`, after the tactics step. Read the Shaman's and the Fallen's think in full (every unit-seed step in order, with the condition for each) into `specs/monsters/ai*.md`.
C. Quest state on load (q-fix-quest-load): when a save joins, how 1.14d copies the save's quest records into the game's quest state (which acts, which bits kept/cleared/derived, in what order, which events fire), into `specs/world/quests.md` §1. Recorded need: join-act2-quests-ama frame 2 (1.14d holds the act 1 words, d2rs only [[7,1]]).
D. Items timing and flags (q-chk-items-drops, q-fix-npc-interact): (1) normal monster drops reach the client one frame later in d2rs: from the binary, the frame (relative to the death mode start) at which 1.14d rolls and sends a monster's drop, into `specs/items/treasure.md` / `monsters` death spec; (2) gamble list items: 1.14d sends byte 5 `00 12`, d2rs `10 00` (mode / location bits): what 1.14d sets for an item placed in a gamble list, into `specs/world/vendors.md` §5.
E. NPC wander (q-fix-npc-interact): town-ama-10k frame 287, Warriv (class 155) wander target ty 4228 (d2rs) vs 4229 (1.14d) with equal unit seeds: read the wander target computation (rounding/offset) into the town NPC spec.
F. Audio (q-fix-audio): (1) REC-1363, the Windows run in pc1-data.md Step 4 ([q-tool-audio-diff] item); (2) from the binary, the sound tick a voice started in the first tick gets (T 0 vs T 1, `specs/audio/sound-table.md` §6.1), and who plays `cursor\windowopen.wav` at T 0.
G. Every other unnumbered `- [session] title` in Step 4/5: number them from 65 on, answer each into the spec it names (binary reads first, then recordings).

Rules: push to `claude/local-pc1-late` after each answered item (the coordinator merges); commit messages name the item letter/number. Mark each answered item in pc1-data.md "answered → <spec §>"; for each answer that changes d2rs behaviour add a build-queue row (`docs/handoff/build-queue.tsv`). Never commit Blizzard files (pre-commit hook). Keep `docs/handoff/pc1-late.md` as the hand-back (same layout as pc1-night.md; push A first, as soon as it is done).
