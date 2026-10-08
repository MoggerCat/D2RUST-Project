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
- **Check first: settled 2026-10-08 (third coordinator)** — `client/stat-lists.md` §1–§2 says base-only 0x1D and the client attaches each equipped item's list from its 0x9C/0x9D property stream (`items/bitstream.md` §4.6, already decoded by `d2_proto::item_bits`); the 0xFE/0xFD pseudo states are d2rs-own. Queued as row `q-item-bonus-wire` (REC-188). Original note: q-charms
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

## Merge log (third coordinator)

Merged into staging: q-belt-stash, q-strings-bind, q-fe-start-flow, q-cube-gaps, q-shrines-labels, q-lighting-detail, q-client-collision, q-save-gaps, q-fe-draw, q-tp-gaps, q-assassin-gaps, q-a2-tyrael-door, q-item-tips, q-menu-options (7,007 tests). Sent back: q-fe-host-screens (keep front_start's registry and q-fe-draw's glyph text; drop its duplicates). New follow-ups from these: lighting blocks-light flags / near-room fills (q-lighting-detail), shrine overhead text 0x26 type 5 (q-shrines-labels; maybe q-unit-fx), 0x61 act videos (q-fe-start-flow), HUD globe numbers (q-strings-bind).

## REC ids

PC 1 loop (`docs/handoff/pc1-loop.md`, branch `claude/local-pc1-s8`): REC-300..349 are PC 1's; cloud ids stay below 300. Merge that branch into staging like a task branch (it changes specs/docs, plus `// Covers:` lines); turn its `q-fix-*` rows into sessions.

Batch launched 2026-10-08 (third coordinator, session_01QeN5r8PoLZsUhwAH2iDzLJ): q-menu-options REC-187, q-item-bonus-wire REC-188, q-fe-draw REC-189, q-fe-host-screens REC-231, q-skill-leap-talon REC-232, q-assassin-gaps REC-233, q-a2-tyrael-door REC-234, q-a4-quest-items REC-235; q-fe-start-flow REC-236 (launch after q-fe-host-screens merges). Batch 2 (same day, user asked for more parallel sessions): q-fe-start-flow REC-236 (launched with batch 2 despite the host overlap), q-strings-bind REC-238, q-shrines-labels REC-239, q-belt-stash REC-240, q-save-gaps REC-241, q-item-tips REC-242, q-tp-gaps REC-243, q-cube-gaps REC-244, q-unit-fx REC-245, q-act3-act5-gaps REC-246, q-lighting-detail REC-247, q-client-collision REC-248, q-a1-dungeons REC-249 (REC-237 = the old REC-QESC-1). Every follow-up below is now a row. Batch 3: q-lighting-blocks REC-250, q-fe-act-videos REC-251, q-hud-globes REC-252, q-equip-rules REC-253, q-a2-duriel-ai REC-254, q-identify-cain REC-255, q-controls-ingame REC-256, q-options-art REC-257. Next free: REC-258.
Batch 3 sessions: q-lighting-blocks session_01FkbE9oAejuWgNUXEQGWLwn, q-fe-act-videos session_01LBf6ogfJsw3YdfvJRQ1wSb, q-hud-globes session_0112rSJPNS9V8wwJ3F9WVpPd, q-equip-rules session_016A5Vq7HpXZb4CintEBhnJc, q-a2-duriel-ai session_019aeyMy8J99P5qzkWGfPvPG, q-identify-cain session_01AsvCYqWt6Q2tEt6cHVvQKj, q-controls-ingame session_01Ffkk35UYvEDbCw1hc3EqGR, q-options-art session_01H4FeiZFYKQxXb1UWnq63ec.
Batch 2 sessions (Sonnet): q-fe-start-flow session_01D8YxnU7ifFU8FTbjqRcq4K, q-strings-bind session_019HoAaT5j7S2gr4JGpJVY1H, q-shrines-labels session_01XGJBp6ahpXvtXT3tDkv13C, q-belt-stash session_01RexUqu1ZMA3suNoPxoGvSg, q-save-gaps session_01RbxjBKGQT9AT4HpZ5VnjfH, q-item-tips session_01TWdwkVAMGo4ANkE9NGnbpY, q-tp-gaps session_016kdVuTpR3ZM1FBnEGWYgi3, q-cube-gaps session_01ExfXkkq7qwDuLzNN51rrsu, q-unit-fx session_01EgKzh5VautVYNZRKSkidUH, q-act3-act5-gaps session_01HhgwinoUMppQA5V5Dxpd52, q-lighting-detail session_01CScXFDiWAkNKRBEsdhnvRC, q-client-collision session_01Sqa13BDnVFJnPaWmgN8Pgx, q-a1-dungeons session_01YCzKS85esX9nF4BaJGJmqh.
Running sessions (Sonnet): q-menu-options session_016djAfuUo6mksJrag4BJS9X, q-item-bonus-wire session_01Fx8FphWjp7gR1w8YSpkv3S, q-fe-draw session_01R7uxXBkqs66q9wvmMypqKu, q-fe-host-screens session_011WjT1AwSGKecTSHgHkSp4v, q-skill-leap-talon session_01LTwd6Jb8ouRzvhjhS8UXJQ, q-assassin-gaps session_01KsJbGT9bjKX5fd7bPkNRpz, q-a2-tyrael-door session_019yV6ZFtRM5GpghZ56UFrXN, q-a4-quest-items session_01ECXzfSJs7BMjsYtJbQDT6b. They report to session_01QeN5r8PoLZsUhwAH2iDzLJ (build-loop.md); a new coordinator must change that line to its own id.

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
- (done: REC-QESC-1 renumbered to REC-237)
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
