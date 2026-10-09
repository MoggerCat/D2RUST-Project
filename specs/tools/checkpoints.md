# Spec: Tools — Checkpoint saves (start a session at its blocker)

- **Status:** draft (2026-10-09): format `checkpoint 1` and the build /
  load check below are ours; `tools/checkpoints/make.py` implements
  them, with 14 definitions in `traces/checkpoints/` (one before each
  act boss and act-gating quest step). d2rs load results: §4 and
  `traces/checkpoints/d2rs-start.tsv`. The 1.14d load of each save is
  queued (`docs/HANDOFF.md` §5) unless recorded here.
- **Target version:** 1.14d (the saves and the quest bits); the format
  is d2rs-own.
- **Crate/module:** `tools/checkpoints/make.py` (Python stdlib); it runs
  `d2s-tool new` / `check` and `d2-client state-dump`.
- **Related specs:** `formats/d2s.md` (the save), `world/quests.md` §1.9
  (the quest bits per act and what opens the next act), `world/waypoints.md`
  §1 (waypoint of a level), `tools/poke.md` §1, §6 (the start pokes,
  `goto`), `tools/playthrough.md` §1 r5 (the `checkpoint` start),
  `tools/state-snapshot.md` (what the load check reads).

## Summary

A playthrough session that works on a blocker (Andariel, the
Hellforge, Baal) should not first play or poke its way through the acts
before it. A **checkpoint** is a character at one milestone: class,
level, stats, skills and gear fit for that point, the quest bits and
waypoints a player there would have, the town of the act, and the
directives that move the player from town to the blocker. A definition
is a small committed text file; the save is generated from it with
`d2s-tool` and never committed (CLAUDE.md rule 1: `*.d2s` is ignored;
the default output is `target/checkpoints/`).

## Inputs

| Name | Type | Source |
|---|---|---|
| definition | text, §2 | `traces/checkpoints/<name>.checkpoint`, `<name>` matching `[a-z0-9-]+` |
| install | the user's 1.14d files | `D2_GAME_DIR` / `--game-dir` (`d2s-tool` reads the tables) |

## Outputs / state changes

`<out>/<name>.d2s` (default `target/checkpoints/`), `<out>/<name>.start`
(the start pokes, one `<frame> <directive>` per line), and with
`--verify` the two state files of §4 (`<name>.load.jsonl`,
`<name>.start.jsonl`) and, with `--record FILE`, the table of §4 r3.
Exit 0 every check passed, 1 a check failed, 3 error.

## Rules

### 1. Which checkpoints

At least one just before each act boss and each act-gating quest step:
Den of Evil, Andariel, Radament, the Summoner, Duriel, Travincal,
Mephisto, Izual, the Hellforge, Diablo, Anya, Nihlathak, the Ancients,
Baal's chamber. Each sets the quest bits of every earlier step of
`world/quests.md` §1.9 (done = bit 0) and the act transitions before it
(`acts=N`, the bits of `quests.md` §8.1), and the waypoints of the acts
before it plus those of the act's levels already passed.

### 2. Definition `checkpoint 1`

1. UTF-8 lines; `#` starts a comment; blank lines are skipped. The
   first line left is `checkpoint 1` (the format version; any other is
   refused).
2. Keywords (one per line, arguments separated by spaces); unknown
   keywords, a wrong argument count, a value out of range and a
   non-repeatable keyword given twice are errors naming the line:

