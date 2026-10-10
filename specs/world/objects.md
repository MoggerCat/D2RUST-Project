# Spec: World — Objects (creation, init, operate; chests, shrines, doors, wells, portals)

- **Status:** draft: every rule below was read from the 1.14d `Game.exe`
  disassembly (addresses inline) and the live 1.14d tables
  (`patch_d2` `objects.txt` 573 rows, `shrines.txt` 23 rows); D2MOO 1.10f
  gave names only. No recording of an object interaction exists yet (open
  questions 1–3). Object population is `world/object-population.md`;
  §16–§18 cover the remaining generic operate and init functions and
  object events 0, 3, 8, 9, 10 (§15). Implemented 2026-10-06 (§1–§14),
  unverified (`d2_sim::world::objects`, `docs/handoff/impl-objects.md`).
  §4 rule 5 (no animation setup at allocation) is confirmed by recording
  `obj1` (open question 2).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::objects` (init and operate dispatch,
  per-function behavior, object events); message layouts in `d2-proto`
- **Related specs:** `sim/units.md` (§1 allocation, §4 mode set, §6.4
  object event types and their schedulers), `sim/tick.md` (§4 room pass,
  §5 timer queue, §6 client pass), `sim/unit-order.md` (§6 update queue),
  `sim/rng.md` (§3 `roll`, §5.2 object-control seed, §5.3 unit seed),
  `sim/path-placement.md` (§2.5 footprint at add, §3 object footprint
  mask), `sim/intents-events.md` + `server-messages.tsv` (S→C 0x0E, 0x2C,
  0x4D, 0x60), `items/treasure.md` §4 (chest drop `0x00585B90`, its `Q`
  argument), `world/waypoints.md` (init 17, operate 23, the C→S 0x13
  object case), `world/quests.md` (quest objects), `monsters/ai.md` (doors
  operated by monsters), `combat/hit.md` (timed-state helper, owned by the
  skills spec), `data/fields.tsv` (`objects`, `shrines`, `levels` offsets),
  `data/fixups.md` §13 (`FrameCnt` × 256), `world/object-population.md`
  (room population, `PopulateFn`), `sim/path-placement.md` §7, §10, §12.2
  (free point, placement, warp), `items/inventory.md` §5.5 (bank
  recount), `world/quests-act2.md`, `world/quests-act3.md` (quest
  inits), `world/objects-2.md` (part 2: §16–§18). Machine table:
  `world/object-functions.tsv`.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 65–76 |
| Inputs | 77–88 |
| Outputs / state changes | 89–95 |
| Rules | 96–97 |
|   1. Object data and unit fields | 98–128 |
|   2. Object control (game +0x10F0) | 129–151 |
|   3. Creation and init dispatch (`0x0054F5D0`) | 152–179 |
|   4. Object animation at a mode change | 180–211 |
|   5. Init functions | 212–286 |
|   6. Preset object classes 574–582 (`0x0054F490`) | 287–323 |
|   7. Operate dispatch | 324–503 |
|   8. Chests and breakables | 504–639 |
|   9. Shrines | 640–761 |
|   10. Doors, operate 8 (`0x00581D40`) | 762–785 |
|   11. Wells, operate 22 (`0x005858A0`) | 786–817 |
|   12. Portals, operate 15 (`0x00584870`) | 818–888 |
|   13. Torch, operate 11 (`0x005843D0`) | 889–893 |
|   14. Client messages | 894–921 |
|   15. Not covered yet | 922–936 |
|   16.–18. Moved | 937–943 |
| Constants & data dependencies | 944–990 |
| Randomness | 991–1037 |
| Edge cases & original bugs | 1038–1104 |
| Test vectors | 1105–1143 |
| Provenance | 1144–1210 |
| Open questions | 1211–1264 |
<!-- /index -->

## Summary

An object is a unit of type 2 (`sim/units.md` §1) whose behavior comes
from its `objects.txt` row through three function numbers: `InitFn`
(run once when the unit is allocated), `OperateFn` (run when a player,
a monster or a skill operates it) and `PopulateFn` (room population,
`world/object-population.md`). Shared state lives in the game's object
control (seed, per-level regions, shrine lists). This spec owns the dispatchers, the
per-object data, the object animation rule, and the generic object
classes: chests and other breakables with their traps, shrines, doors,
wells and portals. Quest objects are owned by the quest specs.

## Inputs

| Name | Type | Source |
|---|---|---|
| object unit, class id, mode, room, x, y | allocation arguments | `sim/units.md` §1 |
| operator (player, monster, or none) | unit | C→S 0x13 (`world/waypoints.md` §5.2), monster AI, skills |
| `objects.txt` row | record (448 bytes) | `data/fields.tsv` |
| `shrines.txt` row | record (184 bytes) | `data/fields.tsv` |
| `levels.txt` `MonLvl1` (+0x10, read as i16), `Act` | table cells | `data/fields.tsv` |
| game frame | u32, game +0xA8 | `sim/tick.md` §2 |
| host tick count | `GetTickCount` | doors, portals (host-only, §10, §12) |

## Outputs / state changes

Object data fields, unit mode, flags and seed; timers on the object
(`sim/units.md` §6.4); item drops (`items/treasure.md` §4); monsters and
missiles created; player stats, states and position; client messages
(§14).

## Rules

### 1. Object data and unit fields

The 0x38-byte object data (unit +0x14, zeroed at allocation,
`0x005553B9`); D2MOO `D2ObjectDataStrc` names:

| Offset | Field | Use |
|---|---|---|
| +0x00 | objects.txt record | set at allocation |
| +0x04 | `InteractType` (u8) | chests/urns: trap type (bits 0–6) + locked (bit 7); shrines: shrine id; wells: charges; portals: destination level |
| +0x08 | shrines.txt record | shrines (`0x00621BB0` writes, `0x00621B70` reads) |
| +0x0C | operator GUID + 1 (0 = none) | shrines (§9) |
| +0x05 | portal flags (u8) | portals: read `0x006222C0`, written `0x00622300`; portal creation ORs 0x3 (`0x0056D0F5`, `0x0056D28C`, event 11 `0x00581459`), operate 15 and 43 OR 0x5 (`0x00584AB6`, `0x00584D43`), `0x0058561D` ORs 0x1; sent in S→C 0x60 (§14) |
| +0x18, +0x1C | destination x, y (u32) | portals: partner lookup `0x00553720` (§12 rule 6) |

Unit fields used here (offsets: `sim/units.md` §2): +0x0C GUID (written
by the init dispatcher), +0x10 mode, +0x20 seed, +0x78 spark byte
(`0x005540A0` writes; 1 = sparkling chest, 2 = trap fire), +0x7C timer
argument (two 8-byte records allocated by `0x0054F520`, owner GUID −1),
+0xA4 hover text, +0xB8 drop item code (0 = none), +0xD4 last host tick
of a door or gate operation.

Unit flags +0xC4 used for objects: 0x1 changed (sends 0x0E, §14), 0x2
selectable, 0x4 attackable, 0x8 cleared at init, 0x80 "keep mode" (blocks
the PreOperate roll, set by §6), 0x100 hover freed, 0x400 sound queued.

objects.txt columns read by the rules below (offsets: `data/fields.tsv`):
`FrameCnt0`–`7`, `FrameDelta0`–`7`, `Start0`–`7`, `CycleAnim1`,
`HasCollision1`, `IsAttackable0`, `Selectable0`–`7`, `PreOperate`,
`Mode1`, `Mode2`, `SubClass`, `MonsterOK`, `Lockable`, `Sync`, `Parm0`–
`Parm3`, `Parm7`, `OperateFn`, `InitFn`.

### 2. Object control (game +0x10F0)

Built once per game by `0x00546C60` (game creation order: `rng.md` §5.2):

1. Allocate 0x1114 bytes; game +0x10F0 := it.
2. Seed (+0x00): `init()`, then one game-seed step, `init_low(lo')`
   (`lo'` is also the function's result).
3. +0x48 := 0 (region of level 0), +0x1110 := 0.
4. For each level id 1 … levels count − 1: a zeroed 0x90-byte region with
   +0x00 (u8) := `levels.Act`, +0x08 := 0x7FFFFFFF, +0x1C := −1; stored
   at +0x48 + 4·id.
5. Shrine lists: count the shrines.txt rows (row 0 included) per
   `effectclass` (+0xB2) for classes 0–7 (8 or more: ignored); +0x1048
   (0xC8 bytes) zeroed; for each class with count > 0 allocate a list;
   then fill, in row order: list[class] (+0x28 + 4·class) gets the row
   index, count (+0x08 + 4·class) increments.

1.14d lists (live shrines.txt): class 0: {0}; 1: {16, 17, 18, 19, 20, 21,
22}; 2: {2, 4}; 3: {3, 5}; 4: {1, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15}.

`0x00546FA0(game, room)` returns the control (the room is ignored);
"control seed" below is its +0x00 seed.

### 3. Creation and init dispatch (`0x0054F5D0`)

Called from unit allocation (`0x00555230`, `sim/units.md` §1) for type 2,
after the object data, the record pointer, the mode argument and the
static path are set, and **before** the unit is added to the world
(`0x00554850`, whose footprint stamp uses the final mode:
`sim/path-placement.md` §2.5). Arguments: game, object, GUID, room, x, y.

1. Unit +0x0C := GUID. Spark byte := 0. Flag 0x8 cleared, hover := 0.
2. Mode ≥ 8 → fatal. Remember this mode as M0.
3. `IsAttackable0` ≠ 0 → flag 0x4 set, else cleared.
4. `InteractType` := 0. Cancel all timers of the object (`0x00540F30`).
5. Class ≥ 573 → fatal. `InitFn` ≥ 80 → fatal.
6. If table `0x00731BC0`[`InitFn`] is non-null: call it with the init
   record {game, object, room, control, objects record, x, y}
   (`object-functions.tsv`, kind `init`).
   Null in 1.14d (`0x0054F6AE` test → skip): init 35, 36, 40, used by
   rows 338 guild vault, 339 trophy case, 340 message board; they get
   no init and continue at rule 7.
7. `Selectable[M0]` ≠ 0 → flag 0x2 set, else cleared. **M0**, the mode
   before the init function, decides (edge case 1).
8. If `PreOperate` ≠ 0 and flag 0x80 is clear: `roll(14)` on the control
   seed; result 0 → set mode 2 (`0x00624690`, `sim/units.md` §4).
9. Allocate the timer argument (`0x0054F520`) and store it at +0x7C.

1.14d live data: `PreOperate` = 1 on 60 rows (operate functions 1: 4,
3: 19, 5: 1, 7: 1, 8: 13, 11: 1, 14: 16, 46: 1, 51: 4).

### 4. Object animation at a mode change

`0x00624690(object, mode)` (`sim/units.md` §4) runs the animation setup
`0x00624390`, whose object branch (mode ≥ 8 fatal) sets:

1. frame count (unit +0x48) := `FrameCnt[mode]` (already × 256);
2. current frame (+0x44) := `Start[mode]` · 256;
3. speed (+0x4C, i16): d := `FrameDelta[mode]` (u16 read as i16). `Sync`
   ≠ 0 → speed := d, no draw. `Sync` = 0 → r := `roll(d >> 3)` on the
   **object's unit seed** (arithmetic shift; `roll(n < 1)` draws nothing),
   speed := r + d − (d >> 4) (16-bit arithmetic shift), then 0 if ≤ 0,
   then at most 0x7FFF.
4. Widths (`0x00624558`–`0x0062458C`): r (unsigned, below 4096) +
   d − (d >> 4) is a 32-bit sum of the sign-extended values; ≤ 0
   (signed) → 0; ≥ 0x7FFF → 0x7FFF; the low 16 bits are stored. The
   current frame is `Start[mode]` (u8) · 256; the frame count is the
   stored u32.
5. **Allocation runs no animation setup.** Unit allocation stores the
   mode argument directly (`0x005553E5`), allocates the zeroed 0x20-byte
   static path (`0x00623520`) and calls §3; neither touches the
   animation. The only §4 runs at creation come from mode sets inside
   the init function or §3 rule 8, and a mode set to the unit's current
   mode does no setup (`0x00624690` only queues the unit and sets flag
   0x1; `sim/units.md` §4). An object whose init leaves its allocation
   mode unchanged keeps speed 0, frame 0 and frame count 0 until its
   first real mode change. **Answered (confirmed by recording)** `obj1`
   (`docs/handoff/local-buddy-q9-rec.md`): 32 allocations, 26 with
   `Sync` = 0; class 37 (init 8 sets mode 2): one `roll(25)` at
   `0x00624563` on the object's seed, `FrameDelta` 200, r = 23, speed
   211; classes 35, 36, 39 allocated in mode 0 with no mode set: no
   setup, no draw.

### 5. Init functions

The full index (address, label, row count, owner) is
`object-functions.tsv`; this section owns the generic ones.

#### 5.1 Shrine, init 1 (`0x0054F9D0`)

1. If `Parm0` ≠ 0: class := 2 for `Parm0` = 1, 3 for `Parm0` = 2; any
   other value: `roll(10)` on the control seed, class := 1 if the result
   is 0, else 4. Then id := shrine pick (rule 3) for (class, level of
   the room).
2. If `Parm0` = 0: up to 8 tries: n := shrines count − 1; id :=
   `roll(n)` + 1 on the control seed (n < 1: no draw, id 1); stop when
   level id ≥ `LevelMin` (+0xB4, unsigned compare).
3. **Shrine pick** (`0x0054F770(class)`, game, level, room): class 0 or
   > 4 → 2. Empty list → fatal. Up to 8 tries: id := list[class][`roll`(
   count[class])] on the control seed; id 0 → 1; stop when level ≥
   `LevelMin`. After 8 failures the last id stays.
4. Remap: 4 → 2, 5 → 3, 16 → 18 (so exchange shrines and Enirhs never
   appear through init).
5. `InteractType` := id; shrine record := shrines row id.

Live data: 93 rows use init 1; 92 of them use operate 2.

#### 5.2 Urn, chest, sparkling chest (inits 2, 3, 57)

Level record = levels row of the room's level; t := `MonLvl1` (+0x10,
i16; the classic Normal value in every difficulty and game type).

- **Init 2 (urn, `0x0054FBB0`):** r := `roll(100)` on the control seed.
  If r < (t / 8, rounded toward 0) + 5 (as u16): `InteractType` := 1 +
  `roll(8)` (`0x004BC500(1, 9)`), else 0.
- **Init 3 (chest, `0x0054FCB0`):** run init 2 (same draws). Then if
  `Lockable` (+0x171) ≠ 0: r := `roll(100)`; r < (t / 2, toward 0) + 8
  (signed) → set bit 0x80, else clear it; not lockable → clear it (no
  draw). Last: `init_low(object seed, (lo' mod 65534) + 1)` with one more
  control-seed step.
- **Init 57 (`0x0054FD90`):** init 3, then spark byte := 1.

Blood Moor (`MonLvl1` 1): trapped 5 %, locked 8 % (lockable rows). Live:
`Lockable` = 1 on 60 rows, 46 of them with operate 4.

#### 5.3 Well, init 16 (`0x00552B30`)

`InteractType` := 2 · (u8)`Parm2` (charges). Live wells: `Parm2` 1 → 2
charges.

#### 5.4 Door, init 5 (`0x00550130`)

Returns at once. A door's state comes only from its allocation mode and
§3 rule 8 (13 door rows have `PreOperate`). Locked doors are mode 6;
no 1.14d path creates one (`world/objects-2.md` §22).

#### 5.5 Portals, inits 11 and 12

- **Init 11 (`0x00550140`, class 59 town portal):** `InteractType` := the
  town level of the room's act (`0x006427F0`, `0x0061AB70`; > 255 fatal).
  If the object is in mode 1: stamp its footprint (`0x00620A70`) and
  schedule ENDANIM (type 1) at frame + (`FrameCnt1` >> 8) + 1.
- **Init 12 (`0x0054FE70`, class 60 permanent portal):** if mode 0 (or
  no object): mode 1, stamp the footprint, ENDANIM at frame +
  (`FrameCnt1` >> 8) + 1. Then `InteractType` := destination by the
  room's level: 1 → 39, 39 → 1, 38 → 4, 4 → 38, 74 → 46, 46 → 74, 73 →
  the act II staff-tomb level (`0x0061AEB0`), the staff-tomb level → 73,
  121 → 109, 109 → 121, 125 → 111, 126 → 112, 127 → 117; 111, 112, 117:
  if not already 125, 126, 127 respectively, set it and schedule event 11
  at frame + 1 (event 11 creates the matching portal, `sim/units.md`
  §6.4). Other levels: unchanged.

The footprint stamp of inits 11 and 12 (`0x00620A70(O, room, x, y)`)
takes the room and point from the init record and the size and mask
from the object's class (`0x00620510`, `0x006205A0`, `0x006209D0`;
`sim/path-placement.md` §3); it does not read the static path, which is
still zeroed then (§4 rule 5).

