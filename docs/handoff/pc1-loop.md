# PC 1 loop: settle what the fast cloud batches leave provisional

Started 2026-10-08 (third coordinator, session_01QeN5r8PoLZsUhwAH2iDzLJ).
The cloud runs 20+ stitching sessions at once (`coordinator-resume.md`).
Their gate keeps the build green, but every spec gap they meet becomes a
`PROVISIONAL` point (M22) with a REC id. PC 1 turns those points into
verified ones, at the pace the cloud produces them. That is where
stability reaches the fast-built parts.

PC 1 is a **spec-writing / local-check** machine (CLAUDE.md "Where work
runs"): it may read `re/`, `../refs/`, `game/`. It does **not** write
code in `crates/` beyond the `// Covers:` claim edits of
`docs/LOCAL-RUN.md` §0.2. A code fix found here becomes a row in
`docs/handoff/build-queue.tsv` (the coordinator launches it) or a line in
the owning spec. Recordings that need a player are PC 2's
(`docs/HANDOFF.md` §7 "PC 2 recording list"): PC 1 only adds entries there.

## Branch and hand-back

- Work on `claude/local-pc1-s8` (create it from `origin/claude/specs-staging-7`).
  Never push to `claude/specs-staging-7` or `main`: the coordinator
  merges your branch into staging between its own merges.
- Before each push: `git fetch origin claude/specs-staging-7 && git merge
  origin/claude/specs-staging-7` (merge, never rebase), then
  `py tools/spec_index.py` (refresh), `py tools/spec_index.py --check`,
  `py tools/coverage.py --check`, `py tools/methods.py check`.
- Push after every finished lane item (small pushes merge cleanly). After
  each push, if the `send_message` tool (claude-code-remote) is
  available, send session_01QeN5r8PoLZsUhwAH2iDzLJ one line: branch, head
  SHA, what changed (spec files, REC ids settled, new queue rows). If it
  is not available, just push; the coordinator polls the branch.
- REC ids: the coordinator hands them out. PC 1 takes new ids from the
  block **REC-300..REC-349** only (cloud sessions use REC-187..299).

## Lanes (run them in parallel with subagents, up to 6 at once)

Model / effort: the user set **medium** for this loop (speed over
depth). Raise one worker to high only for an exactness mismatch the
first pass did not solve (M15). One worker per spec file at a time (M06);
two workers never edit the same spec.

### Lane A — provisional points answerable from the binary (first priority)

1. List the points: `git grep -n "PROVISIONAL" specs crates` on staging,
   plus HANDOFF §7 REC entries for REC-160 and up (the recent batches).
2. Keep only points the 1.14d binary can answer (a function body, a
   table, a constant, a message layout). Points that need a capture go
   to Lane C.
3. Highest first: points that cloud sessions are building on **right
   now** (`coordinator-resume.md` running batches). Current list:
   - REC-177 (3): the weapon-switch C→S 0x60 body `0x005616A0`
     (`sim/intents-events.md` OQ16): requirements recheck, durability,
     which locations trade, messages sent.
   - REC-188 / `client/stat-lists.md` OQ2: which property lists of an
     item (base, magic, set, runeword) the client attaches on equip, and
     the set-list rule (`items/bitstream.md` §4.6, set states 165–169).
   - REC-237 (old REC-QESC-1): Esc game menu art file, frames, entry
     rects, string ids, what Options opens (`ui/panels.md` §3.1,
     `ui/frontend-options.md`).
   - REC-232: `skills/use.md` §5.2 (Whirlwind's per-step hits) — read
     the binary first; only if it stays open, Lane C.
   - REC-233: Lightning Sentry / trap AI and shot count; dual-claw
     second-hand damage.
   - REC-176, REC-173, REC-174, REC-175 (skill gaps, skill moves,
     Tyrael, Harrogath) and REC-180..186 (menus).
4. For each: write the rule into its owner spec with addresses (M03,
   M04: prose and tables, no decompiler output), mark the open question
   answered, replace the `PROVISIONAL:` line with the rule. If the
   implemented code now disagrees with the spec, add a build-queue row
   `q-fix-<topic>` naming the spec rule and the code file. If the code
   already matches, say so in the hand-back so the coordinator can drop
   the `// PROVISIONAL` comment.

### Lane B — local run queue (game files and GPU, no player)

`docs/LOCAL-RUN.md` Batches 1, 2, 3 and 5, then the entries of
`docs/HANDOFF.md` §5 that need only `game/` and the GPU (section C), then
the `docs/handoff/q-*.md` "Local check" blocks of branches already merged
into staging (no player needed: commands that print counts, verify, render
compares). Record per `LOCAL-RUN.md` §0.2 (Done block, `// Covers:`
unlocks, coverage figures). A failure is a finding for the owner spec
and, if code, a `q-fix-<topic>` row; never edit the expected value to pass.

### Lane C — recordings PC 2 needs

For each provisional point that only a capture settles, add or extend an
entry in HANDOFF §7 "PC 2 recording list" (what to capture, the
`Settles:` REC ids, [AUTO] / [MANUAL]). Put the ones on choices that
spread silently (RNG draw order, wire byte layouts) first (M22). Current
must-haves: equipping +stat / +resist gear (0x9C/0x9D and 0x1D after,
REC-163 / REC-177 / REC-188), Whirlwind through a pack (REC-232 if Lane A
leaves it open), W with two weapon sets (0x60, 0x9D moves, 0x97).

### Lane D — spec gaps the next cloud batches need (when A is empty)

`docs/handoff/pc1-spec-gaps.md` **bin** items, in the areas the build
queue touches next (client messages, skills, items, front end). Each
answered question that unblocks a feature gets a build-queue row with the
spec section, so the coordinator can launch it at once.

## Done for a round

A round is one pass over Lane A's current list plus whatever Lanes B–D
finished meanwhile. Write `docs/handoff/pc1-s8.md` (per lane: settled
REC ids, spec files changed, queue rows added, local-check results,
what is left), push, hand back, then start the next round from a fresh
`git fetch` (the cloud adds new PROVISIONAL points every hour). Stop when
Lane A and B are empty or the user stops you.
