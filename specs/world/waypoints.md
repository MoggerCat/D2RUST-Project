# Spec: World — Waypoints

- **Status:** draft: every rule read from the 1.14d `Game.exe` code
  (addresses below) and the live tables (`levels.txt` 39 waypoint rows,
  `objects.txt` 16 waypoint classes, the `Game.exe` flag table); menu,
  close and two same-act travels match the recordings
  `20261006-015956-packets.jsonl` and `20261006-022633-packets.jsonl`
  byte for byte; cross-act travel and first activation are not recorded
  yet (open questions 1, 2).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::waypoints` (index mapping, waypoint
  data, operate function 23, init function 17, 0x49 handler, travel);
  the 0x63/0x49 byte layouts live in `d2-proto`
- **Related specs:** `sim/intents-events.md` (transport, dispatch gate,
  result codes, C→S 0x13/0x49 and S→C 0x63/0x07/0x0D/0x15/0x51 rows of
  the two TSVs); `sim/tick.md` (§3 tick steps, §4 room population, §5
  timer queue: ENDANIM event type 1, object class); `sim/unit-order.md`
  (GUIDs); `sim/rng.md` (§3 `roll`, §5.3 unit seed, §7);
  `data/fields.tsv` (`levels`, `leveldefs`, `objects` offsets);
  `data/fixups.md` §13 (`FrameCnt` × 256); parallel siblings
  `world/quests.md` (quest records and flags; act-transition gates),
  `world/npc.md` (NPC act travel: Warriv, Meshif, Tyrael), 
  `world/vendors.md`, `world/cube.md`. Machine table: `world/waypoints.tsv`.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 50–66 |
| Inputs | 67–81 |
| Outputs / state changes | 82–92 |
| Rules | 93–94 |
|   1. Waypoint index ↔ level | 95–114 |
|   2. Waypoint record ("history") | 115–146 |
|   3. Save field layout (owner of the save format: the character-save spec) | 147–169 |
|   4. Which waypoints are known without operating one | 170–189 |
|   5. Waypoint objects | 190–277 |
|   6. C→S 0x49 TakeOrCloseWp (`0x0054C5D0`) | 278–322 |
|   7. Travel (`0x00584F60`) | 323–378 |
|   8. Timing and message order | 379–400 |
|   9. Town portals | 401–406 |
|   10. Object mode change (consequence used above) | 407–414 |
| Constants & data dependencies | 415–442 |
| Randomness | 443–466 |
| Edge cases & original bugs | 467–503 |
| Test vectors | 504–543 |
| Provenance | 544–582 |
| Open questions | 583–630 |
<!-- /index -->

## Summary

Every level whose `levels.txt` `Waypoint` cell is not 255 has a waypoint
index (0..38 in 1.14d). Each player has one 16-byte waypoint record per
difficulty: a magic word and a bit per index. The Rogue Encampment bit is
always set. A player learns a waypoint by operating a waypoint object
(objects.txt operate function 23): the bit of the object's level is set;
a waypoint object still in its neutral mode then plays its activation
animation and shows no menu; one already active sends message 0x63 (the
record) and the client opens the menu. The client answers with 0x49
(object GUID, destination level, or level 0 to close). The server checks
the object, act, distance and the destination bit, then warps the player
to the destination level's waypoint inside the message drain, in the
same act or across acts. Act towns 2–5 become known through NPC travel
and quest objects, which are hooks into `world/npc.md` and
`world/quests.md`; the waypoint code itself never checks a quest.

## Inputs

| Name | Type | Source |
|---|---|---|
| levels `Waypoint` | u8 at levels record +0xE4 (228) | `levels.txt` (`data/fields.tsv`) |
| levels `Act` | u8 at +0x03 | `levels.txt` |
| leveldefs `Position` | u32 at +0x90 (144) | `leveldefs` table (`data/fields.tsv`) |
| objects `OperateFn`, `InitFn` | u8 at +0x1B3 (435), +0x1B1 (433) | `objects.txt` |
| objects `FrameCnt1` | u32 at +0xDC (220), already × 256 (`fixups.md` §13) | `objects.txt` |
| objects `FrameDelta[m]`, `Start[m]`, `Sync` | u16 at +0xF8+2m, u8 at +0x129+m, u8 at +0x175 | `objects.txt` (anim setup, §10) |
| game difficulty | u8, game +0x6D (0 normal, 1 nightmare, 2 hell) | game |
| C→S 0x13 | `type:u32@1 id:u32@5` | client |
| C→S 0x49 | `wp:u32@1 level:u16@5` (bytes 7–8 ignored) | client |
| waypoint save section | 80 bytes | character save (§3) |

## Outputs / state changes

- Player waypoint records (bits set by activation; magic normalised, §2).
- Waypoint object mode (0 neutral → 1 operating → 2 opened) and one
  ENDANIM timer (type 1) per activation animation.
- Player interact info (unit +0x64 GUID, +0x68 type, +0x6C active).
- Player position, room and act (travel); one arrival record in the
  object control list (§7.1).
- S→C 0x63 (menu), 0x07 and 0x0D (travel, inside the drain); 0x2C sound
  0x13 on the hostile-delay refusal (§6.1).

## Rules

### 1. Waypoint index ↔ level

1. A level's waypoint index is the u8 at levels record +0xE4; 255 = no
   waypoint (`0x00660E00`: level id ≥ the levels record count → none;
   record missing → none). The index of a level is read directly; the
   level of an index is found by scanning level ids 1, 2, … count−1 and
   taking the **first** whose index matches (`0x00660D90`; index ≥ 255 →
   none). Code derives both from the loaded `levels` table (mod patches
   apply); `world/waypoints.tsv` is the 1.14d expectation, not a source.
2. 1.14d data (measured, `patch_d2` `levels.txt`): 39 indexes 0..38, each
   on exactly one level; no duplicates. Act ranges (from `Act`): act 1
   0–8, act 2 9–17, act 3 18–26, act 4 27–29, act 5 30–38. Index order
   is not level order (index 10 = level 48, index 11 = level 42).
3. Index limit: the flag table holds 0x70 (112) entries; any index ≥
   0x70 reaching the bit test or bit set is a fatal assert (`0x00660E50`,
   `0x00660EC0`).
4. Town levels (hard-coded switch `0x006426A0`, reached through
   `0x0061AAF0` for a level id and `0x0061AB00` for a room): 1, 40, 75,
   103, 109. Indexes 0, 9, 18, 27, 30.

### 2. Waypoint record ("history")

One record per difficulty: 16 bytes = 8 little-endian u16 words.

| Bytes | Content |
|---|---|
| 0–1 | magic u16; 0x0102 when valid (bytes `02 01`) |
| 2–15 | bit field: index n is bit n mod 16 of word n/16 + 1, i.e. **byte 2 + n/8, bit n mod 8** |

Rules (all 1.14d):

1. The (word, mask) pair per index comes from the table at `0x00746428`
   (count u32 at `0x00746424` = 0x70; entry n = u16 word index, u16
   mask). Measured on `Game.exe`: entry n = (n/16 + 1, 1 << (n mod 16))
   for n = 0..111. d2rs computes it.
2. **Test** (`0x00660E50(record, n)`): asserts record ≠ null, table count
   = 0x70, n < 0x70; result = word AND mask (non-zero = known).
3. **Set** (`0x00660EC0(record, n)`): same asserts; word |= mask. Bits are
   never cleared by any 1.14d code path.
4. **Allocate** (`0x00660F30`, 16 bytes, called three times per player
   from `0x00621F90`): all zero, magic = 0x0102, bit 0 set. Player data
   +0x1C + 4·d holds the record of difficulty d (`0x00547F50` returns the
   record of the game's difficulty, game +0x6D).
5. **Load copy** (`0x00661030(dest, src)`): copy 16 bytes; if the magic
   is not 0x0102: 0x0000 or 0x0101 → the record is **zeroed** (the code
   writes magic 0x0102 and then overwrites the first dword with 0, so the
   magic ends 0x0000), any other value → fatal assert; in every case bit
   0 is then set. Edge case 1.
6. **Out copy** (`0x006610B0(src, dest)`, for 0x63 and the save): if the
   source magic is not 0x0102: 0x0000 or 0x0101 → the **source** magic
   becomes 0x0102; other → fatal assert. Then copy 16 bytes.

### 3. Save field layout (owner of the save format: the character-save spec)

The waypoint section is 80 bytes (writer `0x005693E0`; readers
`0x0056A3E0`, `0x00532C70`):

| Offset | Size | Content |
|---|---|---|
| +0 | 2 | `57 53` ("WS") |
| +2 | 4 | u32 1 |
| +6 | 2 | u16 0x50 (section size) |
| +8 + 24·d | 24 | difficulty d (0..2): the 16-byte record (§2), then 8 zero bytes |

1. Write: section zeroed, header written, each record through the out
   copy (§2 rule 6), 80 bytes appended; fails (1) if fewer than 80 bytes
   remain.
2. Read: fewer than 80 bytes left, header ≠ "WS", or a record magic
   not in {0x0102, 0x0101, 0x0000} → error (0x10 in `0x0056A3E0`, 3 in
   `0x00532C70`); else each record through the load copy (§2 rule 5).
   The u32 at +2, the size at +6 and the 8 trailing bytes are not read.
3. Measured on the test characters' 1.14d saves (version 96): the
   section starts at file offset 0x279; header bytes `57 53 01 00 00 00
   50 00`. Where the section sits in the file belongs to the save spec.

### 4. Which waypoints are known without operating one

1. Rogue Encampment (index 0) in every difficulty, always: set at
   allocation and after every load copy (§2 rules 4, 5).
2. No code sets bits on game creation, join or act entry. The complete
   list of 1.14d callers of the set function `0x00660EC0`:

| Caller | Sets the index of level | Trigger (owner) |
|---|---|---|
| `0x00584E30` (operate fn 23) | the operated object's level | §5 |
| `0x00579D60` (NPC menu, from C→S 0x38 `0x0054BCA0`), NPC class 155 (Warriv; names per D2MOO) | 40 Lut Gholein | act 1→2 travel, after a quest-6 test (`world/npc.md`, `world/quests.md`) |
| same, Meshif (class 210) | 75 Kurast Docktown | act 2→3, quest 14 test |
| same, Tyrael (class 367), expansion game only (game +0x70) | 109 Harrogath | act 4→5, quest 26 test |
| `0x00584750` (operate fn 46: objects 342 "portal", 408) | 103 Pandemonium Fortress | object in level 102, quest-22 test; then warp to 103 (`world/quests.md`) |
| `0x005B4FF0` from `0x005B5880` (operate fn 73: object 566 "Harrogath") | 109 Harrogath | expansion, quest 26/28 tests (`world/quests.md`) |

3. Readers of the test function besides this spec: the client menu
   (`0x0049C7F0`) and a quest callback `0x0058B990` that tests the index
   of level 123 (`world/quests.md`).

### 5. Waypoint objects

1. Waypoint objects are the `objects.txt` rows with `OperateFn` 23 (table
   `0x00732D18` entry 23 = `0x00584E30`) and `InitFn` 17 (table
   `0x00731BC0` entry 17 = `0x00547210`). 1.14d: 16 classes, 119, 145,
   156, 157, 237, 238, 288, 323, 324, 398, 402, 429, 494, 496, 511, 539;
   all have `Mode0`–`Mode2` = 1; `FrameCnt1` 15 (20 for 494, 496, 511,
   539); `FrameDelta1` 200; `Sync` 1 except 429, 494, 496, 511, 539 (0).
   These are the `objects.txt` cell values. The loaded object record
   holds `FrameCnt0`–`7` shifted left by 8 (`data/fixups.md` §13): record
   +0xDC = 3,840 (15 << 8) or 5,120 (20 << 8), which §5.1 reads back with
   `>> 8`.
2. The level a waypoint object belongs to is the level of its room.
   Which waypoint it activates follows from that level (§1), never from
   its class.

#### 5.1 Init function 17 (`0x00547210`, at object creation)

1. Walk the arrival list (§7.1) from its head. A node matches if its
   room pointer equals the object's room, or the object has a room and
   the node's (x, y) lies inside that room's rectangle (room +0x4C x,
   +0x50 y, +0x54 width, +0x58 height; x ≥ left and x < left + width,
   same for y).
2. First match: if the object is null or in mode 0: set mode 1
   (`0x00624690`) and schedule ENDANIM (type 1) on the object at frame
   + (`FrameCnt1` >> 8) via `0x005417D0` (= frame + 15 or + 20; never ≤
   frame). Then free **that node** and set the list head to null
   (edge case 3). Return.
3. No match: if the object's room is in a town level → set mode 2
   (`0x00624690(object, 2)`). Otherwise the object stays in mode 0.

#### 5.2 Operate function 23 (`0x00584E30`)

Reached from C→S 0x13 (type 2) when the player is in operate range:
`0x0054AA90` (size 9; type > 5 → 2) → `0x00548B00` (object case:
object missing → 1; object mode ≥ 8 → 3; distance > 50 → 1; in range
and unobstructed → stop the player and `0x00584540` → `0x00584420`;
else walk to it and operate on arrival; owner: the object-interaction
spec). `0x00584420` does **not** call the operate function when the
player's interact info is active, player data +0x4C (busy) ≠ 0, or an
item is on the cursor (unless the object class is 267). Operate struct:
game, object, player, object region, class id.

Steps, in order:

1. Level = level of the object's room; record = player's record for the
   game difficulty. If the level has a waypoint index: **set its bit**
   (§2 rule 3). This happens on every operate, in every mode.
2. Object mode 0 (or object null): set mode 1 (`0x00624690`), schedule
   ENDANIM (type 1) at frame + (`FrameCnt1` >> 8) + 1 (`0x005417D0`).
   No menu. Return 1.
3. Mode 1 or 2: if the player is busy (`0x00535060`: interact info
   active, an item on the cursor, or player data +0x4C ≠ 0) → nothing.
   Else build 0x63 (§5.3) with the object's GUID and the record (out
   copy, §2 rule 6), queue it to the player's client (`0x0053D960`, 21
   bytes), then set the player's interact info to (type 2, object GUID)
   (`0x00554120`; ignored if already active). Return 1.
4. Any other mode: nothing. Return 1.

Result of the 0x13 (2026-10-07, `0x00548B19`–`0x00548BD8`,
`0x00584540`, `0x00584420`): the handler's object case returns 1 for a
missing object or distance > 50, 3 for object mode ≥ 8, 0 for the walk
(not in operate range `0x00623660` or blocked `0x00622B50(…, 0x804)` →
approach `0x00548A50`), and for the operate itself the result of
`0x00584540`: 0 → 3, else 0. `0x00584540` returns 0 only when the object
GUID no longer resolves, 1 when the operate is skipped (a monster
operator on an object without the monster flag, or out of range), and
otherwise 1 whatever happens in `0x00584420` (its result slot is set to
1 and never changed; the operate function's own return, 1 for function
23, is dropped). So an operate of a waypoint answers 0 in every mode
and on the busy refusal of step 3. An object class without an
objects.txt record (`0x00640E90` null) is a fatal assert (line 0x2BF).

1.14d differs from D2MOO 1.10f here: the 10-second hostile check
(§6.1) is not in the operate function; it moved to the 0x49 handler.

#### 5.3 S→C 0x63 WaypointMenu (21 bytes)

| Offset | Size | Field |
|---|---|---|
| 0 | 1 | 0x63 |
| 1 | 4 | waypoint object GUID, u32 LE |
| 5 | 16 | the player's record for the game difficulty (§2), magic always `02 01` |

Client handler `0x0045E670` copies the 21 bytes and calls `0x0049CF90`,
which stores the record with the load copy (`0x00661030`) and opens the
menu (client UI spec).

### 6. C→S 0x49 TakeOrCloseWp (`0x0054C5D0`)

Gate: alive (`intents-events.md` §2.3). Size ≠ 9 → 3. Fields: wp = u32
at +1, level = u16 at +5.

#### 6.1 Hostile delay (host clock)

`0x0055B6C0(player, 0)` returns player data +0x160 (`GetTickCount` value
stored when this player last declared hostility, `0x0055B720`, only
caller `0x005A5E50`). If `GetTickCount()` < that + 10000: attach sound
0x13 to the player (`0x00553380`: unit +0x6E = 0x13, +0x70 = target,
update flag 0x400), reset the interact info (`0x00554190`), return 1.
d2rs: hostility needs a second player (out of Phase 0–6 scope); the
value stays 0 and this branch never fires (open question 6).

#### 6.2 Validation (`0x00549570`, returns the result code)

| Step | Test | Result |
|---|---|---|
| 1 | object `wp` (type 2) missing (`0x00552F60`) | 1 |
| 2 | act of the player's room level ≠ act of the object's room level | 2 |
| 3 | max = 22 if player class = 1 (sorceress) else 10; \|player x − object x\| > max or \|player y − object y\| > max (subtiles, `0x00548EF0`; object position from its static path) | 1 |
| 4 | level = 0 | 0 (close, §6.3) |
| 5 | level ≥ levels record count | 3 |
| 6 | player data missing | fatal assert |
| 7 | level has no waypoint index | 3 |
| 8 | the index's bit is clear in the player's record for this difficulty | 2 |
| 9 | otherwise | 0 |

Not checked: that `wp` is a waypoint object, that the menu is open or
the interact info names `wp`, the destination act, expansion, quests.

Step 2 without a room (handoff `impl-world` W1, `0x00549597`–
`0x005495BC`): the act is `0x006427F0(0x0061A1B0(room))`; with no room
`0x0061A1B0` returns level id 0 and `0x006427F0(0)` is act 0. So a
player or object without a room counts as **act 0**, not "no act": a
roomless unit passes step 2 against any Act I unit.

#### 6.3 After validation

1. Result ≠ 0: if the player's interact unit (`0x00554D00`) is the
   object `wp`, reset the interact info. Return the result (1.14d ignores
   it, `intents-events.md` §2.2).
2. Result 0: travel `0x00584F60(game, player, wp, level)` (§7), return 0.

### 7. Travel (`0x00584F60`)

1. Object `wp` missing, no objects.txt record, `OperateFn` ≥ 0x65, or
   operate table entry ≠ `0x00584E30` → nothing (result already 0).
2. Reset the player's interact info (`0x00554190`). This is the whole
   "close" action: level = 0 or level = the object's own level stops
   here.
3. Re-test the destination bit (§2 rule 2); clear → stop.
4. Tile code = 13 if the level is a town (§1 rule 4) or one of 46, 74,
   133, 134, 135, 136; else 0. 1.14d waypoint levels with code 13: 1, 40,
   74, 46, 75, 103, 109 (`waypoints.tsv` `tile_calc`); 133–136 have no
   waypoint.
5. Warp `0x0053AEC0(game, player, level, tile code)` (D2MOO
   `LEVEL_WarpUnit`):
   - destination act ≠ the client's act (`0x005382B0`): act change,
     `0x00537340` then `0x0053ACC0` (D2MOO `LEVEL_ChangeAct`; owner: the
     act/level-change spec; unrecorded, open question 1);
   - same act: spawn search `0x0061B060(act, level, tile code, &x, &y,
     player size)` and, if it found a room, place the player
     `0x00554EA0(game, player, room, x, y, 0, 0)`.
6. Spawn search (`0x0061B060` → `0x0066B2B0`): level init if needed
   (`0x006424A0`). If leveldefs `Position` ≠ 0: tile code 13 → the
   level's waypoint room (`0x0066AD80`); other codes → a tile of that
   code (`0x0066AC40`, draws, see Randomness). If `Position` = 0: the
   waypoint room, else a warp room (`0x0066B1F0`), else the room at the
   level centre, else `0x0066AE70`. The point is converted to subtiles
   (`0x00643560`), **+3 on both axes**, then moved to free coordinates
   (`0x0064E7B0`, mask 0x1C09; not found → fatal assert). Measured: every
   1.14d waypoint level with `Position` = 1 gets code 13, so waypoint
   travel always ends in the waypoint room. Room selection, the free-
   coordinate search and placement: `drlg/levels.md` §10, `sim/path-placement.md` §7, §10, §11.
7. Arrival message: if the player now has a room and it equals the room
   that `0x00619E50(act of level, level, tile code)` returns (same search
   without the free-coordinate step):
   `0x00619E50(act, level, tile code, &x, &y)` asserts a non-null act
   (line 0x221) and makes exactly the call `0x0066B2B0(act +0x48,
   level, tile code, &x, &y)` that rule 6 made, a second time: whatever
   that search does (level init and room activation when needed, the
   chosen room) runs again; the room is then already built and active,
   and no 1.14d waypoint level reaches its drawing branches
   (Randomness 3), so the second call has no effect beyond its result.
   Its x, y are discarded; set the player's mode to 2 at its
   own position (`0x005809D0(game, player, no skill, 2, x, y, 0)`), then
   queue S→C 0x0D to the client (`0x0053B4B0`): unit type 0, player GUID,
   1, **x + 3, y + 3**, 0, 0 (x, y = the player's position after
   placement; edge case 5).
8. Arrival record: `0x00547170(game, player, room)` prepends a node
   {room, player x, player y} to the object control's arrival list
   (object control = game +0x10F0, head at +0x1110). The room may be
   null. Init function 17 consumes it (§5.1).

#### 7.1 Arrival list

Allocated empty with the object control (`0x00546C60`), freed at game end
(`0x00546E90`). Written only by travel (rule 8), read only by init 17.

### 8. Timing and message order

1. 0x13 and 0x49 run in the message drain before a tick
   (`intents-events.md` §2.2), so a menu, close or travel completes
   between two ticks. No timer event is scheduled by the 0x49 path;
   the recorded player timer event 11 (D2MOO "DELAYEDPORTAL") is a
   30-tick periodic event
   from frame 251, unrelated to waypoints (`*-tick.jsonl`).
2. Operate with the menu: 0x63 is the only message the operate function
   queues (recorded: c2s 0x13 then 0x63 with nothing between).
3. Same-act travel, in the drain: 0x07 MapReveal of the placement room
   (from the placement), then 0x0D. Next tick: room messages for the
   destination (0x07, 0x51, 0x08, 0x0A, units), including 0x51 for the
   destination waypoint object, then 0x15 ReassignPlayer at the
   player's position (recorded producer `0x0053BC10`; owner: the
   unit-update spec). The destination waypoint's mode comes from init 17
   (§5.1): 1 when the arrival node matched (recorded Cold Plains), 2 in
   town (recorded Rogue Encampment; open question 3).
4. Close (level 0): no message (recorded).
5. Quest-gated act travel is not a waypoint rule: a later act's town
   index is set only by the §4 table callers.

### 9. Town portals

Town portals do not use this code: they are objects 59/60 with operate
function 15 (`0x00584870`), which applies the same 10-second hostile
delay test as §6.1. Owner: the objects spec.

### 10. Object mode change (consequence used above)

`0x00624690(object, mode)` (owner: objects/units spec): unit type 5 or
null → nothing; same mode → only marks the unit for update; else mark
for update (unit +0xC4 |= 1), store the mode, run the anim setup
`0x00624390`, which for an object reads `FrameCnt[mode]`, `Start[mode]`
and `FrameDelta[mode]`, and for `Sync` = 0 draws once (Randomness).

## Constants & data dependencies

| Constant | Value | Where |
|---|---|---|
| record magic | 0x0102 (accepted on load: 0x0101, 0x0000) | `0x00660F30`, `0x00661030` |
| flag table size | 0x70 | `0x00746424` |
| no-waypoint index | 255 | `0x00660E00` |
| 0x49 range | 10 subtiles, sorceress (class 1) 22 | `0x00549570` |
| hostile delay | 10000 ms (`GetTickCount`) | `0x0054C5D0`, `0x00584870` |
| refusal sound | 0x13 | `0x0054C5D0` |
| tile code | 13 (towns 1, 40, 75, 103, 109 and 46, 74, 133–136), else 0 | `0x00584F60` |
| spawn offset | +3, +3 subtiles; 0x0D adds another +3, +3 | `0x0061B060`, `0x00584F60` |
| free-coordinate mask | 0x1C09 | `0x0061B060` |
| save section | "WS", 1, 0x50, 3 × 24 | `0x005693E0` |
| waypoint operate / init fn | 23 / 17 | `objects.txt`, tables `0x00732D18`, `0x00731BC0` |

`waypoints.tsv` (tab-separated, header row, one row per 1.14d index):
`wp` index, `level` id, `act` (0-based `Act`), `town` (1 for the §1
rule 4 levels), `tile_calc` (§7 rule 4), `level_name` (`LevelName`,
trimmed; informational). It is the expected result of deriving §1 and §7
rule 4 from the live tables: a game-file test (`#[ignore]`,
`D2_GAME_DIR`) compares the derivation with it; code never reads it as
data.