### 6. Preset object classes 574–582 (`0x0054F490`)

DS1 presets may name class ids beyond `objects.txt` (`drlg/preset.md`
§5.3: 580, 581, 582 occur). The preset spawner (`0x00555823`) calls
`0x0054F490(game, room, class, x, y, mode)`: class ≤ 573 → none;
index := class − 574; index ≥ 9 (`0x00731D4C`) → none; else handler
`0x00731D28`[index]:

| Class | Handler | Result |
|---|---|---|
| 574–579 | `0x0054F0D0` | allocate the class/range row of `0x006E1080` (below) with the preset mode; then id := min + `roll(max − min)` on the **new object's unit seed**; 4 or 5 → 2; set `InteractType` and the shrine record (overrides init 1, whose draws already happened) |
| 580 | `0x0054F370` | special chest: c := 371 at level 25, else the class argument (580); run 581's handler with c (its class draw happens at every level); no object → done. Levels 62–64: `roll(100)` on the control seed, spark only if ≤ 50; other levels: spark (`0x005540A0`, 1); level 25 (spark branch): `InteractType` := 3. Then flag 0x80 and a mode-0 set (a no-op unless the init changed the mode) |
| 581 | `0x0054F180` | random chest class by act (below), allocated in **mode 0**: the preset mode is ignored |
| 582 | `0x0054F430` | class from `0x0059D830(game)` (quest spec), allocated in **mode 0** (no room → fatal) |

`0x006E1080` rows {class, min, max}: 574 {136, 2, 6}, 575 {136, 7, 7},
576 {136, 8, 11}, 577 {136, 12, 12}, 578 {136, 1, 5}, 579 {136, 14, 14}.
`max` is exclusive: 576 never gives 11 (resist poison), 578 never 5;
574 gives 2 or 3.

**Preset chest 581** (`0x0054F180(game, room, class, x, y, mode)`): A :=
act of the room's level (`0x006427F0`), L := the level; one control-seed
step, then:

