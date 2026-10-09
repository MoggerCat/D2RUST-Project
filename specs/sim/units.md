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
| Summary | 44–59 |
| Inputs | 60–68 |
| Outputs / state changes | 69–74 |
| Rules | 75–76 |
|   1. Unit kinds | 77–96 |
|   2. Unit record | 97–155 |
|   3. Lifecycle | 156–420 |
|   4. Modes and mode schedules | 421–952 |
|   5. Event dispatch | 953–967 |
|   6. Events per kind | 968–1090 |
|   7. Scheduler inventory (`unit-events.tsv`) | 1091–1112 |
|   8. Collision line between two units | 1113–1117 |
| Constants & data dependencies | 1118–1134 |
| Randomness | 1135–1142 |
| Edge cases & original bugs | 1143–1163 |
| Test vectors | 1164–1223 |
| Provenance | 1224–1310 |
| Open questions | 1311–1390 |
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
| 0 | player | 0 | `0x005348C0` (first: unit flags +0xC4 \|= 0x0E at `0x005348EC`, bits 1–3; bits 2 and 3 make the player a missile target, `missiles.md` §R4.2 shared filter), then `0x005B1880(…, 0)` unless mode 17 | `0x005B1A20`, `0x00535430`, `0x005407A0`, `0x005349D0`, path | `0x00581220` |
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
| +0x18 | act (u8) | act of the allocation room's level; a player's is rewritten by the act change (`world/waypoints.md` §11 rule 15) | `0x005552ED`, `0x0053AE4E` |
| +0x1C | act record | game +0xBC + 4·act (same writers as +0x18) | `0x005552FA`, `0x0053AE56` |
| +0x20, +0x28 | seed, init seed | `rng.md` §5.3 | — |
| +0x2C | path | freed at removal | `0x0055568B` |
| +0x30 | sequence record (null: plain animation) | §4.2 | `0x005539CF` |
| +0x34, +0x3C | sequence frame count, sequence speed | used instead of +0x48/+0x4C when +0x30 ≠ 0 | `0x005539D5` |
| +0x44 | current frame, 8.8 fixed point | 0 at mode start (`0x005533D0`); frame·256 after §4.2 | `0x00553AF1` |
| +0x48 | frame count, 8.8 (animation frames · 256) | set at mode start | `0x005533D0` |
| +0x4C | animation speed (i16, 1/256 frame per tick) | §4.3 | `0x00623F50` |
| +0x4E | action frame (u8): last event byte 1–4 crossed by the latest frame advance | 0 at mode start; frame advance (§4.2) | `0x005533D0`, `0x00623E00`, `0x00621210` |
| +0x50 | AnimData record | `formats/animdata.md` §5 | `0x00620F00` |
| +0x5C | stat list | `sim/stat-lists.md` | `0x00625480` |
| +0x60 | inventory | — | `0x00620F00` |
| +0x64, +0x68, +0x6C | interact info: GUID, type, active (get `0x00554100`, set `0x00554120` ignored while active, reset `0x00554190` → GUID −1, type 6, inactive; `world/npc.md`, `world/cube.md`) | **0, 0, 0 at allocation** (the 0xF4-byte record is zeroed by `0x00620290`; `0x00555230` and the seed calls it makes, `0x00552DF0`, `0x00552EE0`, write none of the three), so a fresh unit reads as inactive with (type 0, GUID 0), not the reset values | `0x00620290`, `0x00555230` |
| +0x74 | quest chain | 0 at allocation, freed at removal | `0x005552FD`, `0x00555644` |
| +0x80 | game | — | `0x005552B0` |
| +0x94, +0x98 | source-unit link: owner type, owner GUID (valid while +0xC8 bit 0x400) | "Owner links" below; `skills/bodies.md` §6.20 | `0x00621C30`, `0x00552FD0` |
| +0xA4 | hover text | event 6 (§6) | `0x00580B70` |
| +0xAC | combat list | own entries dropped at every mode set | `0x0057C980` |
| +0xC4 | flags | bit 0x1 changed (set by every mode set), 0x2 tile, 0x10 new, not yet announced to clients (every allocation; cleared with 0x1 by the room clean-up `0x00553220`; `items/inventory-moves.md` §6.3; D2MOO `INITSEEDSET`), 0x40 cleared by attack-mode starts, 0x100 hover freed, 0x2000 queued (`unit-order.md` §6), 0x10000 dead, 0x80000 monster mode changing | `0x00555230`, `0x00624690`, `0x0057FED8`, `0x005541B8`, `0x005A7C20` |
| +0xC8 | flags 2 | 0x400 source-unit link set (+0x94/+0x98), 0x2000000 expansion (game +0x70 ≠ 0), 0x4000000 server unit (every allocation) | `0x005552B6` |
| +0xD0 | node index: target-node list slot 0–9 (`monsters/ai.md` §5.2), 11 = in no list | 11 at allocation | `0x00555339` |
| +0xDC | timer list head | `unit-order.md` §8 | `0x00553980` |
| +0xE0, +0xE4, +0xE8 | update, hash, room links | `unit-order.md` | — |

"Dead" (`0x005541B0`): a null unit, flag 0x10000, a player in mode 0 or
17, a monster in mode 0 or 12, or any unit of another type.

**Owner links** (2026-10-09, pc1-data Step 4 item 21; the `own` field of
`tools/state-snapshot.md` §2). Three separate links, each 1.14d-confirmed:

1. Monster owner (pet, summon, minion, hireling, pack leader):
   `0x0058F0D0(U)`: U a monster with monster data (+0x14) and AI control
   C (monster data +0x28, `monsters/ai.md` §3.1) whose +0x28 (game) ≠ 0
   → the unit of type u32 C +0x30, GUID u32 C +0x2C (`0x00552F60`); else
   none. A released pack has GUID −1 (§4.6 step 3.1), which resolves to
   none.
2. Source-unit link (missiles; also any unit given one by a skill):
   `0x00552FD0(U)`: +0xC8 bit 0x400 → the unit of type u32 +0x94, GUID
   u32 +0x98; else none. Written only by `0x00621C30` / `0x00621CE0`
   (`skills/bodies.md` §6.20).
3. Item holder: item data (+0x14) +0x5C inventory (0 when no inventory holds it)
   → inventory +0x08 owner unit (`items/inventory.md`
   §1.1). Distinct from item data +0x0C, the owner-player GUID read by
   `0x00629F20` (−1 none; §6.5 event 3, §3.3 compress).

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
   4.1. A player draws no seed here; its load does (recorded 2026-10-09,
      q-fix-real-unit-seed-order). The character load of the join
      (`intents-events.md` §8.2 rule 2, full save and stub alike) runs
      `0x00552DF0` on the player it allocated: one game-seed step `lo'`,
      unit seed `init_low(lo')`, `dwInitSeed` `lo'`. Nothing draws between
      the allocation and this step: it is the first game-seed step after
      the four of game creation (`rng.md` §5.2), before every item the
      load makes (a save's items, a stub's start items) and before the
      act's DRLG (§8.2 rule 4). Recorded under Wine, `-seed 1234`: the
      step at seq 2349 (`0x00552E31`, unit record +0x20 at 0x2EF2520,
      outside the town units' block) gives 4048349444; with `ScnAma` (full
      save, no items) the town's first object takes the next step
      (108806926, seq 7270), with `StubAma` (stub) the first of the eight
      start items does (unit 108806926, item 4040195123, seq 2352–2355).
      The call site inside the load is not identified; the order is.
      The death corpse draws its seed the same way after its allocation
      (`combat/vitals.md` §4.7 r1.4).
