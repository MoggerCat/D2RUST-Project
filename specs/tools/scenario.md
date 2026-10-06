# Spec: Tools — Scenarios (differential runs of 1.14d and d2rs)

- **Status:** implemented: the script parser and writer, the scenario
  trace reader and writer, the comparator (`conformance::scenario`) and
  the d2rs runner (`tools/scenario-run`) pass their tests on synthetic
  data (same scenario twice → identical trace; every injected
  perturbation found at its record). The original-side runner is not
  written (`docs/handoff/scenario-harness.md`); no scenario has been
  compared with 1.14d yet.
- **Target version:** 1.14d (the original side); the formats are d2rs-own.
- **Crate/module:** `conformance::scenario` (`script`, `trace`,
  `compare`); `tools/scenario-run` (d2rs runner and CLI)
- **Related specs:** `traces/FORMAT.md` §Scenario traces (owns the trace
  records); `sim/intents-events.md` §1 (loop order), §6 (exact-match
  comparison); `sim/client-messages.tsv` (typed messages);
  `sim/rng.md` §5.3 (game seed); `world/waypoints.md` §5 (waypoint
  objects); `tools/scenario-masks.tsv` (§6).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 38–54 |
| Inputs | 55–62 |
| Outputs / state changes | 63–69 |
| Rules | 70–71 |
|   1. Files | 72–81 |
|   2. Script syntax | 82–175 |
|   3. Typed messages and references | 176–231 |
|   4. Run model | 232–290 |
|   5. Comparison | 291–321 |
|   6. Masks | 322–342 |
| Edge cases & original bugs | 343–354 |
| Test vectors | 355–370 |
| Provenance | 371–380 |
| Open questions | 381–397 |
<!-- /index -->

## Summary

Differential testing: one scripted **scenario** (a game start and a list
of client→server messages at fixed ticks) runs on the original 1.14d
and on d2rs. Each side writes a **scenario trace** (`traces/FORMAT.md`
§Scenario traces): per tick, the messages it injected, the server→client
messages the server queued, the game seed, and unit snapshots at the
ticks the scenario names. The comparator reads both traces and reports
the **first divergence** (tick, stream, record, byte or field, expected
and got, with context) and a summary. Scripts are ours and committed
(`traces/scenarios/`); traces are written to `traces/raw/` (gitignored).

This spec owns the script format (§2–§3), the run model both runners
follow (§4) and the comparison (§5–§6). Nothing in it is a 1.14d fact
except the cited masks (§6); what the original-side runner needs from
1.14d is listed in Open questions.

## Inputs

| Name | Type | Source |
|---|---|---|
| scenario script | text, §2 | `traces/scenarios/<name>.scenario` |
| game data | the user's install (live) or the synthetic install (CI) | `game/`, `test-fixtures` |
| two traces | JSON lines, `traces/FORMAT.md` §Scenario traces | the two runners |

## Outputs / state changes

- A runner writes one trace per run (§4).
- The comparator writes a report (§5 rule 7) and an exit code: 0 match,
  1 diverged, 2 partial (no divergence, but something not compared),
  3 error (unreadable trace, different scenarios).

## Rules

### 1. Files

1. A script is `traces/scenarios/<name>.scenario`: UTF-8, LF line ends,
   committed. `<name>` matches `[a-z0-9-]+` and equals the script's
   `name` line.
2. A trace is `traces/raw/<name>.<side>.trace.jsonl` by convention
   (`<side>` = `original` or `d2rs`); runners take any output path.
3. Scripts hold only our inputs (numbers, message names, item codes);
   never Blizzard data (`CLAUDE.md` hard rule 1).

### 2. Script syntax

1. Line based. Tokens are separated by runs of spaces and tabs. A token
   that begins with `#` starts a comment that runs to the end of the
   line (`#` inside a token is data, as in `@1:148#1`). Blank lines are
   skipped.
2. The first line that is not blank or a comment is `scenario 1`: the
   format version. A reader rejects any other version.
3. Every other line starts with a keyword (tables below). All lines
   except `at` lines come before the first `at` line, in any order.
   Unknown keywords, missing or extra tokens, a repeated single-use
   line and a value out of range are errors that name the line number;
   nothing is defaulted except where a table says so.
4. Numbers are decimal or `0x` hex, unsigned 32-bit unless a table says
   otherwise; a signed field accepts a leading `-`.
