# Spec: Tools — Playthrough harness (how far d2rs gets through an act)

- **Status:** draft: a crude first version. Format `playthrough 1`, the
  predicates and the verdicts below are ours; `tools/playthrough/playthrough.py`
  implements them, with one objective file per act,
  `traces/playthrough/act1.play` … `act5.play` (run 2026-10-09 on the
  1.14d install, `--all --json`: I 12/14, II 11/15, III 12/14, IV 5/12,
  V 8/13; first blockers `kill-zombie` (I) and `town-start` (II–V: the
  player's act byte is 0 in the act's town)). Not yet described below:
  the `find <filter>` probe with `poke +N` / `frame +N`, sweep mode
  `cross`, the unit test `killed`, `--all` and the `--json` keys (`act`,
  `reached`, `total`, `consecutive`, `first_blocker`, `milestones`); see
  `playthrough.py --help` and its selftest. d2rs only: no 1.14d
  side, so a reached milestone is "the game gets there", never a
  fidelity check (CLAUDE.md rule 10).
- **Target version:** 1.14d (the milestones); the format is d2rs-own.
- **Crate/module:** `tools/playthrough/playthrough.py` (Python stdlib);
  it runs `d2-client state-dump` and `d2s-tool new`.
- **Related specs:** `tools/state-snapshot.md` (the state file it reads),
  `tools/poke.md` (directives, absolute `--poke` form §2 r6),
  `tools/scenario-diff.md` §2 r4 (the `input` script), `world/quests.md`
  and `world/quests-act1*.md` (where the milestones come from).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 43–54 |
| Inputs | 55–62 |
| Outputs / state changes | 63–74 |
| Rules | 75–76 |
|   1. Objective file `playthrough 1` | 77–110 |
|   2. Predicates | 111–134 |
|   3. Verdict per milestone | 135–150 |
|   4. Class × difficulty matrix | 151–218 |
| Constants & data dependencies | 219–224 |
| Randomness | 225–229 |
| Edge cases & original bugs | 230–237 |
| Test vectors | 238–245 |
| Provenance | 246–257 |
| Open questions | 258–281 |
<!-- /index -->

## Summary

The goal is a game a player can finish, Acts I–V. The harness gives a
quick, repeatable answer to "how far does d2rs get?" An objective file
lists one act's milestones (town start, Blood Moor, Den of Evil, …,
Andariel, travel to Act II). Each milestone is its own headless run
(`state-dump`, fresh save and seed). Pokes and saves stand in for the
walking between milestones. Predicates on the per-tick state file decide
whether the milestone was reached. A milestone that is not reached gets
a blocker. Running each milestone separately means one blocker does not
hide the ones after it.

## Inputs

| Name | Type | Source |
|---|---|---|
| objective file | text, §1 | `traces/playthrough/act<N>.play` |
| d2rs game | `target/release/d2-client`, `d2s-tool` (`--build` builds them) | the repo |
| install | `D2_GAME_DIR` / `--game-dir` | the user's 1.14d files |

## Outputs / state changes

- A work dir (default `target/playthrough/act<N>-*/`): the saves, each
  milestone's `<name>.state.jsonl` (and `<name>.probe.jsonl` for a
  sweep), and `<name>.sh` when the command is too long to print.
- On stdout, per act, a table with the milestone, its status and the
  evidence. Then the furthest consecutive milestone reached (n/N), the
  first blocker with its frame and evidence, and the command that
  reproduces it. `--json FILE` also writes `playthrough-result-1`.
- Exit 0: every milestone reached. 1: a blocker. 3: error (bad
  objective file, missing tool, unreadable state file).

## Rules

### 1. Objective file `playthrough 1`

1. UTF-8 lines. `#` starts a comment and blank lines are skipped. The
   first line that is left must be `playthrough 1`.
2. `act <n>`. `save <name> <d2s-tool new args>` names a character, which
   `d2s-tool new <args> -o <work>/<name>.d2s` makes once per run.