Tables read: `levels` (`Waypoint`, `Act`), leveldefs `Position`,
`objects` (`OperateFn`, `InitFn`, `FrameCnt1`, `FrameDelta1`, `Start1`,
`Sync`, mode columns), levels record count.

## Randomness

1. The waypoint functions themselves draw nothing: `0x0054C5D0`,
   `0x00549570`, `0x00584F60`'s own body, `0x00584E30`'s own body,
   `0x00547210`, `0x00660E00`–`0x006610B0` contain no draw (call-graph
   scan for the RNG helpers and the inlined multiplier).
2. Object mode change (operate step 2, init 17 → mode 1 or 2): the anim
   setup `0x00624390` draws once, `roll(FrameDelta[mode] >> 3)` on the
   **object's unit seed** (unit +0x20, `rng.md` §3, §5.3), only when
   `Sync` = 0; `roll(n < 1)` draws nothing. 1.14d: classes 429, 494, 496,
   511, 539 with `FrameDelta1` 200 → `roll(25)`; the result sets the
   animation speed (`FrameDelta + roll − FrameDelta>>4`, clamped 0..0x7FFF).
   Owner of that formula: the objects spec.
3. Travel draws only inside code owned elsewhere, in this order when it
   happens: level generation and room activation of the destination
   (`0x006424A0`, `0x0061B730` → `0x006422A0`: DRLG level/room seeds, only
   for a level or room not yet built/active), then the player mode request
   `0x005809D0` → `0x0057EEC0`, which can draw `roll(100)` on the player's
   unit seed under a state-0x2A condition (player-modes spec). The random
   tile branch `0x0066AC40` and the last fallback `0x0066AE70` are not
   reached for any 1.14d waypoint level (§7 rule 6).
