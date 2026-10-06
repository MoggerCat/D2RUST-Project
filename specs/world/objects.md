# Spec: World — Objects (creation, init, operate; chests, shrines, doors, wells, portals)

- **Status:** draft: every rule below was read from the 1.14d `Game.exe`
  disassembly (addresses inline) and the live 1.14d tables
  (`patch_d2` `objects.txt` 573 rows, `shrines.txt` 23 rows); D2MOO 1.10f
  gave names only. No recording of an object interaction exists yet (open
  questions 1–3). Object population (`0x00552610`, objects.txt
  `PopulateFn`, objgroup) and the operate functions marked `todo` in
  `object-functions.tsv` are not covered yet (§15).
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
  `data/fixups.md` §13 (`FrameCnt` × 256). Machine table:
  `world/object-functions.tsv`.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 57–68 |
| Inputs | 69–80 |
| Outputs / state changes | 81–87 |
| Rules | 88–89 |
|   1. Object data and unit fields | 90–118 |
|   2. Object control (game +0x10F0) | 119–141 |
|   3. Creation and init dispatch (`0x0054F5D0`) | 142–166 |
|   4. Object animation at a mode change | 167–179 |
|   5. Init functions | 180–247 |
|   6. Preset object classes 574–582 (`0x0054F490`) | 248–267 |
|   7. Operate dispatch | 268–297 |
|   8. Chests and breakables | 298–389 |
|   9. Shrines | 390–467 |
|   10. Doors, operate 8 (`0x00581D40`) | 468–485 |
|   11. Wells, operate 22 (`0x005858A0`) | 486–503 |
|   12. Portals, operate 15 (`0x00584870`) | 504–520 |
|   13. Torch, operate 11 (`0x005843D0`) | 521–525 |
|   14. Client messages | 526–543 |
|   15. Not covered yet | 544–552 |
| Constants & data dependencies | 553–588 |
| Randomness | 589–615 |
| Edge cases & original bugs | 616–648 |
| Test vectors | 649–669 |
| Provenance | 670–700 |
| Open questions | 701–722 |
<!-- /index -->

## Summary

An object is a unit of type 2 (`sim/units.md` §1) whose behavior comes
from its `objects.txt` row through three function numbers: `InitFn`
(run once when the unit is allocated), `OperateFn` (run when a player,
a monster or a skill operates it) and `PopulateFn` (room population, not
covered here). Shared state lives in the game's object control (seed,
per-level regions, shrine lists). This spec owns the dispatchers, the
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
§3 rule 8 (13 door rows have `PreOperate`). Locked doors are mode 6.

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

### 6. Preset object classes 574–582 (`0x0054F490`)

DS1 presets may name class ids beyond `objects.txt` (`drlg/preset.md`
§5.3: 580, 581, 582 occur). The preset spawner (`0x00555823`) calls
`0x0054F490(game, room, class, x, y, mode)`: class ≤ 573 → none;
index := class − 574; index ≥ 9 (`0x00731D4C`) → none; else handler
`0x00731D28`[index]:

| Class | Handler | Result |
|---|---|---|
| 574–579 | `0x0054F0D0` | allocate the class/range row of `0x006E1080` (below) with the preset mode; then id := min + `roll(max − min)` on the **new object's unit seed**; 4 or 5 → 2; set `InteractType` and the shrine record (overrides init 1, whose draws already happened) |
| 580 | `0x0054F370` | special chest: level 25 → class 371, else the class argument (580) through 581's path; then for levels 62–64: `roll(100)` on the control seed, spark only if ≤ 50; other levels: spark always; level 25: `InteractType` := 3; then flag 0x80 and mode 0 |
| 581 | `0x0054F180` | random chest class by act (not re-read here: open question 6) |
| 582 | `0x0054F430` | class from `0x0059D830` (quest spec), mode 1-argument allocation |

`0x006E1080` rows {class, min, max}: 574 {136, 2, 6}, 575 {136, 7, 7},
576 {136, 8, 11}, 577 {136, 12, 12}, 578 {136, 1, 5}, 579 {136, 14, 14}.
`max` is exclusive: 576 never gives 11 (resist poison), 578 never 5;
574 gives 2 or 3.

### 7. Operate dispatch

#### 7.1 Entry with range check (`0x00584540`)

Arguments: game, operator, unit type, GUID. Callers: the C→S 0x13 object
case `0x00548B00` (player; range and walk rules in `world/waypoints.md`
§5.2) and monster AI `0x005B0FC1` (`monsters/ai.md`).

1. No such unit → return 0.
2. Operator is a monster and `MonsterOK` (+0x16D) = 0 → return 1.
3. Operator present and not in interact range (`0x00623660`) → return 1.
4. Else run §7.2 and return 1.

Skills call §7.2 directly (`0x005C9B13`, skills spec).

#### 7.2 Dispatch (`0x00584420`)