- Act II (A = 1): L = 74 → {387, 389, 390, 391}[lo' & 3]; else {87,
  88}[lo' & 1].
- Act III (A = 2): L = 83 → {329, 330, 331, 332}[lo' & 3]; else {181,
  183}[lo' & 1].
- Acts I, IV, V: {5, 6, 139, 140, 141, 144, 176, 177, 198, 240, 241,
  242, 243}[lo' mod 13].

A class argument of 371 then replaces the pick (580 at level 25). No
room → fatal. Allocation `0x00555230(2, class, x, y, game, room, 1, 0,
0)` (stack: x, y, game, room, flag 1, mode, 0; presets 574–579 pass the
preset mode, 581 and 582 pass 0); the class's own init (chests: init 3)
runs inside it. Open question 6 answered.

### 7. Operate dispatch

#### 7.1 Entry with range check (`0x00584540`)

Arguments: game, operator, unit type, GUID. Callers: the C→S 0x13 object
case `0x00548B00` (player; range and walk rules in `world/waypoints.md`
§5.2) and monster AI `0x005B0FC1` (`monsters/ai.md`).

1. No such unit → return 0.
2. Operator is a monster and `MonsterOK` (+0x16D) = 0 → return 1.
3. Operator present and not in interact range (`0x00623660`) → return 1.
4. Else run §7.2 and return 1.

**Interact range** `R(U, O)` (`0x00623660`, rule 3; r3, 2026-10-09,
static asm). Positions are whole sub-tiles: for unit types 2, 4 and 5
the static path's +0x0C / +0x10, for players and monsters the dynamic
path's words +0x02 / +0x06 (no fraction); no path reads 0. Sizes come
from `0x00620510` (X) and `0x006205A0` (Y): object = `objects.txt`
`SizeX` / `SizeY` (record +0xD0 / +0xD4), player 2, item 1, monster and
missile from their own records.

1. O absent or not an object (type ≠ 2) → 0.
2. Unit distance `0x00641530(U, O)` (`sim/pathing.md` §9.5) = 0 → 1.
3. Box origin (bx, by) := (O.x − SizeX/2, O.y − SizeY/2), halves
   truncated.
4. SizeX < 1 or SizeY < 1: 1 iff bx − 1 ≤ U.x ≤ bx + 1 and
   by − 1 ≤ U.y ≤ by + 1; else 0.
5. U.x outside [bx − 2, bx + SizeX + 2] or U.y outside
   [by − 2, by + SizeY + 2] → 0.
6. U's X size > 2 (unsigned) → 1.
7. by − 1 ≤ U.y ≤ by + SizeY + 1 → 1.
8. The two outer rows (U.y = by − 2 or by + SizeY + 2) cut the corners:
   1 iff bx − 1 ≤ U.x ≤ bx + SizeX + 1.

So a player is in range anywhere in the object's footprint widened by 2
sub-tiles on each side, less the four corner cells' outer column. The
box is asymmetric about the object's position (x: −2 − SizeX/2 …
SizeX − SizeX/2 + 2). It fits run r3 of REC-94
(`facts/objects/objanim-a1-town.tsv`): waypoint 119 (5 × 5) at
(4899, 4209) has x band [4895, 4904], y band [4205, 4214]; the player
at (4895, 4212) (offset 4 / 3) is in a middle row → 1; at offset 5 x is
4894 → 0 (rule 5). Stash 267 (1 × 1) at (4866, 4229): x band
[4864, 4869]; the player at (4869, 4228) (offset 3 / 1) → 1, offset 4 →
0. Rule 2 gives 1 for the waypoint at once when both offsets are ≤ 3
(size term 2/2 + 5/2 = 3), and for the stash when `dist8_unit` has a
negative entry (offsets ≤ 2 / 0 and ≤ 1 / 1).

Callers: server C→S 0x13 object case `0x00548B78` (§7.3 rule 4; 0 →
walk to the object with `0x00548A50`, player mode 3 toward (2, GUID),
and operate on arrival; nothing is refused or sent), this entry
`0x00584540` (rule 3; 0 → return 1 with no operate: only reached from
§7.3 after its own test passed, or from monster AI and skills); client
`0x00461890` (object with a skill, `ui/controls.md` §6), `0x00461DC0`
(interact sender) and `0x00480C80` (pending action, `client/model.md`
§20): the client polls it each frame of its walk and sends 0x13 on the
first 1, so both server calls of that 0x13 see 1. d2rs reads it as
always 1 for a player (`LocalSeams::object_in_range`,
`crates/d2-client/src/app/single_player.rs`); a 0x13 from outside the box
operates at once where 1.14d walks the player there first. The player's interact info needs no staging before
the operate: §7.2 rule 2 refuses an active one, and the waypoint's
operate sets it itself (`waypoints.md` §5.2 step 3).

Skills call §7.2 directly (`0x005C9B13`, skills spec).

#### 7.2 Dispatch (`0x00584420`)

1. Result out := 1. Find the unit (`0x00552F60`).
2. Operator is a player: refuse (return 0, nothing else) when its
   interact info is active (`0x00554100`), player data +0x4C (busy) ≠ 0,
   or an item is on the cursor unless the object class is 267 (stash).
   Monsters and no operator skip these tests.
3. `OperateFn` ≥ 101 → fatal. Class 121, 122 or 22 → return 0. Table
   `0x00732D18`[`OperateFn`] null (0, 35–38, 60, 74–100) → return 0.
   Of the tabled slots (`0x005844CF` test): operate 35 steeg stone
   (row 337), 36 guild vault (338), 37 trophy case (339), 38 message
   board (340); 60 has no row. Operating those rows does nothing.
4. Call it with {game, object, operator, control, class id} and the
   `OperateFn` number; return its result.

The functions' return values reach no caller that acts on them (§7.1
returns 1); they are listed for fidelity only.

#### 7.3 C→S 0x13 object case (`0x00548B00`, unit type 2)

The handler's result (`sim/intents-events.md`), in order:

1. No object with the GUID (`0x00552F60`) → 1. No objects record →
   fatal.
2. Object mode ≥ 8 → 3.
3. Distance P → O (`0x00641530`) > 50 (signed) → 1.
4. Not in interact range (`0x00623660` = 0), or the line test
   `0x00622B50(P, O, 0x804)` ≠ 0 → walk to the object (`0x00548A50`,
   argument −2 when the message's u32 @9 is nonzero, else −1;
   `sim/pathing.md`) → 0. The walk is the NPC approach of `world/npc.md`
   §2 rule 3 with unit type 2 (answered 2026-10-09, PC 1 night D2;
   asm `0x00548A50`–`0x00548A6F`, recorded below):
   1. Run request `0x00580A70(P, no skill, mode 3, type 2, GUID, 0)`
      (`sim/pathing.md` §1.2: a run toward the object; it clears player
      data +0x150 / +0x154; without stamina it becomes a walk, §1.5).
      Its result is not read. Nothing else happens in this frame: no
      operate, no message to the client.
   2. Queued interaction (`0x00641F20` → `0x00460780`): player data
      +0x150 := 1, +0x154 := −1 (or −2 per the argument), +0x158 := 2,
      +0x15C := the object's GUID.
   3. The run ends by the arrival check (`sim/pathing.md` §9.5 rule 3)
      at the first step where the unit distance `0x00641530(P, O)` is
      ≤ the player path's stop distance 0. Sizes: player 2, object
      `objects.txt` `SizeX` (`0x00620510`; jump table `0x0062057C`:
      type 0 → 2, 1 → monstats2 `SizeX`, 2 → objects record +0xD0, 3 →
      missile `Size`, 4 → 1, 5 → 0). For Δ < 8 on both axes and sizes <
      4 the `dist8_unit` entry decides; its entries 0–2 (Δ = (0,0),
      (1,0), (2,0)) are −1, which returns distance 0 at once, so a 1×1
      object stops the player 2 sub-tiles away on its axis.
   4. On that stop (step result 2 in `0x00580C20`, `sim/pathing.md`
      §9.2 step 6): neutral start (`0x0057F020`), then this handler's
      `0x00548B00(P, type 2, GUID, flag = (+0x154 = −2), game)` again,
      then +0x150 := 0. With the player now in interact range and the
      line clear, rule 5 runs **in the stop frame**. Still out of range
      (or the line blocked) → rule 4 again: a new run and a new queued
      interaction. Distance > 50 or mode ≥ 8 by then → its code, no
      operate. The client sends nothing more.
   **Range test `0x00623660(P, O)`** (read in 1.14d 2026-10-10; REC-1930):
   O not an object → 0; unit distance (`0x00641530`) 0 → 1. Else, with P's
   position (px, py), O's position minus half its size (`SizeX`, `SizeY`
   = `0x00620510`, `0x006205A0`, integer halves; call it (ox, oy), w, h):
   - w < 1 or h < 1: in range iff \|px − ox\| ≤ 1 and \|py − oy\| ≤ 1.
   - else: out unless ox − 2 ≤ px ≤ ox + w + 2 and oy − 2 ≤ py ≤ oy + h + 2;
     a P of size > 2 is then in range; else the rows py < oy − 1 and
     py > oy + h + 1 additionally need ox − 1 ≤ px ≤ ox + w + 1 (the four
     outer corner cells are cut), the rows between are in range.
   The line test `0x00622B50(P, O, 0x804)` takes both units' sizes
   (`0x00620510`: object `SizeX`) and rooms; in the recorded stash walk it
   is clear. Recorded (gen-obj-267, `poke operate @2:267` = the town stash,
   GUID 17, from (4873, 4228)): the run starts at once (mode 3 toward
   (4866, 4229), frame 20), one sub-tile per two frames, and stops at
   (4868, 4229) in frame 30 (mode 5, unit distance 0); d2rs equals every
   frame. Unit distance, table branch: a **negative** `dist8_unit` entry
   returns 0 with no `+1` for a size below 2 (the stash, size 1, stops the
   run at Δ = (2, 0); `sim/pathing.md` §9.5).

