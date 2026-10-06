# Spec: World — Objects (creation, init, operate; chests, shrines, doors, wells, portals)

- **Status:** draft: every rule below was read from the 1.14d `Game.exe`
  disassembly (addresses inline) and the live 1.14d tables
  (`patch_d2` `objects.txt` 573 rows, `shrines.txt` 23 rows); D2MOO 1.10f
  gave names only. No recording of an object interaction exists yet (open
  questions 1–3). Object population is `world/object-population.md`;
  §16–§18 cover the remaining generic operate and init functions and
  object events 0, 3, 8, 9, 10 (§15).
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
  inits). Machine table: `world/object-functions.tsv`.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 63–74 |
| Inputs | 75–86 |
| Outputs / state changes | 87–93 |
| Rules | 94–95 |
|   1. Object data and unit fields | 96–124 |
|   2. Object control (game +0x10F0) | 125–147 |
|   3. Creation and init dispatch (`0x0054F5D0`) | 148–172 |
|   4. Object animation at a mode change | 173–185 |
|   5. Init functions | 186–253 |
|   6. Preset object classes 574–582 (`0x0054F490`) | 254–273 |
|   7. Operate dispatch | 274–303 |
|   8. Chests and breakables | 304–395 |
|   9. Shrines | 396–473 |
|   10. Doors, operate 8 (`0x00581D40`) | 474–491 |
|   11. Wells, operate 22 (`0x005858A0`) | 492–509 |
|   12. Portals, operate 15 (`0x00584870`) | 510–526 |
|   13. Torch, operate 11 (`0x005843D0`) | 527–531 |
|   14. Client messages | 532–549 |
|   15. Not covered yet | 550–562 |
|   16. Operate functions, part 2 | 563–689 |
|   17. Small init functions | 690–716 |
|   18. Object events 0, 3, 8, 9, 10 | 717–776 |
| Constants & data dependencies | 777–823 |
| Randomness | 824–863 |
| Edge cases & original bugs | 864–914 |
| Test vectors | 915–945 |
| Provenance | 946–988 |
| Open questions | 989–1021 |
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

All items of this list are done:

- object population (`0x00552610`, `PopulateFn` 1–9, objgroup density,
  shrine and well limits in the per-level regions): done,
  `world/object-population.md`;
- the `todo` rows of `object-functions.tsv` (trap door, obelisk, secret
  door, armor stand, weapon rack, bookshelf, teleport pad, slime door,
  exploding chest, bank, stairs, jungle stash, gate, torch tiki): done,
  §16; small init functions: done, §17;
- object events 0, 3, 8, 9, 10: done, §18.

### 16. Operate functions, part 2

Operate record as §7.2 (game, object O, operator P, control, class id);
"none" = no object unit; ENDANIM as §8.2; f = game frame. Return values
are listed for fidelity only (§7.2). Live rows are `objects.txt` `Id`s.

#### 16.1 Torch tiki, operate 13 (`0x005843A0`)

Mode 0 (or none) → mode 1; mode 1 → mode 0; other modes nothing.
Returns 0. Live: 37.

#### 16.2 Trap door, operate 16 (`0x00581EB0`)

1. Mode 0 (or none) → mode 2. Return 1.
2. Mode 2: the first unit of type 5 (warp tile) in O's room unit list
   (head +0x74, next +0xE8); none → fatal. Warp P through it
   (`0x005550B0`, `sim/path-placement.md` §12.2). Return 1.
3. Other modes: return 1. (O's room, position and size are read first
   and not used.) Live: 74.

#### 16.3 Obelisk, operate 17 (`0x00582610`)

I := the unit P interacts with (`0x00554D00`: P +0x6C ≠ 0 → the unit
with GUID P +0x64, else none).