5. Steps (`at` lines) are in non-decreasing tick order and each tick is
   ≤ `end`. Steps with the same tick run in script order.
6. The canonical form (what the writer prints) is: `scenario 1`, the
   header lines in the table order, the character lines in the table
   order (stats, skills, waypoints, items in script order), `record`,
   `snapshot every`, `snapshot at`, then the steps; typed fields in
   layout order (§3 rule 2); numbers decimal except `seed` (`0x` and 8
   hex digits) and hex steps (two lower-case hex digits per byte).
   Parsing the canonical form gives the same scenario.

Header (each line once, all required):

| Line | Meaning |
|---|---|
| `name <name>` | as §1 rule 1 |
| `game 1.14d` | only value accepted |
| `seed <0..2^31−1>` | T: the time value of game creation, in place of the clock's (`tools/original-hooks.md` §2 rule 1; §4 rule 1) |
| `init <u32>` | I: the init value written to game +0x7C (original-hooks §2 rule 1); the DRLG seed unless the character has a map ID (§4 rule 1) |
| `difficulty normal\|nightmare\|hell` | game difficulty |
| `expansion yes\|no` | LoD game |
| `end <tick>` | last tick run, 0 ≤ tick ≤ 1,000,000 |

Character: `char save <name>` and/or inline `char` lines; at least
one. `<name>` is a character name (2–15 letters, `_` or `-`, starting
with a letter): the save `<save dir><name>.d2s` the original side loads
(original-hooks §5.3–§5.4). Inline lines describe the same character
for a runner without a save loader (d2rs, until `formats/d2s.md`
exists); that they match the save is the script author's claim, which
a run checks only through the comparison.

| Line | Once | Default | Meaning |
|---|---|---|---|
| `char class <0..6>` | yes | required | character class (charstats row) |
| `char level <1..99>` | yes | 1 | character level: stat 12 only (experience is `char stat 13`) |
| `char area <act 0..4> <level id>` | yes | required | the level the character starts in |
| `char map <u32>` | yes | none | the save's map ID (`.d2s` +0xAB) when it applies (original-hooks §2: the save has reached the difficulty) |
| `char at default` / `char at <x> <y>` | yes | `default` | start position in sub-tiles (absolute); `default`: where the game puts a character entering `area` (§4 rule 3) |
| `char stat <stat id> <i32>` | per id | — | base value (layer 0) of an itemstatcost row, set after creation; overrides the creation value |
| `char skill <skill id> <1..255>` | per id | — | hard skill points |
| `char waypoint <level id>` | per id | — | a known waypoint, on the game's difficulty |
| `char quest <0..40> <u16>` | per index | — | quest flags of quest index n on the game's difficulty |
| `char item <code> <place> [<key> <value>]...` | no | — | an item (item table below) |

Items: `code` is 3–4 characters `[a-z0-9]`. Place: `inv <x> <y>`,
`stash <x> <y>`, `cube <x> <y>` (grid cell, 0..15 each), `belt <slot
0..15>`, `body <bodyloc 1..12>`, or `socket <0..5>`: inside the nearest
earlier item that is not itself in a socket (an error when there is
none). Optional keys, any order (canonical: the table order):

| Key | Value | Rule |
|---|---|---|
| `quality` | `low`, `normal`, `superior`, `magic`, `set`, `rare`, `unique`, `crafted` | once |
| `ilvl` | 1..99 | once |
| `prefix`, `suffix` | magic affix row (u16) | up to 3 each; only with quality `magic`, `rare` or `crafted` |
| `unique` | uniqueitems row | once; exactly when quality is `unique` |
| `set` | setitems row | once; exactly when quality is `set` |
| `runeword` | runes row | once; needs `sockets` |
| `sockets` | 1..6 | once |

Recording:

| Line | Meaning |
|---|---|
| `record <stream>...` | streams to record, each once: `s2c`, `rng`, `units`, `stats`, `frames`. `c2s` (the injected messages) is always recorded and is not listed |
| `snapshot every <n>` | n ≥ 1: snapshot ticks 0, n, 2n, … ≤ `end` |
| `snapshot at <tick>...` | snapshot ticks, each ≤ `end` |

`units` and `stats` are written at the snapshot ticks; `end` is always a
snapshot tick. `snapshot` lines without `units` or `stats` recorded are
an error.