4. Population of the destination rooms in the next tick draws as
   `tick.md` §4 describes.

## Edge cases & original bugs

Reproduced by default.

1. **Load wipe:** a saved record whose magic is 0x0000 or 0x0101 loads
   as all-clear except index 0, with magic 0x0000; the magic becomes
   0x0102 at the next 0x63 or save (§2 rules 5, 6). Any other magic
   rejects the save (§3).
2. **Activation before the menu:** the first operate of a neutral
   waypoint sets the bit and starts the animation but sends no 0x63; the
   player must operate again (mode 1 or 2) to get the menu.
3. **Arrival list:** init 17 frees the matching node and then sets the
   head to null, dropping (leaking until game end) every other node. A
   node whose destination waypoint object is not created through init 17
   stays in the list and can later match any waypoint object created in
   its room or rectangle; the room pointer is only compared, never
   dereferenced.
4. **0x49 without a menu:** any object GUID of the same act within
   10/22 subtiles passes validation; travel needs only the destination
   bit. A non-waypoint GUID returns 0 and does nothing.
5. **0x0D offset:** the arrival 0x0D carries the player's position + 3
   on both axes, while the following 0x15 carries the true position
   (recorded twice).
6. **Same level:** choosing the waypoint's own level closes the menu
   without travel (result 0).