5. Flags |= 0x10; node index := 11.
6. GUID: a monster with flags bit 2 takes the fixed GUID; every other
   unit draws one (`0x00552EE0`, `unit-order.md` §1.3).
7. Per-kind init (§1 table).
   7.1. Not linked yet (read 2026-10-07). Steps 1–7 put the unit in no
      list and give it no room or position: `0x00620290` zero-fills the
      record (0xF4 bytes; monster data 0x60 bytes), so the path (+0x2C)
      of a player, monster or missile is 0, and the static path that
      `0x00623520` allocates for an object, item or tile is zeroed
      (room 0, x = y = 0). Room and position are written only by
      `SUNIT_Add` (step 8): path `0x00649D00(…, room, x, y, unit, 0)`
      (types 0, 1, 3) or static position `0x00620AE0(unit, room, x, y)`
      (types 2, 5, item mode 3), then room-list insert, hash insert
      `0x00553060` and update queue `0x0064C040` (`unit-order.md` §3.1).
      During step 7 the unit's room (`0x00620BB0`) is 0 for every kind.
   7.2. Room and level source: the allocation's `room` argument (stack
      argument 4), the value step 3 takes the act from and step 8 passes
      to `SUNIT_Add`; no per-kind init reads the unit's own room or
      position.
      - Monster `0x00574250` (ECX game, EDX room; stack unit, GUID; no
        x/y): room → region data `0x00547BC0(game +0xF0, room, unit)`;
        → stats and skills `0x00573CB0(game, room, …)` (level from
        `0x0061A1B0(room)`, `monsters/init.md` §7; player-count bonus
        `0x00573930(room, unit)`); monster data +0x58 :=
        `0x0061A1B0(room)` (level id); mode ≠ 0, 12 → quest chain
        `0x00545CD0(game, unit, room, 1)` (level record of
        `0x0061A1B0(room)`). `monsters/init.md` §5 owns the steps.
      - Object `0x0054F5D0` (ECX game, EDX unit; stack GUID, room, x,
        y): room, x and y go only into the InitFn record {game, object,
        room, control, objects record, x, y} (`world/objects.md` §3
        step 6); the control getter `0x00546FA0` ignores the room. An
        InitFn that needs the level or the position reads it from the
        record.
   7.3. The unit's own (empty) placement as seen by an init: a mode set
      inside the init (`0x00624690`: monster mode 0 / 12 through
      `0x00553570`, and many object InitFns) calls `0x0064C040`, which
      finds room 0 and queues nothing (step 8 queues). The monster's
      `0x005533D0` finds +0x2C = 0 and takes the `0x00620F00` branch,
      not `0x00623F50`. HaremBlocker (InitFn 30, `0x0059B7D0`) in quest
      mode 2 frees its footprint (`0x00623830`) at room 0, (0, 0):
      `0x0064DC00` → `0x00463740(room 0)` returns 0, no collision change.
   7.4. Unit lists read by a per-kind init: never a room unit list
      (room +0x74) or the update queue; only game hash lists
      (`unit-order.md` §2), and never for this unit except CountessChest:
      - Monster: player count `0x00535790` walks the player hash (game
        +0x1120, `0x005538D0`); monequip (`0x005D6B60` → `0x00573B20` →
        `0x005606B0`) looks up the item it just created in the item hash
        (`0x00552F60(game, 4, GUID)`).
      - CountessChest (InitFn 47, `0x00595A50`) appends its own GUID to
        the quest's chest list, then `0x005954F0` looks every listed
        GUID up in the object hash (`0x00552F60(…, 2, …)`) and reads the
        room and position (`0x00620BB0`, `0x00620870`) of each one
        found. The new chest is not linked yet, so its own lookup fails
        and it is skipped.
      - HratliStart (49, `0x005B70B0`), NatalyaStart (52, `0x005BCE80`):
        a monster by stored GUID (`0x00552F60(…, 1, …)`). Zoo (79,
        `0x0058E830`) and `0x00544300` (CagedWussie 62, HellForge 48,
        FrozenAnya 74): walk the player / monster hashes (`0x005537D0`,
        game +0x1120, +0x1320).
   7.5. Units created inside a per-kind init (monster spawns of object
      InitFns through `0x005B2F20` / `0x005B3090` → `0x005B2A00`, which
      adds them, `monsters/init.md` §4; GoldPlaceHolder's item; monequip
      items) are allocated, and spawned monsters linked, before this
      unit's step 8. This unit's GUID was drawn first (step 6); its
      room-list insert comes after theirs (prepend: it ends up ahead of
      them in the room list).
