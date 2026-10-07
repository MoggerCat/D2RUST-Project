# Spec: Simulation — Units (kinds, lifecycle, modes, timer events)

- **Status:** draft (no `d2-sim` code yet); rules read from the 1.14d
  `Game.exe` code; the per-kind event rules U1–U10 (Test vectors) pass
  `check_units.py` on all three tick recordings of 2026-10-06 (21,297
  schedules, 48,316 runs, 14,034 schedules checked exactly, 0 errors);
  U4 with animation data and U11 (per call site) need a 0.2.0 recording
  (open question 1).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::units` (unit record, allocation, removal,
  modes, mode schedules, event dispatch)
- **Related specs:** `sim/tick.md` (timer queue, run order, class
  dispatchers §5.6); `sim/unit-order.md` (GUIDs, lists, add/remove
  bookkeeping); `sim/rng.md` §5.3 (unit and item seeds);
  `sim/stats.md`, `sim/stat-lists.md` (events 3, 5, 8, 9, 12 internals);
  `formats/animdata.md` (animation records); `data/fields.tsv`
  (monstats, monstats2, objects columns). Machine-readable:
  `sim/unit-events.tsv` (every 1.14d scheduler call site),
  `sim/unit-handlers.tsv` (dispatch tables).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 43–58 |
| Inputs | 59–67 |
| Outputs / state changes | 68–73 |
| Rules | 74–75 |
|   1. Unit kinds | 76–95 |
|   2. Unit record | 96–134 |
|   3. Lifecycle | 135–302 |
|   4. Modes and mode schedules | 303–445 |
|   5. Event dispatch | 446–460 |
|   6. Events per kind | 461–577 |
|   7. Scheduler inventory (`unit-events.tsv`) | 578–599 |
| Constants & data dependencies | 600–616 |
| Randomness | 617–624 |
| Edge cases & original bugs | 625–643 |
| Test vectors | 644–703 |
| Provenance | 704–743 |
| Open questions | 744–773 |
<!-- /index -->

## Summary

A unit is one of six kinds (player, monster, object, missile, item,
tile). The server allocates it (`0x00555230`), optionally adds it to a
room, changes its mode over time and removes it (`0x00555600`). All unit
behaviour is driven by timer events (`tick.md` §5): a mode change
cancels the unit's pending mode events and schedules new ones from the
mode's animation (action frames as event 0, the end as event 1) or one
every-tick event 0 for moving modes; the other 13 event types are
scheduled by regeneration, AI, states, skills, objects and items. This
spec owns the unit record, the lifecycle, the mode schedules and, per
kind and event type, who schedules what with which expire and arguments
and which handler runs it. Handler internals that belong to later
systems (AI, skills, missiles, objects, items, stats) are named with
their entry points and owners.

## Inputs

| Name | Type | Source |
|---|---|---|
| unit kind, class id, mode, position, room | allocation arguments | the spawning system |
| game frame | i32 | `tick.md` §2 |
| animation record | 160-byte AnimData record | `formats/animdata.md` |
| monstats `aidel`, monstats2 `*mv` bits, objects `FrameCnt1`, `Parm0`/`Parm1`, `Mode2`, `HasCollision2` | table cells | `data/fields.tsv` |

## Outputs / state changes

Unit records, their timers (scheduled through `tick.md` §5.2/§5.3,
cancelled through §5.4), mode and animation fields, update-queue entries
(`unit-order.md` §6).

## Rules

### 1. Unit kinds

| Type | Kind | Timer class | Per-kind init (`0x00555230` table `0x005554E8`) | Free (`0x00555600`) | Event dispatcher (`tick.md` §5.6) |
|---|---|---|---|---|---|
| 0 | player | 0 | `0x005348C0`, then `0x005B1880(…, 0)` unless mode 17 | `0x005B1A20`, `0x00535430`, `0x005407A0`, `0x005349D0`, path | `0x00581220` |
| 1 | monster | 1 | mode := arg; `0x00574250` | `0x005B1A90`, path, `0x005736A0` | `0x005A7F80` |
| 2 | object | 3 | 0x38-byte object data at +0x14 (field 0 = objects.txt row, `0x00640E90`), mode := arg, `0x00623520`, `0x0054F5D0` | `0x00552A00`, `0x00623570`, data | `0x00586AD0` |
| 3 | missile | 2 | mode := arg; `0x0059F8A0` | path, `0x0059F8E0` | `0x005ADCC0` |
| 4 | item | 4 | mode := arg, `0x00623520`, `0x00555D20` | `0x00557EE0`, `0x00627D00`, `0x00623570` | `0x00562DA0` |
| 5 | tile (warp) | none | `0x00623520`; flags \|= 0x2; GUID written at +0x0C here | — | none: never scheduled |

Every free routine of types 0–4 cancels all the unit's timers
(`unit-order.md` §3.2). Mode lists: players 20 (plrmode.txt rows: DT NU
WL RN GH TN TW A1 A2 BL SC TH KK S1 S2 S3 S4 DD, 18 sequence, 19
knockback), monsters 16 (monmode.txt: DT NU WL GH A1 A2 BL SC S1 S2 S3
S4 DD, 13 knockback, 14 sequence, 15 RN), objects 8 (objmode.txt: NU OP
ON S1–S5); row order read from the 1.14d table dump
(`traces/raw/20261006-021210-tables`). Item and missile mode values are
owned by the item and missile specs; no event rule here depends on them.

### 2. Unit record

Fields the simulation uses, 1.14d offsets (D2MOO `D2UnitStrc` names; each
offset seen in the 1.14d code named in the last column):

| Offset | Field | Rule | 1.14d evidence |
|---|---|---|---|
| +0x00 | type | §1 | `0x00555230` |
| +0x04 | class id | row of the kind's table | `0x00555230` |
| +0x08 | memory pool | game +0x1C | `0x00555230` |
| +0x0C | GUID | `unit-order.md` §1 | — |
| +0x10 | mode | §4 | `0x00624690` |
| +0x14 | per-kind data | player/monster/object/missile/item data | `0x005553C8`, `0x005A73E0` |
| +0x18 | act (u8) | act of the allocation room's level | `0x005552ED` |
| +0x1C | act record | game +0xBC + 4·act | `0x005552FA` |
| +0x20, +0x28 | seed, init seed | `rng.md` §5.3 | — |
| +0x2C | path | freed at removal | `0x0055568B` |
| +0x30 | sequence record (null: plain animation) | §4.2 | `0x005539CF` |
| +0x34, +0x3C | sequence frame count, sequence speed | used instead of +0x48/+0x4C when +0x30 ≠ 0 | `0x005539D5` |
| +0x44 | current frame, 8.8 fixed point | 0 at mode start (`0x005533D0`); frame·256 after §4.2 | `0x00553AF1` |
| +0x48 | frame count, 8.8 (animation frames · 256) | set at mode start | `0x005533D0` |
| +0x4C | animation speed (i16, 1/256 frame per tick) | §4.3 | `0x00623F50` |
| +0x4E | action frame (u8) | 0 at mode start | `0x005533D0` |
| +0x50 | AnimData record | `formats/animdata.md` §5 | `0x00620F00` |
| +0x5C | stat list | `sim/stat-lists.md` | `0x00625480` |
| +0x60 | inventory | — | `0x00620F00` |
| +0x74 | quest chain | 0 at allocation, freed at removal | `0x005552FD`, `0x00555644` |
| +0x80 | game | — | `0x005552B0` |
| +0xA4 | hover text | event 6 (§6) | `0x00580B70` |
| +0xAC | combat list | own entries dropped at every mode set | `0x0057C980` |
| +0xC4 | flags | bit 0x1 changed (set by every mode set), 0x2 tile, 0x10 new, not yet announced to clients (every allocation; cleared with 0x1 by the room clean-up `0x00553220`; `items/inventory-moves.md` §6.3; D2MOO `INITSEEDSET`), 0x40 cleared by attack-mode starts, 0x100 hover freed, 0x2000 queued (`unit-order.md` §6), 0x10000 dead, 0x80000 monster mode changing | `0x00555230`, `0x00624690`, `0x0057FED8`, `0x005541B8`, `0x005A7C20` |
| +0xC8 | flags 2 | 0x2000000 expansion (game +0x70 ≠ 0), 0x4000000 server unit (every allocation) | `0x005552B6` |
| +0xD0 | node index | 11 at allocation | `0x00555339` |
| +0xDC | timer list head | `unit-order.md` §8 | `0x00553980` |
| +0xE0, +0xE4, +0xE8 | update, hash, room links | `unit-order.md` | — |

"Dead" (`0x005541B0`): a null unit, flag 0x10000, a player in mode 0 or
17, a monster in mode 0 or 12, or any unit of another type.

### 3. Lifecycle

#### 3.1 Allocation

`0x00555230` (ECX type, EDX class; stack x, y, game, room, flags, mode,
fixed GUID):

1. Monster: class must be a valid monstats row with `enabled` set
   (bit 25 of +0x0C, tested as byte +0x0F & 2); player: class < 7.
   Else return null, nothing allocated.
2. Allocate the record (`0x00620290`); write type, class, pool, game
   (+0x80); flags 2: expansion bit from game +0x70, server bit.
3. Act from the room (`0x0061A1B0`, `0x006427F0`) into +0x18, act record
   into +0x1C; quest chain := 0.
4. Not a player: unit seed (`0x00552DF0`); item: `0x00627C90`, item seed
   (`0x00552E90`) (`rng.md` §5.3).
5. Flags |= 0x10; node index := 11.
6. GUID: a monster with flags bit 2 takes the fixed GUID; every other
   unit draws one (`0x00552EE0`, `unit-order.md` §1.3).
7. Per-kind init (§1 table).
8. Flags & 0x1 (`0x00555443`): `SUNIT_Add` `0x00554850(unit, x, y,
   game, room, 1)` (`unit-order.md` §3.1), after the per-kind init of
   step 7; its result is not tested. Then a player in mode 0 or 17, or a
   monster for which `0x0063EA40` holds and `0x004638A0(class, 0x13)`
   does not, gets path settings (`0x00649560(1)`, `0x00649190(5)`,
   `0x00648C30(0x8000)`; `sim/path-placement.md` §5.3).
9. Flags & 0x1 clear: no `SUNIT_Add` and no path settings; the unit is
   returned as it is after step 7 (seeds drawn, GUID taken, per-kind
   init done), in no room list, hash list or update queue. This is not
   a failure: the allocator returns the unit in both cases, and null
   only from step 1.

Events scheduled by a per-kind init are listed in §6 (missile: §6.3;
objects: their init functions, §6.4).

#### 3.2 Removal

`0x00555600(game, unit)`: type ≥ 6 is fatal. `0x00555580(unit, game,
0)` (interaction cleanup `0x00555500` and footprint removal `0x00649F50`
for types 0–3, room unlink, hash unlink: `unit-order.md` §3.2); free the
quest chain; the kind's free routine (§1, cancels all timers); then
`0x005C0A90`, `0x00571F40` and the record free `0x00620300`. Removal is
immediate (`tick.md` §5.5 consequence 3).

#### 3.3 Compress on room deactivation (`0x005433F0`, ECX game, EDX unit)

Run by tick step 9 for each unit of a room being removed
(`drlg/rooms.md` §8 rule 2). "Store" = keep an inactive record of the
unit for a later restore (`0x00542E10`; records and restore: §3.4). "Detach" = `0x0064C450`: for a
dynamic path (`0x0064FC20`) precise and client x / y := 0, point count
:= 0 and, with a room, previous room := room, the unit leaves the
room's unit list (`0x0064C370`) and path flag 0x2 is set; the footprint
is **not** cleared and the unit is not freed. "Free" = §3.2 (footprint
removed, room and hash unlink, path freed).

S = the room's level has `SaveMonsters` ≠ 0 (leveldefs +0x94,
`0x00642820`) or unit flag 0x2000000 (+0xC4) is set.

| Type | Rule |
|---|---|
| 0 player | has state 7 (`playerbody`, `0x00639DF0`): cancel its type-1 events (`0x00540E60(1, 0)`), flags 2 (+0xC8) \|= 0x100, store, detach; kept. Else: store if S, free |
| 1 monster | the monster rule below |
| 2 object | class 59 or 60 (`Portal`): as a player with state 7 (kept). Else store := S, cleared when objects `Restore` (+0x173) = 0 or unit byte +0x78 (`0x005540D0`) has 0x2, or when `RestoreVirgins` (+0x174) ≠ 0 and mode ≠ 0; cancel type-1 events; store if still set; free |
| 3 missile | free, no store |
| 4 item | store (`0x00541B10`); not freed here |
| 5 tile | store, free |

Monster (`0x005431F0`), store flag K := S, then in order:

1. Mode 12 (dead): one step of the room's seed (room +0x6C, the D2
   step of `sim/rng.md`); K := K and (new low word mod 3 = 0, unsigned).
2. Dead (`0x005541B0`, §2): K := 0 if `0x0063A770`(unit) ≠ 0 (the
   unit has a state whose `states` flag `udead` is set: state-flag list
   33 at data tables `+0x150`, `0x0063A130`) or the alignment
   (`0x006259B0`) is 2.
3. Node index (+0xD0) < 8: alive → K := 1, mode set 1
   (`0x005543B0`), monster data freed (`0x005B1A90`), skip rule 4;
   dead → K := 0.
4. Dead and class 0x16B or 0x16C → K := 0.
5. Monster type flags (monster data +0x16) & 0x18 (unique, minion) and
   dead → K := 0.
6. Unit flag bit 31 (+0xC4): K := 0; when the owner (`0x0058F0D0`) is
   a player, P := its pet test `0x005752B0`.
7. Unit flag 0x200 → K := 0.
8. monstats2 `restore` (+0x130, via monstats `MonStatsEx`): 0 or no row
   → K := 0; 2 → K := 1; 1 → unchanged (1.14d: 18 rows 0, 574 rows 1,
   17 rows 2, measured).
9. P set: flags 2 |= 0x100, store if K, detach (kept, footprint kept).
   Else: store if K, free.

So a monster in a deactivated room is freed with its path and
footprint, except a player's pet (rule 9), which keeps its footprint
cells in the grids that outlive the room.

#### 3.4 Inactive records and restore

Records hang off a per-act list of area nodes at game `+0xD8 + 4·act`
(`act` = `0x006427F0` of the room's level): node (0x18 bytes) = {x, y
(the DRLG room's origin, `0x00619730`), `+0x08` item records, `+0x0C`
monster records, `+0x10` other records, `+0x14` next}; the list is kept
in descending x, a new node going before the first node of smaller x
(`0x00541D20`, `0x00542E30`). Every store pushes its record at the head
of its list.

1. **Monster** (`0x00542E10` → `0x005421A0`, 0x5C bytes): first the
   states of the list at data tables `+0x174` (count `+0x178`) that the
   unit has are removed with their stat lists, and its type-2 events are
   cancelled (`0x00540E60(2, 0)`). Fields: `+0x00`/`+0x04` sub-tile x,
   y; `+0x08` class; `+0x0C` GUID; `+0x10` unit flags; `+0x14` flag-ex;
   `+0x18` bits: 1 type flag 0x1 (also `0x00544F20`), 2 champion, 4 dead
   (mode 12 or 0), 8 / 0x10 owner bits, 0x20 minion, 0x40
   `0x00573540(unit, 2)`, 0x80 / 0x100 / 0x200 alignment 1 / 2 / 0,
   0x400 node index (`+0xD0`) ≠ 11 (node index < 8 is fatal 0x56A),
   0x800 superunique; `+0x1C` / `+0x20` owner GUID and value
   (`0x0058F440`, kept when the owner exists and is of type 1; else
   `+0x1C` = −1); `+0x28` `0x005B0D60(monster data +0x28)`; `+0x2C`
   `0x00573520`; `+0x30` u16 `0x005A0140`; `+0x32..+0x3A` the 9 umod
   bytes; `+0x3C` u16 superunique index; `+0x40` stat 13, `+0x44`
   `0x00625D10`, `+0x48` stat 6 (life); `+0x54` the game frame; `+0x58`
   next.
2. **Item** (`0x00542E30` → `0x00541B10`): the item is serialized with
   the item bit stream in save form (`0x006313E0(item, buf, 0x400, 1,
   1, 0)`, `items/bitstream.md`); record = {next, ground expiry
   (`0x00558A10`, absolute frame, 0 = never), `0x00629F20(item)`, length
   u16, the bytes}; then its socketed items and the item itself are
   freed (`0x00555600`). So the item rule of §3.3 ("not freed here")
   holds for `0x005433F0` only: the store frees it.
3. **Other types** (`0x00542E30`, 0x34 bytes): x, y, type, class;
   `+0x10` mode; type 0: `+0x20` GUID; type 2: a mode-1 object whose
   `objects` byte `+0x109` is 0 and `+0x141` ≠ 0 is first set to mode 2
   (recorded as 2); with `0x00621B00(unit)` the object's pending event
   5 time (`0x005415A0`) is kept in `+0x20` (high 16 bits) and `+0x24`
   (low 16), else classes 59, 60, 100 keep their GUID in `+0x20` and the
   others the unit byte `+0x78` in `+0x24`; `+0x28` object-data byte
   `+4`, `+0x2C` unit `+0xB8`. All: `+0x14` game frame, `+0x18` unit
   flags, `+0x1C` flag-ex, `+0x30` next.
4. **Restore** (`0x00542B40(game, room)`, from the first population of
   a room, `monsters/population.md` §1 rule 1): the node of the room is
   unlinked and the lists are restored in this order, each from its head
   (so the reverse of store order):
   1. monsters (`0x005424F0`): a record with flag-ex 0x100 (a kept pet)
      re-places the existing unit of that GUID (`0x00554A30`) and
      re-links its owner (`0x0058F350`); else the monster is spawned
      again **with the stored GUID** (`+0x0C`): minion records through
      `0x005A46E0`, type-flag-1 records through `0x005A4440` (with the
      umods, superunique index and champion bit), others through the
      plain spawn; dead records in mode 12, others in mode 1. Records
      with flag 0x400 and alignment 1 or 2 (0x80 / 0x100) are skipped. In
      level 108 with `0x005B5210` true only class 243 records are
      restored.
   2. items (`0x00541AC0`): a record whose expiry ≠ 0 and < the current
      frame is dropped; others are re-created from their stream
      (`0x00541990`).
   3. other records: flag-ex 0x100 (kept units: player bodies, portals
      59/60) → the unit of that type and GUID is placed again
      (`0x00554A30`) and gets unit flag 0x10; a type-0 one must be in
      mode 0x11 (else fatal 0x156) and is set to it; a type-2 one gets
      its object-data byte back. Else a **new unit** (new GUID) is
      created (`0x005557D0(game, room, type, class, x, y, mode, flags)`;
      type 1 never occurs here); type 2 then: with `0x00621B00` and a
      stored time `t` > 0, event 5 at `max(t, frame + 1)`
      (`0x005417D0`), byte `+4` and `0x00621BB0(unit, 0x006414B0(byte))`;
      else when `objects` SubClass (`+0x167`) has 0x20 (shrines) the
      byte regrows by elapsed time over the row's `+0x178` period, capped
      at `0x00552AA0`, with event-2 re-schedules; else byte `+4`, unit
      byte `+0x78`, `+0xB8` (when ≠ 0) come back.
   Every record is freed after use.

### 4. Modes and mode schedules

#### 4.1 Setting a mode

`0x00553570(game, unit, mode)`: drop the unit's own entries from its
combat list (`0x0057C980`), then `0x00624690(unit, mode)`: tile: nothing;
a new mode is written to +0x10, the unit is queued for update
(`unit-order.md` §6.2), flags |= 1, the unit's TEMPONLY stat lists are
removed (`0x006272E0`, `stat-lists.md` §8.9) and the animation fields
re-initialised (`0x00624390`); the same mode only queues
the unit and sets flag 1 (not for a monster staying in mode 1). Setting
a mode schedules nothing by itself. Mode starts that animate then call,
in this order: prepare animation `0x005533D0` (action frame := 0;
sequence modes — player 18, monster 14 with a sequence — load the
sequence and its rate; otherwise sequence := null, current frame := 0,
rate `0x00623F50` when the unit has a path, else only the AnimData
lookup `0x00620F00`; frame count := AnimData frames · 256), cancel
`0x00553990` (the unit's events of type 0, then type 1, any argument),
then §4.2. Movement starts set the mode, cancel and schedule §4.4.

#### 4.2 Animation schedule (events 0 and 1)

Inputs: frame f; speed s and frame count F (+0x3C/+0x34 with a
sequence, else +0x4C/+0x48); event bytes E[i] (sequence: `0x006634C0`,
owned by the sequence spec; else AnimData record +0x10 + i); start index.

Main form `0x005539B0` (start index = frame bonus b = `0x00623B10(unit)`):

1. s = 0: schedule event 1 at f + 1; stop (+0x44 unchanged).
2. n = f, i = b, k = 0, a = 256·b + s.
3. While a < F (signed): n += 1; while i ≤ a >> 8 (arithmetic) and
   i < 144: E[i] ∈ {1, 2, 4} → event 0 at n, args (E[i], k), k += 1;
   E[i] = 3 → event 0 at n, args (3, 0); other values nothing; i += 1.
   Then a += s.
4. If n = f: n = f + 1. Schedule event 1 at n + 1, args (0, 0).
5. Current frame +0x44 := f · 256.

So events of one schedule are scheduled in frame order, each action
event before the end, and the end is at f + max(⌈(F − 256·b) / s⌉, 2)
for s > 0, f + 1 for s = 0. Event bytes before the bonus frame are
skipped.

Variants, used by skills to rewind or shorten a running animation; each
first cancels the unit's type-0 and type-1 events itself, sets +0x44 and
then loops from start index c − 1 with a = 256·c + s, numbering every
action event 0 (a2 = 0, events 1–4 alike), with no 144 bound (index −1
reads the byte before the event array: AnimData +0x0F):

| Function | Argument p | c | +0x44 |
|---|---|---|---|
| `0x00553B10` | percent | (100 − p)·(f − cur) / 100, truncated toward 0; cur = +0x44 >> 8 | (f − c)·256 |
| `0x00553C70` | frames (p ≤ 0: nothing at all) | f − cur − p | (f − c)·256 |
| `0x00553DC0` | start frame | p | (f − p)·256 |

Speed 0 gives event 1 at f + 1 in all forms. Callers: `0x00553B10` from
`0x0056E210`; `0x00553C70` from skills `0x005C8CA0`, `0x005C8E30`,
`0x005D68A0`, `0x005D69D0`; `0x00553DC0` from skills `0x005CF900` (3),
`0x005D9580`, `0x005DA120`, `0x005DA7E0`.

#### 4.3 Animation rate and frame bonus (dependency)

Speed +0x4C is computed by `0x00623F50` from the AnimData speed (+0x0C)
and percentages from stats 67, 68, 69 and states (clamps 15–175 %); the
frame bonus `0x00623B10` is non-zero only for some player modes (table
`0x006E8E60`, by class and weapon type). Both are owned by a future
animation-rate spec (open question 2); §4.2 takes them as inputs.

#### 4.4 Every-tick movement (event 0, every tick)

`0x00553F00(game, unit)`: cancel the unit's type-0 events, then schedule
an every-tick type-0 event, args (0, 0) (`tick.md` §5.3). Callers: player
movement start `0x0057F090`, monster mode set `0x005A7C20`; both cancel
types 0 and 1 first (`0x00553990`).

#### 4.5 Player mode starts

`0x005809D0` (to a position) and `0x00580A70` (to a unit, by GUID) check
the request (`0x0057EDD0`, `0x0057EEC0`; rules: `sim/pathing.md` §1.3–§1.4), set the target and call the
mode's function from table `0x006E1740` (20 rows × {position form, unit
form}; null forms fatal):

| Modes | Start functions | Schedule |
|---|---|---|
| 0 DT | —, `0x00580EC0` | §4.2 (after death bookkeeping) |
| 1 NU, 5 TN | `0x0057F020` | none: cancel 0/1, mode 1, or 5 in town (`0x0061AB00`) |
| 2 WL, 3 RN, 6 TW, 19 KB | `0x0057F1F0`, `0x0057F190` → `0x0057F090` | §4.4; RN stays RN while stat 10 ≠ 0, else it is treated as WL; WL and TW become TW in a town room (`0x0061AB00`), WL elsewhere; KB sets path values 8 and 5 |
| 4 GH | `0x0057FE30` | §4.2 |
| 9 BL | `0x0057FDC0` | §4.2 |
| 7, 8, 10–16, 18 | `0x0057FE90`, `0x0057FEF0` | §4.2, then flags &= ~0x40 and `0x0056FAF0` |
| 17 DD | `0x0057FCA0` | none (corpse; character save) |

Event 0 for a player (`0x005811D0`) calls the mode's action function from
table `0x00732C10` with (a1, a2): DT, DD `0x0057F070` and NU, TN
`0x0057F080` (return 1), WL/RN/TW/KB `0x00580C20` (one movement step),
GH, BL `0x0057FE20` (return 1), attack, cast and skill modes
`0x00580460` (the action frame: skill/attack, owned by the skills spec).
Result 2 runs the ENDANIM handler at once. Event 1 (`0x00581020`):
attack-mode cleanup (`0x00580310`, `0x00580380`) for A1, A2, TH and for
units whose item row is flagged (`0x00643CE0`); then mode 0 → DD
(`0x0057FCA0`); mode 19 → GH start with argument −1 (table entry
`0x006E1760`) after the request check `0x0057EEC0(4, 1, 0)`; every other
mode → neutral `0x0057F020`.

#### 4.6 Monster mode set

`0x005A7C20(game, mode-change record, flag)` (record: mode, unit,
target, x, y, …): state 54 is a fatal assertion; path and AI-state
bookkeeping for every mode but GH (monster spec); the new mode's start
function from the mode table (below) runs, and if it returns 0 the
neutral start `0x005A73E0` runs instead; flags |= 0x80000, animation
prepared (`0x005533D0`), cancel 0/1, then, if the record of the mode the
unit is now in has its schedule flag: every-tick (§4.4) when
`0x005A6B10` says the mode moves, else §4.2.

Mode table (`0x005A78A0`): 16-byte records {start, event-0 function,
event-1 function, schedule flag} at `0x006E2260` + 16·mode; classes
243–418, 543 and 544–709 with a set byte +0x1A5 in their monstats row
use per-class records (`0x006E22D0`–`0x006E23A0`; monster spec). Null
record or function: fallback `0x005A7B30`.

| Mode | Start | Event 0 | Event 1 | Schedules | Moves (`0x006E23D0`) |
|---|---|---|---|---|---|
| 0 DT | `0x005A6FF0` | `0x005A7350` | `0x005A72B0` | yes | no |
| 1 NU | `0x005A73E0` | `0x005A6DE0` | — | no | — |
| 2 WL | `0x005A7520` | `0x005A8490` | — | yes | always |
| 3 GH | `0x005A7580` | `0x005A6DE0` | `0x005A8030` | yes | no |
| 4 A1, 5 A2, 7 SC, 8 S1, 9 S2 | `0x005A75C0` | `0x005A7670` | `0x005A8030` | yes | if monstats2 `A1mv`…`S2mv` bit (+0x104, bit = mode) |
| 6 BL | `0x005A77C0` | `0x005A6DE0` | `0x005A8030` | yes | no |
| 10 S3 | `0x005A7490` | `0x005A74A0` | `0x005A74D0` | no | (`S3mv`) |
| 11 S4 | `0x005A74E0` | `0x005A6DE0` | — | no | (`S4mv`) |
| 12 DD | `0x005A7390` | `0x005A6DE0` | — | no | no |
| 13 KB | `0x005A77D0` | `0x005A8630` | `0x005A8520` | yes | always |
| 14 SQ | `0x005A7870` | `0x005A8670` | `0x005A8030` | yes | always |
| 15 RN | `0x005A7550` | `0x005A84F0` | — | yes | always |

Event 0 and event 1 for a monster (`0x005A7BA0`, `0x005A7BE0`) call the
current mode's event-0 / event-1 function. Neutral start `0x005A73E0`
sets mode 1 and, unless the unit has a type-2 timer with expire > f
(`0x005415A0`: smallest positive expire of its type-2 timers, 0 if
none), schedules event 2: f + 45 with state 21, else f + `aidel`
(monstats +0x4F, the Normal column, or +0x4F + difficulty (game +0x6D)
when game +0x6A or game +0x74 is non-zero; 0 → 15).

### 5. Event dispatch

Class dispatchers and run order: `tick.md` §5.5–§5.6. Handler per (kind,
type): `unit-handlers.tsv` (read from tables `0x006E1810`, `0x006E2490`,
`0x006E19B0`, `0x006E117C`). Differences that matter:

1. Players and monsters pass (a1, a2) to the handler; a null entry does
   nothing.
2. Objects and items call `handler(game, unit)` without type or
   arguments; a null entry is a fatal assertion (`IsBadCodePtr`, then
   exit). No 1.14d scheduler targets a null entry (U1).
3. Missiles ignore the type (`tick.md` §5.6).
4. A timed event with expire −1 through `0x005416B0` becomes an
   every-tick event and loses its callback: `tick.md` §5.2 r2 (owner).

### 6. Events per kind

Expire is given relative to the scheduling frame f; every expire ≤ f
becomes f + 1 (`tick.md` §5.2). Sites: `unit-events.tsv`.

#### 6.1 Player

| Type | Scheduled by | Expire, args | Handler does |
|---|---|---|---|
| 0 | §4.2, §4.4; skills `0x005C8CA0` (f + 2, a1 4), `0x005CF900` (f + 1, a1 1), `0x005D1350` (f + 3) | §4 | §4.5 |
| 1 | §4.2; skills `0x005CF900`, `0x005C8C10`, `0x005CC3B0`, `0x005DA120`, `0x005DA7E0` | §4.2 or per skill | §4.5 |
| 3 | join `0x00534AD0`; handler itself; damage `0x0057AC50`, `0x0057ADD0`, `0x0057C6C0`; skill items `0x005BE3F0`, `0x005BE7B0`, `0x005BEAC0`; `0x0054CED0` | f + 1, (0, 0) | `0x00580810`: reschedule at f + 1 with the same args **first**, then, if not dead, life (`0x00580610`), and when it returns non-zero, stamina and mana (`0x00580500`, `0x005806F0`); `0x00580610` always returns 1, so all three run (`sim/stat-lists.md` §10.1). So a player has a regen event every frame from its join on |
| 5, 8, 9, 12 | states, skills, shrines, items (`unit-events.tsv`) | per scheduler | `0x0056D790`, `0x0056FCB0`, `0x0056FE40`, `0x00580800` (remove expired states, `0x00627460(unit, f)`): `sim/stat-lists.md` |
| 6 | hover text set `0x0054A290`; handler | the hover's timeout | `0x00580B70`: timeout (`0x006611D0`) ≤ f → free the hover, +0xA4 := 0, queue for update, flags \|= 0x100; else reschedule at the timeout |
| 11 | join `0x00534AD0` (f + 250); handler (f + 30) | (0, 0) | `0x00580BE0`: party refresh (`0x005406A0`), pet refresh (`0x00575630`), reschedule f + 30. D2MOO calls it DELAYEDPORTAL; in 1.14d it is a 30-frame refresh |
| 13 | trade and vendor code `0x00567620`–`0x00568D10` | per caller | `0x005689D0` (trade spec) |
| 14 | `0x00554EA0` | f + 50, callback `0x00554570` | the callback (skill cooldown end); the class table entry is null |
| 2, 4, 7, 10 | none for players | — | null entries |

Join `0x00534AD0`: neutral mode start (`0x005809D0`, mode 1), then
event 3 at f + 1, then event 11 at f + 250.

#### 6.2 Monster

| Type | Scheduled by | Expire, args | Handler does |
|---|---|---|---|
| 0, 1 | §4.6; skills `0x005CC4E0`, `0x005CC690`, `0x005CC3B0`; AI `0x005ECEE0`, `0x005ED2A0`, `0x005F6B70` | §4.2 / §4.4; per caller | mode functions, §4.6 |
| 2 | neutral start `0x005A73E0`; `0x005A74E0` (f + 15); `0x005A8520` (f + 1, or f + 15 / f + 45 for base id 78); NPC talk `0x00548B00`, `0x0054CA10` (f + 1); restore `0x00542B40`; damage `0x0057B170`, `0x0057B230`; `0x00573780` (f + 2); AI and skill code (30 sites) | §4.6 or per AI | `0x005B1740`: AI think (AI spec; per-class AI record from monster data +0x28, pre-checks `0x005B10E0`, `0x005B1650`, `0x005B13E0`, then the AI function at record +4) |
| 3 | damage `0x0057AC50`, `0x0057ADD0`, `0x0057C6C0`; skill items; handler | f + 1, (0, 0) | `0x005A6920`: life regeneration (stat 74, `sim/stats.md`); reschedules at f + 1 unless state 52 holds and the rate is ≥ 0; a zero rate cancels its type-3 events |
| 6 | `0x005DE330`; handler | timeout | `0x005A7F00`, as the player's |
| 7 | monster unique mods `0x005A1330`–`0x005A4230`, quests, skills | per caller | `0x005A4370` (monster spec) |
| 10 | `0x005C3B30`, `0x005C3DE0` (f + skill value) | — | `0x005A7F70` (AI spec) |
| 5, 8, 9, 12 | as players | — | `0x0056D790`, `0x0056FCB0`, `0x0056FE40`, `0x005A7EF0`: `sim/stat-lists.md` |
| 4, 11, 13, 14 | — | — | null |

Types 0, 1, 2, 6, 7, 9, 10, 11, 13, 14 are dropped for a frozen monster
(`tick.md` §5.6).

Type 7 (`0x005A4370`) runs every mode-2 umod callback of the monster,
whoever scheduled the event; the bodies, the type-7 and type-2 sites of
the umod callbacks, the think restart `0x00573780` (its two type-2
sites) and where the umod dispatcher runs inside the monster mode set
`0x005A7C20` (mode 0 before the start function, never for GH; mode 1
after the animation prepare, before the cancel of events 0 / 1) are
owned by `monsters/umod-callbacks.md` (§2, §3.5, §4–§27).

#### 6.3 Missile

Only one event: the every-tick type 0 scheduled at creation
(`0x0059F8A0`, after cancelling all the missile's timers). Its handler
runs the missile's server-do function (`tick.md` §5.6; missile spec).
REMOVESTATE sites in missile code (`0x005AAE10`, `0x005AB110`,
`0x005ADB00`) schedule on the units hit, not on the missile.

#### 6.4 Object

Handlers take (game, object) only (§5). objects.txt columns by
`data/fields.tsv`: `FrameCnt1` +0xDC (stored · 256; "fc1" below is the
column value), `Parm0` +0x178, `Parm1` +0x17C.

| Type | Handler | Scheduled by (expire) |
|---|---|---|
| 0 | `0x00581700` (trap tick, D2MOO) | object inits `0x0054F860`, `0x0054FB40` (f + 25); handler (f + 15 + roll mod 35) |
| 1 | `0x00581490`: mode 1 (OP) → 2 (ON) when `Mode2` ≠ 0 (no update queued), then, if `HasCollision2` = 0, `0x00623830` | operate and init functions: f + fc1 + 1 (29 sites), f + fc1 (17 sites, e.g. `0x00545850`, quest objects `0x0058BD50`…), f + 2·fc1 (`0x005B5630`) |
| 2 | `0x00581510` (D2MOO: well refill) | `0x005858A0`: f + `Parm0` + 1 |
| 3 | `0x005818B0` | `0x0054FB90` (f + 25); handler (f + 15, f + fc1 + 1) |
| 4 | `0x005817A0` (trap) | `0x00582510` (f + 35) |
| 5 | `0x005814D0` (shrine reset: mode 0, data +0x0C := 0) | shrine operate `0x00583C70` (f + 1200·minutes + 1); restore `0x00542B40` |
| 6 | `0x00581620` (hover) | `0x00583C70` (f + 300) |
| 7 | `0x00581A10` → `0x005449E0` (quest object event) | quest code (`unit-events.tsv`) |
| 8 | `0x00581250` | `0x0054F860` (f + 60), `0x0054FDB0` (f + 25 + roll mod 250); handler (f + 25 + roll mod 250, f + 1000, f + 600; and event 1 at f + fc1 + 1) |
| 9 | `0x00585CE0` | `0x005501F0` (f + 35), `0x005869F0` (f + 20); handler (f + fc1 + 1, f + 25, f + 10) |
| 10 | `0x00586850` | `0x00583F10` (f + `Parm1` + 1) |
| 11 | `0x00581410`: creates a portal object (`0x0056CF40`; object row by level: 111 → 125, 112 → 126, else 127) | `0x0054FE70` (f + 1) |
| 12–14 | null (fatal) | none |

Object internals are owned by the objects spec; the seed of the rolls
(D2MOO: the object-control seed, `rng.md` §5.2) is not confirmed here
(open question 4).

#### 6.5 Item

| Type | Handler | Scheduled by (expire) |
|---|---|---|
| 3 | `0x00562D30`: (a) item not broken (flag 0x100) and with durability (`0x00629930`): replenish(rate 252, value 72, max = max durability `0x00625E00`); (b) only when (a) did not run or returned 0, and the item is stackable (`0x006289F0`): replenish(rate 253, value 70, max = max stack `0x006295B0`). Replenish `0x00562C40`: r := total(rate); r = 0 → return 0 (no reschedule). v := total(value) + 1; v > max → return 1 (no reschedule: the chain ends when full). Else set base value := v; if the item's owner GUID (`0x00629F20`) ≠ −1 names a player with a client, send the stat update `0x0053D130`(client, item, 1, value stat, v, 0); if v ≥ 1 and the item is broken, `0x0055F900` (repair path, items spec). Reschedule event 3 at f + max(2500 / r + 1, 125) (integer division), args (0, 0); return 1 | start `0x00558530` (stat 252, durability), `0x00558580` (stat 253, quantity), `0x0055A2A0`: if r ≠ 0 and no type-3 event is pending, f + 2500 / r + 1 (integer division, no minimum) |
| 4 | `0x0055F120` (returns at once) | none found |
| 12 | `0x0055F130`: `0x00627460(item, f)` | `sim/stat-lists.md` |
| others | null (fatal) | none |

#### 6.6 Unit event records (unit +0x90)

Not timer events: the hooks `combat/damage.md` §5.4 runs. A unit keeps
a doubly linked list of 0x20-byte records at +0x90 (prev +0x18, next
+0x1C; null head = none). Record: event id (u8 +0x00, `events.txt`
index), flags (u16 +0x02: 1 running, 2 remove pending), owner kind
(+0x04), key (+0x08), two values passed to the function (+0x0C,
+0x10), function (+0x14).

1. Add (`0x005C0AD0`(ECX game, EDX unit; event, v0, v1, function,
   kind, key)): unit null → 0. Allocate, zero, fill, **prepend** at the
   head; return the record. By table index (`0x0056E740`, same order
   with a function index instead of a pointer): index > 0x31 or table
   `0x007325B0`[index] null → 0, nothing added.
2. Find (`0x005C0BE0`(unit; kind, key, v0)): the first record from the
   head with equal kind, key and v0, else null.
3. Remove (`0x005C0B50`(ECX game, EDX unit; kind, key)): every record
   with that kind and key: running → set remove pending; else unlink
   and free.
4. Trigger (`0x005C0C30`(ECX game, EDX event; unit, other, arg)): unit
   null → 0. From the head, each record whose event id matches: set
   running, call function(ECX game; unit, other, arg, v0, v1), clear
   running; then, if remove pending or kind = 0 (one-shot), unlink and
   free it. The next record is read after the call (a record the
   function adds at the head is not visited). Returns the last called
   function's result, 0 when none ran. Most recent registration runs
   first.

### 7. Scheduler inventory (`unit-events.tsv`)

All 269 call sites of the public scheduling functions in 1.14d
(`0x005417D0` 265, `0x00541800` 1, `0x00541650` 3; found by scanning
`.text` for rel32 calls), one row each:

| Column | Meaning |
|---|---|
| `site` | address of the call instruction |
| `function` | containing function (Ghidra entry, or the code after the nearest int3 padding) |
| `api` | `timed` (`0x005417D0`), `timed_cb` (`0x00541800`), `every` (`0x00541650`) |
| `type` | event type (an immediate at every site) |
| `classes` | unit kinds the site schedules for |
| `expire` | `f+N`; `f+fc1+1`, `f+fc1`, `f+2*fc1`, `f+parm0+1`, `f+parm1+1` (object's own objects.txt row); `f+aidel`; `f+15+rnd35`, `f+25+rnd250` (roll of the object-control seed); `f+2500/r+1`, `f+max(2500/r+1,125)`; `f+1200*m+1`; `anim` (§4.2); `hover`; `-1`; `calc` (computed by the owner); alternatives joined by `\|` |
| `a1`, `a2` | immediate arguments, or `reg` |
| `callback` | 0 or the callback address |
| `proof` | `code`: kind proven by reading the site's function; `file`: kind from the 1.14d source-file family (path strings `.\UNIT\SUnit.cpp` etc. in neighbouring functions) |
| `owner` | spec owning the rule (this spec's § or a later spec) |

Counts by type: 0: 20, 1: 60, 2: 30, 3: 18, 4: 1, 5: 6, 6: 6, 7: 45,
8: 7, 9: 7, 10: 3, 11: 3, 12: 42, 13: 20, 14: 1.

## Constants & data dependencies

| Constant | Value | Use |
|---|---|---|
| event bytes per AnimData record | 144 | §4.2 main-form bound |
| event byte values scheduled | 1, 2, 4 (numbered), 3 (unnumbered) | §4.2 |
| player regen period | 1 frame | §6.1 |
| player refresh (event 11) | 250 after join, then 30 | §6.1 |
| skill cooldown | 50 frames | §6.1 |
| monster AI default | 15 (aidel 0), 45 (state 21) | §4.6 |
| replenish | 2500 / rate + 1, min 125 in the handler | §6.5 |

Data read: AnimData (frames, speed, events), monstats `aidel` (+0x4F,
+0x50, +0x51), monstats2 `A1mv`…`S4mv`, objects `FrameCnt1`, `Parm0`,
`Parm1`, `Mode2`, `HasCollision2`; stats 10, 52, 67–69, 74, 252, 253 and
states 1, 21, 52, 54 (owners in `sim/stats.md`, `sim/stat-lists.md`).

## Randomness

Allocation: one game-seed step per non-player unit, a second per item
(`rng.md` §5.3). Object events 0 and 8 roll their delays from the
object-control seed (§6.4). Mode schedules, regeneration and the event
11 refresh draw nothing. AI, skills and missiles draw inside their
handlers (owned by their specs).

## Edge cases & original bugs

1. Variants of §4.2 have no 144 bound and can start at index −1: they
   read event bytes outside the AnimData event array (reproduce from
   the record bytes: index −1 is AnimData +0x0F, the speed's high byte).
2. The main form stops reading events at index 143 but keeps counting
   frames: an animation longer than 144 frames has no events past 143.
3. A negative speed (i16) never ends the loop of §4.2; `0x00623F50`
   clamps rates to ≥ 0, so it does not occur from the rate code.
4. The player movement step `0x00580C20` reads `GetTickCount` to store a
   position history in the player data (+0xA0 index, +0xA4 time,
   +0xA8 entries, 20 slots, every > 25 ms of wall clock and > 45 units of
   distance). Wall-clock state inside the simulation: open question 3.
5. Object and item handlers ignore the event arguments; a null table
   entry for them is fatal, for players and monsters silent.
6. The neutral AI delay uses the difficulty's `aidel` column only when
   game +0x6A or game +0x74 is non-zero, else the Normal column on every
   difficulty (§4.6; meaning of the two fields: open question 7).

## Test vectors

Comparison (exact): for every tick, the ordered list of timer schedules
— (unit type, GUID, event type, list, requested expire, a1, a2,
callback) — and cancels made by `d2-sim` equals the recording's, from
the same start state and messages; together with `tick.md`'s comparison
of runs. Until `d2-sim` has units, `tools/trace-recorder/check_units.py`
checks each recording against these rules:

| Id | Rule |
|---|---|
| U1 | every schedule and run (kind, type, list) has a 1.14d scheduler (`unit-events.tsv`) and, for objects and items, a handler; with `site`: the exact row (type, kind, list, constant args) |
| U2 | callback 0, except type 14 with `0x00554570` |
| U3 | every-tick type 0 has args (0, 0) |
| U4 | player/monster mode schedules: no live type-0/1 timer of the unit when it starts (cancel first, §4.1); action events in frame order before exactly one event 1, a1 ∈ 1–4, a2 numbered (§4.2) or all 0; event 1 ≥ f + 2 with action events, ≥ f + 1 without; with an `anim` record, the whole list equals §4.2 |
| U5 | player event 3: f + 1; every run reschedules once with its own args |
| U6 | monster event 3: f + 1, args (0, 0) |
| U7 | player event 11: f + 250 the first time, f + 30 from every run, exactly once per run |
| U8 | object event 1: f + fc1 + 1, f + fc1 or f + 2·fc1 (one of the site rules; exact per site with U11) |
| U9 | object event 2: f + `Parm0` + 1 |
| U10 | a monster's event 2 scheduled during its own event-1 run: f + aidel (0 → 15) or f + 45 |
| U11 | with `site`: the TSV expire rule of the site, when it is a formula of f and table data |

Synthetic vectors (§4.2, f = 100, AnimData event byte 6 = 1 unless
stated; `check_units.py` `anim_schedule`, CI-safe):

| Form, inputs | Schedules (type, expire, a1, a2) |
|---|---|
| main, F = 12·256, s = 256, b = 0 | (0, 106, 1, 0), (1, 112, 0, 0) |
| main, F = 12·256, s = 128 | (0, 112, 1, 0), (1, 124, 0, 0) |
| main, F = 12·256, s = 300, bytes 3:1, 4:2, 6:3, 7:4 | (0, 103, 1, 0), (0, 104, 2, 1), (0, 106, 3, 0), (0, 106, 4, 2), (1, 111, 0, 0) |
| main, F = 16·256, s = 256, b = 5, bytes 2:1, 9:1 | (0, 104, 1, 0), (1, 111, 0, 0) |
| main, F = 256, s = 256, no events | (1, 102, 0, 0) |
| main, s = 0 | (1, 101, 0, 0) |
| `0x00553DC0`, p = 3, F = 12·256, s = 256 | (0, 103, 1, 0), (1, 109, 0, 0) |
| `0x00553B10`, p = 50, cur = 96·256 | (0, 104, 1, 0), (1, 110, 0, 0) |
| `0x00553C70`, p = 2, cur = 96·256 | (0, 104, 1, 0), (1, 110, 0, 0) |
| `0x00553C70`, p = 0 | nothing (no cancel either) |

Recorded (`record_tick.py` 0.1.0 files of `tick.md` Test vectors;
tables from `traces/raw/20261006-021210-tables`):

| Recording | Schedules / runs | Checked exactly | Rule counts | Errors |
|---|---|---|---|---|
| `20261006-015554-tick.jsonl` | 7,674 / 16,704 | 5,216 | U4 106 groups, U5 4,901, U6 133, U7 157, U8 2, U10 23 | 0 |
| `20261006-021854-tick.jsonl` | 2,761 / 6,718 | 1,625 | U4 35, U5 1,571, U7 46, U10 8 | 0 |
| `20261006-022304-tick.jsonl` | 10,862 / 24,894 | 7,193 | U4 146, U5 4,631, U6 2,362, U7 148, U8 2, U9 2, U10 48 | 0 |

Observed (kind, type, list) combinations, all callbacks null: player 0
every/timed, 1, 3, 11, 12; monster 0 every/timed, 1, 2, 3; missile 0
every; object 0, 1, 2, 5, 6; item 3. Not yet observed: player 5, 6, 8,
9, 13, 14; monster 5–10, 12; object 3, 4, 7–11.

`check_units.py --perturb N` (expire + 1 at the N-th exactly checked
schedule; e.g. N = 3000 on `022304` → record 78479, U6) and the
`--selftest` (15 perturbations of a hand-built recording, plus a removed
regen reschedule) report exactly the changed record (M08). Unverified:
U4 against animation data, U11, object rolls, item replenish, monster
AI from AI functions, everything in "not yet observed" (open question 1).

## Provenance

- **1.14d `Game.exe`** (SHA-256 `631066c1…adaaf`): every address read
  from the disassembly (`tools/ghidra/disasm.py`) with the Ghidra
  decompile as a guide; scheduler sites by rel32 scan (`disasm.py xref`)
  of `0x005417D0`, `0x00541800`, `0x00541650`; event type, arguments and
  expire expression read from the instructions before each call.
  Dispatch tables `0x006E1810`, `0x006E2490`, `0x006E19B0`, `0x006E117C`,
  mode tables `0x006E1740`, `0x00732C10`, `0x006E2260`, `0x006E23D0`, the
  §4.2 event jump table `0x00553AFC` and the allocation jump table
  `0x005554E8` read from the file image. Source-file families from the
  1.14d path strings (`.\UNIT\SUnit.cpp`, `.\OBJECTS\ObjMode.cpp`, …).
  §3.3: `0x005433F0` and `0x005431F0` (disassembled: the switch, the
  mask 0x18 in EDX, `cmp [esi+0xD0], 8`), `0x0064C450`, `0x0064FC20`
  (globals `0x006EB7C8` / `0x006EB7CC`: 0 in the image, no writer),
  `0x00642820` (leveldefs +0x94 = `SaveMonsters`, `data/fields.tsv`),
  `0x00540E60`, `0x005540D0`, `0x0058F0D0`, `0x005A0180`; objects
  +0x173 / +0x174 and monstats2 +0x130 from `data/fields.tsv`; row
  counts from the 1.14d `patch_d2` `monstats2.txt`, `objects.txt`,
  `states.txt`.
- **D2MOO** (1.10f) `D2Game/src/UNIT/SUnit.cpp` (`sub_6FCBCE70` = main
  form, `sub_6FCBCFD0`, `sub_6FCBD120`, `D2GAME_SKILLS_RewindSkillEx` =
  variants, `sub_6FCBD3A0` = §4.4), `MONSTER/MonsterMode.cpp`,
  `PLAYER/PlrModes.cpp`, `OBJECTS/ObjMode.cpp`, `Objects.cpp`,
  `Common/Units.h` (field names): same structure. Differences found in
  1.14d: the monster mode set is restructured (`0x005A7C20`: moving test
  `0x005A6B10` with table `0x006E23D0`); `0x00553DC0` increments the
  frame before scheduling (same results); player event 11 is a periodic
  refresh; the neutral AI delay reads `aidel` per difficulty only under
  game +0x6A / +0x74.
- **Recorded**: `check_units.py` on the three tick recordings (Test
  vectors); `record_tick.py` 0.2.0 adds `site`, `cl`, `m` to `set`
  records and `anim` records (hooks `0x005539CC`, `0x00553B10`,
  `0x00553C70`, `0x00553DC0`) for U4 and U11.
Ghidra backlog (2026-10-06): store `0x00542E10` (falls through to
`0x00542E30` for non-monsters), `0x005421A0`, `0x00541B10`, node lists
`0x00541D20`; restore `0x00542B40` → `0x005424F0`, `0x005418C0`,
`0x00541AC0`; `0x0063A770` → state-flag list 33 (`udead`, live
`states.txt`).

## Open questions

1. A 0.2.0 recording (`record_tick.py`, `docs/HANDOFF.md` §5) must run
   U4 with `anim` records and U11 with `site` on every schedule, and
   reach the unobserved combinations (combat with skills, shrines,
   wells, a trade, a cooldown skill).
2. Animation rate `0x00623F50` and frame bonus `0x00623B10` (inputs of
   §4.2): owner spec and formulas; to be checked against the logged
   +0x4C and bonus of `anim` records.
3. Player position history read from `GetTickCount` (edge case 4): does
   it feed any outcome (client correction, anti-cheat), and what does
   `d2-sim` use instead?
4. Object delays rolled from the object-control seed (events 0, 8):
   confirm the draw with an RNG + tick recording.
5. Item event 4 has a no-op handler and no scheduler: confirm no 1.14d
   path schedules it (none among the 269 sites).
6. `proof = file` rows of `unit-events.tsv` (kind assigned from the
   source-file family): each is settled by a `site` observation or by
   reading its function.
7. Game +0x6A and +0x74 (§4.6, edge case 6): which game types set them;
   a Nightmare single-player recording settles which `aidel` column
   single player uses.
8. Inactive storage: records and restore answered in §3.4 (monster
   GUIDs are kept; other units come back with new GUIDs; restore order
   monsters, items, others, each newest first). Open: the meaning of
   node index (`+0xD0`) 11 and of the fields from `0x005B0D60`,
   `0x00573520`, `0x005A0140`, `0x00625D10`; whether a restored item
   keeps its GUID (`0x00541990`); a recording leaving and re-entering a
   wilderness area confirms the order (`unit-order.md` OQ3).