7. **Busy:** with the interact info active (menu already open, NPC
   chat), operating any object does nothing (`0x00584420`), so the bit is
   not set either.
8. **Hostile delay** uses wall-clock `GetTickCount`; it is host-only and
   cannot fire in single player (§6.1).
9. **Index ≥ 0x70** in a modded `levels.txt` → fatal assert at the first
   bit test or set.
10. **Town portal delay differs:** operate function 15 (`0x00584870`)
    tests hostile time + 5000 ms (`0x005848C1`), not 10000 ms as §9 and
    the constants row say for it; the 10000 ms applies to the 0x49
    handler only. Owner of the portal rule: `world/objects.md` §12.

## Test vectors

Synthetic (CI). Record bytes are the 16 bytes at 0x63 offset 5.

| Input | Expected | Source |
|---|---|---|
| allocate | `0201 0100 0000 0000 0000 0000 0000 0000` | §2 rule 4 |
| set index 2 on allocate | `0201 0500 0000 …` | §2 |
| towns 0, 9, 18, 27, 30 | `0201 0102 0448 0000 0000 0000 0000 0000` | byte 2 + n/8 |
| all 0..38 | `0201 ffff ffff 7f00 0000 0000 0000 0000` | §2 |
| load copy of `0101 0300 …` | `0000 0100 0000 …`; next out copy `0201 0100 …` | edge case 1 |
| load copy of `0201 0300 …` | `0201 0300 …` | §2 rule 5 |
| save read, record magic 0x0103 | error 0x10 (`0x0056A3E0`) | §3 |
| 0x63, GUID 0x33, all 0..38 | `63 33000000` + `0201 ffff ffff 7f00 0000 0000 0000 0000` (21 bytes) | §5.3 |
| 0x49 level 0, object in act, \|dx\| = 10 | result 0, interact reset, no message | §6, §7 |
| 0x49, amazon \|dx\| = 11 | 1 | §6.2 step 3 |
| 0x49, sorceress \|dx\| = 22, level 3 known | travel | §6.2 |
| 0x49 level 2 (Blood Moor, no waypoint) | 3 | step 7 |
| 0x49 level 0x200 | 3 | step 5 |
| 0x49 level 4, index 2 clear | 2, interact reset if `wp` is the interact unit | step 8 |
| 0x49 object in another act | 2 | step 2 |
| 0x49 unknown GUID | 1 | step 1 |