5. Else stop P's path (`0x00648730`) and run §7.1 (`0x00584540(game,
   P, 2, GUID)`): its result 0 (object gone) → 3, else → 0.

Recorded (1.14d, ScnAma, `-seed 1234`, Act I town, C→S 0x13 {2, stash
GUID 17} injected for frame 4, `record_state.py`; stash 267 `bank`,
SizeX 1, at (4866, 4229)): player at (4873, 4228) in mode 5; frame 4
mode 3 (run), target (4866, 4229), (4872, 4228); one sub-tile every two
frames: f6 4871, f8 4870, f10 (4869, 4229), f12 (4868, 4229); frame 13
mode 5 at (4868, 4229) (Δ = (2, 0): unit distance 0) and the stash opens
in that frame (S→C 0x77 at frame 13, q-tool-interact-pokes Wine run of
`interact-operate-stash`, the same flow from `poke operate`).

#### 7.4 Call forms (2026-10-09, static asm; `tools/poke.md` §4 rule 10)

| Function | Registers | Stack from [ESP+4] | `ret` | EAX | Proof |
|---|---|---|---|---|---|
| C→S 0x13 handler `0x0054AA90` | ECX game, EDX player | message (id, u32 type @1, u32 GUID @5), size 9 | 8 | §7.3 code; 2 for type > 5 | `0x0054AA93`, `0x0054AAA6`–`0x0054AAAF`, `ret 8` at `0x0054AACB` |
| §7.3 `0x00548B00` | ECX player, EDX unit type (2) | GUID, walk flag (0x13: 0), game | 0xC | §7.3 code | `0x0054AABE`–`0x0054AAC4`; object case `0x00548B19` (table `0x00548E8C`); `ret 0xC` at `0x00548BB6`, `0x00548BD9` |
| §7.1 `0x00584540` | ECX game, EDX operator (0 = none: no range test) | unit type (low 16 bits read, `0x0058454B`), GUID | 8 | 0 no unit; 1 refused or done (§7.2's out word) | `0x00548B9A`–`0x00548BA2` call site; `0x00584549` / `0x00584551`; `ret 8` at `0x00584564`, `0x005845C0` |
| §7.2 `0x00584420` | ECX game, EDX operator | unit type (low 16 bits), GUID, pointer to a u32 out word (set to 1 first, `0x0058443A`) | 0xC | 0 refused or no operate function; else the operate function's own result | call site `0x0058459B`–`0x005845B2`; `0x0058442C` EBX := ECX, `0x00584431` EDI := EDX; `ret 0xC` at `0x005844AA`, `0x00584516` |

The operate function is called (`0x0058450E`) with ECX = a 0x14-byte
record on §7.2's stack {game, object, operator, control
(`0x00546FA0`), class} and EDX = the `OperateFn` number.

**Operating at once.** §7.2 `0x00584420` with the player as operator
is the deepest entry that keeps the player tests: it skips §7.3's
distance > 50, object mode ≥ 8 and range / line tests and the walk,
and §7.1's range test. Needs: the object exists (GUID in the object
list); the player's interact info inactive (`0x00554100` = 0), player
data +0x4C = 0 and no cursor item unless class 267 (§7.2 rule 2); the
out word writable. §7.3 also stops the player's path (`0x00648730`)
first; a direct call leaves it moving. §7.1 with operator 0 skips the
range test as well but hands the operate function no operator (the
waypoint's operate sets the operator's interact info,
`world/waypoints.md` §5.2 step 3), so it is no substitute.

So an operate or a walk gives 0; the operate function's own result
never reaches this value.

### 8. Chests and breakables

Common pieces:

- **Chest drop** `D(Q)` = `0x00585B90` with the operate record and
  quality `Q` (`items/treasure.md` §4); it returns the first item or
  none.
- **Magic test** (`0x0062A0F0`): the item exists, is type 4 and its
  quality is 4…9.
- **Code drop** `C(code)` = `0x00585970(game, object, code, 0)`
  (`world/objects-2.md` §20.7): one item of that 4-character code.
- **Open** = the end of the chest operate (§8.1 rule 7).
- **Trap arm** = §8.3 with `InteractType` & 0x7F.

#### 8.1 Chest, operate 4 (`0x00585F60`)

1. Object mode ≠ 0 → return 1 (nothing).
2. Locked (`InteractType` bit 0x80): unless the operator is class 6
   (assassin), the key test `0x0055F140` must pass: the first inventory
   item of type 41 (key) on page 0 (item data +0x45 = 0) loses one
   quantity (stat 70; the client gets the stat update) or, at quantity
   1, is removed (`0x0055E000`). No key → sound 22 on the operator,
   return 1 (chest stays closed). Pass (or assassin) → sound 11 on the
   operator, sent at once to its client (`0x00571740`), picks := 2.
   Unlocked: picks := 1.
3. Sparkling (spark byte bit 0): `roll(100)` on the control seed; Q :=
   6 if < 5, else 4. Otherwise Q := 0.
4. **Class 397** (special chest): r := `roll(10000)`:
   - r < 200: up to 2 tries `D(7)`; a null result goes to the tail; a
     magic result → open; two non-magic results → tail. r < 600: same
     with `D(5)`; r < 1200: `D(6)`.
     (Picks, sparkle and Q of rules 2–3 are ignored.)
   - r < 3200: up to 10 `D(4)`, counting items and magic items; stop at
     3 magic; any item dropped → open; none → tail.
   - r < 6200: up to 10 `D(4)`: magic → count m, non-magic → count n;
     stop at m = 2. If n = 0: one `D(0)`, n := 1 if it dropped. If n ≥ 7
     → open. Else (7 − n) × `C('gld ')`, open.
   - else: tail.
   - **Tail:** k := 0 (every path reaches it with no counted item); up
     to 10 `D(4)`: magic → stop,
     non-magic → k += 1; then (4 − k) × `D(0)` if k < 4; then 5 ×
     `C('gld ')`, 2 × `C('hp3 ')`, 2 × `C('mp3 ')`; open.
5. **Other chests:** r := `roll(100)` on the control seed. If r < 25 and
   not sparkling and not locked → no drop (the chest still opens).
   Else `picks` × `D(Q)` counting magic; if sparkling and no magic: up to
   10 more `D(Q)` until one is magic.
6. Rule 5 always ends in open.
7. **Open:** `Mode1` (+0x140) ≠ 0 → mode 1 and ENDANIM at frame +
   (`FrameCnt1` >> 8) + 1; else mode 2. Clear flag 0x2. If the drop item
   code (+0xB8) ≠ 0: drop it at the object (`0x00559A30(game, object,
   quality 2, &level, none, type −1, act flag 1)`, `world/objects-2.md`
   §20.4). Trap arm. Return 1. What clients receive and in which order:
   `world/objects-2.md` §28.
8. Every path returns 1 (`0x00586406`). The assassin exemption tests the
   operator's class id only (`[P +0x04] = 6`, no type test), so any
   operator unit of class 6 skips the key. A locked chest with no
   operator calls the key test with no unit, which is fatal
   (`0x0055F173`).
9. A `D(Q)` that returns no item counts as neither item nor magic in
   every loop of rules 4–5; in the three small class-397 bands a null
   result goes to the tail at once (no second try).

#### 8.2 Breakables

| Fn | Address | Rule (all: only in mode 0, else return 1) |
|---|---|---|
| 1 casket | `0x00586410` | `D(0)` first; **no item → nothing changes**. Item: mode 1, clear 0x2, ENDANIM f + fc1 + 1; r := `roll(10000)`, r ≥ 8192 → one trap monster (`0x005474C0`, spawn `0x00582280`, arg 8); `HasCollision1` = 0 → free the footprint (`0x00623830`); trap arm |
| 3 urn | `0x005866C0` | mode 1, clear 0x2, ENDANIM; `roll(100)` ≤ 20 → `D(0)`; footprint as casket; trap arm |
| 5 barrel | `0x005868A0` | player operator: its skill 1 (`0x006439B0`) is started on the barrel (`0x00580A70`; D2MOO: the player's attack mode toward the object); mode 1, clear 0x2 (when the object exists), free the footprint; `roll(10000)` ≥ 8192 → trap monster; `roll(100)` ≤ 20 → `D(0)`; ENDANIM. No trap arm |
| 7 exploding barrel | `0x00584330` | mode 1, clear 0x2; `0x00584240`: for each unit in the object's room (not itself, not tiles): players not in mode 17 and monsters not in mode 12 within range 3 take trap damage (`0x005DFA00`, combat spec); objects of class 11 in mode 0 within distance 2 explode now (recursively, before this barrel continues); then free the footprint, ENDANIM |
| 14 corpse / crate | `0x005867A0` | `D(0)` (always); mode 1; ENDANIM; clear 0x2; footprint as casket; trap arm |
| 68 evil urn | `0x00586520` | `D(0)` first, no item → nothing; mode 1, clear 0x2, ENDANIM; `roll(255)` ≤ `Parm7` → spawn the first monster of the level's region list that can walk (`0x005A43E0`); footprint as casket; trap arm |

"ENDANIM" = event 1 at frame + (`FrameCnt1` >> 8) + 1. All rolls on the
control seed. 61 rows use operate 14, 25 operate 3, 13 operate 1.

Return values (one exit each): casket, urn, corpse, evil urn → 1;
barrel (5) and exploding barrel (7) → 0.

**Exploding barrel ranges** (`0x00584240`): B := the barrel's position
(`0x00620870`); only the barrel's room unit list is walked (head +0x74,
next +0xE8). A player (not mode 17) or monster (not mode 12) is hit when
dx² + dy² ≤ 9 (`0x00641890(unit, B, 3)`: dx, dy = |B − unit position|,
static-path position for objects, items and tiles, path position
otherwise; inclusive). Another barrel (object class 11 in mode 0)
explodes when `0x006416D0(barrel, other)` ≤ 2 (signed, size-adjusted
metric of `missiles/missiles.md` §R9.5); the recursion reuses the
operate record with only the object replaced (same operator). Items,
missiles, tiles and the barrel itself are skipped.

#### 8.3 Traps

**Arm** (`0x00582510(t)`): t ≥ 10 (`0x00732D14`) or handler null (t =
0) → nothing. If the handler is the monster handler (t = 8, 9): m :=
trap monster id (`0x005474C0`); m = 234 and the object's level is in act
I → nothing. Otherwise schedule event 4 at frame + 35 and attach sound 13
to the object (no target).

**Event 4** (`0x005817A0`): t := `InteractType` & 0x7F, L := level of
the object; t ≥ 10 → nothing. Substitute handler 2 when t = 8 and L ≥
75, when t = 3 and L < 40 and L ≠ 25, and when t is 1 or 4 and L < 40.
Then run handler[t] (table `0x00732CEC`):

| t | Handler | Effect |
|---|---|---|
| 1 | `0x00582490` | trap monster 330 (trap-lightning) |
| 2, 6 | `0x005824B0` | 326 (trap-firebolt) |
| 3 | `0x005824D0` | 329 (trap-poisoncloud) |
| 4 | `0x005824F0` | 369 (trap-nova) |
| 5, 7 | `0x00582380` | object 162 at the object's position (spark byte 2); object 160 at x + 1 when inside the room (spark byte 2) |
| 8, 9 | `0x005822F0` | 1 + (`roll` & 1) on the control seed monsters of the trap monster id (each through `0x00582280`, arg 8) |

Trap monsters 1–4 go through `0x00582420`: try the object's spot
(`0x005B3090`, flag 0x88), else a free spot (mask 0x3F11), spawn
(`0x005B2F20`, monsters spec).

**Fire objects** (t = 5, 7; `0x00582380`) are allocated in **mode 1**
(`0x00555230(…, flag 1, mode 1, 0)`): object 162 at the trap's position,
then object 160 at (x + 1, y) only when x0 ≤ x + 1 < x0 + width of the
room rectangle (`0x00619730`; y is not tested). Both get spark byte 2; a
failed 160 allocation is fatal (`0x005540A0` asserts).

**Trap monster id** (`0x005474C0`, no draws; open question 3 answered):
L := the object's level, G := the control's region of L (§2). G +0x1C in
0 … monstats count − 1 → return it. Else G +0x1C := 234, and the level's
monster region (`0x00547BB0(game +0xF0, L)`, `monsters/population.md`
§2.2) is walked over its first `+0x10` entries in order; for each entry
class c, the first family range holding c caches its **base** and ends
the walk: first the act range (Act II, act index 1: [96, 101) → 96,
mummies; other acts: [5, 10) → 5, zombies), then [0, 4) → 0, [170, 174)
→ 170, [274, 278) → 274, [379, 383) → 379, [383, 387) → 383, [387, 391)
→ 387 (skeletons, archers, mages). No match → 234 (flying scimitar)
stays cached; trap arm skips 234 in act I (`0x00582250`). Trap 8/9 take
their control step (count 1 + (lo' & 1)) before this lookup; an id
outside 0 … count − 1 spawns nothing.

### 9. Shrines

#### 9.1 Operate 2 (`0x00583C70`)

1. Operator GUID field (+0x0C) ≠ 0 or object mode ≠ 0 → return 0.
2. +0x0C := operator GUID + 1 (no operator: 0). Queue for update,
   flag 0x1, mode 1.
3. Hover: free an existing hover; create one with the text of
   `"%d"`(3683 + shrine id) (`0x00661110`, lifetime 8 · length + 125
   frames; the server text is the decimal digits themselves: format
   `"%d"` at `0x006D57BC`, `0x00583CFA`–`0x00583D26`, read 2026-10-09;
   so length counts the digits); if created: store it, queue, flag
   0x100, event 6 at frame + 300.
4. Effect: code := shrine record `Code`; code < 24 (`0x00732EAC`) and
   table `0x006E1850`[code] non-null → call it; else run the refill
   effect (`0x005828E0`).
5. `reset time in minutes` (+0x10) = m ≠ 0 → event 5 at frame + 1200·m
   + 1.
6. If the mode is still 1, `CycleAnim1` = 0 and `Mode2` ≠ 0 → ENDANIM at
   frame + (`FrameCnt1` >> 8) + 1. Return 1.

**Event 5** (`0x005814D0`): only for an object with `SubClass` bit 0:
queue, flag 0x1, mode 0, +0x0C := 0 (the shrine can be used again).
**Event 6** (`0x00581620`): hover expired (expiry ≤ frame) → free it,
queue, flag 0x100; else reschedule at its expiry.
How clients show the hover text and the shrine's mouse-over name:
`world/objects-2.md` §26.4.

**No guards in 1.14d:** `0x00583C70` reads the shrine record
(`0x00621B70`) only at rule 4, after rules 1–3, and uses it unchecked;
every live operate-2 row has `InitFn` 1, which always sets it. With no
operator, rule 2 stores 0 (the shrine stays usable) and the effect
functions get the missing operator unchecked. Live shrine rows have
`MonsterOK` 0 and both the 0x13 path and skills pass a unit, so neither
case occurs with live data.

#### 9.2 Effects (table `0x006E1850`, 3 dwords: function, stat, state)

P = player (operator), S = shrine record. Stats: 6 life, 8 mana, 10
stamina (8.8 fixed). "State(st, stat, v, cb)" = the timed-state helper
`0x0056E970` (skills spec; via `0x00582800`) on P with source the
object, duration `Duration in frames` (+0x0C), that stat and value.
The request fields (skill 0, level 0), the helper's refusals and
replacement for shrines, and the remove callbacks of codes 12 and 14:
`world/objects-2.md` §26.

| Code | Fn | Effect |
|---|---|---|
| 1, 0, 23 | `0x005828E0` | life += max − life (`0x00625D10`), mana += max − mana (`0x00625D60`) (base stat adds) |
| 2 | `0x00582860` | life to max (1.14d: no doubling) |
| 3 | `0x005828A0` | mana to max |
| 4 | `0x00582940` | d := `Arg0` · (life >> 8) / 100; life −= d · 256; mana += `Arg1` · d · 256 / 100 (unsigned; base adds below) |
| 5 | `0x005829A0` | d := `Arg0` · mana / 100; mana −= d; life += `Arg1` · d / 100 (unsigned; base adds below) |
| 6, 8–11, 13, 15 | `0x00583B30` | State(table state, table stat, V(stat, `Arg0`)) |
| 7 | `0x005839B0` | v := V(19, `Arg0`); State(129, stat 25, `Arg1`), then stat 19 := v on that list |
| 12 | `0x00583BF0` | State(134, stat 11, 0, remove callback `0x00583BD0`); refresh P's skills (`0x0056DE40`); the +2 comes from state 134 (`skills/levels.md`) |
| 14 | `0x00583A70` | stamina := max (`0x00625DB0`); v := V(162, `Arg0`); State(136, 162, v, cb `0x00583A40`); on that list stamina := 2v, stat 28 := 1000 |
| 16 | `0x00582A00` | reverses P's name in place (player data +0x00: `_strrev`, then `strncpy` of 16 bytes; no message); unreachable through init |
| 17 | `0x00582A30` | free spot near P (`0x0064E7B0`, size 3, mask 0x1C09); none → nothing; town portal object 59 at spot + (5, 5) to P's act town (`0x0056D130`), created in **P's room** (not the room the spot was found in) |
| 18 | `0x00582C40` | gem upgrade (§9.3) |
| 19 | `0x00582DA0` | storm (§9.3) |
| 20 | `0x00583050` | nearest eligible monster to P becomes unique (`0x0065A800`, callback `0x00582750`; monsters spec) |
| 21, 22 | `0x005830E0`, `0x00583410` | potions + missiles (§9.3) |

Codes 1–5 read unit totals (`0x00625480`) and change life and mana
only by base-stat adds (`0x006272B0`, a 0 add writes nothing): code 4
adds −d · 256 to stat 6 and the gain to stat 8, code 5 −d to stat 8 and
the gain to stat 6; codes 1–3 add max − total.

Table stat/state pairs: 6 → 171 / 128, 8 → 39 / 131, 9 → 43 / 132, 10 →
41 / 130, 11 → 45 / 133, 13 → 27 / 135, 15 → 85 / 137. 1.14d values
(patch_d2 shrines.txt): armor 100, resists 75, mana recharge 400,
experience 50 (d2exp has 0); durations 2400–4800.

**V(stat, a)** (`0x00583840`, a = `Arg0`): stats 11, 27, 39, 41, 43, 45,
85, 171 → a; 19 → a · (P's to-hit rating of skill 0 with its weapon,
`0x00583740`) / 100; 21 → a · (stat 23 if two-handed wielding else 21)
/ 100; other → a · stat(P) / 100 (signed, toward 0). Stamina shrine:
stat 162 is "other" → a · stat162(P) / 100.

#### 9.3 Magic shrine details

- **Gem (18):** walk P's backpack gems (item type 20); the first with a
  better-gem code (≠ `non `) is replaced by one better gem dropped near
  P (`0x00582AC0`), and the walk stops. None: `roll(6)` on **P's unit
  seed** picks `gcw`, `gcr`, `gcg`, `gcb`, `gcy`, `gcv` (0–5), dropped
  near P.
- **Storm (19):** every player and monster (not dead) within `Arg1` of
  the shrine loses (life >> 8) · `Arg0` / 100 · 256 (signed); then 16
  missiles 62 owned by P from the shrine: for i, j in 1..4, target
  offset (±5i, ±5j) with + for odd, − for even; skill level := P level /
  5 clamped to 1..8.
- **Exploding (21) / poison (22):** n := `Arg0` + `roll(Arg1 − Arg0)` on
  the **shrine's unit seed** (5–9 with the live values); n potions `opm`
  / `gpm` (quantity 1) dropped near P; then 6 missiles 45 / 48 from the
  shrine to offsets (−6, 6), (−6, −6), (0, 6), (0, −6), (6, 6), (6, −6),
  owner P, flags 0x520, skill level as storm.
- **Storm, read in 1.14d** (`0x00582DA0`): targets come from the unit
  finder (`0x0065A950` / `0x0065AC70`, `missiles/bodies-2.md`) around
  the shrine's position with radius `Arg1` and filter `0x00582710`
  (players not in mode 0 or 17; monsters with `0x0063EA40` = 0). Each
  found player or monster not dead (`0x005541B0` = 0) gets a base-stat
  add (`0x006272B0`) on stat 6 of −trunc((life >> 8) · `Arg0` / 100) ·
  256 (signed division toward 0, life = unit total). Missiles: i outer,
  j inner (1 … 4); parameter record (`missiles/missiles.md` §R2.1):
  flags 3 (position given, target relative), owner P, class 62, start
  := the shrine's position, target offset (+5i if i odd else −5i, +5j if
  j odd else −5j), skill level := clamp(stat 12 of P / 5 (signed), 1,
  8); created by `0x0059FA30`.
- **Item drop near P** (`0x00582AC0`, gem 18 and the potions): the request
  names no source unit (its unit fields stay zero), item level from P's
  level, quality 2, spot from the floor search at P (`0x00555DA0`: start
  (x+2, y+3) when a room exists there). The item takes its seeds from the
  game seed (derive, then one step) and its own seed stays `{start, 666}`
  with no draw; quantity (stat 70) := 1. Verified: `gen-shrine-18`.
- **Potion drop** (21, 22; inline in `0x005830E0` / `0x00583410`): code
  index `0x00633680('opm ' / 'gpm ')` (−1 → fatal); per potion a free
  item spot next to P (`0x00555DA0`); none → that potion is skipped but
  counted; else a zeroed 0x84-byte item record is filled (code index,
  spot, P's item level `0x00558200`) and created by `0x00558D90` (items
  spec), quantity (stat 70) := 1 (`0x00627260`), queued with flag 0x1.

### 10. Doors, operate 8 (`0x00581D40`)

1. t := `GetTickCount`; t < unit +0xD4 + 500 → return 1 (host clock).
2. By mode (> 6 → return 1):
   - **0 closed:** free the footprint (`0x00623830`), mode 2, +0xD4 := t.
   - **6 locked:** key test (§8.1 rule 2) fails → sound 22 on the
     operator, return; passes → as mode 0.
   - **2 open, 5 blocked:** footprint cells free of mask 0x8180
     (`0x0064D800`, player 0x80, monster 0x100, 0x8000) → stamp the
     footprint (`0x00620A70`), mode 0, +0xD4 := t. Else any 0x8000 cell
     → mode 4, +0xD4 := t. Else mode 5 (+0xD4 := t), unless already 5
     (then nothing, no tick update).
   - 1, 3, 4: nothing.
3. Return 1.

Footprint mask of doors (`IsDoor` ≠ 0): `sim/path-placement.md` §3.
Live: 23 rows use operate 8, all `IsDoor` = 1.

The locked case runs the key test with **no** assassin exemption (unlike
§8.1). The debounce compares unsigned 32-bit values: refused while t <
(+0xD4 + 500) mod 2^32 (`jb` at `0x00581DA6`); the gate (§16.13) and the
portal hostile delay (§12 rule 2: refused while (hostile + 5000) mod
2^32 > t, `0x005848CF`) do the same.

### 11. Wells, operate 22 (`0x005858A0`)

1. c := charges (`InteractType`); c = 0 → return 0.
2. Heal (`0x00585720`, P = operator): life < max and `Parm3` & 2 →
   life := min(life + (max · `Parm1` >> 8), max) (unsigned), client
   update; mana the same with `Parm3` & 1; stamina the same (always);
   remove P's states 2 (poison) and 1 (freeze) stat lists; `0x00578C20`
   (curable states, `world/objects-2.md` §21);
   heal P's pets (callback `0x005856A0`). Used := any of these changed.
3. Not used → return 0. Used: c := c − 1; M := 2 · `Parm2`; if c ≤ M and
   c mod (M / 2) = 0 → mode := 2 − c / `Parm2`. Store c; event 2 at
   frame + `Parm0` + 1. Return 0.

**Event 2** (`0x00581510`, refill): c / `Parm2` ≥ 2 → fatal; c := c + 1
(more than M → fatal); same mode rule; queue, flag 0x1. Live wells:
`Parm0` 750, `Parm1` 128 (50 %), `Parm2` 1, `Parm3` 3: modes 0 → 1 → 2
as charges go 2 → 1 → 0 (refills: 1 → mode 1, 2 → mode 0); one charge
returns 751 frames after each use.

Heal details (`0x00585720`): life (6), mana (8) and stamina (10) change
only while the unit total is below the maximum (`0x00625D10`,
`0x00625D60`, `0x00625DB0`): new := min(total + ((max · `Parm1`) >> 8),
max), unsigned 32-bit, written by set-stat (`0x00627260`) and sent to
P's client (`0x00548520`). "Used" is set by each such write, by removing
a state-2 or state-1 stat list, by `0x00578C20(P)` ≠ 0, or by the pet
callback `0x005856A0` (through `0x00574DE0`, sharing the flag). The
charge helpers are M = 2 · `Parm2` (`0x00552AA0`) and the constant 2
(`0x00552AB0`), so the mode rule's divisor is `Parm2`; a row with
`Parm2` = 0 starts with 0 charges and never heals. The operate's mode
set is the normal `0x00624690`; event 2 sets the mode the same way and
then queues with flag 0x1 again.

### 12. Portals, operate 15 (`0x00584870`)

Classes 59 (town portal) and 60 (permanent portal). Rules read in 1.14d:

1. Player busy (`0x00535060`) → return 0. If `0x00535B10(game, player)`
   is nonzero (D2MOO `D2GAME_IteratePlayers`), only the portal's owner
   (timer argument GUID, `0x00552B10`) passes.
2. Hostile delay: `GetTickCount` < hostile time (`0x0055B6C0`) + 5000 →
   sound 19, return 0 (host clock; 5000 ms, not the 10000 of the
   waypoint 0x49 handler).
3. Party, quest-flag and owner checks; destination from `InteractType`
   (the level) or the linked portal; free spot (mask 0x1C09, player
   size); player placed with arrival offset + (5, 5); sound 8; S→C 0x0D;
   class 59 used by its owner removes both portals; mode-1 portals get
   ENDANIM; P gets state 102 (`just_portaled`) for 75 frames (event 12).
   Rules 4–14 give these steps in order (open question 7 answered).
4. Owner gate: o := timer-argument owner GUID (`0x00552B10`). If o ≠ −1
   and o ≠ P's GUID: P's party id (`0x00554630`, u16) = 0xFFFF → sound
   19 on P (target P), return 0. Else the owner player (`0x00552F60(game,
   0, o)`): found and party ids differ → the same refusal; not found →
   go on.
5. P must exist and be a player, else fatal (`0x0058494F`); 1.14d has
   no earlier null guard (D2MOO returns 0). Rows 59 and 60 have
   `MonsterOK` 0, so §7.1 stops monsters first.
6. Partner L := `0x00553720(game, O)`: in the act of `InteractType`, at
   the destination point (object data +0x18, +0x1C); no room there →
   stream one (`0x0061A140`) and, if found, `0x0052D0F0(game, room)`;
   then L := the unit of type O +0x94 and GUID O +0x98 (`0x00552F60`);
   L not an object → none.
7. Quest gate, only when O has a room: Q := P's quest record for the
   game difficulty (player data +0x10 + 4 · game +0x6D), none → refuse
   (sound 19 on P, return 0); D := leveldefs record of `InteractType`
   (`0x0061E470`, 0x9C bytes), none → refuse. Class 59 only: u := player
   data +0x48 (`0x005353F0`, D2MOO `dwUniqueId`); u ≠ O's GUID and L
   exists and u ≠ L's GUID (`0x00451F50`) → q := `QuestFlagEx` (+0x04)
   in an expansion game (game +0x70 ≠ 0), else `QuestFlag` (+0x00); q >
   0 and quest flag (q, bit 0) clear (`0x0065C310(Q, q, 0)`) → refuse.
   No L skips the class-59 test.
8. Destination: L exists → its position (`0x0045ADF0`, `0x0045AE20`)
   and room R. Else R := `0x0061B060(game +0xBC + 4 · act, InteractType,
   0, &x, &y, 3)`: tile index 0, free-point size 3
   (`sim/path-placement.md` §11; open question 16).
9. O's portal flags |= 5 (§1). P's room in a town (`0x0061AB00`) →
   `0x00543B90(game, level of P's room, level of R, P)` (quest
   change-level hook, `world/quests.md`).
