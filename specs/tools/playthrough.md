# Spec: Tools — Playthrough harness (how far d2rs gets through an act)

- **Status:** draft: a crude first version. Format `playthrough 1`, the
  predicates and the verdicts below are ours; `tools/playthrough/playthrough.py`
  implements them and `traces/playthrough/act1.play` is the first
  objective file (run 2026-10-09 on the 1.14d install: 12 of 14
  milestones reached, first blocker `kill-zombie`). d2rs only: no 1.14d
  side, so a reached milestone is "the game gets there", never a
  fidelity check (CLAUDE.md rule 10).
- **Target version:** 1.14d (the milestones); the format is d2rs-own.
- **Crate/module:** `tools/playthrough/playthrough.py` (Python stdlib);
  it runs `d2-client state-dump` and `d2s-tool new`.
- **Related specs:** `tools/state-snapshot.md` (the state file it reads),
  `tools/poke.md` (directives, absolute `--poke` form §2 r6),
  `tools/scenario-diff.md` §2 r4 (the `input` script), `world/quests.md`
  and `world/quests-act1*.md` (where the milestones come from).

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
| `act1.play` on the 1.14d install, 2026-10-09 | 12/14 reached; `kill-zombie` and `andariel-killed` stuck (see Open questions 1–2) |

## Provenance

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
3. There are no quest flags in the state file yet. Milestones that need
   them ("Den cleared", quest complete, Act II unlocked) are missing; the
   minimal addition would be the player's quest record in `state-1`.
4. Acts II–V objective files are not written yet.