Real (`#[ignore]`, recordings; frame = tick after which the message was
drained):

| Recording, frame | C→S | S→C, in order |
|---|---|---|
| `015956`, 1556 | `13 02000000 0b000000` | `63 0b000000 0201 0100 0000 0000 0000 0000 0000 0000` (town, index 0 only) |
| `015956`, 1631 | `49 0b000000 0000 0000` (close) | none; result 0 |
| `022633`, 108 | `13 02000000 0a000000` | `63 0a000000 0201 0300 0000 0000 0000 0000 0000 0000` |
| `022633`, 132 | `49 0a000000 0300 0000` (→ level 3) | `07 d003 e003 03`, `0d 00 01000000 01 2013 8413 00 00`; tick 133: … `51 02 17000000 7700 1e13 8213 01 00` (waypoint, mode 1) … `15 00 01000000 1d13 8113 01` |
| `022633`, 1269 | `13 02000000 33000000` | `63 33000000 0201 0300 0000 0000 0000 0000 0000 0000` |
| `022633`, 1291 | `49 33000000 0100 0000` (→ level 1) | `07 a803 8803 01`, `0d 00 01000000 01 5d12 b311 00 00`; tick 1292: … `51 02 49000000 7700 5b12 b111 02 00` (town waypoint, mode 2) … `15 00 01000000 5a12 b011 01` |

