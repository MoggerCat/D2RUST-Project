# Coordinator resume — playable-build loop (2026-10-08)

Everything a new coordinator session (any account) needs to pick up the
overnight build loop. The loop itself is in the repo: the shared session
rules are `docs/handoff/build-loop.md`, the task rows are
`docs/handoff/build-queue.tsv`, and each finished task left
`docs/handoff/q-<task>.md` (done / left / local check).

## State at hand-over (06:30 UTC)

- `main`: includes PR #47 (everything merged up to ~05:30 UTC).
- `claude/specs-staging-7`: green, about 70 tasks merged (6,779 tests
  sim/server/client/test-fixtures). Everything after #47 is here only:
  open a PR from a snapshot branch (see "Ship" below) to land it.
- Branches pushed but **not merged** into staging (sessions told to push
  and stop at 06:25 UTC; merge each, one at a time, with the gate):
  `claude/q-npc-approach` (merged locally, gate interrupted by a full
  disk — re-merge it), `claude/q-a4` (sent back: keep DEFAULT_SEED in
  `tests/app_play_monster_ai.rs`, find why the zombie idles),
  `claude/q-barb`, `claude/q-paladin`, `claude/q-sorc`, `claude/q-druid`,
  `claude/q-assassin`, `claude/q-mercs-acts` (just started, likely
  partial).
- Queue rows not started: `q-item-uniques`, `q-gamble`, `q-charms`,
  `q-levels-warps-all`, `q-light-radius-detail`, `q-render-polish`,
  `q-perf`, `q-config`.

## REC ids

Highest used in staging: REC-151 (q-necro). Reserved for the unmerged
branches: 143 q-a4, 149 q-npc-approach, 152 q-barb, 153 q-paladin,
154 q-sorc, 155 q-druid, 156 q-assassin, 157 q-mercs-acts. Next free:
REC-158. Sessions often pick a taken id; on merge, renumber only the
branch's new lines with `python3 tools/coord/renum.py REC-OLD REC-NEW`
(run during an uncommitted merge, from the repo root).

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

Keep at most 8 running; each reports back with `send_message`.

## Ship

Create a snapshot branch from staging (`claude/release-8b`, so CI is not
restarted by later merges), open a PR to `main`, wait for CI (jobs: check,
tools, clippy-core, test-sim, test-rest, client), merge with
`expectedHeadSha`. Before opening, also run locally:
`cargo clippy --workspace --exclude d2-client --all-targets -- -D warnings`
and `cargo run -p depcheck` (CI builds the workspace without Bevy:
`d2-client` must never be a dependency of another crate).

## Follow-ups reported by sessions (not yet queued)

- strings: bind `TableStrings::by_id` in NPC menu / HUD / item-name panels
- REC-QESC-1 is a nonstandard id; renumber
- shrine timed-state effects; client object mouse-over label
- warp-tile picking by click; hireling follow live check (local)
- belt key labels / hover text / highlight rects
- stash gold kinds 3/4
- save: mouse skills, act byte, merc/golem/corpse items, runeword refresh
- identify: Tome of Town Portal, descfunc > 4, belt/shop tips
- town portal: in-town cast, 0x82 owner name, state 102; vendor buy 0x9C action 12
- cube in start items, cube tool tips, close when cube gone
- unit state tints
- socketed gem stats reach the wearer (equip wiring); combat uses the equipped weapon
- server emits 0x26 system lines (pickup, can't use); overhead bubbles
- Book of Skill, Jerhyn/Meshif travel east; Act III waypoints
- Duriel's lair, boss-tomb entrance, Horadric Staff/orifice flow, Act II dungeon population
- Act V: Ancients statues, warp gates; Act IV: Hellforge, seals, Diablo
- Necromancer curses / Corpse Explosion / Raise Skeleton / Revive (need a target in the harness)

## Local checks (the user's PC, with game files)

Every `docs/handoff/q-*.md` ends with its local check (exact commands
and what to see); `docs/HANDOFF.md` §5 is the local run queue.