8. Flags & 0x1 (`0x00555443`): `SUNIT_Add` `0x00554850(unit, x, y,
   game, room, 1)` (`unit-order.md` §3.1), after the per-kind init of
   step 7; its result is not tested. Then a player in mode 0 or 17, or a
   monster for which `0x0063EA40` holds and `0x004638A0(class, 0x13)`
   does not, gets path settings (`0x00649560(1)`, `0x00649190(5)`,
   `0x00648C30(0x8000)`; `sim/path-placement.md` §5.3 rule 3).
   `0x0063EA40(U)`: U is a monster in mode 0 (DT) or 12 (DD).
   `0x004638A0(class, 0x13)`: monstats2 flag 19 of the class's
   monstats2 row (bit array at record +4). The three calls: remove the
   footprint (force), pattern +0x48 := 5, mask := 0x8000 and restamp
   (read 2026-10-09).
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
   `+0x1C` = −1); `+0x28` the AI special state (AI control +0x00,
   `monsters/ai.md` §3.1; `0x005B0D60(monster data +0x28)`); `+0x2C`
   monster data +0x58, the level id (`monsters/init.md` §5 step 6;
   `0x00573520`, 0 without monster data); `+0x30` u16 the name seed
   (monster data +0x14; `0x005A0140`, 0x1506 without monster data);
   `+0x32..+0x3A` the 9 umod bytes; `+0x3C` u16 superunique index;
   `+0x40` stat 13, `+0x44` max life (stat 7 total, layer 0;
   `0x00625D10`, 0 without a stat list), `+0x48` stat 6 (life); `+0x54` the game frame; `+0x58`
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
      (`0x00541990`) by the record reader `0x00558CB0`
      (`world/vendors.md` §7.3 step 3), which allocates a **new item
      unit (new GUID)**. The item's ground expiry (item data +0x24) :=
      the stored expiry when it is 0 or ≥ frame (game +0xA8) + 15,000,
      else frame + 15,000. A record with socketed items (filled-socket
      count from the header peek `0x0062E410`) reads each child record
      in turn (a missing child is fatal 0x10C), puts it in the item's
      inventory (+0x60, created with `0x0063ABD0` when absent;
      `0x0063B210(inv, child, 1)`) and refreshes the item
      (`0x0055FE60`).
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
in this order: prepare animation `0x005533D0` (`skills/sequences.md` §2; action frame := 0;
sequence modes — player 18, monster 14 with a sequence — load the
sequence and its rate; otherwise sequence := null, current frame := 0,
rate `0x00623F50` when the unit has a path, else only the AnimData
lookup `0x00620F00`; frame count := AnimData frames · 256), cancel
`0x00553990` (the unit's events of type 0, then type 1, any argument),
then §4.2. Movement starts set the mode, cancel and schedule §4.4.

PROVISIONAL: the re-init `0x00624390` of a player or monster sets
action frame := 0, frame := frame bonus · 256, the AnimData record of
the new mode, frame count +0x48 := its frames · 256 and, for a unit
with a path, speed +0x4C := the rate `0x00623F50` (§4.7), as the prepare
step does for a plain animation; the velocity half and the sequence
loads are left to the mode starts (because no spec writes its player /
monster branch, and a 1.14d Amazon standing in town after the join
reads +0x48 = 4096, +0x4C = 80, the AMTNHTH record, with no animated
start run: REC-590 `poke-fallen-town` state diff); settled by REC-592
(record_state.py `fr`, `fc`, `sp` across a mode change without an
animated start: join in town, walk start, a monster's NU after a
think).

#### 4.2 Animation schedule (events 0 and 1)

Inputs: frame f; speed s and frame count F (+0x3C/+0x34 with a
sequence, else +0x4C/+0x48); event bytes E[i] (sequence: `0x006634C0`,
owned by `skills/sequences.md`; else AnimData record +0x10 + i); start index.

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

**Frame advance and +0x4E** (REC-701, 2026-10-09). The only writer of
+0x4E on a monster's event-0 path is the frame advance `0x00623E00(U)`,
called by the event-0 function itself (`0x005A7670` at `0x005A76E9`,
`0x005A7701`, `0x005A7734`; also `0x005A8670` SQ, client `0x00463390`,
`0x004B1280`). The timer's args (E[i], k) are not read by `0x005A7670`
and nothing on the timer run writes +0x4E. Rule:

1. +0x4E := 0.
2. No sequence (+0x30 = 0): j = cur >> 8, + 1 when speed ≥ 256; cur
   += speed. While cur ≥ F: every j < F >> 8 with E[j] ∈ 1..4 (j < 144)
   sets +0x4E := E[j]; cur −= F; j = 0; cur += 256·b (frame bonus).
   Then every j ≤ cur >> 8 likewise. So +0x4E ends as the last action
   byte in the frames crossed this advance, else 0.
3. Sequence (+0x30 ≠ 0): flags +0xC4 &= ~0x4000; seq pos +0x38 += +0x3C
   (wrapping at +0x34); +0x48 −= +0x3C; `0x00621210(old pos)` reads the
   sequence frame: +0x44 := frame·256, +0x40 := mode, +0x4E := its
   event byte; mode changed → +0xC4 |= 0x4000.

`0x006218D0(U, i)` (set +0x4E from E[i] when 1–4) has no callers.

Speed 0 gives event 1 at f + 1 in all forms. Callers: `0x00553B10` from
`0x0056E210`; `0x00553C70` from skills `0x005C8CA0`, `0x005C8E30`,
`0x005D68A0`, `0x005D69D0`; `0x00553DC0` from skills `0x005CF900` (3),
`0x005D9580`, `0x005DA120`, `0x005DA7E0`.

#### 4.3 Animation rate and frame bonus (dependency)

Speed +0x4C is computed by `0x00623F50` from the AnimData speed (+0x0C)
and percentages from stats 67, 68, 69 and states (clamps 15–175 %); the
frame bonus `0x00623B10` is non-zero only for some player modes (table
`0x006E8E60`, by class and weapon type). Both are §4.7 (answered
2026-10-07); §4.2 takes them as inputs.

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
current mode's event-0 / event-1 function. The A-family event 0 `0x005A7670` ignores the schedule args; its +0x4E test reads the byte its own frame advance `0x00623E00` just wrote (§4.2 "Frame advance and +0x4E"; trigger rule `skills/use.md` §5.2). Neutral start `0x005A73E0`
sets mode 1 and, unless the unit has a type-2 timer with expire > f
(`0x005415A0`: smallest positive expire of its type-2 timers, 0 if
none), schedules event 2: f + 45 with state 21, else f + `aidel`
(monstats +0x4F, the Normal column, or +0x4F + difficulty (game +0x6D)
when game +0x6A or game +0x74 is non-zero; 0 → 15).

**Death and dead functions** (2026-10-08; this section owns them,
`monsters/init.md` links here). All take ECX game, EDX the mode-change
record R (unit U = R +4) and return 1; "set mode m" = the plain mode
set `0x00553570(game, U, m)` (§4.1). **Mode 12 (DD) is always set by
`0x00553570`**, from three places: DT event 0, DT event 1 and the DD
start; `0x005A7C20` never writes it itself.

1. **DT start** `0x005A6FF0`:
   1. Set mode 0.
   2. Death clean-up `0x005A6520(U, R byte +0x14)` (ESI U, EBX game,
      `ret 4`): U's overhead record (+0xA4) non-null → freed
      (`0x006611A0`), +0xA4 := 0, U queued for update (`0x0064C040`),
      flags (+0xC4) |= 0x100; `0x0058F6C0(U)`; `0x005B1A90(game, U)`;
      flags &= ~0x800C; `0x00627540(U)` (`stat-lists.md`);
      `0x00639FB0(U, boss)` with boss = `0x0063E9F0(monstats row, U)`;
      U has state group `hide` (`0x0063A320`) → flags &= ~0x2; the
      class's monstats2 flag 0x13 (`0x004638A0(class, 0x13)`) clear →
      dead-body footprint `0x00649F70(U, 1)` (`skills/bodies-3.md`
      §3.9); `0x006488A0(path, R byte +0x14)`; `0x005738D0(game, U)`.
   3. Treasure gate `0x005A6830(game, R, 0)` (`items/treasure.md` §3.1);
      evil-killed counter `0x00547E50` (`monsters/population.md`);
      `0x0061AFA0(U's room, U's GUID)`.
   3.1. Helpers of steps 1.2–1.3 (read 2026-10-08):
      - `0x0058F6C0(U)` (ECX U), pack-leader handover. C := U's AI
        control (`monsters/ai.md` §3.1). C flags (+0x08) bit 0x1 clear
        → nothing. Bit 0x1 set, 0x2 clear (tail at `0x0058F660`): every
        GUID of C's minion list (+0x34, next at +4) that resolves
        (`0x00552F60`) to a monster gets owner-ex (+0x2C, +0x30) :=
        (−1, 1) and +0x28 := C +0x28 (the pack is released). Both set
        → `0x0058F530`: N := the first list entry that resolves to a
        unit (none → nothing); N a monster → N's control flags |= 0x2,
        |= 0x1 (`0x005DD230`), owner-ex := (N's GUID, 1), +0x28 := C
        +0x28. Then C's owner-ex := (N's GUID, 1), +0x28 := C +0x28,
        and U joins its owner's minion list (`0x0058F100(U)`). Then,
        from the list head, every entry resolving to a unit other than
        N: a monster gets owner-ex (N's GUID, 1) and +0x28 := C +0x28;
        each such unit joins its owner's list (`0x0058F100`).
      - `0x005B1A90(game, U)`: U leaves its target-node list, +0xD0 :=
        11 (`skills/bodies-2.md`, the slot-list paragraph).
      - `0x00639FB0(U, boss)`: over the W words of U's state bits
        (`stat-lists.md` §9.1; nothing without an extended list): every
        set bit not in the keep mask is cleared and marked in the
        changed half. Keep mask = states `bossstaydeath` (bitset 15,
        data tables +0x108) when boss ≠ 0, else `plrstaydeath`
        (bitset 13, +0x100) for a player, else `monstaydeath` (bitset
        14, +0x104). U is then queued for update (`0x0064C040`). The
        state lists are not touched here.
      - `0x005738D0(game, U)` (ECX game, EDX U): cancel U's type-2 and
        type-3 events (`0x00540E60(2, 0)`, `(3, 0)`).
      - `0x0061AFA0(room, GUID)`: room +0x38 + 4 · (room byte +0x14) :=
        GUID, then byte +0x14 := (byte + 1) & 3: a ring of the room's
        last four dead GUIDs (null room → nothing).
   4. U's monstats `deathDmg` (+0x0E bit 4) clear → done. Else by
      `BaseId` (row +0x02; a non-monster takes the last branch):
      - 212 `bonefetish1`: (x, y) = U's position; m =
        `skill_missile(game, 117, U, 0, 1, 0, 0, x, y, 1)`
        (`skills/bodies.md` §2.4), none → done. H = pct(maxHP of stats
        by level (`monsters/init.md` §8.1, L-flag = game +0x74, d =
        game +0x6D, U's `level(12)`, flags 1), difficultylevels
        `MonsterCEDamagePercent` (+0x3C), 100); b = pct(H, 60, 100)
        (inline); dmg = b + `roll(H − b)` on U's seed; record (zeroed
        0x70) physical (+0x08) := dmg << 7; area damage `0x0057E090(game,
        m, x, y, r 5, rec, 0, 0, null, 0x581)` (`monsters/umod-callbacks.md`
        §3.2): players only.
      - 441 `siegebeast1` (`0x005A6EB0(game, U)`): O = U's owner
        (`0x00552FD0`, the rider); none or no monstats row → done. At
        O's position with a free spot (`0x0064E7B0(O's room, &pos,
        0x00620510(O, 0x3C01, 0), …)` ≠ 0) and U's alignment
        (`0x006259B0`) = 0: skill use `0x005DEAD0(game, O, mode = O's
        monstats +0x180, skill = `Skill1` +0x170, 0, x, y)`; success →
        delete O's thinks (`0x00540E60(game, O, 2, 0)`). Otherwise kill
        O (`0x0057CCB0(game, O, 0, 1)`, `combat/damage.md` §7.2).
      - other (`0x005A6DF0`, EDI U, EBX game): for each unit P of U's
        own room list (+0x74, next +0xE8; no neighbour rooms): P a
        player not in mode 17, distance `0x006416D0(U, P)` ≤ 2 and
        `0x00622B50(P, U, 0x3C01)` = 0 → record (zeroed 0x70): result
        flags (+0x04) := 1, | 4 when P lacks state 54
        (`uninterruptable`); physical (+0x08) := P's `hitpoints(6)` >> 5;
        prepare `0x0057C1E0`, apply `0x0057C6C0(game, U, P, 1, rec)`,
        reaction `0x0057CEE0` (`combat/damage.md` §5, §7.1).
2. **DT event 0** `0x005A7350`: BaseId 78 (`0x0063E8D0(U, 0)`): path
   step `0x00554CA0`, animation refresh `0x00623E00`, and set mode 12
   only when the animation is complete (`0x006217C0`); any other
   monster: set mode 12 at once. (`sim/intents-events.md` §7 rule 3
   gives the client messages.)
3. **DT event 1** `0x005A72B0`: set mode 12; skill event 13
   (`0x005C0C30(game, 13, U, 0, 0)`, `skills/bodies.md` §2.18); then
   monstats `SplEndDeath` (+0x1A4): 1 → `minion1` (+0x26, i16) in 0 …
   class count → class reinit `0x00574370(game, U, minion1, 1)`
   (`monsters/init.md` §27) then think restart `0x00573780`; 2 → kill
   U's owner (`0x00552FD0`) with `0x0057CCB0(game, O, 0, 1)`; else
   nothing.
4. **DD start** `0x005A7390` (a mode set straight to 12, e.g. creation
   in mode 12 or a corpse restore): U in mode ≠ 0 → death clean-up
   `0x005A6520(U, R byte +0x14)` (step 1.2); set mode 12; cancel U's
   events of types 8 and 9 (`0x00540E60(game, U, 8, 0)`, then 9). A U
   already in mode 0 skips the clean-up (its DT start ran it).

Draws: only the bonefetish branch (U's seed, one `roll`, plus the
missile creation's own); the siege-beast skill use per its spec.

**What keeps a dead monster dead** (2026-10-09, read from `0x005A7C20`,
`0x005541B0`, `0x005A73E0`, `0x005A75C0`, `0x005A7F80`, `0x005B1740`;
the playthrough's "returns to mode 1 with hp 0" question):

- *Sequence.* The kill requests mode 0 through the mode set
   `0x005A7C20` → DT start (rule 1): mode 0, the death clean-up, the
   treasure gate. DT schedules its animation events (mode table:
   schedules = yes). DT event 0 sets mode 12 at once (every monster but
   BaseId 78, which waits for the animation end); otherwise DT event 1
   at the animation end sets mode 12 (rule 3). Mode 12 is set by the
   plain mode set `0x00553570`, never by a request, and DD schedules
   nothing (mode table: schedules = no), so no event of U runs after
   it unless something else schedules one.
- *Why the AI stops.* There is **no** dead test in the think: the
   class event handler `0x005A7F80` drops a type-2 event only for a
   frozen **live** unit (`0x005541B0` = 0), and the think body
   `0x005B1740` runs whatever AI the control names. What stops it is
   that no think exists: the death clean-up's `0x005738D0(game, U)`
   cancels U's type-2 (think) and type-3 events (rule 1.2, last call),
   and none of the type-2 schedulers runs for a dead U afterwards:
   `0x00573780` returns for mode 0 or 12 (`monsters/ai.md` §1.2); the
   freeze end `0x0057B170` does nothing for a dead unit (`ai.md` §1.1);
   the neutral start `0x005A73E0`, the knockback end and every AI
   tactic run only from a mode start or a think, which a dead U no
   longer gets; the damage path refuses a dead defender
   (`combat/damage.md` §5.2, `0x005541B0`).
- *A mode request to a dead monster.* `0x005A7C20` does **not** refuse
   it. With U dead (`0x005541B0`(U) = 1: U null, flag 0x10000 (+0xC6
   bit 0), or a monster in mode 0 or 12): it skips the path stop
   `0x00649400`; for a mode ≠ 3 it still writes the path target and
   runs the movement set-up; then it runs the requested mode's start:
   - a start that returns 0 (WL / RN with no path point, a GH class
     without mode 3, A/S with a moving mode and no point): skill
     cleared, then, because the requested mode is not 0 or 12 and U is
     dead, `0x005A7C20` **returns 1 with the mode unchanged**;
   - GH start: U in mode 0 or 12 → returns 1, mode unchanged (rule 6);
   - the NU start `0x005A73E0` sets **mode 1** and schedules a think
     (f + `aidel`, 0 → 15); the attack / skill start `0x005A75C0`
     (modes 4, 5, 7, 8, 9) sets its mode whenever it does not return 0
     (rule 7); the other starts as their rules state. So a mode-1 or attack request on a dead monster
     *does* bring it out of mode 12 with hp 0; 1.14d never makes one,
     because of the previous point.
   A request for mode 0 or 12 on a dead unit runs DT start / DD start
   again (DD start skips the clean-up when U is in mode 0, rule 4).
- *The symptom's shape.* A think left pending at death fires
   `aidel` frames later (15 for the Act I monsters, `ai.md` §1.3); the
   AI's first idle (`0x005DE080`, `ai.md` §1.2: "if the anim mode is
   not neutral, first a mode change to neutral") requests mode 1 →
   point 3 → mode 1 with hp 0. So a death start without the
   `0x005738D0` cancel shows exactly "mode 1, hp 0, about 15 frames
   after death".

**The other start and event functions** (1.14d-read 2026-10-08,
`0x005A7490`–`0x005A7891`, `0x005A8490`–`0x005A872F`). Same calling
convention; "set mode m" as above. A start returning 0 makes
`0x005A7C20` run the neutral start instead. The path fields, the
velocity request and the path compute of a moving request are done by
`0x005A7C20` **before** the start runs (`monsters/ai.md` §7.5).

5. **WL start** `0x005A7520`, **RN start** `0x005A7550`: path point
   count (+0x28, `0x00648780`) = 0 → return 0 (neutral); else set mode
   2 (WL) / 15 (RN), return 1. So a walk or run whose compute found no
   point never enters the mode.
6. **GH start** `0x005A7580`: U null, or U in mode 0 or 12 → return 1,
   mode unchanged. Class without mode 3 (`0x0046C140(class, 3)`) →
   return 0. Else set mode 3, return 1.
7. **Attack / skill start** `0x005A75C0` (modes 4, 5, 7, 8, 9; m = R
   mode): m a moving mode for U (`0x005A6B10`, `monsters/ai.md` §7.1):
   point count ≠ 0 → compute the path again (`0x00649970(path, U, 0)`)
   and go on; point count 0 → U's `BaseId` 110 (`vulture1`): compute,
   set mode m, return 1 (no skill start); any other → return 0. Then
   (and directly for a non-moving m): set mode m; U has a used skill
   (`0x00620250`) → skill start `0x0056FAF0(game, U)` (`skills/use.md`
   §5.3; its result ignored); return 1. So AI attacks and skill modes
   do enter their mode; the used skill is the one the request set
   (`monsters/ai.md` §7.1 `0x005DEAD0`).
8. **BL start** `0x005A77C0`: set mode 6, return 1.
9. **KB start** `0x005A77D0`: class without mode 3 or without mode 13
   (`0x0046C140`) → return 0. U null or in mode 0 → return 1, mode
   unchanged. Else path type := 8 (set type `0x00648CF0`, `sim/pathing.md`
   §12.5), distance budget (+0x90, `0x00648E40`) := 10 for `BaseId` 78
   (`sandleaper1`), else 5; compute (`0x00649970(path, U, 0)`); set mode
   13; return 1.
10. **SQ start** `0x005A7870`: set mode 14; U flags (+0xC4) &= ~0x40;
    return the skill start `0x0056FAF0(game, U)` (0 → neutral).
11. **S3** (mode 10): start `0x005A7490` sets mode 10, returns 1. Event
    0 `0x005A74A0`: step `0x00554CA0` (result ignored), animation refresh
    `0x00623E00`, animation complete (`0x006217C0`) → set mode 11;
    returns 1. Event 1 `0x005A74D0` does nothing. (Mode 10 has no
    schedule flag, so `0x005A7C20` schedules neither.)
12. **S4 start** `0x005A74E0`: set mode 11; think (event type 2) at f +
    15 (`0x005417D0(game, U, 2, f + 15, 0, 0)`); return 1.
13. **Event 0 of the moving modes** (ECX game, EDX U; 1 = go on, 2 =
    stopped):
    - WL `0x005A8490`: U has state 13 → `0x005C9D90(game, U)`; U has
      state 22 → `0x005CE4F0(game, U)` (skills spec); neither result is
      tested, both run before the step and never gate it. Step
      `0x00554CA0` (`sim/pathing.md` §9.3): 2 → mode end `0x005A8030`
      (`monsters/ai.md` §1.4), return 2; else 1.
    - RN `0x005A84F0`: the WL body without the two state calls.
    - KB `0x005A8630`: step (result ignored), animation refresh,
      animation complete → KB event 1 `0x005A8520` and return 2; else 1.
    - SQ `0x005A8670`: animation complete → mode end, return 2. Else E
      := the used skill, f := its E flags (`0x006446A0`), "do left" :=
      1. f bit 0 (moving skill): target check `0x00553490`, step; result
      2 → E flags := f | 2 (`0x00644660`), do `0x0056FC50` (`skills/use.md`
      §5.4), do left := 0. Then by U's frame code (byte +0x4E): 4 → do;
      else do left, U flag 0x40 clear and code 1 or 2 → do. Animation
      refresh; return 1.
14. **KB event 1** `0x005A8520`: path type reset `0x00648DC0`
    (`sim/pathing.md` §1.5 rule 1). Class without mode 3 → think at f +
    1, done. A monster with `BaseId` 78 → think at f + 45 with state 21,
    else f + 15, done. Otherwise: used skill := none, U +0xB0 := 0xA0,
    mode request 3 (record zeroed, mode 3, unit U, byte +0x15 := 100;
    `0x005A7C20(game, &req, 1)`).

#### 4.7 Animation rate `0x00623F50` and frame bonus `0x00623B10`

**Rate** `0x00623F50(unit U, file, line)` (`ret 0xC`; 23 callers: every
mode start and animation re-init, `0x00624390`, `0x005735A0`). It
writes the speed U +0x4C (and in the cast and attack cases also the
sequence speed +0x3C) and, in two cases, the path velocity
(`sim/pathing.md` §8.1 owns the velocity rules; this section owns the
speed).

Definitions:

- (T, C, M) = the draw identity of (type, class, mode +0x10) after the
  disguise substitution `0x00645270` (`render/unit-composite.md` §1.1).
- s = the AnimData speed (+0x0C) of the record that `0x00620F00(U, T,
  C, M)` stores in U +0x50 (`formats/animdata.md` §5).
- E(r), the diminished item bonus of row r of table `0x006E8E24`
  ({flag, c, stat}, 12 bytes): v = the unit's item/skill value of the
  stat (`0x00625500(U, stat, 0)`); flag = 1 and v ≠ 0 → v := c·v / (c
  + v) (i32, truncating; every 1.14d row has flag 1). Rows: 0 stat 93
  `item_fasterattackrate`, c 120; 1 stat 99 `item_fastergethitrate`,
  120; 2 stat 105 `item_fastercastrate`, 120; 3 stat 102
  `item_fasterblockrate`, 120; 4 stat 96 `item_fastermovevelocity`, 150.
- total(k) = the unit total of stat k (`0x00625480(U, k, 0)`).
- D(x, f) = s·f / 100 as unsigned 32-bit (the product is taken as i32,
  then divided unsigned), then 0 if ≤ 0 (signed) and at most 0x7FFF.
  A negative f therefore gives 0x7FFF.
- Mode tables (`0x006E8A00` players by mode 0–19; monsters by mode 0–15:
  `0x006E8B90` for class < 410, `0x006E8CD0` for class ≥ 410), 5 i32 per
  mode: V-skill, V, A-skill, A, fixed. 1.14d: players V = 2, 3, 6; A = 7,
  8, 11, 12; V-skill = A-skill = 13–16, 18; fixed = 0, 17. Monsters
  (< 410): V = 2, 8–11, 15; A = 4, 5; V-skill = A-skill = 8–11, 14;
  fixed = 0, 12. Monsters ≥ 410: as < 410 without V for 8–11.

Steps, first match wins:

1. U none, type 4 (item) or type ≥ 5 → nothing.
2. Look up A (above).
3. **Cast** (`0x006216E0`): T = 0 and M = 10 (SC), or T = 0, M = 18 (SQ)
   and the used skill's (`0x00620250`) skills `seqtrans` (+0x12) is 10;
   or T = 1 and M = 7 (SC). Speed := D(s, min(100 + E(2), 175)); +0x3C :=
   the same. No lower clamp.
4. **Block**: (T, M) = (0, 9) or (1, 6). f = 50, or 100 when U has state
   101 `holyshield`; q = D(s, f + E(3)), at least 1. Speed := q.
5. **Get hit**: (T, M) = (0, 4) or (1, 3). Speed := D(s, 50 + E(1)).
6. **Knockback**: (T, M) = (0, 19) or (1, 13). Speed := w clamped to
   0..0x7FFF, w = `0x006213D0(T, C, M)`: player: 101 when M = 3, else
   213; monster: monstats run speed u16 +0x38 when M = 15, else walk
   speed u16 +0x36 (`data/fixups.md` §8). Velocity := 0x1000.
7. **Velocity modes** (`0x006214A0`, `sim/pathing.md` §8.1 rule 2): U
   without a path (+0x2C) → nothing at all. p = max(E(4) + total(67
   `velocitypercent`), 25). Speed := w·p / 100 (i32, truncating; then 0
   if ≤ 0, at most 0x7FFF), w as in step 6; velocity := base·p / 100
   (`sim/pathing.md` §8.1).
8. **Attack modes** (`0x00621580`): the mode row's A ≠ 0, or A-skill ≠ 0
   and the used skill's skills row has `UseAttackRate` (bit 19 of the
   flags at +0x04). Then:
   1. Dual-wield stat toggle `0x00623C80(U, 1)` (below).
   2. v = E(0) + total(68 `attackrate`).
   3. U can dual-wield (`0x006235A0`: player class 4 or 6, monster class
      417 or 418) and the items at body locations 4 and 5
      (`0x0063BDE0`), each counted only when usable (`0x0062A4E0`), are
      both of item type 45 `weap`: v += (a4 + a5) / 2 − a4 (i32, the
      halving truncates toward 0), a_k = the item's total(68).
   4. U itself (not T) is a player in mode 18 (+0x10): v −= 30.
   5. v := clamp(v, 15, 175).
   6. b = `0x00646170(U)` when U has a state of group 38 (`meleeonly`,
      `0x0063A7B0`) and that value is ≠ 0; else s. `0x00646170`: only a
      player in a were-form (`0x0063A400`): W = the attack weapon
      (below, flag 0); n = W's attack frames `0x0062A710(U, W)`
      (`skills/bodies-2.md` §2.25), or 19 without W; n ≤ 0 → 0; result
      = (U +0x48 with the low byte cleared) / n (i32, truncating).
   7. Speed := +0x3C := D with b in place of s and f = v.
9. **Fixed** (`0x00621630`, the row's fixed ≠ 0): speed := s clamped to
   0..0x7FFF.
10. Otherwise: speed := D(s, clamp(total(69 `other_animrate`), 15,
    175)).

PROVISIONAL: d2rs computes the monster w of steps 6–7 (monstats +0x36 /
+0x38, `data/fixups.md` §8) at the rate call from AnimData with the
host's COF composer for (class b, mode 2 / 15), not the fixup's monster
composer (because the typed monstats rows carry no +0x36 / +0x38);
settled by REC-593 (a walking town NPC's `sp`, e.g. Charsi 192 in
`poke-fallen-town-unpinned`, and a run of a monster with `BaseId` ≠ its
row, in `record_state.py` against `d2-client state-dump`).

Measured (revision 2026-10-09, q-scenes-compare): the local player's
town walk (mode 6, w = 213, p = 100) is drawn at server tick T with frame
`((T − c) · 213 >> 8) mod 8`, c = the tick of the walk request that
started the walk; a new click while still walking keeps c (`a1-walk-n`
… `-w`, 1.14d frames 0, 3, 7, 3, 6, 2 at ticks 32 … 102, c = 22).
The run (mode 3) the same with speed w · p / 100, w = 101 and p = the
run's velocity percent 100 · `RunVelocity` / `WalkVelocity` (150 for the
sorceress' 9 / 6: speed 151), c = the run click that changed the mode
(a walk ↔ run change restarts c): `a1-run-n` … `-nw`, frames 5, 6, 6,
6, 6, 7, 7, 7 at ticks 150 … 248, c = 140. PROVISIONAL (REC-516): p as
that ratio (because w = 101 at p = 100 fits no start; settled by a run
of a class with another RunVelocity / WalkVelocity ratio).

Steps 7 and 8 assert (fatal) for types 2 and 3; no 1.14d caller passes
an object or missile (objects take `0x00624390`'s own branch,
`world/objects.md`). The rate draws nothing.

**Dual-wield stat toggle** `0x00623C80(U, flag)` (dual-capable units
only): S = the attack weapon (flag), W = the weapon in use
(`0x0063BEF0`). "Active" = `0x00625820(item, 0)` ≠ 0; "on / off" =
`0x00627910(U, item, 1 / 0)` (`sim/stat-lists.md` §8.4).
S none: W active → W on. S = W: W active → W on; the other weapon
(`0x0063BF90`) active → off. S ≠ W: S active → S on; W active → W off.

**Attack weapon** `0x00623990(U, flag)` (`ret 8`): no inventory → none.
D = the weapon pick `0x0063C9B0` (`skills/bodies-3.md` §3.3: location
4, else 5, usable and type `weap`). U not dual-capable → D. Used skill
(`0x006439A0`) or its skills row missing → D. q(k) =
`0x0063C050(inventory, k)`: the first of the 11 equip slots whose item
is, for type classes 2, 3, 12 (`1hs`, `1ht`, `ht1`; `0x00629FE0`): with
k = 5 the weapon in use, with k = 6 not the weapon in use; for other
items: its items `component` (+0x115) = k. By the skill's `weapsel`
(+0x168):

| weapsel | X |
|---|---|
| 1 | q(6) |
| 2 | A = q(5), B = q(6); A of type `weap`, B of type `weap` and the skill flag 0x2000 (`0x006446A0`) → B; A `weap` otherwise → A; A none or not `weap` → B |
| 3 | n = U +0x38 >> 8 (arithmetic); odd = n mod 2 (signed) for a server unit (+0xC8 bit 0x4000000), else n > 5; n = 0 → q(5); flag = 0 → q(6); odd ≠ 0 → q(6); else q(5) |
| 4 | none, and D := none |
| other | A = q(5) when of type `weap`, else q(6) |

Result: X when it is of type `weap` and usable (`0x0062A4E0`), else D.

**Frame bonus** `0x00623B10(U)` (`ret 4`; start index of §4.2): (T, C,
M) as above; T ≠ 0 → 0. M = 7 or 8 (A1, A2), or M = 15 or 16 (S3, S4)
when `0x006235A0(U)` holds: W = `0x00623990(U, 1)`; c = W's type class
(`0x00629FE0`; 0 without W); bonus = i32 table `0x006E8E60`[7·c + C].
Every other mode → 0. 1.14d table (84 entries, 12 type classes × 7
player classes): Amazon (0) and Sorceress (1): 1 for type class 0, 2
for type classes 2–6 (`1hs`, `1ht`, `stf`, `2hs`, `2ht`); everything
else 0. Type class 12 (`ht1`) indexes past the table into zero bytes
(`0x006E8FB0`…): 0.

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
| 13 | trade and vendor code `0x00567620`–`0x00568D10` | per caller | `0x005689D0`: reschedules itself (`unit-events.tsv` rows 55–63). Out of scope (Phases 0–6): its player-trade body (multiplayer only) |
| 14 | `0x00554EA0` | f + 50, callback `0x00554570` | the callback (skill cooldown end); the class table entry is null |
| 2, 4, 7, 10 | none for players | — | null entries |

Join `0x00534AD0`: neutral mode start (`0x005809D0`, mode 1), then
event 3 at f + 1, then event 11 at f + 250.

#### 6.2 Monster

| Type | Scheduled by | Expire, args | Handler does |
|---|---|---|---|
| 0, 1 | §4.6; skills `0x005CC4E0`, `0x005CC690`, `0x005CC3B0`; AI `0x005ECEE0`, `0x005ED2A0`, `0x005F6B70` | §4.2 / §4.4; per caller | mode functions, §4.6 |
| 2 | neutral start `0x005A73E0`; `0x005A74E0` (f + 15); `0x005A8520` (f + 1, or f + 15 / f + 45 for base id 78); NPC talk `0x00548B00`, `0x0054CA10` (f + 1); damage `0x0057B170`, `0x0057B230`; `0x00573780` (f + 2); AI and skill code (30 sites) | §4.6 or per AI | `0x005B1740`: AI think (AI spec; per-class AI record from monster data +0x28, pre-checks `0x005B10E0`, `0x005B1650`, `0x005B13E0`, then the AI function at record +4) |
| 3 | damage `0x0057AC50`, `0x0057ADD0`, `0x0057C6C0`; skill items; handler | f + 1, (0, 0) | `0x005A6920`: life regeneration (stat 74, `sim/stats.md`); reschedules at f + 1 unless state 52 holds and the rate is ≥ 0; a zero rate cancels its type-3 events |
| 6 | `0x005DE330`; handler | timeout | `0x005A7F00`, as the player's |
| 7 | monster unique mods `0x005A1330`–`0x005A4230`, quests, skills | per caller | `0x005A4370` (monster spec) |
| 10 | `0x005C3B30`, `0x005C3DE0` (f + skill value) | — | `0x005A7F70` (AI spec) |
| 5, 8, 9, 12 | as players | — | `0x0056D790`, `0x0056FCB0`, `0x0056FE40`, `0x005A7EF0`: `sim/stat-lists.md` |
| 4, 11, 13, 14 | — | — | null |

Types 0, 1, 2, 6, 7, 9, 10, 11, 13, 14 are dropped for a frozen monster
(`tick.md` §5.6).

Restore `0x00542B40` schedules no monster event (corrected
2026-10-07): its only two timer sites, `0x00542C87` (type 5) and
`0x00542D89` (type 2), sit after its saved-record type test = 2
(object; `0x00542C33`), so they schedule on restored objects only
(§6.4; `unit-events.tsv`, proof `code`).

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
| 1 | `0x00581490` (corrected 2026-10-07): only when the mode (u16 at +0x10) is 1 (OP) and objects `Mode2` (+0x141) ≠ 0: the mode field := 2 (ON) by a direct write (no mode set, no update queued, flags unchanged), then, still inside that branch, `HasCollision2` (+0x122) = 0 → footprint free `0x00623830`; otherwise nothing | operate and init functions: f + fc1 + 1 (29 sites), f + fc1 (17 sites, e.g. `0x00545850`, quest objects `0x0058BD50`…), f + 2·fc1 (`0x005B5630`) |
| 2 | `0x00581510` (D2MOO: well refill) | `0x005858A0`: f + `Parm0` + 1; restore `0x00542B40` (site `0x00542D89`) |
| 3 | `0x005818B0` | `0x0054FB90` (f + 25); handler (f + 15, f + fc1 + 1) |
| 4 | `0x005817A0` (trap) | `0x00582510` (f + 35) |
| 5 | `0x005814D0` (shrine reset: mode 0, data +0x0C := 0) | shrine operate `0x00583C70` (f + 1200·minutes + 1); restore `0x00542B40` (site `0x00542C87`) |
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
| 3 | `0x00562D30`: (a) item not broken (flag 0x100) and with durability (`0x00629930`): replenish(rate 252, value 72, max = max durability `0x00625E00`); (b) only when (a) did not run or returned 0, and the item is stackable (`0x006289F0`): replenish(rate 253, value 70, max = max stack `0x006295B0`). Replenish `0x00562C40`: r := total(rate); r = 0 → return 0 (no reschedule). v := total(value) + 1; v > max → return 1 (no reschedule: the chain ends when full). Else set base value := v; if the item's owner GUID (`0x00629F20`) ≠ −1 names a player with a client, send the stat update `0x0053D130`(client, item, 1, value stat, v, 0); if v ≥ 1 and the item is broken, `0x0055F900` (repair, `items/generation.md` §12.1). Reschedule event 3 at f + max(2500 / r + 1, 125) (integer division), args (0, 0); return 1 | start `0x00558530` (stat 252, durability), `0x00558580` (stat 253, quantity), `0x0055A2A0`: if r ≠ 0 and no type-3 event is pending, f + 2500 / r + 1 (integer division, no minimum) |
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

### 8. Collision line between two units

Owned by `render/draw-order-2.md` §15.1 (`0x00622AA0`, every caller
and mask) and §16 (the line test).

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
   difficulty (§4.6). +0x6A is the game type (3 in single player) and
   +0x74 the ladder flag (open question 7), so single player uses the
   difficulty's column.

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

- §3.1 r4.1 (2026-10-09, revision, q-fix-real-unit-seed-order): added
  from two Wine recordings of 1.14d (`tools/cloud-game/run.sh --python
  -- tools/trace-recorder/record_rng.py --auto ScnAma --seed 1234
  --input "wait 3; end"`, and the same with `--auto StubAma`). Before
  this revision the spec named no player seed, d2rs drew none, and every
  later unit of the game took the step 1.14d gives the next one (town
  objects one step early: the 2026-10-09 report's "from object guid 7 on
  no d2rs unit seed appears in 1.14d's draws"). The handoff's reading
  that the start items took the in-between steps was a misreading: the
  `ScnAma` save has no items. Checked by
  `d2-client/tests/app_unit_seed_order.rs` (real data) and
  `units::tests::the_player_load_draws_the_recorded_unit_seed_before_the_town`.
- §3.1 steps 7.1–7.5 (2026-10-07): `all.asm` `0x00555230` jump-table
  cases (argument registers at `0x00555393`, `0x005553ED`), `0x00574250`,
  `0x0054F5D0`, `0x00554850`, `0x00620290`, `0x00623520`, `0x00620AE0`,
  `0x00620BB0`, `0x0064C040`, `0x005533D0`, `0x00623830`, `0x0064DC00`,
  `0x00463740`; list reads found by a call-graph walk (depth 3–4) from
  `0x00574250` and every InitFn of `world/object-functions.tsv` for calls
  to `0x00620BB0`, `0x00620870`, `0x00552F60`, the room-list routines and
  references to room +0x74 / unit +0xE4, +0xE8 / game +0x1120…+0x1B20,
  each hit read (`0x005954F0`, `0x005B70B0`, `0x005BCE80`, `0x005537D0`,
  `0x005538D0`, `0x005606B0`, `0x005435C0` = quest list, not a unit
  list; `0x0058EAC0` +0xE8 = quest data).
- §4.6 death functions (2026-10-08): `all.asm` `0x005A6FF0`,
  `0x005A6520`, `0x005A6DF0`, `0x005A6EB0`; `disasm.py at 0x5A72B0` for
  `0x005A72B0`, `0x005A7350`, `0x005A7390` (not in the export); monstats
  bit `deathDmg` from `data/fields.tsv`, mask `[0x006CE278]` = 0x10
  (mask table `0x006CE268` dumped: 1, 2, 4, …); BaseId 212 / 441 names
  from `patch_d2` `monstats.txt`.
- §4.6 rules 5–14 (2026-10-08, gaps MV1–MV3 of
  `docs/handoff/impl-path-motion.md`): `disasm.py fn` on `0x005A7490`,
  `0x005A74A0`, `0x005A74E0`, `0x005A7520`, `0x005A7550`, `0x005A7580`,
  `0x005A75C0`, `0x005A77C0`, `0x005A77D0`, `0x005A7870`, `0x005A8490`,
  `0x005A84F0`, `0x005A8520`, `0x005A8630`, `0x005A8670` (none in the
  Ghidra export); helpers `0x00648780` (+0x28), `0x00648E40` (+0x90);
  `BaseId` 110 = `vulture1` from `patch_d2` `monstats.txt`.
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

§4.7 (2026-10-07): 1.14d asm of `0x00623F50`, `0x00623B10`,
`0x00623990`, `0x00623C80`, `0x006216E0`, `0x006214A0`, `0x00621580`,
`0x00621630`, `0x00621740`, `0x006213D0`, `0x00621360`, `0x006235A0`,
`0x00646170`, `0x0063C050`, `0x00628660`; tables read from
`game/Game.exe`: `0x006E8E24` (animstat rows), `0x006E8A00`,
`0x006E8B90`, `0x006E8CD0` (mode rows), `0x006E8E60` (frame bonus,
checked zero through `0x006E8FDC`), jump tables `0x00623C04` /
`0x00623C10`, `0x00623AFC`; stat and state names from live
`itemstatcost.txt` / `states.txt`. Open question 3: scan of all.asm for
`+0xA8 + 8·i` accesses.

## Open questions

1. ~~A 0.2.0 recording (`record_tick.py`, `docs/HANDOFF.md` §5) must run
   U4 with `anim` records and U11 with `site` on every schedule, and
   reach the unobserved combinations (combat with skills, shrines,
   wells, a trade, a cooldown skill).~~ → PC 2 recording list.
2. Answered (2026-10-07): §4.7 (owner: this spec), read from the 1.14d
   asm with its tables; still to be checked against the logged +0x4C
   and bonus of `anim` records (Open question 1).
3. Answered (2026-10-07): it feeds pet movement. The only readers of
   the entries (scan of every `+0xA8 + 8·i` access) are the pet AI
   helpers `0x005E3930` and `0x005E3EA0` (`monsters/ai-bodies-6.md`,
   pet and hireling move); the timestamp +0xA4 is read only by the walk
   step's 25 ms gate (`0x00580C20`) and written there and by the
   teleport write `0x00554FD0` (`sim/path-placement.md` §10 rule 7).
   `d2-sim` uses now = frame · 40 ms: one walk step per frame at 25
   frames per second is always more than 25 ms after a write of an
   earlier frame, and a walk step in the frame of a teleport write is
   gated, as it is in real time. A recording that moves a player with a
   hireling (positions of the 20 entries per frame) confirms it.
4. ~~Object delays rolled from the object-control seed (events 0, 8):
   confirm the draw with an RNG + tick recording.~~ → PC 2 recording list.
5. Answered (2026-10-07): the only type-4 site of the 269
   (`0x00582595`, trap arm `0x00582510`) schedules on objects (unit +4
   of the operate record; `unit-events.tsv`, proof code); no 1.14d path
   schedules item event 4.
6. Partly answered (2026-10-07): 130 of the 208 `file` rows were read
   and are now `code`. Corrections: restore `0x00542B40` schedules types
   5 and 2 on objects only (saved type-2 records; §6.2's "restore" entry
   for monster type 2 was wrong, and object type 2 also comes from it;
   §6.2 and §6.4 corrected 2026-10-07);
   the quest event-7 sites schedule on objects except `0x0059584E`
   (the Countess, a monster); `0x0054D11F` on the hireling (monster);
   freeze `0x0057B216`, `0x0057B3E9` on monsters only; the wisp buff
   `0x005F4268` on a player. Site `0x00586800` is in `0x005867A0`.
   ~~The 78 rows left `file` (state timers, damage, missile hits, item
   use, most trade sites) need their callers traced; each names its
   owner.~~ → PC 2 recording list.
7. Answered (2026-10-07): both are written once, by game creation
   `0x00530BF0` from C→S message 0x67 (`tools/original-hooks.md` §5.2,
   caller `0x0053F17A`). Game +0x6A (u8) is the game type, message
   byte +0x11 (`0x00530CFF`); the client sets it from its own game type
   `[0x007A0610]` (`0x00477CA0`): 0 (single player) → 3, 6 → 1, 8 → 2,
   any other → 0; the single-player recording sends 3. Game +0x74
   (u32) is the ladder flag, bit 21 (0x200000) of the creation flags
   (message dword +0x27, `0x00530D4C`–`0x00530D59`); the client's
   default flags 0x100004 have it clear. So single player has +0x6A = 3
   and the neutral AI delay reads the difficulty's `aidel` column
   (`0x005A7446`–`0x005A745C`: +0x4F + difficulty +0x6D); only a game
   with type 0 and no ladder bit uses the Normal column on every
   difficulty. A Nightmare single-player recording (U10 delays) still
   confirms it.
8. Inactive storage: records and restore answered in §3.4 (monster
   GUIDs are kept; other units come back with new GUIDs; restore order
   monsters, items, others, each newest first). Answered 2026-10-07:
   node index (`+0xD0`) is the unit's target-node list (game +0x10F8,
   lists 0–7 per player, 8 and 9 special); 11 = in no list.
   Answered 2026-10-07 (the inserts, owner `monsters/ai.md` §5.2 with
   its slot table): a player gets the first empty slot 0–7 at join
   (`0x005B1880`); its attached units go right after the head
   (`0x005B1900`, `skills/bodies.md` §6.3); slots 8 and 9 are filled
   newest first by `0x005B1990`. Each insert requires node index 11
   and sets it to the slot. Answered (2026-10-08): the fields from
   `0x005B0D60`, `0x00573520`, `0x005A0140`, `0x00625D10` are the AI
   special state, level id, name seed and max life (§3.4 rule 1); a
   restored item does **not** keep its GUID (`0x00541990` →
   `0x00558CB0` allocates a new unit; §3.4 rule 4.2, with the expiry
   floor). ~~A recording leaving and re-entering a wilderness area
   confirms the order (`unit-order.md` OQ3).~~ → PC 2 recording list.
9. Answered (2026-10-08, `docs/handoff/e2e-night-flows.md` and
   `docs/handoff/impl-monster-death.md` Left 5): the bodies of the DT
   start `0x005A6FF0`, DT event 1 `0x005A72B0` and DD start `0x005A7390`
   and the mode-12 setter (`0x00553570`, never `0x005A7C20` itself) are
   §4.6 "Death and dead functions". Answered (2026-10-08): `0x0058F6C0`,
   `0x005B1A90`, `0x00639FB0`, `0x0061AFA0`, `0x005738D0` are §4.6 DT
   start step 3.1. ~~The R +0x14 byte (passed to `0x005A6520` and
   `0x006488A0`; its writer is the mode-request builder, 54 callers of
   `0x005A7C20`) and a recorded kill of a bonefetish1 (area damage at
   death) confirming branch 1.4.~~ → PC 2 recording list.