In both travels the player lands at (waypoint object x − 1, y − 1).
Save: the test character with Cold Plains has the section `5753
01000000 5000` + `0201 0300 0000…` (normal) + `0201 0100…` ×2 at offset
0x279; its 0x63 at frame 108 carries the same 16 bytes.

## Provenance

- 1.14d `Game.exe` (all.asm exports; tables read from the PE `.data`):
  0x49 handler `0x0054C5D0`, validator `0x00549570`, range test
  `0x00548EF0`, hostile time `0x0055B6C0`/`0x0055B720`, travel
  `0x00584F60`, operate fn 23 `0x00584E30` (operate table `0x00732D18`
  entry 23), init fn 17 `0x00547210` (init table `0x00731BC0` entry 17),
  arrival node `0x00547170`, operate dispatch `0x00584420`/`0x00584540`,
  interaction `0x00548B00`, interact info `0x00554100`/`0x00554120`/
  `0x00554190`/`0x00554D00`, busy `0x00535060`, record helpers
  `0x00660D90`, `0x00660E00`, `0x00660E50`, `0x00660EC0`, `0x00660F30`,
  `0x00661030`, `0x006610B0`, flag table `0x00746424`/`0x00746428`
  (dumped), save `0x005693E0`, `0x0056A3E0`, `0x00532C70`, warp
  `0x0053AEC0`, spawn `0x0061B060`, `0x0066B2B0`, `0x00619E50`, town
  switch `0x006426A0` (dumped), 0x63 sender `0x0053D960`, client
  `0x0045E670`, activation callers `0x00579D60`, `0x00584750`,
  `0x005B4FF0`/`0x005B5880`, object event table `0x006E19B0` entry 1 =
  `0x00581490`, mode change `0x00624690`/`0x00624390`.