10. Free point from R: `0x0064E7B0(R, &point, P's size (0x00620510),
    0x1C09, 0)` (`sim/path-placement.md` §7); none → return 0 (rule 9's
    changes stay).
11. Place P: `0x00554EA0(game, P, room, x, y, 0, 0)`
    (`sim/path-placement.md` §10), failure → fatal. Sound 8 on P (no
    target). Player mode set `0x005809D0(game, P, 0, 2, x + 5, y + 5,
    0)` (player-mode spec). S→C 0x0D to P's client (`0x0053B4B0`): P's
    type, GUID, 1, x + 5, y + 5, 0, 0.
12. L exists and O's class ≠ 60 and u = L's GUID → remove both portals:
    O (`0x0061A270(room, 2, GUID)`, free `0x00555600`, `0x0061AED0(room,
    1)`), then `0x0058CF50(game, L)` (Act V quest hook), then L the same
    way. L exists, class 59 and u ≠ L's GUID → nothing. Else (no L, or
    class 60): O in mode 1 → ENDANIM at f + (`FrameCnt1` >> 8) + 1.
13. State 102: stat list `0x006251F0(pool, 2, f + 75, 0, P's type)`;
    created → event 12 on P at f + 75, state 102 on (`0x00639DB0`), list
    state 102 (`0x006252D0`), remove callback `0x0056E900`
    (`0x00625CE0`), attach (`0x00626E10`). Every path returns 0.
    The callback is the default one (`skills/bodies.md` §2.8), event 12
    the expiry walk (`sim/stat-lists.md` §10.4); list owner and repeat
    uses: `world/objects-2.md` §27.3 rule 4. The town portal pair's
    creation, owner and every closer: `world/objects-2.md` §27.
