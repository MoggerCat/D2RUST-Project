# Playability matrix: act × class × difficulty

Branch `claude/q-tool-class-difficulty` (session q-tool-class-difficulty).
Harness: `tools/playthrough/playthrough.py` matrix mode (spec
`specs/tools/playthrough.md` §4). Run 2026-10-09 on the 1.14d install
(private data repo); binaries built from staging-7 @ af5b1dee plus this
branch.

**Partial:** the run was stopped at the user's wrap-up. Finished:
classes.play (21/21 cells), act1.play (21/21) and act2.play (16/21; the
druid's Nightmare and Hell cells and all three assassin cells did not
run). Acts III–V, act4-blockers.play and milestones-a3-baal.play did not
run in the matrix.

To reproduce (about 30 min per act file with `--jobs 4` on 4 cores):

    D2_GAME_DIR=/home/user/game python3 tools/playthrough/playthrough.py \
      traces/playthrough/classes.play traces/playthrough/act1.play ... \
      --class all --difficulty all --jobs 4 --json m.json --markdown m.md

Each `--json` run also writes `m.json.cells.jsonl`, one line per finished
cell, so a run that stops early keeps the cells it finished.

How the saves are built: each cell's `save` lines become the class
profile. `--profiles` prints the arguments. A profile has:
- level 30 / 60 / 85;
- the role skills at 20, with the main skill on the right button;
- a tiered weapon, off-hand and body armour;
- `--difficulty-unlocked`, plus bit 0 of every `world/quests.md` §1.9
  slot and `acts=4` for each lower difficulty.

The q-tool-checkpoints saves were not used: no interface had landed yet.

## Reached / total per cell (first blocker)

### Normal

| act (file) | ama | sor | nec | pal | bar | dru | ass |
|---|---|---|---|---|---|---|---|
| 1 (act1.play) | 12/17 `kill-zombie` | 12/17 `kill-zombie` | 12/17 `kill-zombie` | 12/17 `kill-zombie` | 12/17 `kill-zombie` | 13/17 `den-of-evil-done` | 12/17 `kill-zombie` |
| 1 (classes.play) | 2/5 `main-skill-kill` | 2/4 `main-skill-kill` | 2/5 `main-skill-kill` | 1/4 `main-skill-kill` | 3/4 `main-skill-kill` | 4/6 `cast-then-walk` | 3/6 `main-skill-kill` |
| 2 (act2.play) | 11/15 `town-start` | 11/15 `town-start` | 11/15 `town-start` | 11/15 `town-start` | 11/15 `town-start` | 11/15 `town-start` | - |

### Nightmare

| act (file) | ama | sor | nec | pal | bar | dru | ass |
|---|---|---|---|---|---|---|---|
| 1 (act1.play) | 12/17 `kill-zombie` | 12/17 `kill-zombie` | 12/17 `kill-zombie` | 12/17 `kill-zombie` | 12/17 `kill-zombie` | 12/17 `kill-zombie` | 12/17 `kill-zombie` |
| 1 (classes.play) | 2/5 `main-skill-kill` | 2/4 `main-skill-kill` | 2/5 `main-skill-kill` | 1/4 `main-skill-kill` | 3/4 `main-skill-kill` | 4/6 `cast-then-walk` | 3/6 `main-skill-kill` |
| 2 (act2.play) | 11/15 `town-start` | 11/15 `town-start` | 11/15 `town-start` | 11/15 `town-start` | 11/15 `town-start` | - | - |

### Hell

| act (file) | ama | sor | nec | pal | bar | dru | ass |
|---|---|---|---|---|---|---|---|
| 1 (act1.play) | 12/17 `kill-zombie` | 12/17 `kill-zombie` | 12/17 `kill-zombie` | 12/17 `kill-zombie` | 12/17 `kill-zombie` | 12/17 `kill-zombie` | 12/17 `kill-zombie` |
| 1 (classes.play) | 2/5 `main-skill-kill` | 2/4 `main-skill-kill` | 2/5 `main-skill-kill` | 1/4 `main-skill-kill` | 3/4 `main-skill-kill` | 4/6 `cast-then-walk` | 3/6 `main-skill-kill` |
| 2 (act2.play) | 11/15 `town-start` | 11/15 `town-start` | 11/15 `town-start` | 11/15 `town-start` | 11/15 `town-start` | - | - |