| Keyword | Arguments | Meaning |
|---|---|---|
| `name` | 2–15 letters | the character name (unique among the definitions) |
| `title` | text | what the checkpoint is |
| `class` | `ama sor nec pal bar dru ass` | class |
| `level` | 1–99 | character level (`d2s-tool --level`) |
| `expansion` | — | expansion character (needed for act 4 = Act V) |
| `hardcore` | — | hardcore |
| `difficulty` | `normal nightmare hell` | the difficulty of the town byte and of the game |
| `act` | 0–4 | the town act (`d2s-tool --act`) |
| `stat` (rep.) | stat 0–15, value | base stat as stored (life / mana / stamina 6–11 in 1/256) |
| `skill` (rep.) | class-list index 0–29, level | skill byte |
| `left-skill`, `right-skill` | skill id | the mouse skills |
| `gold` | n | stat 14 |
| `quests` (rep.) | `[diff:]acts=N` or `[diff:]slot.bit` … | quest bits |
| `waypoints` (rep.) | `[diff:]lv=LEVEL` or `[diff:]index` … | waypoints; `lv=` is the waypoint of a `levels` row (`waypoints.md` §1 r1) |
| `item` (rep.) | a `d2s-tool --item` value | gear, belt, inventory |
| `start` (rep.) | frame ≥ 1, directive | a start poke (`poke.md` §2 r6), frames non-decreasing |
| `need` (rep.) | `player lv\|act\|lvl N` | what holds after the start pokes (§4 r2) |
| `note` (rep.) | text | free text |

3. Required: `name`, `title`, `class`, `level`, `difficulty`, `act`.

### 3. Building the save

`d2s-tool new` with the flags of each keyword, then `--act A
--difficulty D`, and fixed `--seed 1 --map-seed 1 --time 0x60000000`,
so one definition always gives the same bytes; then `d2s-tool check`
(the save rewrites byte for byte).

### 4. The load check (d2rs)

1. `d2-client state-dump --save <name>.d2s --seed 1 --difficulty D`
   twice: the **load** run (`--ticks`, default 25, no pokes) and the
   **start** run (the start pokes; ticks = the larger of `--ticks` and,
   per start poke, its frame + 20, or + 420 for a `goto`, whose walk
   may take up to 400 steps, `poke.md` §6 r3.4).
2. Passes when, at the load run's last snapshot, the player's `lvl` is
   the definition's level, its `act` the definition's act, and every
   explicit `slot.bit` of the game's difficulty is set in `q`; and
   after the start run every start poke's result is `ok` and each `need`
   holds.
3. `--record FILE` writes `checkpoint-start-1`: two `#` lines (format,
   tool and command), a note line, then one tab-separated row per
   checkpoint: name, load verdict, `lvl`, `act`, `lv`, `x`, `y`, `hp`,
   `mp`, the set quest bits, the start verdict, the start `lv`, `x`, `y`
   and the start pokes' results. These are d2rs facts only.
4. The 1.14d side of the load check is the same save loaded by the
   original (`tools/cloud-game/`, or PC 1): the character enters the
   town of its act at its level, with the quest log showing the
   definition's steps done. Queued per checkpoint in `docs/HANDOFF.md`
   §5 until run.

## Constants & data dependencies

Ids in definitions are row indices of the user's tables (`levels`,
`monstats`, `objects`) named in each file's comments; `make.py` reads
no table (`d2s-tool` does).

## Randomness

None: fixed seeds and times (§3).

## Edge cases & original bugs

1. Bit 13 of a quest slot is cleared on load (`quests.md` §1.6), so a
   definition sets bit 0 (and the precondition bits it needs), never 13.
2. A save's stats are written as given: a definition that allocates
   points also sets life, mana and stamina (the sorceress definitions
   use creation 40 / 35 / 74, +1 / +2 / +1 per level, +2 life and +1
   stamina per vitality, +2 mana per energy).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| `make.py --selftest` | the definitions parse; every malformed line kind refused; the `d2s-tool` flags of a definition; the load check verdicts | synthetic |
| `make.py --verify --record traces/checkpoints/d2rs-start.tsv` on the 1.14d install | every checkpoint loads in d2rs (§4 r2) | the table |

## Provenance

d2rs-own tool. Quest bits and gating from `world/quests.md` §1.9 and the
act quest specs; ids as `traces/playthrough/act*.play`.

## Open questions

None.