14. Rule 1's `0x00535B10(game, P)` is 1 when, for some living player u
    (`0x005538D0` walk, callback `0x00535AF0`), P's player-list entry
    toward u has flag 8 (hostile; `0x0055B300(P, u, 8)`); then only the
    owner may use the portal, others get 0 with no sound.

### 13. Torch, operate 11 (`0x005843D0`)

Mode 0 (or no object) → mode 1 and clear flag 0x2; modes 1, 2 → mode 0;
others nothing. Return 1.

### 14. Client messages

Object update pass (`0x00581AD0`, per queued object and client;
`sim/unit-order.md` §6):

1. Flag 0x1 → S→C 0x0E (`0x00581A20` → builder `0x0053B470`, 12 bytes):
   type 2 @1 (u8), GUID @2 (u32), 3 @6 (u8), flag 0x2 as 0/1 @7 (u8),
   mode @8 (u32). Then by `SubClass`: bit 2 → S→C 0x60 (`0x0053D900`,
   portal); else mode 1 and bit 0 (shrine) and operator field ≠ 0 →
   S→C 0x4D (`0x0053D4D0`, 17 bytes): 2 @1, object GUID @2 (u32),
   operator GUID @6 (u32), shrine `Code` @10 (u8), 0 @11, @13, @15
   (u16 each).
2. Flag 0x400 → sound (`0x00571740`); flag 0x100 → hover message
   (`0x00571620`); flags 2 bit 0 → `0x00597890`; then `0x00571CD0`.

Sound ids used here: 8 (portal), 11 (unlock), 13 (trap armed), 19
(portal refused), 22 (locked).