3. `milestone <name>` opens a block. The block's lines are `use <save>`
   (required), `seed <n>` (default 1), `difficulty <d>`, `deadline <F>`
   (required: server ticks to run), `poke <f> <directive>` (repeatable;
   `poke.md` §2 r6, absolute frame), `input <script>` (scenario-diff §2
   r4), `sweep <F0> <every> <R> <step> [spiral|grid]`, `need <pred>`
   (one or more) and `note <text>`. Unknown keywords, a missing `use` /
   `deadline` / `need` and a duplicate name are errors.
4. **sweep.** A probe run up to frame F0 − 1 finds the player's
   position (cx, cy). Then from F0 on, one absolute `pos @player x y`
   poke runs every `every` frames over the square of half-side R around
   (cx, cy), `step` sub-tiles apart, ending back at (cx, cy). `spiral`
   (the default) goes outward from the centre, so each target is next
   to the last one. `grid` goes row by row from the (−R, −R) corner,
   serpentine. A sweep exists because units only exist in active rooms
   near a player. A refused `pos` is expected and is not a blocker.
5. **checkpoint** (2026-10-09). `checkpoint <name>` in a milestone
   replaces `use <save>`: the save is built from
   `traces/checkpoints/<name>.checkpoint` (`tools/checkpoints.md` §3),
   the milestone's difficulty defaults to the checkpoint's, and the
   checkpoint's start pokes run first, so they are `@p0`, `@p1`, … and
   the milestone's own pokes follow. A milestone has exactly one of
   `use` and `checkpoint`.