1. Mode 0 (or none): P busy (`0x00535060` = 1) → nothing. P has no item
   of type 20 (gem) in its item list (`0x005825B0`: inventory walk with
   the item-type test `0x00629BB0`) → sound 19 on P with target P
   (`0x00553380`: sound id at unit +0x6E, target at +0x70, queue, flag
   0x400). Else: P's interact := (type 2, O's GUID) (`0x00554120`: only
   when P +0x6C = 0; then +0x64 GUID, +0x68 type, +0x6C := 1), mode 3,
   S→C 0x58 (7 bytes, builder `0x0053D8D0`) to P's client: 0x58, O's
   GUID @1 (u32), 0 @5 (u8), byte @6 never written (stack contents).
2. Mode 3 and I = O: clear P's interact (`0x00554190`: +0x64 := −1,
   +0x68 := 6, +0x6C := 0), mode 1.
3. Return 1 in every case. Live: 80, 90 (90 has `InitFn` 1, so it also
   holds a shrine record).

#### 16.4 Secret door, operate 18 (`0x00583FF0`)

Mode 0 (or none): mode 1, clear flag 0x2, ENDANIM, free the footprint
(`0x00623830`). Return 1. Live: 129.

#### 16.5 Armor stand, operate 19 (`0x00584160`); weapon rack, operate 20 (`0x005841D0`)

Mode 0 (or none): drop at O's position in O's room, armor
`0x005594C0(game, room, &pos, −1, 0, 0)` or weapon `0x00559630(…)`
(items spec); then mode 2, clear flag 0x2. Return 1. The drop's draws
come before the mode change's §4 draw. Live: stands 104, 105, 550, 551;
racks 106, 107, 548, 549.

#### 16.6 Bookshelf, operate 26 (`0x00584060`)

Mode 0 (or none):

1. Mode 2, clear flag 0x2.
2. r := C step, lo' mod 20 (C = control seed). r < 13: C step, lo' & 1 =
   0 → code `tsc `, 1 → `isc `. r ≥ 13: C step, lo' & 1 = 0 → `tbk `, 1 →
   `ibk `. Drop item code (+0xB8) := it.
3. Drop it at O (`0x00559A30`, quality argument 2; items spec).

Return 1. Live: 179, 180 (`InitFn` 2: their trap byte is never armed
here).

#### 16.7 Teleport pad, operate 27 (`0x00581BF0`)

1. O's room R0 none → return 0.
2. Partner T: in R0's unit list the first object (type 2) ≠ O with O's
   class; else, in R0's adjacency array order (`drlg/rooms.md` §6, R0
   itself skipped), the first such object of each room; RT := the room
   it was found in. None → return 0.
3. Point := T's position; free point from RT (`0x0064E7B0`, size 3, mask
   0x1C09, no fallback; `sim/path-placement.md` §7); none → return 0.
4. Place P (`0x00554EA0(game, P, RT, x, y, exact 0, alt 0)`,
   `sim/path-placement.md` §10); failure → return 0.
5. S→C 0x07 (`0x0053BC50`) to P's client: RT's tile x and y (room rect
   +0x10, +0x14) and RT's level; queue P for update (`0x0064C040`); P
   flags 2 (+0xC8) |= 0x10000. Return 0.

O's mode is not tested. Live: 192, 304, 305, 306.

#### 16.8 Slime door, operate 29 (`0x005821A0`)

Mode ≠ 0 → return 1. Else mode 1, free the footprint, flag 0x2 :=
`Selectable[new mode]`, and ENDANIM only when `Mode2` (+0x141) ≠ 0.
Return 1. Live: 229, 230.

#### 16.9 Exploding chest, operate 30 (`0x00581CD0`)

Mode 0 (or none): trap damage `0x005DFA00(game, O, P, 0)`, then `(game,
O, P, 1)` (combat spec); mode 1; ENDANIM. Returns 0. Live: 250
(exploding cow), 454.

#### 16.10 Bank, operate 32 (`0x00564CD0`)

Only for class 267. P's room and O's room both in a town level
(`0x0061AB00`): P's interact := (2, O's GUID) (`0x00554120`); S→C 0x77
(`0x0053CAB0`) with action 0x10 to P's client; scroll/tome recount
(`0x0055FA40`, `items/inventory.md` §5.5). Returns 0. §7.2 rule 2 lets
P operate it with an item on the cursor.

