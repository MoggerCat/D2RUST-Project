# Coordinator resume — playable-build loop (2026-10-08)

## Coordinator run 2026-10-09 evening (session_01KcnkwCTXbuv5ZbToEUpBSj)

Merged onto integ-local (gate r1): q-diff-skills-2 (room-cancel rule done twice: both tests kept), q-play-act5 (0x005B2F20 quest spawn done twice: kept `MonsterWorld::spawn_at`, dropped `place_monster`), q-diff-combat-a1 (poke `goto` + `msg` both kept; `Schedule::run_due` takes the link), q-prov-recording-2 (kept staging's bridge/objects GUID counter), q-fixture-migrate-2, coord-resume-2, q-fix-server-store-fill, q-tool-soak; then local-pc1-test (docs) and local-pc1-play (45-file merge, its own gate).

Fix sessions launched 17:05 UTC (each branch carries `docs/handoff/<branch>-task.md` with its brief and the session rules; REC blocks from 1000):

| Branch | Session | Model | REC | Area |
|---|---|---|---|---|
| q-fix-input-lock | session_01JQwtFjADMhRsW7QNGqCiie | Opus | 1000–1009 | cast/attack input lock (#1) |
| q-fix-client-crash | session_01RdQgHcwijQxkvdvx6nPuAW | Opus | 1010–1019 | geom.rs:197 crash; walk desync / re-target |
| q-fix-boss-damage | session_01QjgkBNuhD5xisApMttxSB8 | Opus | 1020–1029 | Andariel never dies, Radament at 256 hp |
| q-fix-items-shop | session_01BBQcb5iXAv9CoWsyhDqVge | Sonnet | 1030–1039 | shop "no room", belt after load, monster items 0x00573B20 |
| q-fix-npc-menus | session_01VzxhMLTVzSSr24GoMX684d | Sonnet | 1040–1049 | Natalya, Halbu, Nihlathak; Esc in quest log |
| q-fix-a1-den-wp | session_011nm8wnAZ8dXwKuehwW9a8D | Sonnet | 1050–1059 | Den of Evil flag; Cold Plains waypoint (+15,+5) |
| q-fix-class-rows | session_017Zjk6EsU1ooMqhtmpz54ih | Sonnet | 1060–1069 | q-fix-pt-whirlwind / right-aura / pet-warp-follow / blessed-hammer |
| q-fix-depcheck | session_01Bw9umZerDFScSEkXWhvchg | Sonnet | 1070–1079 | rng_trace thread_local behind a feature |
| q-fix-pt-sweep | session_01BU2ZCAxsiRDowTgb9cTSki | Sonnet | 1080–1089 | sweep hops ≤16 + fallback ring, goto cells; A3–V matrix |

| q-fix-monster-death | session_01Paf5PKDcW6k9hvqKunfebM | Opus | 1090–1099 | dead monsters stand up (p4-death-cleanup), drop spot |
| q-fix-player-hit | session_01U91VHQHBy7Xp4uXJjJ3AC4 | Sonnet | 1100–1109 | player never a missile target (c6), owner flags (c7) |
| q-fix-monster-ai | session_01URviZERs7KxnRnDD9NkMyi | Sonnet | 1110–1119 | Fallen / Quill Rat AI (c2, c3), Fallen think seed step |
| q-fix-seed-order | session_01JhQc4nNShp9YNmxNXpqAdx | Opus | 1120–1129 | unit seed order, Blood Moor population, rng creation draws |
| q-fix-save-input | session_01TU8gxrozxP5xiyq7rEDW1L | Sonnet | 1130–1139 | belt key send, save item seed, 0x67 byte 18, cursor reload |
| q-fix-room-links | session_016xcFMkKoz6pTSqDuPJRwm3 | Sonnet | 1140–1149 | town objects after WP return, static/drop room links |
| q-fix-act2-play | session_01NaR6yMxmEgnEfm3Fj9DK3D | Sonnet | 1150–1159 | Act II playthrough blockers |
| q-fix-act4-play | session_013J2Srxix169AQQjRUYSHvj | Sonnet | 1160–1169 | Act IV playthrough blockers |
| q-fix-act5-play | session_015YNGBGrwtHjFyby4tCDZR6 | Sonnet | 1170–1179 | Act V: Ancients link, quest superuniques, milestones |
| q-fix-check-triage | session_01B2eB5Q2V444GgPtPfrhLNK | Sonnet | 1180–1189 | all 88 scenario-diff checks vs 1.14d → checks-status.md |
| q-fix-difficulty-a1a2 | session_01LiMn42LTayZFyizFKwHzQo | Sonnet | 1200–1209 | Nightmare / Hell, Acts I–II |

Batch 2 launched 17:29 UTC; q-fix-realdata-baseline (REC 1190–1199) session_01Hv1NPtDa5KwdQ56KbYdTYm launched 17:41 at the user's request (21 sessions). Next free REC block: 1210.
pc1-data Step 4: last number 46.

## State at pause (2026-10-09 ~15:50 UTC, end of the day run)

The user paused every session to continue later. Each session was told to
push and write its hand-back `docs/handoff/<branch>.md` (done / in
progress / next / open RECs / repro commands). Read those first.

**Goal set by the user:** "99% playable": the whole game can be played
start to finish with the 1.14d experience (exact match stays the bar,
rule 10).

**Measure:** `python3 tools/playthrough/playthrough.py --all --json out.json`
(per-act milestones; teleport-based) and
`traces/playthrough/classes.play --class all --difficulty all` (matrix,
`docs/handoff/playability-matrix.md`). Last table: I 12/17, II 12/15,
III 13–14/14, IV 7/12, V 9/13 (V's Anya / Nihlathak / Throne Baal are
reached on the install in tests/play_act5.rs; the sweep hops are the
limit).

**Top blockers for the next run (owner area in brackets):**
1. Cast/attack input lock: after one attack or cast the local player
   stays in mode 7 and `bridge::click::can_act` refuses every later click
   (18/21 class cells) [skills cast / client input, q-diff-skills-2].
2. Killed monsters stand back up (mode 1, hp 0), bosses included
   [q-diff-combat-a1; q-fix-p4-death-cleanup].
3. Client crash: `path/walk/geom.rs:197` direction_vector index from
   `MonsterMotion::frame` in Cold Plains (autoplay probe, frame 3265)
   [client tracks, q-fix-p6-client-arrival-guids].
4. Player position desync / walk re-target on a click while walking
   (soak + side-by-side a1-walk-s) [same owner].
5. Esc in an NPC dialog sends no 0x30 and breaks later talks [client UI].
6. Town objects gone after a waypoint return [progression,
   q-fix-pc1-proto-items].
7. Shop buy "no room"; belt potions not in the client model after load
   [items, q-fix-server-store-fill].
8. CI depcheck red: `d2-sim/src/debug/rng_trace.rs` thread_local
   [q-tool-state-diff]. Blocks a release to main; add `cargo run -p
   depcheck` to the gate once fixed.
9. Tools: `goto` must land on a free, missile-passable cell; the sweep
   needs hops ≤16 [q-tool-checkpoints / playthrough].

**Fixed today (in staging):** save-start act + Act III seed, monster melee
damage (exact hp vs 1.14d), Act I arrival (205 creations, 90 frames),
item generation incl. affixes (36 items), object collision footprints,
Larzuk / barbarian rescue, run animation, control panel, Den / Cave 1
equal 160 frames, 11 + 16 skill checks, real-data rigs (214/222).

**Tools now in the repo:** scenario-diff (all channels take pokes, sends,
shared input), playthrough (+ class × difficulty), `tools/coord/`
(sync.sh, route.py, owners.tsv, realdata.py, playtable.py),
`tools/soak/`, `tools/sidebyside/` (pages go to the PRIVATE repo
`reports/side-by-side/`), `tools/coverage-map/`, `tools/perf/`,
checkpoints + goto, the Windows build workflow (artifact
`d2rs-windows-<sha8>` per staging push; `docs/PLAYTEST.md`). The
autoplay bot was dropped by the user; only `d2-client autoplay-host` and
the fixed probe route remain as a real-input smoke test.

**How the coordinator ran the loop (scripts in
`tools/coord/coordinator/`):** `gate.sh` (fmt, clippy -D warnings on
4 crates, nextest, coverage, spec_index, conflict markers),
`gatepush.sh <tag>` (gate, push to staging only on `GATE fail=0` and a
fast-forward; cleans the cache under 9 GB free), `mergebatch.sh b…`
(merge onto staging, union docs, auto-resolve spec index tables, regen
indexes; abort a branch on any other conflict), `autoloop.sh` (merges
every branch in `branches.txt` that is ahead, gates, pushes; unattended).
They assume the repo at /home/user/D2RUST-Project and write logs next to
themselves; copy them to a scratch dir before use. Never run a merge in
the gate's worktree while a gate runs.

**Session rules that worked:** report only on fixed blockers / blocked /
done; stay in your area (owners.tsv); sync every 30 min; REC blocks per
session (see each hand-back); PC 1 questions as unnumbered
`[session] title` items in pc1-data.md Step 4 (the coordinator numbers
them; last number 46). PC 1: one `Game.exe` open at a time
(`%TEMP%\d2-game.lock`, pc1-data.md top).


Everything a new coordinator session (any account) needs to pick up the
overnight build loop. The loop itself is in the repo: the shared session
rules are `docs/handoff/build-loop.md`, the task rows are
`docs/handoff/build-queue.tsv`, and each finished task left
`docs/handoff/q-<task>.md` (done / left / local check).

## State at hand-over (2026-10-09 06:00 UTC, end of the overnight run)

- Read `docs/handoff/overnight-report.md` first (what merged, what was
  found against 1.14d, remaining work with sizes, what waits on PC 1).
- `claude/specs-staging-7` = `c50da245` + this doc: green (7,433 tests on
  d2-sim / d2-server / d2-client / test-fixtures). Release snapshot
  `claude/release-8m` was cut from it for `main`.
- Gate: run the gate and push in one step, pushing only on `GATE fail=0`
  and a fast-forward; the gate also fails on leftover conflict markers.
  Never push a merge whose gate result you have not read.
- Next run, in order: merge q-facts-scenes (`e86302ac`, exporter conflict
  with q-cloud-game; regenerate `facts/render/sprites.tsv` with the tool);
  `q-fix-real-unit-seed-order`; server store fill; fixture-migrate G4–G7.
- Overnight rules and REC blocks: `docs/handoff/overnight-loop.md` (next
  free cloud id REC-490).

## Earlier state (2026-10-08 ~09:20 UTC, second account switch)

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

Merged into staging: q-belt-stash, q-strings-bind, q-fe-start-flow, q-cube-gaps, q-shrines-labels, q-lighting-detail, q-client-collision, q-save-gaps, q-fe-draw, q-tp-gaps, q-assassin-gaps, q-a2-tyrael-door, q-item-tips, q-menu-options (7,007 tests; shipped to main as PR #51, merged 2026-10-08 11:21 UTC), then q-item-bonus-wire, q-unit-fx, q-fe-host-screens, q-act3-act5-gaps, q-a1-dungeons, q-skill-leap-talon, q-a4-quest-items (7,033 tests; snapshot release-8f = PR #52, merged 11:57 UTC). All batch 1-2 rows are merged. Then q-a2-duriel-ai, q-controls-ingame, q-equip-rules, q-fe-act-videos, q-hud-globes (7,039 tests). q-a1-vis-links REC-259 launched (session_014DD4WWts3FwUbe5Cx93cAv). q-fe-text-widths REC-258 launched (session_01BWwKswhrWVQFvRWAoFJejM). Batch 4: q-chest-drops REC-260, q-dungeon-builds REC-261, q-vendor-store-sync REC-262, q-shrine-overhead REC-263, q-ui-rects REC-264, q-save-swap REC-265, q-preview-skill-seams REC-266, q-cube-transmute REC-267. Batch 5 (user: finish the code fast): q-pending-audit REC-268, q-char-panel-full REC-269, q-skill-tree REC-270, q-equip-backgrounds REC-271, q-weather-passes REC-272, q-view-unit-facts REC-273, q-a2-charge-jab REC-274, q-move-anims REC-275, q-trap-missiles REC-276, q-play-smoke REC-277. Smoke batch (Opus, user: finish fast; prompt carries the scenario, no queue row): q-smoke-town REC-278 session_01XFJUwrMVvcw4V56tuQHXaF, q-smoke-combat REC-279 session_01KF1bwW7fZ6YhqsohxjZfYo, q-smoke-travel REC-280 session_01Uf3GecZHQEXagQ4fuF5Esw, q-smoke-items REC-281 session_014k7G8Z2ZH3ftJgrjUn6Vdo, q-smoke-save REC-282 session_01Gx9N93T6TXqJZSpktc8REk, q-smoke-quests REC-283 session_013cBnbZjiYCFJ7n2ox7jW9M, q-smoke-frontend REC-284 session_01KDcV14ztvJvG8AhSvE3wZo. release-8g = PR #53. Next free: REC-286 (REC-285 = q-flaky-transport, session_01VT4CrLjjxWHr5VfAsnRkxU).
Batch 5 sessions: q-pending-audit session_0128Qoiu1xBLMm7Mpx15NqbQ, q-char-panel-full session_018myM4vHJjumxoXY5zcFhek, q-skill-tree session_01LviAv68fW8Hcf4ygoPDtg9, q-equip-backgrounds session_01N8vAezfTZeewNXUso6iHVR, q-weather-passes session_01PaEq3MPbfLBRrpww5kUmmP, q-view-unit-facts session_01EuLUoSdWiZsed7EXPrKrLu, q-a2-charge-jab session_013mCAkw3tX4pz21JDoEc4yh, q-move-anims session_01LLVFSTjG9h67w8Hzui2ACV, q-trap-missiles session_011GfD3DtZKrJuZV8gE5ibZL, q-play-smoke session_014L5Sex4meFi34BFFm2kDc5 (Opus).
Batch 4 sessions: q-chest-drops session_016uD1rnyqFEdy6bbyJfC9Ef, q-dungeon-builds session_019HpGm2kPPBrv6dzhVkTrQT, q-vendor-store-sync session_01QPJDHZi7isPWR9TYQZzeow, q-shrine-overhead session_01PnaXBPEREWuWxgV5n4rKXM, q-ui-rects session_01MWTqBiv9VKt3rvmFcykQRP, q-save-swap session_01YKHcKJeAKPdHT6PQ4wX1nM, q-preview-skill-seams session_01HftCt92TFxqhDtVecSfAPn, q-cube-transmute session_01Pj9ipsQwrFjWw49jiFquPu. q-fe-host-screens was sent back once (duplicate registry), then merged. New follow-ups from these: lighting blocks-light flags / near-room fills (q-lighting-detail), shrine overhead text 0x26 type 5 (q-shrines-labels; maybe q-unit-fx), 0x61 act videos (q-fe-start-flow), HUD globe numbers (q-strings-bind).

## REC ids

PC 1 autotest loop (`docs/handoff/pc1-autotest.md`, branch `claude/local-pc1-test`, REC-350..399; replaces pc1-loop while it runs). PC 1 loop (`docs/handoff/pc1-loop.md`, branch `claude/local-pc1-s8`): REC-300..349 are PC 1's; cloud ids stay below 300. Merge that branch into staging like a task branch (it changes specs/docs, plus `// Covers:` lines); turn its `q-fix-*` rows into sessions.

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