6. **goto.** `goto <frame> unit [<type>:]<class>` or `goto <frame>
   preset <level> [<type>:]<class>` is `poke <frame> goto …`
   (`poke.md` §6): the walk to the target. Its record is written when
   it lands (GUID = the target's, so `g @pI` names the target) or fails
   (a refused poke: `stuck`).

### 2. Predicates

1. `player <field> <op> <n>`: a field of the player unit
   (`state-snapshot.md` §2: `lv`, `act`, `m`, `x`, `lvl`, `hp`, …). `op`
   is one of `== != >= <= > <`. `player moved <op> <n>`: the larger of
   |Δx| and |Δy| from the player's first snapshot with a position.
2. `unit [ut T] [cl C,…] [lv L,…] [g G | g @pI] <test>`: the units that
   match every filter. `g @pI` is the GUID created by the milestone's
   I-th poke (0-based, in file order, sweep pokes after). The tests:
   `present` (at least one matches), `absent` (none matches), `count
   <op> <n>`, `seen` (one matched in any snapshot up to now), and `dead`
   (one matched earlier, and now every match is in a death mode or
   gone). The death modes are DT 0 and DD 17 for players, DT 0 and DD 12
   for monsters.
3. `need ever <pred>`: the predicate held at some snapshot.
4. `quest <slot> <bit> set|clear`: bit `bit` (0–15) of quest slot
   `slot` (0–41) in the player's quest flag record of the game's
   difficulty (`world/quests.md` §1.1–§1.4: bit 16·slot + bit of the
   96-byte record, LSB first), read from the player's `q`
   (`state-snapshot.md` §2: `[slot, word]` per non-zero slot; a slot not
   listed is 0). A player without `q` fails the predicate ("no quest
   record"). With `ever`: the bit had that value at some snapshot. The
   checkpoints to test are `world/quests.md` §1.9 (done = bit 0).

### 3. Verdict per milestone

1. The state-dump exits non-zero → `crash`; the evidence is the first
   stderr line with `panicked` or `Error:`.
2. Otherwise the predicates are evaluated at the deadline (the last
   snapshot), `ever` ones over the whole run. All hold → `reached`, and
   the report gives the first frame at which all the plain ones held.
3. Otherwise the first failing predicate gives the status:
   `missing-unit` when a non-sweep poke was `unresolved` or a unit test
   matched nothing; `wrong-level` for a failing `player lv`; `stuck` for
   everything else, including a refused (`failed`) poke. The evidence
   always ends with the player's `lv`, `m` and position at the deadline.
4. Furthest = how many milestones, counted from the first, are reached
   with no gap. The first blocker is the first milestone in file order
   that was not reached.

### 4. Class × difficulty matrix

1. `--class L` (comma list of `ama sor nec pal bar dru ass`, or `all`)
   and / or `--difficulty L` (`normal nightmare hell`, or `all`) turn
   the run into a matrix: every objective file runs once per (class,
   difficulty) cell. A missing `--class` means all seven, a missing
   `--difficulty` means `normal`. `--jobs N` runs N cells at once. Each
   cell has its own work dir `act<N>-<file>-<class>-<difficulty>/`.
2. `only class <list>` and `only difficulty <list>` in a milestone block
   (repeatable, one per kind) drop the milestone from the cells they
   exclude; then the saves no remaining milestone uses are dropped.
   Outside a matrix the milestone's class is its save's `--class` and its
   difficulty its `difficulty` line (default `normal`).
3. **Cell saves.** In a matrix each `save` line's `d2s-tool new`
   arguments are rewritten: `--class --name --level --skill --all-skills
   --stat --item --left-skill --right-skill --difficulty
   --difficulty-unlocked` are replaced by the class profile and the
   difficulty tier; `--quests --waypoints --act --gold --expansion
   --hardcore` are kept. The profile (`PROFILES`, `TIERS` in
   `playthrough.py`): level 30 / 60 / 85; base strength, energy,
   dexterity, vitality and life / mana (stats 0–3, 6–9) per tier; every
   role skill of the class at 20; right skill = the role a
   `--right-skill {role}` names, else `main`; left skill 0; a tiered
   weapon (and quiver, shield or class helm) per class and a tiered body
   armour, equipped. Difficulty d > 0 adds `--difficulty-unlocked d`
   and, for every lower difficulty, `acts=4` (3 in a classic save) and
   bit 0 of each quest slot `world/quests.md` §1.9 lists (§1.8 rule 4:
   the smallest record every completion reader accepts). Every cell adds
   `--difficulty d` (the town byte, with the save's `--act`), and each
   milestone's state-dump runs with `--difficulty d`.
4. **Roles.** `{role}` in any line is replaced by the cell's profile
   value before parsing: skill ids `{main}` (the class's main attack),
   `{summon}`, `{aura}`, `{buff}`, `{shift}`, `{trap}`, `{move}`,
   `{fire}`; unit classes `{pet}` (what `{summon}` creates) and
   `{trapunit}`. A role the class lacks is an error unless an `only
   class` line drops the milestone.
5. `player <field> delta <op> <n>`: the field's value minus its value in
   the player's first snapshot with a position. `player moved …` and
   `… delta …` take `since F`: measured from the player's first
   snapshot at frame ≥ F instead.
6. **Report.** Per difficulty a table of act (file) rows × class
   columns with reached / total; then the distinct blockers: each (file,
   milestone, status) that is not reached in some cell, with the cells,
   one cell's evidence and its command. A save `d2s-tool` refuses is the
   cell's `no-save` blocker. `--json` writes `playthrough-matrix-1`
   (`classes`, `difficulties`, `cells`: the per-act summary keys plus
   `class`, `difficulty`, `work`, `milestones`; `blockers`), `--markdown`
   the tables. `--profiles` prints each cell's `d2s-tool new` arguments
   for a save with only `--expansion`. Exit 0 when nothing is blocked,
   else 1.
7. `unit … lvl L,…` filters on the unit's level stat (`lvl`, stat 12;
   monsters: `monsters/init.md` §7).
8. `@pI` in a poke (any argument): the unit the milestone's poke I
   (0-based, file order) created. A probe run up to the frame before the
   first poke naming one reads the GUIDs from its poke records; `@pI`
   becomes `<type>/<guid>` (`poke.md` §1 r1; type 1 for `spawn` and
   `superunique`, 2 `object`, 3 `missile`, 4 `item`). The earlier pokes
   must be in file and frame order. A poke that created nothing gives
   `missing-unit`.
9. `traces/playthrough/classes.play` holds the class milestones: the
   main skill kills a Quill Rat set to 1 hp (no Quill Rat resistance
   reaches 100 on any difficulty; a Hell Zombie is cold immune); summons appear and follow the player
   through a waypoint warp (`world/hirelings.md` §6 rule 1); Holy Fire
   kills a 1-hp Zombie; Battle Orders and Werewolf raise the player's
   `hpx`; a Lightning Sentry appears and kills; Teleport moves the
   player 8+ sub-tiles in under 10 frames; per difficulty a Blood Moor
   Zombie's level (`monsters/init.md` §7).

## Constants & data dependencies

Ids in objective files are row indices of the user's tables
(`levels.txt`, `monstats.txt`, `objects.txt`). The harness reads no
table; the file comments name each id.

## Randomness

None in the harness. Each run is deterministic for its save, seed,
pokes and input (`state-snapshot.md` test vector 5).

## Edge cases & original bugs

1. Act I town NPCs whose rooms are not active at the start (Akara) are
   not in the snapshot; `town-start` leaves them out. Not checked
   against 1.14d.
2. `pos` across the outdoor grid can land the player in a neighbouring
   level. That is why sweep milestones test `ever player lv`.

## Test vectors

| Input | Expected |
|---|---|
| `--selftest`: synthetic state file (header, 3 snaps, a spawn poke) | town → `wrong-level`; kill → `reached` at f3; revived unit → `stuck` "1 alive"; `unresolved` spawn → `missing-unit`; `ever` holds at f1; spiral and grid sweep points; exit codes 0 / 1 |
| `--selftest`: a fixed 96-byte record (slot 1 = 0x2002, slot 6 = 1, slot 41 = 0x8000) as `q` | `quest_bit` set for 1.1, 1.13, 6.0, 41.15, clear for 1.0, 41.14, 0.0, 7.0; `quest 42 0`, `quest 1 16`, `quest 1 0 on` and wrong arity rejected; `quest 6 0 set` stuck with "quest 6.0 clear", `ever` reached at f2; no `q` → stuck "no 'q'"; `need ever quest …` parsed |
| `act1.play` on the 1.14d install, 2026-10-09 | 12/14 reached; `kill-zombie` and `andariel-killed` stuck (see Open questions 1–2) |

## Provenance

§4 profile ids: skills.txt `Id` / `charclass` / `summon`, monstats.txt
`hcIdx`, weapons.txt / armor.txt `code`, levels.txt `MonLvl*`, read
2026-10-09 from the private data repo's `extracted/Patch_D2.mpq` and
`extracted/d2exp.mpq` tables (`python3 -I` csv reads); the role choice
(which skill is a class's main attack) is ours.

d2rs-own tool. The milestone choice comes from `world/quests.md` and the
Act I quest specs. Level, monster and object ids were read from the
install's tables with `mpq-tool extract`.

## Open questions

1. `kill-zombie`: a Zombie spawned next to a Sorceress dies to her
   Frost Nova (right click; mode 0 and hp 0 at f50). At f65 it is back
   in mode 5 / 1 with hp 0 and stays alive. This looks like a d2rs death
   bug. Not compared with 1.14d.
2. `andariel-killed`: Andariel, moved next to the player with hp set to
   1, never takes damage. The right click (Frost Nova) in Catacombs 4
   gives player mode 2 (walk) or 1, not cast mode 10, even though the
   same save casts in the Blood Moor. A `missile` poke (Fire Bolt) was
   also tried and did not hit. It is still open whether this is the
   headless input or the game.
3. *Answered* (2026-10-09): the player's quest record is `q` in
   `state-1` and the `quest` predicate (§2 r4) reads it; `act1.play` has
   `den-of-evil-done`, `andariel-done` and `act2-open`. Open: those three
   need an NPC talk (Akara msg 76, Warriv msg 183, Warriv's "Go east"),
   and the d2rs headless input has no hover pick, so a click on an NPC
   walks instead of interacting (`scenario-diff.md` Open question 4).
   Until an NPC-interaction input or poke exists they are expected
   blockers whose evidence is the quest bit still clear.
4. Acts II–V objective files are written (`act2.play` … `act5.play`);
   `act4-forge.play` (2026-10-09) starts every milestone from the
   `a4-hellforge` checkpoint.