1. Result out := 1. Find the unit (`0x00552F60`).
2. Operator is a player: refuse (return 0, nothing else) when its
   interact info is active (`0x00554100`), player data +0x4C (busy) ≠ 0,
   or an item is on the cursor unless the object class is 267 (stash).
   Monsters and no operator skip these tests.
3. `OperateFn` ≥ 101 → fatal. Class 121, 122 or 22 → return 0. Table
   `0x00732D18`[`OperateFn`] null (0, 35–38, 60, 74–100) → return 0.
4. Call it with {game, object, operator, control, class id} and the
   `OperateFn` number; return its result.

The functions' return values reach no caller that acts on them (§7.1
returns 1); they are listed for fidelity only.

### 8. Chests and breakables

Common pieces:

- **Chest drop** `D(Q)` = `0x00585B90` with the operate record and
  quality `Q` (`items/treasure.md` §4); it returns the first item or
  none.
- **Magic test** (`0x0062A0F0`): the item exists, is type 4 and its
  quality is 4…9.
- **Code drop** `C(code)` = `0x00585970(game, object, code, 0)` (items
  spec): one item of that 4-character code.
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
   code (+0xB8) ≠ 0: drop it at the object (`0x00559A30`, items spec).
   Trap arm. Return 1.

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

### 9. Shrines

#### 9.1 Operate 2 (`0x00583C70`)

1. Operator GUID field (+0x0C) ≠ 0 or object mode ≠ 0 → return 0.
2. +0x0C := operator GUID + 1 (no operator: 0). Queue for update,
   flag 0x1, mode 1.
3. Hover: free an existing hover; create one with the text of
   `"%d"`(3683 + shrine id) (`0x00661110`, lifetime 8 · length + 125
   frames); if created: store it, queue, flag 0x100, event 6 at frame +
   300.
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

#### 9.2 Effects (table `0x006E1850`, 3 dwords: function, stat, state)

P = player (operator), S = shrine record. Stats: 6 life, 8 mana, 10
stamina (8.8 fixed). "State(st, stat, v, cb)" = the timed-state helper
`0x0056E970` (skills spec; via `0x00582800`) on P with source the
object, duration `Duration in frames` (+0x0C), that stat and value.

| Code | Fn | Effect |
|---|---|---|
| 1, 0, 23 | `0x005828E0` | life += max − life (`0x00625D10`), mana += max − mana (`0x00625D60`) (base stat adds) |
| 2 | `0x00582860` | life to max (1.14d: no doubling) |
| 3 | `0x005828A0` | mana to max |
| 4 | `0x00582940` | d := `Arg0` · (life >> 8) / 100; life −= d · 256; mana += `Arg1` · d · 256 / 100 (unsigned) |
| 5 | `0x005829A0` | d := `Arg0` · mana / 100; mana −= d; life += `Arg1` · d / 100 |
| 6, 8–11, 13, 15 | `0x00583B30` | State(table state, table stat, V(stat, `Arg0`)) |
| 7 | `0x005839B0` | v := V(19, `Arg0`); State(129, stat 25, `Arg1`), then stat 19 := v on that list |
| 12 | `0x00583BF0` | State(134, stat 11, 0, remove callback `0x00583BD0`); refresh P's skills (`0x0056DE40`); the +2 comes from state 134 (`skills/levels.md`) |
| 14 | `0x00583A70` | stamina := max (`0x00625DB0`); v := V(162, `Arg0`); State(136, 162, v, cb `0x00583A40`); on that list stamina := 2v, stat 28 := 1000 |
| 16 | `0x00582A00` | (unreachable through init) |
| 17 | `0x00582A30` | free spot near P (`0x0064E7B0`, size 3, mask 0x1C09); town portal object 59 at spot + (5, 5) to P's act town (`0x0056D130`) |
| 18 | `0x00582C40` | gem upgrade (§9.3) |
| 19 | `0x00582DA0` | storm (§9.3) |
| 20 | `0x00583050` | nearest eligible monster to P becomes unique (`0x0065A800`, callback `0x00582750`; monsters spec) |
| 21, 22 | `0x005830E0`, `0x00583410` | potions + missiles (§9.3) |

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

### 11. Wells, operate 22 (`0x005858A0`)

1. c := charges (`InteractType`); c = 0 → return 0.
2. Heal (`0x00585720`, P = operator): life < max and `Parm3` & 2 →
   life := min(life + (max · `Parm1` >> 8), max) (unsigned), client
   update; mana the same with `Parm3` & 1; stamina the same (always);
   remove P's states 2 (poison) and 1 (freeze) stat lists; `0x00578C20`;
   heal P's pets (callback `0x005856A0`). Used := any of these changed.
3. Not used → return 0. Used: c := c − 1; M := 2 · `Parm2`; if c ≤ M and
   c mod (M / 2) = 0 → mode := 2 − c / `Parm2`. Store c; event 2 at
   frame + `Parm0` + 1. Return 0.