Builder details: `0x0053D4D0` (17 bytes) writes @0 0x4D, or 0x9A when
its last argument is nonzero (the shrine passes 0), @1 the unit type,
@2 u32, @6 u32, @10 u8, @11, @13, @15 u16. The shrine call passes the
object GUID, the operator field − 1 (§9.1 stores GUID + 1, so this is
the operator's GUID), byte 0 of the shrine record (`Code`) and zeros.
It is sent only when `SubClass` ≠ 0, bit 2 clear, the mode (u32) = 1,
bit 0 set and the operator field ≠ 0. S→C 0x60 (`0x0053D900`, 7 bytes):
@0 0x60, @1 portal flags (object data +0x05, `0x006222C0`), @2
`InteractType` (destination level, u8), @3 object GUID (u32).

### 15. Not covered yet

All items of this list are done:

- object population (`0x00552610`, `PopulateFn` 1–9, objgroup density,
  shrine and well limits in the per-level regions): done,
  `world/object-population.md`;
- the `todo` rows of `object-functions.tsv` (trap door, obelisk, secret
  door, armor stand, weapon rack, bookshelf, teleport pad, slime door,
  exploding chest, bank, stairs, jungle stash, gate, torch tiki): done,
  §16; small init functions: done, §17;
- object events 0, 3, 8, 9, 10: done, §18.
- the client object functions (`ClientFn` 1–18, table `0x007277F0`):
  done, `world/objects-client.md` §25–§28.

### 16.–18. Moved

§16 (operate functions, part 2), §17 (small init functions) and §18
(object events) are in `world/objects-2.md`, numbers kept; §19 and later
continue there too. A reference to §16–§18 in this file means that file.
§25–§28 (client object functions) are in `world/objects-client.md`.

## Constants & data dependencies

| Constant | Value | Where |
|---|---|---|
| object control size / region size | 0x1114 / 0x90 | `0x00546C60` |
| init / operate / populate / preset tables | `0x00731BC0` (80), `0x00732D18` (bound 101, 74 used), `0x00731D00` (10), `0x00731D28` (9) | §3, §7, §6 |
| PreOperate chance | `roll(14)` = 0 | `0x0054F72F` |
| class / InitFn / mode bounds | 573 / 80 / 8 | `0x0054F5D0` |
| refused classes | 22, 121, 122 | `0x00584420` |
| stash class (cursor allowed) | 267 | `0x00584420` |
| trap / lock thresholds | `MonLvl1`/8 + 5, `MonLvl1`/2 + 8 (%) | §5.2 |
| chest seed | `lo' mod 65534 + 1` | `0x0054FD4C` |
| empty chest | `roll(100)` < 25 | `0x00586319` |
| class-397 thresholds | 200, 600, 1200, 3200, 6200 of 10000 | `0x005860C1`–`0x0058625D` |
| trap monster chance | `roll(10000)` & 0xFFFFE000 ≠ 0 | casket, barrel |
| trap delay / sound | 35 frames / 13 | `0x00582510` |
| shrine hover | 3683 + id, event 6 at +300 | `0x00583CFA` |
| shrine reset | 1200 frames per minute, + 1 | `0x00583DF3` |
| shrine effect table | 24 entries × 12 bytes | `0x006E1850`, `0x00732EAC` |
| door debounce | 500 ms host ticks | `0x00581D9F` |
| door masks | 0x8180, 0x8000 | `0x00581DF8`, `0x00581E2E` |
| portal hostile delay | 5000 ms | `0x005848C1` |
| well refill | `Parm0` + 1 frames | `0x00585941` |
| obelisk | gem type 20, sound 19, mode 3, S→C 0x58 | `0x00582610` |
| teleport pad | free point size 3, mask 0x1C09 | `0x00581B50` |
| bank | class 267, S→C 0x77 action 0x10 | `0x00564CD0` |
| bookshelf | lo' mod 20 < 13 scroll, else book | `0x00584060` |
| goo pile trap | lo' mod 1000 ≤ 332 → trap 3 | `0x0054FC50` |
| fire tick, burn range | f + 15 + lo' mod 35; `Parm0` + 1 | `0x00581700`, `0x00581680` |
| spike trap delays | 25, 15, fc1 + 1 frames | `0x005818B0` |
| fissure | f + 25 + lo' mod 250 | `0x0054FDB0`, `0x00581250` |
| period-of-day objects | 1000 / 600 frames | `0x00581250` |
| trapped soul | 90 of 100; 10, 20, 25, 35 frames | `0x005869F0`, `0x00585CE0`, `0x005501F0` |
| gate debounce | 500 ms host ticks | `0x00582080` |

Tables read: `objects` (§1 columns), `shrines` (`Code`, `Arg0`, `Arg1`,
`Duration in frames`, `reset time in minutes`, `effectclass`,
`LevelMin`), `levels` (`MonLvl1`, `Act`), `itemtypes` (key 41, gem 20).

`object-functions.tsv` (tab-separated, header row): `kind` (`init`,
`operate`, `populate`, `preset`), `index` (function number, or class id
for `preset`), `address` (1.14d table entry; 0 = null entry), `label`
(D2MOO-derived name, informational), `rows` (live `objects.txt` rows
using it; `-` for presets), `owner` (§ of this spec, another spec, or
`todo`). A game-file test reads the three `Game.exe` tables and the live
`objects.txt` and compares addresses and row counts with it.

## Randomness

Seeds: **C** = object-control seed (§2), **U** = the object's unit seed,
**P** = the player's unit seed. In order:

1. Creation (§3): the init function's draws (below), then PreOperate
   `roll(14)` on C (only for `PreOperate` rows without flag 0x80). Mode
   changes inside init or PreOperate add the §4 draw on U when `Sync` =
   0.
2. Init 1: [`roll(10)` C if `Parm0` ∉ {0, 1, 2}] then 1–8 × `roll(n)` C.
   Init 2: `roll(100)` C [+ `roll(8)` C]. Init 3: init 2, [`roll(100)` C
   if lockable], one step C → `init_low(U)`.
3. Presets 574–579: `roll(max − min)` on U after the object's init.
   Preset 580: `roll(100)` C for levels 62–64.
4. Chest operate: [sparkle `roll(100)` C]; class 397: `roll(10000)` C,
   else `roll(100)` C; then the chest-drop draws (`items/treasure.md`
   §4, on the dropping unit's seed) in drop order; the open mode change
   (§4, U); then the drop-code item's creation draws
   (`world/objects-2.md` Randomness 5).
5. Casket: drop draws, mode change, `roll(10000)` C. Urn: mode change,
   `roll(100)` C, drop draws. Barrel: mode change, `roll(10000)` C,
   `roll(100)` C, drops. Evil urn: drops, mode change, `roll(255)` C.
6. Shrine operate: mode change (U), then the effect: gem `roll(6)` P;
   exploding/poison `roll` on U; the warping and storm effects draw in
   code owned by other specs.
7. Trap event 8/9: one step C (`& 1`).
8. Door, well, torch: only the §4 draw of their mode changes.
9. Bookshelf: mode change (U), C step (mod 20), C step (& 1), the drop's
   draws. Armor stand, weapon rack: the drop's draws, then the mode
   change (U).
10. Trapped soul operate 48: C step (mod 100), then the drop's draws or
    the mode change (U).
11. Inits of §17: goo pile one C step; hell brazier one C step [+ mode
    change]; fissure one C step; gold placeholder: mode change, one C
    step, then two C steps per point and the gold drop's draws; torch,
    braziers, fire, init 26: only mode-change draws.
12. Events: 0 one C step per tick, after the burn; 8 (fissure) one C
    step, then the mode change; 3 none; 9 and 10 only the drop's draws.
13. Teleport pad, trap door, stairs: the free-point and warp code of
    `sim/path-placement.md` (no draws found there).
14. Preset 581: one C step before the allocation (so before the chest's
    init draws); preset 580: that step, the init's draws, then [`roll(100)`
    C for levels 62–64]. Trap monster id (§8.3): no draws.
15. Shrine storm (19): no draws of its own; the missiles' creation draws
    (`missiles/missiles.md`). Potions (21, 22): the item creation draws,
    per potion, after the count `roll` on U.

## Edge cases & original bugs

Reproduced by default.

1. **Selectable from the old mode:** §3 rule 7 uses the mode from before
   the init function and PreOperate. An object opened by init or by the
   1-in-14 PreOperate roll keeps the selectable flag of mode 0.
2. **Chest/urn use classic Normal monster level:** `MonLvl1` is read in
   every difficulty and in expansion games.
3. **Casket and evil urn stay closed** when the chest drop returns no
   item; the player can operate again and every try draws again.
4. **Unreachable shrines:** init remaps 4 → 2, 5 → 3, 16 → 18, so
   exchange shrines and Enirhs never appear from init; preset classes
   576 and 578 cannot pick their `max` id (exclusive bound).
5. **Health/mana shrines (2, 3) only fill** to maximum; the table text
   "doubles" is not 1.14d behavior.
6. **Stamina shrine value:** V(162, 200) = 200 · stat162 / 100, which is
   0 for a player without stamina bonus, so stamina = 0 and stat 162 = 0
   on the state list; only the 1000 recovery bonus and the refill act.
7. **Class 397 ignores lock and sparkle** for its drops; a locked one
   still consumes a key.
8. **Door re-close blocked:** a corpse (0x8000) in the doorway sets mode
   4; a player or monster sets mode 5 and repeated operates do nothing
   until the doorway is clear.
9. **Host clock:** doors (500 ms) and portals (5000 ms) read
   `GetTickCount`; d2rs must take this as an input (determinism rule),
   never from `d2-sim` itself.
10. **Well returns 0** even when it heals (no caller uses it).
11. **Exploding barrel chain** recurses inside the room walk, so chained
    barrels finish (footprint freed, ENDANIM scheduled) before the first.
12. **Trap 9 never comes from init** (1 + `roll(8)` gives 1–8); it exists
    only for data that sets it.
13. **Fire event writes the mode field directly** (§18.1): no animation
    reset, no draw, no changed flag, so clients get no mode update.
14. **Obelisk S→C 0x58 byte 6** is never written: the 7th byte is
    whatever is on the stack.
15. **Teleport pad** ignores its own mode and takes the first partner
    found (own room first, then adjacency order).
16. **Trap door** in mode 2 with no warp tile in its own room is a fatal
    error; stairs (47) search the adjacent rooms too.
17. **Stairs 50 only warp**: from mode 0 it returns 0 and never opens.
18. **Gold placeholder room test drifts** (§17): the room lookup uses the
    last accepted point plus the new offset, the drop uses the init point
    plus the offset.
19. **Gate closes over units**: operate 61 restamps its footprint in mode
    2 without the occupancy test doors use.
20. **Fissure stops itself**: event 8 on class 399 in a mode other than 0
    or 2 does not reschedule.
21. **Trapped soul with no drop goes inert**: mode 2, counter ≥ 2, no
    event pending.
22. **Presets 580–582 ignore the preset mode** (mode 0); 580 at level
    25 still takes 581's class step before 371 replaces the pick.
23. **Key bypass by class id**: any operator of class 6 (also a monster
    row 6) opens a locked chest without a key (§8.1 rule 8).
24. **Barrels return 0**, every other breakable 1 (no caller reads it).
25. **Trap monster is the family base** (§8.3): a level whose first
    matching entry is zombie3 spawns zombie1 (5); cached per level for
    the game.
26. **Enirhs (16) reverses the player's name** in the server's player
    data; unreachable through init and presets.
27. **Shrine 17 portal room**: created in P's room even when the free
    spot lies in a neighbouring room.
28. **No null-operator guard** in the portal (fatal assert) or the chest
    lock (fatal in the key test).
29. **Portals close only for the linked owner**: removal needs player
    data +0x48 = the linked portal's GUID; class 60 never removes.

## Test vectors

Synthetic (CI), RNG per `rng.md` §3:

| Input | Expected | Source |
|---|---|---|
| chest init 3, lockable, `MonLvl1` 1, C = {1, 666} | draws 51 (not trapped), 31 (not locked); `InteractType` 0; U := `init_low(55249)`; C after = {671516612, 330169957} | §5.2 |
| same, C = {410, 666} | 0 (trapped), trap 8, 0 (locked): `InteractType` 0x88; U := `init_low(48426)` | §5.2 |
| shrine init 1, `Parm0` 3, level 2, C = {1, 666} | `roll(10)` 1 → class 4; picks 4 (id 9, `LevelMin` 26 > 2), 8 (id 13): id 13 | §5.1 |
| shrine init 1, `Parm0` 0, level 2, C = {1, 666} | `roll(22)` = 21 → id 22 | §5.1 |
| shrine init 1, `Parm0` 3, level 2, C = {4, 666} | `roll(10)` 0 → class 1; picks 2 (id 18, `LevelMin` 4), 3 (id 19): id 19 | §5.1 |
| PreOperate, C = {1, 666} | `roll(14)` = 9: mode unchanged | §3 |
| anim, `FrameDelta` 256, `Sync` 0, U = {12345, 666} | `roll(32)` = 23; speed 263 | §4 |
| preset 576 | id ∈ {8, 9, 10} | §6 |
| well, `Parm2` 1: charges 2, use, use, refill, refill | modes 1, 2, 1, 0 | §11 |
| door mode 2, doorway has a player (0x80) only | mode 5, tick stored | §10 |
| bookshelf, C = {1, 666} | 1791398751 mod 20 = 11 < 13; 791599131 & 1 = 1 → `isc ` | §16.6 |
| goo pile, C = {10, 666} | lo' 734112332 mod 1000 = 332 → `InteractType` 3 | §17 |
| goo pile, C = {1, 666} | 751 → `InteractType` 0 | §17 |
| fissure init, C = {1, 666} | event 8 at f + 26 | §17 |
| hell brazier init, C = {1, 666} | lo' & 1 = 1 → mode 1 | §17 |
| gold placeholder, C = {1, 666} | n = 6 + 1 = 7; first offset (3, 0) | §17 |
| fire event 0, C = {1, 666} | event 0 at f + 15 + 16 = f + 31 | §18.1 |
| trapped soul operate, C = {1, 666} | 51 < 90 → `D(0)` | §18.4 |
| gate, +0xD4 = 1000, tick 1400 | nothing (1400 < 1500) | §16.13 |
| spike trap s = 0, one player at distance 0 | sound 13, s := 1, event 3 at f + 25 | §18.2 |
| preset 581, Act I level, C = {1, 666} | lo' 1791398751 mod 13 = 12 → class 243 | §6 |
| preset 581, level 74, C = {1, 666} | lo' & 3 = 3 → class 391 | §6 |
| preset 581, Act II not 74 / Act III not 83, C = {1, 666} | lo' & 1 = 1 → 88 / 183 | §6 |
| preset 580, level 62, C = {1, 666} | class 88; next step 791599131 mod 100 = 31 ≤ 50 → spark | §6 |
| animation, `FrameDelta` 200, `Sync` 0, mode change | `roll(25)`; r = 23 → speed 211 (recording `obj1`, class 37) | §4 |
| object allocated in mode 0, init sets no mode | no draw; speed 0, frame count 0 | §4 r5 |
| storm, P level 23 / 3 / 50 | missile skill level 4 / 1 / 8 | §9.3 |
| exploding barrel at (10, 10), player at (12, 12) / (13, 11) | hit (8 ≤ 9) / not hit (10 > 9) | §8.2 |

Game-file (`#[ignore]`, `D2_GAME_DIR`): `object-functions.tsv` equals the
three tables of the 1.14d `Game.exe` and the live row counts; the shrine
lists of §2 from the live `shrines.txt`.

## Provenance

- §7.4 call forms (2026-10-09): `0x0054AA90`, `0x00548B00`,
  `0x00584540`, `0x00584420` in `all.asm`, at the instruction addresses
  cited in the table.
- 1.14d `Game.exe`, read with `tools/ghidra/disasm.py` and the Ghidra
  export: init dispatch `0x0054F5D0`, control `0x00546C60`/`0x00546FA0`/
  `0x00546FB0`, inits `0x0054F9D0`, `0x0054F770`, `0x0054FBB0`,
  `0x0054FCB0`, `0x0054FD90`, `0x00552B30`, `0x00550130`, `0x00550140`,
  `0x0054FE70`; presets `0x0054F490`, `0x0054F0D0`, `0x0054F370`,
  `0x0054F430`; anim `0x00624390`; operate `0x00584540`, `0x00584420`;
  chest `0x00585F60`, key `0x0055F140`; breakables `0x00586410`,
  `0x005866C0`, `0x005868A0`, `0x00584330`/`0x00584240`, `0x005867A0`,
  `0x00586520`; traps `0x00582510`, `0x005817A0`, `0x00582420`,
  `0x00582380`, `0x005822F0`, `0x00582250`, `0x00582280`; shrine
  `0x00583C70` and the effect functions of §9.2, `0x00583840`,
  `0x00582800`, hover `0x00661110`; events `0x005814D0`, `0x00581620`,
  `0x00581510`; door `0x00581D40`; well `0x005858A0`, `0x00585720`;
  portal `0x00584870`; torch `0x005843D0`; messages `0x00581AD0`,
  `0x00581A20`, `0x0053B470`, `0x0053D4D0`. Tables dumped from the image:
  `0x00731BC0`, `0x00732D18`, `0x00731D00`, `0x00731D28`, `0x006E1080`,
  `0x006E1850`, `0x00732CEC`.
- Live tables: `game/extracted/patch_d2/data/global/excel/objects.txt`,
  `shrines.txt`, `levels.txt`, `monstats.txt`, `itemstatcost.txt`,
  `states.txt` (names of the ids quoted).
- D2MOO 1.10f (`Objects.cpp`, `ObjMode.cpp`, `ObjRgn.cpp`) for names and
  structure only. Differences found in 1.14d: Selectable uses the mode
  before init (D2MOO: after); the control builds every level region
  (D2MOO stores them all in slot 0); operate dispatch returns 0 for a
  null table entry instead of asserting and checks `OperateFn` < 101;
  the portal hostile delay is 5000 ms in both; the well heal moved to
  `0x00585720` and the well returns 0; the shrine table entries match
  D2MOO's.
- §16–§18: operate `0x005843A0`, `0x00581EB0`, `0x00582610`
  (`0x005825B0`, `0x00554D00`, `0x00554120`, `0x00554190`,
  `0x0053D8D0`), `0x00583FF0`, `0x00584160`, `0x005841D0`, `0x00584060`,
  `0x00581BF0`/`0x00581B50`, `0x005821A0`, `0x00581CD0`, `0x00564CD0`,
  `0x00581F60`, `0x00582180`, `0x00583F10`, `0x00582080`, `0x005869F0`;
  inits `0x005500C0`, `0x0054F860`, `0x00594020`, `0x005500D0`,
  `0x0054FB40`, `0x0054FB90`, `0x005500E0`, `0x0054FC50`, `0x0054F8C0`,
  `0x005500F0`, `0x0054FDB0`, `0x005501F0`; events `0x00581700`,
  `0x00581680`, `0x005818B0`, `0x00581250`, `0x00585CE0`, `0x00586850`.
  D2MOO `ObjMode.cpp` gave the names; 1.14d differences: the obelisk
  sends 0x58 with an unwritten 7th byte; the exploding chest, torch tiki
  and teleport pad return 0; the trap door asserts on a missing tile.
- Answers of the implementation questions (2026-10-07): `0x00624558`–
  `0x0062458C`, `0x005553E5`, `0x00623520`, `0x00624690` (§4);
  `0x00620A70` (§5.5); `0x0054F180`, `0x0054F370`, `0x0054F430`,
  `0x0054F0D0` (§6); `0x00548B00` (§7.3); `0x00585F60`, `0x0055F140`
  (§8.1); `0x00584240`, `0x00641890` (§8.2); `0x00582380`, `0x005474C0`,
  `0x00582250`, `0x005822F0` (§8.3); `0x00583C70`, `0x00621B70`,
  `0x00582940`, `0x005829A0`, `0x00582A00`, `0x00582A30`, `0x00582DA0`,
  `0x00582710`, `0x005830E0`, `0x00583410` (§9); `0x00581D40` (§10);
  `0x005858A0`, `0x00585720`, `0x00552AA0`, `0x00552AB0` (§11);
  `0x00584870`, `0x00553720`, `0x00535B10`, `0x005353F0`, `0x0061E470`
  (§12); `0x00581A20`, `0x0053D4D0`, `0x0053D900` (§14); `0x00581490`
  (§18.6). D2MOO `OBJECTS_OperateFunction15_Portal` matched the portal
  order; 1.14d differs: no null-operator return. Recording `obj1`
  (`docs/handoff/local-buddy-q9-rec.md`, `record_objects.py` 0.1.0)
  confirmed §4 rule 5.
- §7.1 r3 interact range (2026-10-09, pc1-day4, static asm):
  `0x00623660`, `0x00641530`, `0x00620510`, `0x006205A0`,
  `0x006488C0`, `0x00648900`, call sites `0x00548B78`, `0x00584597`,
  `0x00461890`, `0x00461DC0`, `0x00480C80` (all `call 0x623660` in
  `all.asm`); `objects.txt` 119 / 267 `SizeX`/`SizeY` (`patch_d2`);
  checked against REC-94 run r3.

## Open questions

1. **Needs recording**: no recording of a chest, shrine, door, well or
   portal yet: record
   `packets` + RNG traces for one of each to confirm draw order and the
   0x0E/0x4D bytes. Chest: REC-260, expected sequence
   `world/objects-2.md` §28 (vectors there).
2. **Answered (confirmed by recording `obj1`)**: no animation setup runs
   before the init function; §4 rule 5 (`0x005553E5`, `0x00624690`).
3. **Answered**: `0x005474C0` read in 1.14d, §8.3 "Trap monster id".
4. Meaning of the constant 3 at 0x0E offset 6 and of 0x4D's zero fields
   (client handler `0x0045CD10` and the 0x4D client handler).
   **Answered** for this spec: `world/objects-2.md` §23.
5. Callback `0x00582750` and the eligibility test of the warping shrine
   (20); owner monsters spec. **Answered** for this spec: the 1.14d
   test is a cross-file request to the monsters spec
   (`docs/handoff/pc2-spec-objects.md`).
6. **Answered**: §6 "Preset chest 581" (`0x0054F180`).
7. **Answered**: §12 rules 4–14 (`0x00584870`).
8. Who sets door mode 6 (locked): DS1 preset modes or population; settled
   by a DS1 survey of door object modes. **Answered**: nothing in 1.14d
   (`world/objects-2.md` §22: allocation modes, preset modes, mode-6
   sets).
9. Obelisk: what completes it after mode 3 (gem use, reward) lives in
   the client UI and item code; find the C→S message and its handler.
   **Answered**: C→S 0x44 → `0x005852E0` → power-up `0x00585240`
   (`world/objects-2.md` §19).
10. **Needs recording**: fire event 0's direct mode write (§18.1): confirm with a packets
    trace that clients see no 1 → 2 update for fires.
    Settled for the binary (1.14d-read 2026-10-09, REC-07; the packets trace stays the
    conformance check): clients see no 1 → 2 update for fires. `0x00581700` stores +0x10 := 2
    at `0x00581717` and otherwise calls only `0x00620BB0`, the control getter `0x00546FA0`, burn
    `0x00581680` (damage `0x005DFA00`) and the scheduler `0x005417D0`; none sets flag 0x1 or
    queues the unit (`0x0064C040`), so the §14 update pass sends no 0x0E for it.
11. **Answered**: owner cells of init 51 and operate 48 are §18.4, init
    13 is §17 (CODE-TABLE CHANGE commit).
12. **Answered** for this spec: the 0x58 builder is `0x0053D8D0`; the
    row change is a cross-file request (`docs/handoff/pc2-spec-objects.md`).
13. Drops `0x005594C0` (armor), `0x00559630` (weapon), `0x00559300`
    (gold), `0x00559A30` (by code).
    **Answered**: `world/objects-2.md` §20; request quality per caller
    of `0x00559A30` and `0x00585970`: `items/quality.md` open question 2 (with the class picks
    `0x00555E70`, `0x00555FB0`, `0x005560F0`, `0x00556240`).
14. **Needs recording** (`obj1` data): two class-37 allocations show speed 0 at return in
    mode 2; §4 rule 5 predicts an allocation mode of 2 (init 8's set is
    then a no-op). Check their mode argument in `obj1-objects.jsonl`.
15. Well heal: what `0x00578C20(P)` removes or resets (it counts as
    "used", §11); stat-list / states spec. **Answered**: it removes
    every stat list of a `curable` state P has (`world/objects-2.md`
    §21).
16. `0x0061B060(…, 3)` (portal arrival point without a partner, §12
    rule 8): the DRLG spec owns which point kind 3 is. **Answered**:
    3 is the free-point size (§12 rule 8, `0x00584A59`).