## Acts III–V (q-fix-pt-sweep, 2026-10-09)

Branch `claude/q-fix-pt-sweep` @ the commit that adds this section (with
`claude/q-fix-boss-damage` @ ffbd7886 merged). All 63 cells ran (7
classes × 3 difficulties × 3 acts). Command (about 2 h with `--jobs 3`):

    D2_GAME_DIR=/home/user/game python3 tools/playthrough/playthrough.py \
      traces/playthrough/act3.play traces/playthrough/act4.play traces/playthrough/act5.play \
      --class all --difficulty all --jobs 3 --json m.json --markdown m.md

The sweep milestones that missed their target (`flayer-jungle-decoy`,
`hellforge`, `hephasto-present`, `frozen-anya`, `nihlathak-*`,
`baal-throne`) now `goto preset` it (REC-1081), so the cells below
measure the game, not the sweep.

#### Normal

| act (file) | ama | sor | nec | pal | bar | dru | ass |
|---|---|---|---|---|---|---|---|
| 3 (act3.play) | 15/15 | 15/15 | 15/15 | 15/15 | 15/15 | 15/15 | 15/15 |
| 4 (act4.play) | 10/12 `diablo-present` | 10/12 `diablo-present` | 10/12 `diablo-present` | 10/12 `diablo-present` | 10/12 `diablo-present` | 10/12 `diablo-present` | 10/12 `diablo-present` |
| 5 (act5.play) | 13/13 | 13/13 | 13/13 | 13/13 | 13/13 | 13/13 | 13/13 |

#### Nightmare

| act (file) | ama | sor | nec | pal | bar | dru | ass |
|---|---|---|---|---|---|---|---|
| 3 (act3.play) | 15/15 | 15/15 | 15/15 | 15/15 | 15/15 | 15/15 | 15/15 |
| 4 (act4.play) | 10/12 `diablo-present` | 10/12 `diablo-present` | 10/12 `diablo-present` | 10/12 `diablo-present` | 10/12 `diablo-present` | 10/12 `diablo-present` | 10/12 `diablo-present` |
| 5 (act5.play) | 12/13 `nihlathak-killed` | 13/13 | 12/13 `nihlathak-killed` | 12/13 `nihlathak-killed` | 12/13 `nihlathak-killed` | 12/13 `nihlathak-killed` | 13/13 |

#### Hell

| act (file) | ama | sor | nec | pal | bar | dru | ass |
|---|---|---|---|---|---|---|---|
| 3 (act3.play) | 14/15 `council-killed` | 14/15 `council-killed` | 14/15 `council-killed` | 14/15 `council-killed` | 14/15 `council-killed` | 14/15 `council-killed` | 14/15 `council-killed` |
| 4 (act4.play) | 10/12 `diablo-present` | 10/12 `diablo-present` | 10/12 `diablo-present` | 10/12 `diablo-present` | 10/12 `diablo-present` | 10/12 `diablo-present` | 10/12 `diablo-present` |
| 5 (act5.play) | 12/13 `nihlathak-killed` | 13/13 | 12/13 `nihlathak-killed` | 12/13 `nihlathak-killed` | 12/13 `nihlathak-killed` | 12/13 `nihlathak-killed` | 13/13 |