Steps:

| Line | Meaning |
|---|---|
| `at <tick> hex <byte>...` | a raw C→S message: 1 to 516 bytes, two hex digits each |
| `at <tick> msg <Name> <field>=<value>...` | a typed C→S message (§3) |
| `at <tick> spawn <class> <x> <y> <kind> [umod <id>...]` | a monster spawned by the server (§3.1) |

### 3. Typed messages and references

1. `<Name>` is a `name` of `sim/client-messages.tsv` whose transport
   size is fixed and non-zero and whose layout fields are all `u8`,
   `u16`, `u32`, `uN` or `bitN`; any other message is written as `hex`.
2. Every layout field is given exactly once (any order; the canonical
   order is the layout's). The message is the row's fixed size, byte 0
   is the row id, bytes no field covers are 0; fields are stored little
   endian at their offsets; `uN` and `bitN` fields are OR-ed into the
   u32 at their offset. A number that does not fit its field is an
   error (parse time for numbers, injection time for references).
3. A value is a number (§2 rule 4) or a reference, resolved at the
   injection of the step (§4 rule 2), on the state after tick t − 1:

<!-- rows -->
| Reference | Value |
|---|---|
| `@player` | the scenario character's GUID |
| `@x`, `@y`, `@x+N`, `@x-N`, `@y+N`, `@y-N` | the character's current position (sub-tiles) ± N |
| `@<type>`, `@<type>#<n>` | GUID of the n-th (from 0) unit of unit type 0–5, in ascending GUID order |
| `@<type>:<class>`, `@<type>:<class>#<n>` | the same among units of that class |
| `@wp`, `@wp#<n>` | the same among objects (type 2) whose `objects` row has operate function 23 (`world/waypoints.md` §5) |

4. The units searched are every unit in the game's unit lists
   (`sim/unit-order.md`), the character included. Position is the
   unit's path position (`sim/pathing.md`; static position for objects
   and items).
5. A reference with no unit, or a position outside its field, does
   not stop the run: the step is written as an unresolved `c2s` record
   (`traces/FORMAT.md`), nothing is injected, and the run goes on.

#### 3.1 Spawn steps

1. A spawn step makes the server create monsters through the
   original's own population functions (`monsters/population.md`,
   `monsters/init.md`), not through a message. `class` is a monstats
   row; `x`, `y` are sub-tiles (numbers or references, §3 rule 3); the
   room is the active room that contains the point.
2. `kind` and the call sequence both runners make:

<!-- rows -->
| kind | umods | Calls, in order |
|---|---|---|
| `normal` | none | one placement `0x005B2A00` at (x, y): mode 1, radius −1, flags 0 (population.md placement) |
| `random-boss` | none | random boss `0x005A43E0`(room, no coordinate list, class, champion allowed, x, y, no warp check), then champion minions `0x0054E1E0` (population.md §6.2, §6.4; the room-population path) |
| `champion` | exactly 1 | boss spawn `0x005A09E0` at (x, y) (§6.3), champion pack member `0x005A48C0`(boss, umod) (init.md §16.2), champion minions `0x0054E1E0` (§6.4) |
| `unique` | 1–9 | boss spawn `0x005A09E0` at (x, y), the umods appended to the boss's list in order, then unique minions and modifier init `0x005A2120`(boss, 3, 6) (§6.5, init.md §18) |

3. The step's `spawn` record (FORMAT.md) holds the GUID of the unit
   the first call returned, or `failed` when it returned none; the
   minions and every draw of the calls show in the unit and rng
   records. Spawns run at (a) of §4 rule 2, in script order with the
   messages (which only queue), so their draws come before the drain.
4. A spawn with a reference that does not resolve is written as an
   unresolved `spawn` record and nothing is spawned (§3 rule 5).

### 4. Run model

Both runners follow these rules; a runner that cannot follow one says
so in its trace header's `gaps` (FORMAT.md) instead of approximating.

1. **Game.** A single-player game (game type +0x6A = 3,
   original-hooks §5.2) created with `difficulty` and `expansion`, one
   client (id 0), the scenario's character joined in `area`. The game
   seed is `{seed, 666}`, then stepped once (`0x0052C2C6`); game +0x7C
   is `init` (original-hooks §2 rule 1: the original-side runner sets
   both at `0x0052C2BB` and `0x0052C2E3`). Every seed game creation
   derives from the game seed follows (`sim/rng.md` §5.2). Each act's
   DRLG seed is `init_low` of `char map` when given, else of `init`
   (original-hooks §2 rule 2).