#### 16.11 Stairs, operate 47 (`0x00581F60`) and 50 (`0x00582180`)

- **47:** mode 0 (or none) → mode 1, ENDANIM. Mode 2 → the first warp
  tile (type 5) in O's room, else in the adjacency rooms (O's room
  skipped) in order; found → warp P (`0x005550B0`); none → nothing.
  Return 1. Live: 194, 195.
- **50:** O present and its mode's low 16 bits = 2 → run 47 (the warp);
  else return 0, so mode 0 never opens. Live: 386 (its mode comes from
  init 53, `world/quests-act3.md` §7.7).

#### 16.12 Jungle stash, operate 51 (`0x00583F10`)

Mode 0 (or none): mode 1, clear flag 0x2, ENDANIM, event 10 at f +
`Parm1` + 1, free the footprint when `HasCollision1` (+0x121) = 0, trap
arm with `InteractType` & 0x7F (§8.3), timer-argument owner GUID := P's
GUID (none: −1; `0x00552AF0`). Return 1. The drop is event 10 (§18.5).
Live: 185–188 (`InitFn` 2, so trapped by §5.2; `Parm1` 11, 34, 13, 33).

#### 16.13 Harrogath gate, operate 61 (`0x00582080`)

t := `GetTickCount`; t < +0xD4 + 500 → return 1 (host clock, as §10).
Mode 0 → free the footprint, mode 1, +0xD4 := t, ENDANIM. Mode 2 → stamp
the footprint at O's room and position (`0x00620A70`), mode 0, +0xD4 :=
t. Other modes nothing. Return 1. Unlike doors there is no occupancy
test before closing. Live: 449, 508.

### 17. Small init functions

Init record as §3 rule 6 (game, O, room, control, record, x, y). Mode
sets here run before the unit is added to the world (§3).

| Init | Address | Live rows | Rule |
|---|---|---|---|
| 8 torch | `0x005500C0` | 29, 37, 38, 102, 117 | mode 2 |
| 10 | `0x0054F860` | none | room level (`0x0061A1B0`) = 1 → event 8 at f + 60; else mode 2 and event 0 at f + 25 |
| 13 invisible object | `0x00594020` | 61 | quest chain 4 record exists (`0x00543640(game, 4)`) → link O to it (`0x005436B0(…, O, 4)`, `world/quests.md` §4.6); else mode 2 unless O is already in mode 2 |
| 14 brazier | `0x005500D0` | 101 | mode 1 |
| 22 fire | `0x0054FB40` | 160–162, 245, 345–347 | `Mode2` (+0x141) ≠ 0 and mode 0 (or none) and `Mode0` (+0x13F) = 0 → mode 2; then event 0 at f + 25 |
| 24 spike floor trap | `0x0054FB90` | 196, 261 | event 3 at f + 25 |
| 26 | `0x005500E0` | 259, 373 | mode 1 |
| 27 goo pile | `0x0054FC50` | 266 | C step: lo' mod 1000 ≤ 332 → `InteractType` := 3 (poison trap, §8.3), else 0 |
| 28 gold placeholder | `0x0054F8C0` | 269 | below |
| 34 hell brazier fire | `0x005500F0` | 358, 359 | C step: lo' & 1 = 1 → mode 1 |
| 37 | `0x0059DA50` | none | `world/quests-act2.md` §8.8 |
| 58 fissure | `0x0054FDB0` | 399 | C step; event 8 at f + 25 + lo' mod 250 |

