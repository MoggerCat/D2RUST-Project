# Coordinator resume — playable-build loop (2026-10-08)

Everything a new coordinator session (any account) needs to pick up the
overnight build loop. The loop itself is in the repo: the shared session
rules are `docs/handoff/build-loop.md`, the task rows are
`docs/handoff/build-queue.tsv`, and each finished task left
`docs/handoff/q-<task>.md` (done / left / local check).

## State at hand-over (2026-10-08 ~09:20 UTC, second account switch)

- `main`: includes PR #50 (everything merged in this file; staging = main + this doc).
- `claude/specs-staging-7`: green (6,855 tests sim/server/client/
  test-fixtures). Merged after #48: q-mercs-acts, q-paladin,
  q-item-uniques, q-weapon-combat, q-assassin, q-gamble, q-barb,
  q-charms, q-a4-endgame, q-a2-duriel, pc1-frontend (menu specs + 10
  q-menu rows), q-render-polish, q-wp-warp, q-levels-warps-all,
  q-menu-shell, q-config, q-perf, q-light-radius-detail. If a snapshot
  PR (`claude/release-8c`) is open, it carries these; merge it on green.
- Also merged (after PR #49 opened; not in it): q-a2-tyrael, q-town-gaps,
  q-menu-host, q-menu-loading, q-menu-difficulty, q-menu-create,
  q-menu-cinematics, q-menu-controls (6,900 tests).
- **Check first (logged test change, q-town-gaps merge):** q-charms
  (REC-163) sent item-bonus totals in the base stat messages 0x1D;
  q-town-gaps changed that to base-only 0x1D plus bonus lists on pseudo
  states 0xFE/0xFD (0xA9), and rewrote the two vitals_sync tests to
  match. Neither is the original's wire behavior (the 1.14d client sums
  item stats itself): settle it against `specs/` and a trace, then fix
  one way. The frame-loop / single-player packet counts in the same merge
  only add Gheed's and Charsi's packets.
- Also merged: q-skill-moves, q-skill-gaps, q-a4-harrogath, q-menu-main,
  q-menu-charselect, q-menu-credits (6,943 tests). No sessions are
  running at hand-over; every launched task is merged.
- Open items the last sessions reported (queue them as rows):
  - front end: the shared draw() ignores pressed state and has no fire
    overlay; the host does not yet draw credits rows, create's hero
    DrawItems or the controls draw_list; Options needs an arm into
    CONTROLS; difficulty/char-select/host wiring of the 0x67 start flags;
    loading screen not wired to the bridge; text drawn as bars only
  - skills: Whirlwind hits along the path (needs a trace for use.md
    §5.2), Leap landing damage, Dragon Talon kick damage, Lightning
    Sentry's missiles, dual claws
  - Act II: Tyrael's door (153), Tyrael/Duriel placed by the world
  - Act IV: soulstone / hammer / gem item rows (needs synthetic
    ItemTables + InvTables in GameParts::synthetic: its own session)
- Queue row not started: `q-menu-options` (needs q-config, merged; it
  replaces q-config's simple Esc Options page with the full menu tree).
  Use REC-187.

## REC ids

Staging uses up to REC-186 plus REC-230 (REC-231 was renumbered away everywhere) (q-levels-warps-all); the front-
end specs use REC-200..213 and REC-220..229. 164, 169 and 171 were
reserved and never used. Next free: REC-187 (then 188, 189, 231+). Every menu session picked REC-231 (the next number after 230): tell sessions their id explicitly.
Sessions often pick a taken id; on merge, renumber only the branch's new
lines with `python3 tools/coord/renum.py REC-OLD REC-NEW` (run during an
uncommitted merge, from the repo root).

## Merge recipe (per branch)

```
git fetch origin claude/specs-staging-7 claude/<task>
git checkout claude/specs-staging-7 && git merge --ff-only origin/claude/specs-staging-7
git merge --no-ff --no-commit origin/claude/<task>
python3 tools/coord/union.py docs/HANDOFF.md   # HANDOFF §7 conflicts are always "keep both"
# other conflicts: resolve per hunk (usually both sides; check braces)
git diff HEAD | grep -E '^-\s*assert'           # any removed assertion must cite a spec rule
CARGO_INCREMENTAL=0 cargo clippy -p d2-sim -p d2-server -p d2-client -p test-fixtures --all-targets -- -D warnings
CARGO_INCREMENTAL=0 cargo nextest run -p d2-sim -p d2-server -p d2-client -p test-fixtures --no-fail-fast
python3 tools/coverage.py --check && python3 tools/spec_index.py --check
git commit (message: what was resolved, REC renumbering, logged test changes) && git push
```

Never weaken a test (no seed pinning, no "expect the bug"); send the
branch back to its session instead. Disk: cloud containers fill up — run
`rm -rf target/debug/incremental target/nextest` after each gate and
`find target/debug/deps -mmin +60 -delete` when `df` shows < 5 GB.

## Launching a task session

Pre-create the branch from staging (`mcp__github__create_branch`), then
create a Sonnet session on it with this prompt:

> Your task is `<task>`. First run `git fetch origin claude/specs-staging-7
> && git merge --ff-only origin/claude/specs-staging-7`. Then read
> docs/handoff/build-loop.md (<task> = <task>) and do the row `<task>` of
> docs/handoff/build-queue.tsv. Push only to claude/<task>. Use REC-<N> for
> any provisional note.

Up to ~17 at once has worked; each reports back with `send_message`.
Two sessions touching the same system (e.g. q-item-uniques and
q-weapon-combat both built the worn-item stat link) is the usual merge
cost: keep one implementation, never both.

## Ship

Create a snapshot branch from staging (`claude/release-8b`, so CI is not
restarted by later merges), open a PR to `main`, wait for CI (jobs: check,
tools, clippy-core, test-sim, test-rest, client), merge with
`expectedHeadSha`. Before opening, also run locally:
`cargo clippy --workspace --exclude d2-client --all-targets -- -D warnings`
and `cargo run -p depcheck` (CI builds the workspace without Bevy:
`d2-client` must never be a dependency of another crate).

## Follow-ups reported by sessions (not yet queued)

Items now covered by a merged task or a running row above are removed.

- strings: bind `TableStrings::by_id` in NPC menu / HUD / item-name panels
- REC-QESC-1 is a nonstandard id; renumber
- shrine timed-state effects; client object mouse-over label
- belt key labels / hover text / highlight rects; stash gold kinds 3/4
- save: mouse skills, act byte, merc/golem/corpse items, runeword refresh
- identify: Tome of Town Portal, descfunc > 4, belt/shop tips
- town portal: in-town cast, 0x82 owner name, state 102; vendor buy 0x9C action 12
- cube in start items, cube tool tips, close when cube gone
- unit state tints; server 0x26 system lines; overhead bubbles
- Book of Skill; Act III waypoints; Act V Ancients statues and warp gates
- set bonus lines in item tips; equip_rules switch still off
- lighting: per-block floor/wall gradients, blocks-light flags, Levels ambient
- line_clear answers "clear" until the client has collision (REC-152)
- Act I dungeons beyond the existing ones are not in the synthetic world

## Local checks (the user's PC, with game files)

Every `docs/handoff/q-*.md` ends with its local check (exact commands
and what to see); `docs/HANDOFF.md` §5 is the local run queue.