2. **Ticks.** Tick 0 is the first server tick after the tick in which
   client 0 first reaches state 4 (in game; original-hooks §1 rule 5,
   §3 rule 2): on 1.14d the frame F0 + 1, where F0 is that tick's
   frame (game +0xA8); tick t is frame F0 + 1 + t. Steps of tick t are
   injected at the first stop at `0x0044F136` after the return of
   frame F0 + t (original-hooks §3 rule 1). For each tick t = 0 …
   `end`, in order: (a) resolve the steps of tick t and inject them,
   in script order, at the net send (`sim/intents-events.md` §2.1
   rules 3–4: the classifier and the server queues apply), after the
   client's duplicate filter (§2.1 rule 1 does not apply: a scripted
   message is never dropped by the sender);
   (b) the drain (§2.1 rule 7) and dispatch of everything queued;
   (c) the tick (`sim/tick.md` §3); (d) the records of tick t.
3. **Start position.** `char at default` is the position the game
   gives a character entering `area` (`drlg/levels.md` §10; open
   question 1). d2rs does not place a joining character yet and stands
   it 5 sub-tiles right of and below the area's first waypoint object
   (`test-fixtures` staging), and writes that as a gap.
4. **c2s.** Every message step of tick t, in order: its bytes as
   injected, or the unresolved reference (§3 rule 5); spawn steps write
   `spawn` records (§3.1 rule 3). Both count in the step index `i`.
5. **s2c.** Every message queued for the character's client from the
   start of (a) to the end of (c), in queue order: the O(F) of
   `sim/intents-events.md` §6 rule 1 for F = tick t.
6. **rng.** The game seed (`rng.md` §5.2) before (a) and after (c).
   A runner that can see single draws also writes each draw of the
   game seed in that window (`draw` records); runners that cannot omit
   them and do not list `rng-draws` in `streams`.
7. **units.** At a snapshot tick, after (c): every unit of the game's
   unit lists, ordered by (unit type, GUID): type, GUID, class, mode,
   position (§3 rule 4), life and mana (stats 6 and 8, layer 0: the
   raw value in the unit's full stat array, fixed point × 256, not the
   unit-total reader; 0 when absent or without a stat list;
   original-hooks §4 rule 3).
8. **stats.** At a snapshot tick, after (c): for each player unit, the
   base entries of its stat list (stat, layer, value), ordered by
   (stat, layer).
9. **frames.** Reserved: an index-frame hash per tick
   (`render/capture.md` §6). Neither runner writes it yet; a runner
   asked for it lists it as a gap.
10. A runner does not stop at a server-side rejection (a refused,
    dropped or malformed message): that is behaviour to compare. It
    stops only when it cannot build the game or the script names
    something it cannot run; then it writes no trace.

### 5. Comparison

1. **Same scenario.** The two headers must agree on `format`,
   `version`, `game_version`, `scenario`, `scenario_sha256`, `seed`,
   `init` and `end`, and both traces must end with their `end` record; otherwise
   the comparison is an error, not a divergence.
2. **Streams.** `c2s` is always compared. A stream the scenario
   records is compared when both headers list it in `streams`;
   `rng-draws` when both list it. A requested stream missing on a side
   is reported as not compared.
3. **Order.** Tick by tick from 0; within a tick the streams in the
   order `c2s`, `spawn`, `s2c`, `rng`, `draw`, `unit`, `stats` (`spawn`
   belongs to the `c2s` stream); within a stream
   record by record. The first difference ends the comparison.
4. **Records.** A record present on one side only is a difference
   (`missing` / `extra`). `s2c`: client, then length, then the bytes
   from offset 0, skipping masked bytes (§6). `c2s`: resolved vs
   unresolved, then the bytes. Other records: field by field in the
   order of their table in FORMAT.md; a `draw`'s `site` is never
   compared (a 1.14d address against a d2rs label).
5. **Divergence.** Reported as tick, stream, record index within the
   tick's stream, where (`bytes[k]` or a field name), expected (the
   original's value) and got (d2rs'), and context: for messages, the
   message id and size and up to 8 bytes either side of the offset on
   both sides; the previous record of the stream in that tick, if any.