- Live tables: `patch_d2` `levels.txt` (39 waypoint rows, `Position`,
  `Act`) and `objects.txt` (16 waypoint classes; operate fns 15, 46, 73
  rows), measured with a script; `waypoints.tsv` is that measurement.
- Recordings: `20261006-015956-packets.jsonl` (menu + close),
  `20261006-022633-packets.jsonl` (two menus, two same-act travels);
  `*-tick.jsonl` (event 11 schedule). Saves of the test characters
  (section bytes).
- D2MOO 1.10f (hint): `D2Waypoints.cpp` (record helpers),
  `ObjMode.cpp` (`OBJECTS_OperateFunction23_Waypoint`,
  `D2GAME_WAYPOINT_Unk_6FC79600`), `ObjRgn.cpp`
  (`OBJECTS_InitFunction17_Waypoint`, `OBJRGN_AllocObjectRoomCoords`),
  `PlrMsg.cpp` (`Rcv0x49`), `PlrSave2.cpp` (section), `Level.cpp`
  (`LEVEL_WarpUnit`), `DrlgDrlgWarp.cpp` (spawn search). Each rule above
  was re-read in the 1.14d code. Differences: the hostile-delay test
  moved from the operate function to the 0x49 handler; 1.14d splits the
  0x49 checks into `0x00549570` and runs the travel through `0x00584F60`
  (same checks, same order); tile code 13 also for levels 133–136
  (1.10f: towns, 46, 74); the flag table replaces the computed
  word/mask (same values). Same as 1.10f: the load-wipe bug, the list
  reset in init 17, the +3 in 0x0D.