**Event 2** (`0x00581510`, refill): c / `Parm2` ≥ 2 → fatal; c := c + 1
(more than M → fatal); same mode rule; queue, flag 0x1. Live wells:
`Parm0` 750, `Parm1` 128 (50 %), `Parm2` 1, `Parm3` 3: modes 0 → 1 → 2
as charges go 2 → 1 → 0 (refills: 1 → mode 1, 2 → mode 0); one charge
returns 751 frames after each use.

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
   ENDANIM; P gets state 102 (`just_portaled`) for 75 frames (event 12). These steps are
   not yet re-read in detail (open question 7).

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

### 15. Not covered yet

Object population (`0x00552610`, `PopulateFn` 1–9, objgroup density,
shrine and well limits in the per-level regions), the `todo` rows of
`object-functions.tsv` (trap door, obelisk, secret door, armor stand,
weapon rack, bookshelf, teleport pad, slime door, exploding chest, bank,
stairs, jungle stash, gate, torch tiki, small init functions), object
events 0, 3, 8, 9, 10.

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
   (§4, U).
5. Casket: drop draws, mode change, `roll(10000)` C. Urn: mode change,
   `roll(100)` C, drop draws. Barrel: mode change, `roll(10000)` C,
   `roll(100)` C, drops. Evil urn: drops, mode change, `roll(255)` C.
6. Shrine operate: mode change (U), then the effect: gem `roll(6)` P;
   exploding/poison `roll` on U; the warping and storm effects draw in
   code owned by other specs.
7. Trap event 8/9: one step C (`& 1`).
8. Door, well, torch: only the §4 draw of their mode changes.

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

## Test vectors

Synthetic (CI), RNG per `rng.md` §3:

| Input | Expected | Source |
|---|---|---|
| chest init 3, lockable, `MonLvl1` 1, C = {1, 666} | draws 51 (not trapped), 31 (not locked); `InteractType` 0; U := `init_low(55249)`; C after = {671516612, 330169957} | §5.2 |
| same, C = {410, 666} | 0 (trapped), trap 8, 0 (locked): `InteractType` 0x88; U := `init_low(48426)` | §5.2 |
| shrine init 1, `Parm0` 3, level 2, C = {1, 666} | `roll(10)` 1 → class 4; picks 4 (id 8, `LevelMin` 5 > 2), 8 (id 13): id 13 | §5.1 |
| shrine init 1, `Parm0` 0, level 2, C = {1, 666} | `roll(22)` = 21 → id 22 | §5.1 |
| shrine init 1, `Parm0` 3, level 2, C = {4, 666} | `roll(10)` 0 → class 1; picks 2 (id 18, `LevelMin` 4), 3 (id 19): id 19 | §5.1 |
| PreOperate, C = {1, 666} | `roll(14)` = 9: mode unchanged | §3 |
| anim, `FrameDelta` 256, `Sync` 0, U = {12345, 666} | `roll(32)` = 23; speed 263 | §4 |
| preset 576 | id ∈ {8, 9, 10} | §6 |
| well, `Parm2` 1: charges 2, use, use, refill, refill | modes 1, 2, 1, 0 | §11 |
| door mode 2, doorway has a player (0x80) only | mode 5, tick stored | §10 |

Game-file (`#[ignore]`, `D2_GAME_DIR`): `object-functions.tsv` equals the
three tables of the 1.14d `Game.exe` and the live row counts; the shrine
lists of §2 from the live `shrines.txt`.

## Provenance

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

## Open questions

1. No recording of a chest, shrine, door, well or portal yet: record
   `packets` + RNG traces for one of each to confirm draw order and the
   0x0E/0x4D bytes.
2. Whether the allocation sets the animation (§4 draw) before the init
   function runs (the mode is stored directly at `0x005553E5`); settled
   by a trace of an object allocation with `Sync` = 0.
3. Trap monster choice `0x005474C0` (per-level cache; D2MOO
   `OBJRGN_GetTrapMonsterId`): not re-read in 1.14d; decides which
   monsters casket/barrel/trap 8–9 spawn.
4. Meaning of the constant 3 at 0x0E offset 6 and of 0x4D's zero fields
   (client handler `0x0045CD10` and the 0x4D client handler).
5. Callback `0x00582750` and the eligibility test of the warping shrine
   (20); owner monsters spec.
6. Preset 581 (`0x0054F180`): act-dependent random chest class and its
   draw; settled by reading it against D2MOO `OBJECTS_SpawnPresetChest`.
7. Portal operate steps of §12 rule 3 in order, with their quest-flag
   gate (`levels`/leveldefs quest columns).
8. Who sets door mode 6 (locked): DS1 preset modes or population; settled
   by a DS1 survey of door object modes.