**Gold placeholder** (init 28): mode ≠ 0 → nothing. Else mode 2; n :=
(C step, lo' mod 9) + 1; L := (x, y). n times: dx := C step & 3, dy := C
step & 3; room lookup from the init room at (L.x + dx, L.y + dy)
(`0x00463740`); found room = the init room → P := (x + dx, y + dy), L :=
P, and if the point query at P with mask 0x3F11 is free
(`0x0064D800(room, P, 1, 1)`): gold drop at P (`0x00559300`, items spec).

### 18. Object events 0, 3, 8, 9, 10

Handlers (game, O) as `sim/units.md` §6.4. **Burn** (`0x00581680`): for
each player (type 0) in O's room unit list not in mode 17 with distance
(`0x006416D0`, `missiles/missiles.md` §R9.5) ≤ `Parm0` + 1: trap damage
`0x005DFA00(game, O, player, 1)`. Only O's own room is scanned.

#### 18.1 Event 0, fire (`0x00581700`)

1. O in mode 1 → its mode field := 2, written directly (no mode set: no
   animation, draw or update).
2. Burn.
3. C step; event 0 at f + 15 + lo' mod 35.

#### 18.2 Event 3, spike floor trap (`0x005818B0`)

State s := `InteractType`; "on it" = a player not in mode 17 at
distance ≤ 0.

- s = 0: for each player on it, sound 13 on O (no target; one per
  player). Any → s := 1, event 3 at f + 25. None → event 3 at f + 15.
- s = 1: each player on it takes trap damage `(game, O, player, 0)`.
  Any → s := 2 and mode 1; none → s := 0. Event 3 at f + (`FrameCnt1` >>
  8) + 1.
- s = 2: O's mode at entry ≠ 0 → mode 0, s := 0. Event 3 at f + 15.
- other s: event 3 at f + 15.

No draws.

#### 18.3 Event 8 (`0x00581250`)

- **Class 399 (fissure):** mode (low 16 bits) not 0 or 2 → nothing, and
  no reschedule. Else C step; event 8 at f + 25 + lo' mod 250; mode 1;
  ENDANIM.
- **Other classes** (init 10 only; no live row): p := Act I period of
  day (`0x0061C100(game act 0, none)`; `sim/stats.md` §8 note). p = 0 →
  mode 1 or 2 → mode 0; event 8 at f + 1000. p in 1..3 → mode 0 → mode 1
  and ENDANIM; event 8 at f + 1000. Other p → event 8 at f + 600.

#### 18.4 Event 9, trapped souls and burning bodies (`0x00585CE0`)

Schedulers: init 51 (`0x005501F0`): mode 1 or 2 → event 9 at f + 35.
Operate 48 (`0x005869F0`): mode ≠ 0 → return 0; r := C step mod 100; r <
90 → `D(0)` (§8, operator P), an item → mode 5; r ≥ 90 → mode 1,
ENDANIM, event 9 at f + 20. Returns 0. Live: 380, 381.

Handler:

- mode 1 → event 9 at f + 10.
- mode 2 → `InteractType` += 1 (u8). Below 2 → burn, event 9 at f + 25.
  Else `D(0)` with no operator; an item → mode 3 and event 9 at f +
  (`FrameCnt3` (+0xE4) >> 8) + 1; no item → nothing more.
- mode 3 → mode 4.
- other modes: nothing.

#### 18.5 Event 10, jungle stash drop (`0x00586850`)

`D(0)` with operator := the player (`0x00552F60(game, type 0, GUID)`)
whose GUID is O's timer-argument owner (`0x00552B10`); gone → none.

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
   (§4, U).
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
9. Obelisk: what completes it after mode 3 (gem use, reward) lives in
   the client UI and item code; find the C→S message and its handler.
10. Fire event 0's direct mode write (§18.1): confirm with a packets
    trace that clients see no 1 → 2 update for fires.
11. `object-functions.tsv` gives init 51 and operate 48 (trapped souls)
    to `world/quests.md`, which does not cover them; §18.4 states them.
    Settle the owner cell.
12. `sim/server-messages.tsv` row 0x58 has no builder; it is
    `0x0053D8D0` (owner `sim/intents-events.md`).
13. Drops `0x005594C0` (armor), `0x00559630` (weapon), `0x00559300`
    (gold), `0x00559A30` (by code): items spec, not yet specified.