## Open questions

1. Cross-act waypoint travel: message order (0x05/0x03/0x53 …) and
   whether §7 rule 7 sends 0x0D. Settle: record a 0x49 to another act.
2. First activation of a neutral waypoint: confirm no 0x63, the 0x0E/0x51
   mode messages and the ENDANIM frame. Settle: record operating a new
   waypoint twice.
3. Why the recorded town waypoint after travel is mode 2: init 17 with no
   arrival match, or a waypoint restored from inactive storage without
   init. Settle: trace `0x00547210` entry/result and the arrival list
   during a town arrival.
4. Object ENDANIM handler `0x00581490` (not in the exports): D2MOO sets
   mode 1 → 2 directly when `Mode2` ≠ 0, without the anim setup. Settle:
   Ghidra function at `0x00581490`.
   Answered (2026-10-07, `disasm.py at 0x581490`; `sim/units.md` §6.4
   event 1): null object → nothing; the u16 mode at +0x10 ≠ 1 →
   nothing; objects record byte +0x141 (`Mode2`) = 0 → nothing; else a
   direct dword write of 2 to +0x10 (`0x005814B0`; no `0x00624690`, no
   update queued, no animation setup, no draw), then, when record byte
   +0x122 (`HasCollision2`) = 0, the footprint is freed (`0x00623830`).
   Waypoint classes have `Mode2` = 1, so a waypoint in mode 1 is in mode
   2 after its ENDANIM fires.
5. Is player data +0x160 zero for a fresh player in 1.14d (allocation
   zeroes it)? Settle: read it in a running single-player game.
   Answered (2026-10-07): yes. Player data (0x16C bytes) is allocated
   and zeroed by `0x00621F90` (`_memset` at `0x00621FEC`; callers the
   player init `0x00534922` and the client `0x00460D0D`), and the only
   server write to +0x160 is `0x0055B76B` in `0x0055B720`
   (`GetTickCount`), reached only from `0x005A5F51` (hostility). The
   other `mov [reg + 0x160]` sites are client code (`0x00421E10`,
   `0x004B83A0`).
6. Multiplayer conversion of the 10 s wall-clock hostile delay to ticks
   (Phase 7 decision, not a fidelity fact).
7. Classic (non-expansion) games: nothing in the waypoint path blocks an
   act-5 index; whether a classic record can ever hold one. Settle: grep
   the save-load path for expansion masking.
   Answered (2026-10-07): the load path masks nothing. Both section
   readers (`0x0056A3E0`, legacy `0x00532C70`) check only "WS" and the
   three magics and pass each record to the load copy `0x00661030`,
   which copies all 16 bytes, applies only the magic rule (§2 rule 5)
   and sets bit 0; neither reads the game's or the character's
   expansion flag. So a classic character keeps every act-5 bit its
   file holds, and travel to such an index is then not blocked by the
   waypoint path (this question's premise). The in-game setters are the
   six callers of `0x00660EC0` (`0x0057A6B4`, `0x0057A739`, `0x0057A7BC`,
   `0x005847FC`, `0x00584E7C`, `0x005B501C`); whether a classic game can
   reach one with an act-5 index belongs to their owners.