6. **Verdict.** `match`: no divergence, every requested stream compared
   on both sides and no gaps; `partial`: no divergence, but a stream
   not compared or a gap on a side; `diverged`: a divergence.
7. **Summary.** Records compared per stream, ticks compared, masked
   bytes skipped, streams not compared, both sides' gaps.

### 6. Masks

1. `tools/scenario-masks.tsv` lists the S→C bytes the original does not
   write or fills from the clock: `id`, `offset`, `length` (`*` = to the
   end of the message), `source` (the spec that states it). Masks apply
   to `s2c` records of that id on both sides; the message length is
   still compared.
2. Nothing else is masked (`sim/intents-events.md` §6 rule 3). A new
   mask needs a spec that states the bytes are unwritten or
   clock-derived.
3. The table is read with strict errors: four columns, a hex id that is
   an S→C id (≤ 0xB4), a decimal offset, a decimal length or `*`.

<!-- rows -->
| id | bytes | why |
|---|---|---|
| 0x2A | 3–6 | not written by the builder (`world/npc.md` §9, edge case 1) |
| 0x50 | 13–14 | uninitialized stack in the sender (`world/quests.md` open question 5) |
| 0x58 | 6 | not written (`world/npc.md` §8.1, edge case 10) |
| 0x8F | 1–end | Pong: clock values, a transport row (`sim/intents-events.md` §6 rule 3) |

## Edge cases & original bugs

1. A step that targets a unit the game does not have yet (a monster not
   spawned) is unresolved on that side only: the comparator reports it
   as a `c2s` divergence at that tick (§3 rule 5, §5 rule 4).
2. Two steps at one tick are drained in one frame, in order (§2 rule 5,
   §4 rule 2).
3. A step at the `end` tick is injected and its replies are recorded
   (§4 rule 2: (a) comes before (c)).
4. `snapshot every` larger than `end` gives the snapshot ticks 0 and
   `end`.

## Test vectors

All synthetic (CI): `tools/scenario-run/tests/`,
`crates/conformance/src/scenario/` unit tests.

| Input | Expected |
|---|---|
| every script in `traces/scenarios/`, parsed and written | canonical text parses to the same scenario; writing it again gives the same text |
| random scenarios (proptest) | write → parse round trip is the identity |
| each malformed line of the strict-error table (unknown keyword, repeated line, step out of order, tick > end, missing field, number too wide, unknown message) | an error naming that line |
| `msg Walk x=10 y=20` | `01 0a 00 14 00` |
| `msg SelectSkill skill=36 left=0 item=0xFFFFFFFF` | `3c 24 00 00 00 ff ff ff ff` |
| every starter scenario run twice on the synthetic install | byte-identical traces |
| a trace against itself with one byte, field or record changed (each perturbation of every stream) | `diverged` at exactly that tick, stream, index and offset |
| two traces differing only in masked bytes of 0x2A | `match` |

## Provenance

- d2rs-own format and run model: no 1.14d fact in §1–§5. The loop order
  (§4 rule 2) and the O(F) definition (§4 rule 5) restate
  `sim/intents-events.md` §1 and §6 rule 1 by reference.
- Masks (§6): each row cites the spec that states the bytes are not
  written or clock-derived.
- Typed messages (§3) are encoded from the generated client message
  table (`d2_proto::CLIENT_MESSAGES`, from `sim/client-messages.tsv`).

## Open questions

Answered by `tools/original-hooks.md` (on `claude/specs-staging`):
seed override and DRLG seed (§2), tick 0 and the injection stop (§1
rule 5, §3), game type 3 (§5.2), the unit snapshot fields (§4). Open:

1. Where 1.14d places a character entering a level (`levels.md` §10),
   so `char at default` means the same on both sides (a save loads at
   its town start; `char at <x> <y>` needs a placement write).
2. The conventions of the §3.1 spawn calls outside room population and
   the monster data field of the umod list (handoff §4 Q12–Q14).
3. How a joining character gets items of a given quality, affixes,
   unique / set / runeword id and sockets, and quest flags (handoff §4
   Q3), for the original side and for `char save`.
4. The base stat array of an extended stat list (for `stats` records;
   original-hooks §4 gives the full array only).