| File, milestone | Cells | Evidence | Route |
|---|---|---|---|
| act4 `diablo-present`, `diablo-killed` | 21/21 | Diablo (cl 243) is not in the level: he appears only after the five seals open, which no milestone does | `q-a4-endgame` (`docs/handoff/build-queue.tsv`); the milestone needs a seals poke or step first |
| act3 `council-killed` | 7/7 on Hell only | the Council member (cl 345) never reaches a death mode within 1000 frames on Hell (Normal and Nightmare pass) | boss damage / Hell resists: `q-fix-boss-damage` |
| act5 `nihlathak-killed` | 5/7 on Nightmare and Hell (not sor, ass) | Nihlathak (cl 526) not killed by the poked Fire Bolt 10 frames after the 256-hp stat poke | boss damage: `q-fix-boss-damage`; the sorceress and assassin pass, so likely a class damage-type vs resist seam |

`python3 tools/coord/route.py` could not route the two `stuck` rows (no
predicate-to-owner rule); the Diablo row routes to the monster-init
session and is owned by the Act IV endgame session.

## Distinct blockers

| File, milestone | Cells | Evidence | Row |
|---|---|---|---|
| classes `cast-then-walk`; the later casts of `main-skill-kill` | 18/21 (all but bar) | after one cast the client sends no C→S message for any click | `q-fix-pt-cast-input-lock` (skills) |
| classes `main-skill-kill` (ama, sor, nec) | 9 | the Quill Rat reaches hp 0, then returns to mode 1 | existing `q-fix-p4-death-cleanup` |
| classes `summon-follows-wp` | 12/12 summon cells | the pet stays in Cold Plains after the same-act warp | `q-fix-pt-pet-warp-follow` (levels) |
| classes `aura-applies` | pal 3/3 | Holy Fire on the right does no damage | `q-fix-pt-right-aura` (skills) |
| classes `main-skill-kill` (bar) | 3/3 | Whirlwind: 0x4D sent, the player stays in mode 1 | `q-fix-pt-whirlwind` (skills) |
| classes `main-skill-kill` (pal) | 3/3 | Blessed Hammer missile 92 never damages at 2–5 sub-tiles | `q-fix-pt-blessed-hammer` (skills) |
| act1 `kill-zombie` | 20/21 | written for the sorceress (rclick at (450, 300), a full-hp Zombie; the Hell Zombie is cold immune), plus the death bug | existing `q-fix-p4-death-cleanup`; the class-aware kill is classes `main-skill-kill` |
| act1 `den-of-evil-done`, `andariel-done`, `act2-open` | 21/21 | NPC talk: no hover pick in headless play | existing `q-fix-b-headless-unit-click-keys` |
| act1 `andariel-killed` | 21/21 | the Fire Bolt poke never kills Andariel | existing: `playthrough.md` Open question 2 |
| act2 `town-start`, `radament-killed`, `summoner-present`, `summoner-killed` | 16/16 that ran | the same in every cell: does not depend on class or difficulty | existing Act II blockers |

These work on every class and difficulty:
- the summon casts (Valkyrie, Clay Golem, Spirit Wolf, Shadow Warrior);
- Teleport;
- Battle Orders and Werewolf raise `hpx`;
- Lightning Sentry kills;
- the monster level per difficulty (Blood Moor Zombie 1 / 36 / 67);
- every Act I / II milestone the sorceress reached.

So far no blocker depends on the difficulty.

## Left

- Run act4-blockers, milestones-a3-baal and the 5 missing act2 cells
  in the matrix (command above); Acts III–V ran (section above).
- Use q-tool-checkpoints' saves (now in staging): a checkpoint name,
  `traces/checkpoints/<name>.checkpoint`, built with `python3
  tools/checkpoints/make.py <name>`. In Python, `make.parse()` then
  `make.d2s_args()` give the `d2s-tool new` arguments: a sorceress with
  her level, stats, skills, gear, quest bits, waypoints and act, all on
  Normal. Layer `cell_save_args` on top of them: it already replaces the
  class, skills, gear and difficulty, and keeps the quest, waypoint and
  act flags.
- Step 3 (the matrix with the autoplay bot) was dropped by the
  coordinator.
