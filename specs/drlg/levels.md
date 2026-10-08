# Spec: DRLG — Acts, levels, level seeds and warps

- **Status:** draft (no `d2-sim` code yet); every rule read from the 1.14d
  `Game.exe` code; the seed rules are confirmed on the recording
  `traces/raw/20261005-232125-rng.jsonl` (Act 1 entry, single player:
  DRLG seed, start seed, 21 level seeds and 47 room seeds of the server
  build and the same values in the client's copy).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::drlg::level` (act DRLG, level list, level
  seeds, vis/warp records, spawn rooms, logical rooms and the population
  queries of §11)
- **Related specs:** `monsters/population.md` (reads §11);
  `drlg/rooms.md` (DRLG rooms, rooms-near order,
  activation, active rooms); `drlg/preset.md`, `drlg/maze.md`,
  `drlg/outdoor.md` (what each level type generates, and the act-wide
  placement of outdoor levels); `sim/rng.md` (generator, seed derivation,
  §5.4 DRLG seeds); `sim/tick.md` (steps 3, 9, 10 that drive the room and
  level lifecycle); `sim/unit-order.md` (act room list).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 47–62 |
| Inputs | 63–72 |
| Outputs / state changes | 73–78 |
| Rules | 79–80 |
|   1. Structures (1.14d layout, for recorders and checks) | 81–107 |
|   2. Act creation (server) | 108–124 |
|   3. DRLG creation (`0x00642DA0`) | 125–159 |
|   4. Level list, get-or-allocate | 160–177 |
|   5. Level generation (`0x006424A0`, D2MOO `DRLG_InitLevel`) | 178–209 |
|   6. Level position, size, act number | 210–226 |
|   7. Vis and warp records | 227–250 |
|   8. Coordinates to rooms | 251–264 |
|   9. Level lifecycle: activity and freeing | 265–313 |
|   10. Spawn room in a level (`0x0066B2B0`) | 314–376 |
|   11. Logical rooms (coordinate lists) and population queries | 377–656 |
|   12. Level connections: Act I, Act III, Act V | 657–1009 |
| Constants & data dependencies | 1010–1030 |
| Randomness | 1031–1049 |
| Edge cases & original bugs | 1050–1074 |
| Test vectors | 1075–1154 |
| Provenance | 1155–1216 |
| Open questions | 1217–1305 |
<!-- /index -->

## Summary

Each act of a game has one DRLG ("dungeon random level generator"): a
seed, the act's levels and the per-level vis/warp records. The server
creates an act the first time it is needed. Creating it seeds the DRLG
from the game's map seed, draws the start seed, draws the act's fixed
choices (Act 2 tombs, Act 3 jungle link), lets the outdoor placer create
and position the act's levels (`drlg/outdoor.md`), then generates the
town. Every level's seed is `init_low(start seed + level id)` and is
re-initialized on every (re)generation, so a level is identical each time
it is built. Levels are generated on demand (a room is looked up in them,
a warp leads into them) and freed again when no client has been near them
for a while; the rooms of a regenerated level remember whether they were
populated. This spec owns the act and level lifecycle, the level seeds,
vis/warp records, coordinate lookups and the spawn-room choice.

## Inputs

| Name | Type | Source |
|---|---|---|
| act number | 0..4 | the game (first need of an act) |
| init seed | u32 | game +0x7C at act creation; single player: the character's map ID (`rng.md` §5.4, OQ 2) |
| difficulty | u8 | game +0x6D |
| level tables | `levels`/`leveldefs`, `lvlwarp` | `data/loading.md` (leveldefs columns: `data/fields.tsv`) |
| client mode | flag | server DRLG: 0; client copy: 1 (`DRLGFLAG_ONCLIENT`) |

## Outputs / state changes

The act's DRLG (seed, start seed, act choices), its level list, each
level's seed, size, position, rooms (built by the type specs) and warp
data; the drlg seed and level seeds advance as listed under Randomness.

## Rules

### 1. Structures (1.14d layout, for recorders and checks)

| Struct | Size | Field | Offset |
|---|---|---|---|
| act | 0x60 | environment / drlg / init seed / act no / town level id (server) | +0x04 / +0x48 / +0x0C / +0x14 / +0x08 |
| act | | active-room list head; pending-room flag; client flag; memory pool; room callback | +0x10; +0x54; +0x50; +0x5C; +0x4C |
| drlg | 0x48C | DRLG seed (lo, hi) | +0x00, +0x04 |
| drlg | | rooms allocated (stat); rooms built since last client update | +0x08; +0x98 (u8) |
| drlg | | flags (bit 0 = on client); game | +0x8C; +0x9C |
| drlg | | Act 2 staff tomb level; Act 2 boss tomb level; Act 3 jungle-link bit | +0x94; +0x484; +0x474 |
| drlg | | room status lists 0..3 (list head = a dummy room, stride 0xEC) | +0xA0 |
| drlg | | difficulty (u8); automap callbacks; init seed copy | +0x450; +0x454, +0x488; +0x458 |
| drlg | | start seed (`dwStartSeed`); vis/warp record list; level list head | +0x470; +0x90; +0x47C |
| drlg | | memory pool; act no (u8); act | +0x478; +0x480; +0x46C |
| level | 0x230 | DRLG type (1 maze, 2 preset, 3 outdoor); flags (0x10 = automap reveal) | +0x00; +0x04 |
| level | | room count (+1 per room linked at the list head, `0x0066B970`; −1 per room freed, `0x0066C100`; 0 when the rooms are freed, `0x00642010`, which also sizes +0x22C by it); first room; activity count; inactive frames | +0x08; +0x10; +0x0C; +0x1D4 |
| level | | position x, y; width, height (tiles) | +0x1C, +0x20; +0x24, +0x28 |
| level | | type info (DrlgType 2: preset info, whose +0x04 is the preset direction, u32, written by the act link driver at `0x00677490` for levels 1 and 40 and `0x006774F9` for 27, `drlg/outdoor.md` §2.3 step 4) | +0x14 |
| level | | spawn-tile records (x, y, tile index; stride 12); count | +0x2C; +0x1D8 |
| level | | next level; drlg; level type; level seed (lo, hi); level id | +0x1AC; +0x1B4; +0x1C0; +0x1C4, +0x1C8; +0x1D0 |
| level | | warp-room centres x[9], y[9]; count; populated-room memory | +0x1E0, +0x204; +0x228; +0x22C |
| level | | coordinate-list counter (§11.2) | +0x1DC |
| level | | Act III jungle clearing count; jungle block ids (`drlg/outdoor-act3-act5.md` §2.8) | +0x1B8; +0x1BC |
| vis/warp record | 0x48 | level id; vis[8]; warp[8]; next | +0x00; +0x04; +0x24; +0x44 |

Coordinates are in tiles; subtile = tile × 5 (`0x00643560`).

### 2. Act creation (server)

1. An act is created when the server first needs it: game creation
   path `0x0052C210` and the act change path `0x0053ACC0` (via
   `0x0053AEC0`), both through `0x0053AC70`; the act pointer is stored at
   game +0xBC + 4·act. Acts are never created eagerly.
2. `0x0053AC70` passes: act number, init seed = game +0x7C, difficulty
   game +0x6D, the town level id (table `0x006E7D1C`: 1, 40, 75, 103, 109;
   in an arena game the arena's level instead), no automap callbacks.
3. `0x006194A0` allocates the act (0x60 bytes, zeroed), stores the init
   seed, act number, client flag and pool; server: stores the town level
   id and creates the DRLG with that town id and flags 0; client copy:
   town id 0 and flags 1. Then the environment (`0x0061BE40`, day/night,
   not DRLG) and the act's edge floor record at act +0x18 (`0x00642A30`:
   first tile of a fixed key from the base tile library, no draw;
   `render/draw-order-2.md` OQ 2).

### 3. DRLG creation (`0x00642DA0`)

In order:

1. Allocate 0x48C bytes, zeroed; store pool, act number, act, flags,
   game, difficulty, callbacks, init seed copy.
2. DRLG seed := `init_low(init seed)` → `{init seed, 666}`.
3. **Draw 1 (DRLG seed):** one step; `dwStartSeed` := `lo'`.
4. Act-specific draws on the DRLG seed (act number 0-based):
   - act 1 (Act II): repeat { `a` := next step `lo' mod 7`; `b` := next
     step `lo' mod 7` } while `a == b`. Staff tomb := 66 + `a` (+0x94),
     boss tomb := 66 + `b` (+0x484). Levels 66..72 are Tal Rasha's
     tombs 1..7. Two draws per try.
   - act 2 (Act III): one step; jungle-link bit := `lo' & 1` (+0x474),
     read by the jungle placer (`drlg/outdoor.md`).
   - acts 0, 3, 4: no draw here.
5. Load the act's base tile library (`0x00641F60`: Act 1
   `Tiles\Act1\Town\Floor.dt1`, Act 2 `Tiles\Act2\Town\Ground.dt1`,
   Act 3 `Tiles\ACT3\Kurast\sets.dt1`; acts 4–5 none). No draw.
6. Initialize the four room status lists (`0x0061B7E0`, `drlg/rooms.md`
   §5).
7. Create and place the act's levels: `0x00678AD0(drlg, act)`
   (D2MOO `DRLGOUTPLACE_CreateLevelConnections`, owner `drlg/outdoor.md`).
   It allocates levels (§4, each allocation seeds the level). Its link
   checks draw from a **copy** of the DRLG seed (Act 1 sites `0x00676165`,
   `0x00676469`, `0x0067649F`, `0x00676669`, `0x0067669F`: the copy
   restarts from the current DRLG seed, so seq 2430–2431 repeat 2419–2420);
   only `0x006774C0`/`0x006774DB` (Act 1) and the Act III jungle placer
   advance the DRLG seed itself (`drlg/outdoor.md`).
8. Server only (town id ≠ 0): get-or-allocate the town level (§4) and
   generate it (§5).

D2MOO 1.10f loads the tile library inside the act switch, before the
status lists; 1.14d moved it to step 5. No outcome differs (no draws).

### 4. Level list, get-or-allocate

1. Levels of a DRLG form a singly linked list (head drlg +0x47C, next
   level +0x1AC). A new level is **prepended**.
2. Get-or-allocate (`0x00642BB0`, inlined in many callers): walk the list
   for the level id; if absent, allocate it (§4.3). Every lookup by id in
   the DRLG uses it, so merely asking for a level (vis arrays, activity
   counts, dependencies) allocates it. Allocation never generates rooms.
3. **Allocate** (`0x00642AE0`): 0x230 bytes zeroed; drlg, level id;
   level type := leveldefs `LevelType` (+0x34), DRLG type := leveldefs
   `DrlgType` (+0x30); if the DRLG is a client copy, level flag 0x10;
   level seed := `init_low(dwStartSeed + level id)` (u32 wrap, no draw);
   then the type's level-data init: maze `0x00673B10`, preset
   `0x00667430` (draws: `drlg/preset.md`), outdoor `0x00675320`; then
   prepend.
4. Leveldefs `DrlgType` 0 (level 0 "Null") or any other value: no type
   init, no generation (D2MOO asserts; 1.14d silently does nothing).

### 5. Level generation (`0x006424A0`, D2MOO `DRLG_InitLevel`)

1. Level seed := `init_low(dwStartSeed + level id)` again. Every
   generation of a level therefore starts from the same seed, and its
   rooms get the same seeds in the same order (`drlg/rooms.md` §2).
2. Dispatch by DRLG type: maze `0x00673B30` (`drlg/maze.md`), preset
   `0x00668100` (`drlg/preset.md`), outdoor `0x00675360`
   (`drlg/outdoor.md`).
3. **Populated-room memory** (`0x00642390`): if the level has rooms and a
   memory array (+0x22C, §9), the i-th room of the level list (from the
   head) gets room "other flags" (+0x60) bit 0 set when entry i ≠ 0.
4. **Warp-room centres** (`0x006423D0`): walk the level's rooms (list
   order); a room qualifies if it has a waypoint flag (room flags
   0x30000) or a warp flag bit `0x10 << i` (i = 0..7) whose warp id
   (§7) is not −1. For each qualifying room append
   `((x + w/2)·5, (y + h/2)·5)` (tile rect of the room, integer division,
   then subtile scale) to +0x1E0/+0x204 and increment +0x228. No bound
   check (9 slots). The slot addresses are taken from the count before
   writing, the tile centre is written, then both slots are scaled ×5 in
   place, then the count is incremented. A 10th qualifying room (i = 9)
   therefore writes its x into y[0] (+0x204) and its y into the count
   (+0x228): afterwards y[0] = 5·(x + w/2) of that room and count =
   5·(y + h/2) + 1, and an 11th qualifying room writes past the level
   record (+0x1E0 + 4·count). Readers (§11.5) loop to that count.
   Reproduce up to the 10th room; an 11th is out of scope (memory
   corruption; open question 6).
5. A level is generated on demand: when a room is looked up in a level
   without rooms (§8.1, §10), when a warp link needs the target level's
   rooms (`drlg/rooms.md` §3.3), and at DRLG creation for the server's
   town. Generation draws only from the level seed and the new rooms'
   seeds (type specs, `drlg/rooms.md` §2).

### 6. Level position, size, act number

1. Size and position (`0x00642D10`, D2MOO `DRLG_SetLevelPositionAndSize`;
   tail of the maze and preset level-data inits `0x00673B10`, `0x00667430`,
   i.e. at allocation, after the preset file draw): width := leveldefs `SizeX[difficulty]`,
   height := `SizeY[difficulty]`; if `Depend` ≠ 0, get-or-allocate that
   level and add its position; position := (depend x, y or 0) +
   (`OffsetX`, `OffsetY`). Outdoor levels are positioned by
   `drlg/outdoor.md` instead.
2. After a maze level's rooms are built, the maze generator
   (`0x00673B30`) shifts every room so the rooms' bounding box starts at
   the level position (`0x00642590`; fatal error if the box exceeds the
   level size).
3. Act of a level id (`0x006427F0`): the first `a` in 0..4 with
   `id < T[a+1]`, T = [1, 40, 75, 103, 109, 1024] (`0x006EB2F0`);
   ids ≥ 1024 → 0. Town test (`0x006426A0`): ids 1, 40, 75, 103, 109.

### 7. Vis and warp records

1. Each level has 8 vis slots (neighbouring level ids, 0 = none) and 8
   warp slots (lvlwarp `Id`, −1 = none), from leveldefs `Vis0..7`
   (+0x48) and `Warp0..7` (+0x68).
2. A DRLG keeps editable copies: the record list at drlg +0x90 (prepend).
   `0x00642860` (D2MOO `DRLG_GetDrlgWarpFromLevelId`) returns the record
   for a level, creating it from leveldefs if absent. Readers
   (`0x0066C040` vis array, `0x0066AEC0` warp array) return the record's
   arrays if one exists, else leveldefs' arrays; a record with level id 0
   is a fatal error.
3. `0x00642920` (set warp; D2MOO `DRLG_SetWarpId`, used by the outdoor
   placer): if some slot has vis = V, set its warp := W and stop. Else if
   the requested slot is −1, take the first slot with vis = 0 and warp =
   −1; write vis[slot] := V, warp[slot] := W.
4. Warp id of slot i of a level: `0x0066AF30`. lvlwarp record of a slot
   (`0x0066AF50` → `0x0061F310`): the first lvlwarp row (file order) whose
   `Id` equals the slot's warp id and whose `Direction` matches: the
   request is `'b'`, or the row's direction is `'b'`, or equal. No row is a
   fatal error. Room warp links always request `'b'`
   (`drlg/rooms.md` §3.3).
5. Warp flags on rooms (room flags bit `0x10 << i`) mean "this room holds
   the exit toward vis slot i"; the type specs set them.

### 8. Coordinates to rooms

1. Room at (x, y) (`0x00642C30`, D2MOO `DRLG_GetRoomExFromCoordinates`;
   arguments: drlg, hint room, level or none): if a hint room is given and
   contains the point, it; else the first room of the hint's rooms-near
   array (array order, skipping the hint) that contains it. Else the level:
   the given one, or the first level of the DRLG list whose rect contains
   the point (`0x006427A0`), or level id 0 if none (get-or-allocate,
   i.e. a "Null" level is allocated); generate it if it has no rooms
   (§5); return the first room of its list containing the point
   (`0x00642630`), or none.
2. Containment (`0x0066B980`): `x0 ≤ x < x0 + w` and `y0 ≤ y < y0 + h`
   (half-open; `0x0066B9D0` is the closed variant).

### 9. Level lifecycle: activity and freeing

1. **Activity counts** (`0x00643020`, called when a client changes room,
   `0x00537B50`, and when it leaves, `0x00539DA0`): if the old and new
   rooms are in different levels: old level and each of its 8 vis levels
   (get-or-allocate): count −1; then new level and its vis levels: count
   +1. Every touched level: inactive frames (+0x1D4) := 10, count
   clamped to ≥ 0 (`0x00642F70`). Vis slots with id 0 are skipped.
2. **Free inactive levels** (tick step 10, every 11 frames:
   `0x0061AA20` → `0x00643200`; D2MOO `DRLG_UpdateAndFreeInactiveRooms`):
   for each level in list order with count 0 and rooms: if inactive
   frames > 0, decrement; else run the free test; on failure frames := 10,
   on success free the level's rooms (§9.4).
3. **Free test** (`0x00643060`): fails if any room of the level has a
   status < 4 or an active room (room flag 0x100000). Then for each vis
   slot 0..7 with a level id L (get-or-allocate): if L has rooms, build
   the mask of warp-flag bits `0x10 << j` for every slot j of L's vis
   array equal to this level; fail if any room of L carrying one of those
   bits has a status < 4 or flag 0x100000; else free those rooms' warp
   links (`0x0066B4F0`) and rooms-near arrays (they are rebuilt on next
   use). After all 8 slots: success.
   - The mask depends only on L's vis ids, never on warp ids: slots
     with warp −1 (outdoor neighbours) are included. Their border rooms
     carry those warp-flag bits and are the only rooms of L whose
     rooms-near arrays can hold rooms of this level (`drlg/rooms.md`
     §3 rule 3, W = −1 branch), so after a successful test no room of
     L keeps a near entry or warp link into this level.
   - Per slot the order is: test every room of L, then free. A refusal
     at slot k returns after slots < k have already freed their L
     rooms' warp links and near arrays (the level is not freed; those
     arrays are rebuilt on next use).
   - Per freed room: warp links always (`0x0066B4F0`, list +0x4C := 0);
     the near array only when non-null (pointer +0x08 and count +0x2C
     := 0).
   - Only this level's vis slots are walked: a level L that lists this
     level in its vis array while this level does not list L is not
     tested and keeps its arrays.
   - A missing L is allocated (get-or-allocate inlined); L without
     rooms is skipped.
4. **Free rooms, keep level** (`0x00642010` with keep = 1): size the
   populated-room memory (+0x22C) to the room count (allocate once),
   store each room's "other flags" bit 0 in list order, free every room
   (`0x0066C100`), free the preset map (+0x1B0), reset the type data
   (maze `0x00673FE0`, preset `0x006674D0`, outdoor `0x006754C0`), clear
   spawn-tile records and count, warp-room centres and count, free the
   build list (+0x1CC). The level stays in the list with its seed field;
   the next generation re-seeds it (§5.1) and restores bit 0 (§5.3).
5. Freeing never draws. Whole-DRLG free at game end: `0x00642190`.

### 10. Spawn room in a level (`0x0066B2B0`)

Used when a unit enters a level without a fixed position (waypoint,
town arrival, act change: D2MOO `DUNGEON_FindActSpawnLocation`).
Arguments: ECX the act's DRLG record, EDX level id; stack tile index,
&x, &y. Entry points: `0x0061B060` (`sim/path-placement.md` §11) and
`0x00619E50`(act, level, tile index, &x, &y; callers `0x0054DBE9`,
`0x00585094`), which only asserts the act (null → fatal) and passes its
DRLG (act +0x48): both are this one search, with the same draws.

1. Get-or-allocate the level; generate it if it has no rooms.
2. If leveldefs `Position` (+0x90) ≠ 0 (levels 1, 38, 40, 46, 54, 73,
   74, 75, 102, 103, 109, 121, 125, 126, 127, 132, 134, 135):
   - tile index 13: the waypoint room (§10.4);
   - else `0x0066AC40`: count spawn-tile records matching the requested
     index `t` (record index = `t`, or table `0x006EED88[t].a ≠ 0` and
     `.b` equal for both indexes; table a/b pairs for t = 0..13: (1,0),
     (0,0)×4, (1,1), (0,1)×4, (0,2), (0,3), (0,4), (0,5)); if the count
     n > 0, **draw `roll(n)` on the level seed** (power-of-two masks
     included) and take the r+1-th match (r = result); n = 0: no draw,
     record 0. Position := that record; room := room at it (§8.1).
   - **Class rule**, spelled out from the table: a record with index e
     matches request t when e = t, or a[t] = 1 and b[e] = b[t]. Only
     t = 0 and t = 5 have a = 1, so request 0 matches records 0–4,
     request 5 matches records 5–9, and every other request (1–4, 6–13)
     matches only its own index. b[e] is read for the record's index e
     (`0x006EED8C + 8e`), so a record index above 13 reads past the
     table; such a record still matches its own index exactly.
   - Pick walk: k := r + 1 (0 when n = 0); records are visited from
     index 0; each match decrements k; the walk stops at the first record
     where k ≤ 0 after the test (with n = 0 that is record 0). Position
     := (+0x2C + 12i, +0x30 + 12i) of the stopping record i, room := room
     at it (`0x00642C30` with this level, no hint).
3. Else: the waypoint room (§10.4); else the first room (list order)
   with a warp flag whose warp id ≠ −1 (`0x0066B1F0`); else the room
   containing (posX + w/2 − 2, posY + h/2 − 2); else
   **`roll(room count)` on the level seed** (`0x0066AE70`, helper
   `0x0045C3E0`) and walk that many rooms from the list head.
4. Waypoint room (`0x0066AD80`): the first room with a waypoint flag;
   its preset units are built (`drlg/rooms.md` §4.5); the position is the
   first preset object (type 2) with class < 573 whose objects.txt
   subclass has bit 0x40 (waypoint): (room x + px / 5, room y + py / 5).
5. When no position was set, it is the room's centre (x + w/2, y + h/2).
   The chosen room is made active (`drlg/rooms.md` §4.5).
6. The result is the chosen room's +0x30; (x, y) start at (−1, −1)
   (`0x0066B2B0`). Rule 5's centre default runs only on the rule 3
   path. On the `Position` ≠ 0 path (rule 2) there is no default and no
   null test:
   - Tile index 13 with no waypoint room: `0x0066AD80` returns null, and
     the activation `0x0061B730` reads null +0x28. 1.14d crashes there
     (access violation, no error code).
   - A waypoint room without a waypoint object returns that room with
     (x, y) left at (−1, −1).
   - A spawn-tile record whose position is in no room (`0x00642C30`
     null) crashes the same way.
   d2rs reports each crash as a fatal error (`NoWaypointRoom` for the
   first). It never substitutes a room.
7. Rule 3 with every fallback failing (no waypoint room, no warp room,
   no room at the centre point, and `0x0066AE70` returns null, e.g. a
   level with no rooms): the function returns 0 with (x, y) = (−1,
   −1). Nothing is activated, and there is no draw beyond those of rule
   3.

### 11. Logical rooms (coordinate lists) and population queries

Owner of the DRLG data the monster population reads
(`monsters/population.md` §3, §6, §9): coordinate lists, the populated
level, the populated-room count, warp points and the kind-11 spawn
location; §11.6 maps every population read to its owner. D2MOO names: `DrlgDrlgLogic.cpp` (`D2DrlgLogicalRoomInfoStrc`,
`D2RoomCoordListStrc`) and the `DUNGEON_*` wrappers. Every rule below
is read from the 1.14d functions named; D2MOO matches in structure and
tables, differences are noted.

#### 11.1 Structures

Logical-room info (0x34 bytes, DRLG room +0x64, `.\DRLG\DrlgLogic.cpp`):

| Offset | Field |
|---|---|
| +0x00 | flags: 1 = one record for the whole room (§11.2), 2 = built from grids (§11.3) |
| +0x04 | number of records allocated as one array (§11.3 step 7) |
| +0x08 | index grid, (W+1) × (H+1) u32 (grid layout as `drlg/outdoor.md`), flag 2 only |
| +0x1C | record grid: a coordinate-record pointer per tile, flag 2 only |
| +0x30 | first coordinate record |

Coordinate record (0x30 bytes):

| Offset | Field |
|---|---|
| +0x00..+0x0C | box: x0, y0, x1, y1 (level tiles, x1/y1 exclusive) |
| +0x10..+0x1C | clipped box: x0, y0, x1, y1 (what population reads) |
| +0x20 | node flag (0/1) |
| +0x24 | not written by this code (0) |
| +0x28 | index |
| +0x2C | next |

Level +0x1DC is the level's coordinate-list counter (zero at
allocation; §9.4 does not reset it).

#### 11.2 When the lists are built

At the end of a room's tile fill (`drlg/rooms.md` §9.5), after the fill
pass, the animation step and the record-count freeze:

1. Preset room (type 2, `0x00666AC0` at `0x00666DAF`) whose lvlprest
   `Logicals` is non-zero: the grid build (§11.3, `0x0066D110`) from the
   room's wall layer 0 orientation grid, floor layer 0 grid and wall
   layer 0 grid (preset room data +0x60, +0xB0, +0x10). 446 of the 1,091
   1.14d lvlprest rows have `Logicals` 1 (patch_d2 `lvlprest.txt`).
2. Every other preset room, and every outdoor-grid room (type 1, at the
   end of `0x0067D710`): one record (`0x0066CCB0`): info flag 1, info
   +0x04 = 1; **level counter := 1** (always, overwriting it); one zeroed
   record with index 1 (the counter), node 0, box and clipped box both
   the room's tile rect (x, y, x + w, y + h).
3. Rooms of other types get no info; the readers of §11.4 are then fatal
   (error 0x2CD / 0x29C). No 1.14d room reaches this.

Building draws nothing. Freeing the room tiles (`drlg/rooms.md` §9.2,
`0x0066F1A0`) and freeing the room release the info, its grids and every
reachable record (`0x0066C6E0`); the next build makes new lists with new
indexes from the current level counter.

#### 11.3 Grid build (`0x0066D110`)

With room tile rect (X, Y, W, H), cells (x, y) for 0 ≤ x ≤ W,
0 ≤ y ≤ H (the shared far edge included):

1. Allocate the info (flag 2) and a zeroed index grid.
2. Tree marks (`0x0066C7F0`): every wall record whose flags have 0x4
   and no layer bits (0x1C000 clear) ORs 8 into the room's wall layer 0
   cell. Wall records always carry layer bits (`drlg/rooms.md` §9.5
   record flags), so this changes nothing in 1.14d (D2MOO: same test).
3. If the level counter is 0, set it to 1. `start` := counter.
4. **Blocker grid** B (local, zeroed; `0x0066C870`): mark 1 at every
   cell holding a wall record of this room with layer bits exactly
   0x4000 (layer 0), type ≠ 15 (roof) and no flag 0x800 (object wall).
   Then for each room N of the rooms-near list (array order), N ≠ this
   room, N with a tile grid: for every non-floor link list node of N
   (`drlg/rooms.md` §9.6), every record of its chain with the same
   test: if (N.x + rec x, N.y + rec y) is inside or on the border of
   this room's rect (`0x0066B9D0`), mark B at that point minus (X, Y).
   Layout (`0x0066D1A5`–`0x0066D1C3`, grid init `0x0067CBF0`): B is a
   (W+1) × (H+1) grid, row-major, cell (x, y) at index y·(W+1) + x, row
   offsets r·(W+1); cells and row offsets are fixed stack buffers of
   1,024 and 256 entries, zeroed for (W+1)(H+1) cells. Every point marked
   lies inside or on the border of the rect, so 0 ≤ x ≤ W, 0 ≤ y ≤ H. The
   buffers never overflow in 1.14d: grid-built rooms are preset rooms,
   at most 8 × 8 tiles in multi-room mode and at most 12 × 12 in the
   maze's single-room mode (`drlg/maze.md` §9 step 3), so at most 169
   cells and 13 rows.
5. **Regions** (`0x0066C580`): for y = 0..H (outer), x = 0..W (inner):
   if the index cell lacks 0x10000000: counter += 1; current mark M :=
   (counter & 0x0FFFFFFF) | 0x10000000; if the floor layer 0 cell v has
   (v & 0x01E0FF00) = 0x01E00000 (main index bits 21–24 all set, sub
   index 0: keys (30, 0), (31, 0), (62, 0), (63, 0)) or bit 31 (hidden):
   M |= 0x20000000 (node); then fill (x, y, direction −1). The counter
   is incremented even when the fill marks nothing (a blocked start
   cell whose rule lacks bit 1). D2MOO tests main index 30 exactly.
6. **Fill** (`0x0066C3D0`, recursive; directions 0 = +x, 1 = +y, 2 = −x,
   3 = −y, offsets `0x006EEE14`): loop:
   1. Stop unless (X + x, Y + y) is inside or on the border of the room
      rect; stop if the index cell already has 0x10000000.
   2. B(x, y) = 0: OR M into the index cell, fill the four neighbours
      in direction order 0, 1, 2, 3 (each with its direction), stop.
   3. Else with o = the orientation cell (0 when that grid is empty,
      `0x0067C480`): rule R = T2[d + 5·T1[o] + 1] with T1 = `0x006EEEA0`
      (20 entries by orientation: −1, 0, 1, 2, 2, 0, 1, 3, 0, 1, 0, 1,
      4, −1, 4, 0, 0, 0, 0, 0) and T2 = `0x006EEE38` (26 entries: 23,
      0, 5, 21, 17, 15, 3, 0, 9, 7, 39, 0, 0, 5, 3, then 31 × 10, then
      0). For T1 = −1 (orientations 0 and 13) the index falls before T2:
      R = 0xFFFFFFFF for d = −1 and d = 2, R = 0 for d = 0, 1, 3 (the
      dwords at `0x006EEE24`–`0x006EEE34`: the end of the offset table
      and one zero dword).
      Orientations above 19 read past T1 into the source-file name
      string at `0x006EEEF0` (".\DRLG\DrlgLogic.cpp"), whose dwords give
      a T2 index far outside the image: not reproducible. Not reached:
      no wall layer of any lvlprest DS1 holds an orientation above 19
      (measured over the 2,058 `File1`–`File6` references of
      `lvlprest.txt`, 872 of them in rows with `Logicals` 1, d2exp copies
      over d2data, version < 7 values mapped by `drlg/preset.md` §5.2
      step 6; the `Patch_D2.mpq` overrides are `preset.md` OQ 5).
   4. R & 1: OR M into the cell. R & 2 and d ≠ 2: fill (x+1, y, 0).
      R & 4 and d ≠ 3: fill (x, y+1, 1). R & 8 and d ≠ 0: fill (x−1, y,
      2). R & 16 and d ≠ 1: fill (x, y−1, 3).
   5. R & 32: x += 1, y += 1, d := −1, repeat from 1; else stop.
7. Records: info +0x04 := counter − start + 1; allocate that many
   zeroed records as **one array** at info +0x30; level counter +=
   info +0x04.
8. **Rectangles** (`0x0066CA50`): a zeroed record grid; for y = 0..H
   (outer), x = 0..W (inner), a cell whose record-grid entry is empty
   starts a new record: index := index cell & 0x0FFFFFFF, node := index
   cell has 0x20000000; **prepended** to the info list (+0x30). Width:
   from x rightwards while the cell has the same index and no record;
   height: from y downwards while every cell of [x, x1) in the row has
   the same index and no record. Every cell of the box gets the record
   in the record grid (overwrite). Box := (x + X, y + Y, x1 + X,
   y1 + Y); clipped box := box with x1 ≤ X + W and y1 ≤ Y + H; if
   x0 ≥ X + W or y0 ≥ Y + H the clipped box is all zero.
9. Wall records (`0x0066C9C0`): each wall record's +0x10 := the record
   grid entry at its cell (0 in one-record rooms). Read by drawing code,
   not by the simulation.
10. **Merge with neighbours** (`0x0066D040`): for each room N of the
    rooms-near list (array order), N ≠ this room, N with info: if the
    two rects overlap or touch (gap test of `drlg/outdoor.md` §2.6 with
    margin 1: runs when both gaps are < 1): for x = X..X+W the cells
    (x, Y) and (x, Y+H); then for y = Y..Y+H the cells (X, y) and
    (X+W, y). Per cell (`0x0066CF60`, tile coordinates): skip unless
    inside or on the border of N's rect; take N's record and this room's
    record at the cell (one-record rooms: their only record); if both
    indexes are non-zero, the two rooms' levels have the same id, the
    indexes differ and the node flags are equal: **rename** this room's
    index to N's.
11. Rename (`0x0066C770`; room, old, new): only rooms with grid-built
    info: every record with index old gets new; if any did, recurse into
    every rooms-near entry (array order) ≠ the room whose level has the
    same id, with the same old and new. One-record rooms are never
    renamed.

Consequences (reproduce them):

- The array of step 7 stays at the **end** of the list: its first
  element (index 0, node 0, both boxes zero, next 0) is the last record
  of every grid-built room (D2MOO: same). Its other elements are never
  linked.
- List order: the reverse of the cell scan of step 8, then that zeroed
  record.
- Cells no fill reached keep index 0 and form index-0 records.
- Indexes depend on the order rooms are built (activation order) and on
  one-record rooms resetting the counter (§11.2 step 2); readers only
  compare them (§11.4).
- No bound check on the local blocker grid (1,024 cells, 256 rows) or on
  orientation values above 19; neither is reached by 1.14d data (steps
  4 and 6).

#### 11.4 Lookups

| 1.14d | Arguments | Result |
|---|---|---|
| `0x0061AD50` (`0x0066CF30`) | active room | first record of its DRLG room's info; fatal 0x2CD without info |
| `0x0061AD30` (`0x0066CEB0`) | active room, subtile x, y | one-record room: its record; else the record grid at (x/5 − X, y/5 − Y) (C division, toward zero) |
| `0x0061B130` (`0x0066CE30`) | active room, subtile x, y | the room containing (x, y) among the room and its adjacency array (`0x00463740`); none → **0**; else that room's record at the point as above → its index; null record → −1 |

No bound check: a point outside the room's (W+1) × (H+1) cells reads
outside the grid. The record grid is one block (`0x0067CB80`, from
`0x0066CA50`): H+1 row offsets r·(W+1), then (W+1)(H+1) cell pointers.
For a point with cell (cx, cy) = (x/5 − X, y/5 − Y):

| Case | 1.14d read |
|---|---|
| 0 ≤ cy ≤ H and 0 ≤ cy·(W+1) + cx < (W+1)(H+1) | that cell of the block: cx outside 0..W wraps into the row before or after (a real record) |
| 0 ≤ cy ≤ H, −(H+1) ≤ cy·(W+1) + cx < 0 | a row-offset integer used as a record pointer (garbage) |
| any other | memory outside the block (not reproducible) |

Who can pass such a point: `0x0061B130` never (it first finds the room
whose sub-tile rect contains the point, `0x00463740`, so 0 ≤ cx < W, 0 ≤
cy < H); `0x0061AD30` from `0x0054E0ED` (pack at a preset point, the
preset unit's own room, `monsters/population.md` §11.5), `0x005B1302`,
`0x005E3CE2` and `0x005E441D` (a unit's own room and position) stay in
the grid. Only `0x005B2E86` in `0x005B2A00` (spawn with spread,
`missiles/bodies-2.md`) passes its start room with the found spawn point,
which the spread search may take from a neighbouring room (the
`0x0061B130` test there compares only the logical index): that read
can leave the grid. Reproduce the first table row; for the other two
d2rs returns no record (a choice, the original's value is not
reproducible).

#### 11.5 Populated level, room count, warp points, kind-11 location

1. **Populated level** (`0x0061A1F0` → `0x0066BB20`, active room):
   null room → 0; DRLG room flag 0x800000 (no population) → 0; else the
   level id of the room's level.
2. **Populated-room count** (`0x0061ABF0` → `0x00642BE0`; act, level
   id): the level of that id in the act's DRLG list (allocated if absent,
   §4.3; allocation never generates rooms); the number of its rooms
   (from level +0x10, next +0x24) without flag 0x800000. A level without
   rooms counts 0.
3. **Warp points** (`0x0061AC10` → `0x00642380`, active room; null →
   fatal 0x5B9): the warp-room centres (§5.4) of the room's own level:
   x[i] at +0x1E0 + 4i, y[i] at +0x204 + 4i, count at +0x228, subtiles.
4. **Kind-11 location** (as `0x0054DB50` asks it): `0x00619E50(act of
   the room's level (§6.3; act table at game +0xBC), level id, 11, &x,
   &y)` → `0x0066B2B0`, the §10 spawn-room choice with tile index 11,
   **with all its effects**: the level is generated if it has no rooms;
   for levels with `Position` ≠ 0 the §10 step 2 class pick (request 11
   matches only index-11 records; `roll(n)` on the level seed when
   n > 0); otherwise the §10 step 3 fallbacks (which may draw
   `roll(room count)` on the level seed); and the chosen room is
   streamed (`drlg/rooms.md` §4.3). x, y are tiles, −1 when no room was
   found; the caller scales them ×5.

#### 11.6 Population read map (for hosts)

Every DRLG read the room population makes (`monsters/population.md`
§3–§9, §11–§12), with its owner. A host that answers each of these
from the rules named runs room population; nothing else in the DRLG is
read by it.

| Read | 1.14d | Owner |
|---|---|---|
| coordinate list of an active room (first record, then next +0x2C) | `0x0061AD50` | §11.4 (lists built §11.2–§11.3) |
| coordinate record at a sub-tile point | `0x0061AD30` | §11.4 |
| coordinate index at a sub-tile point (room or an adjacent room) | `0x0061B130` | §11.4 |
| populated level (0 for a flag-0x800000 room) | `0x0061A1F0` | §11.5 item 1 |
| level id of an active room (no flag test; regions, `WarpDist`) | `0x0061A1B0` → `0x0066BAB0` | DRLG room +0x58 → level +0x1D0; null room → 0 |
| populated-room count of a level | `0x0061ABF0` | §11.5 item 2 |
| warp points of the room's level | `0x0061AC10` | §11.5 item 3 |
| kind-11 spawn location | `0x00619E50` → `0x0066B2B0` | §11.5 item 4, §10 |
| act of a level id | `0x006427F0` | §6 rule 3 |
| active-room sub-tile box | `0x00619730` | active room +0x4C (x, y, w, h), `drlg/rooms.md` §1 |
| active-room seed (every placement draw) | — | `drlg/rooms.md` §5 step 4 |
| room adjacency (cross-room lookups) | `0x00463740` | `drlg/rooms.md` §6 |
| collision tests and free points | `0x0064D9B0`, `0x0064E7B0`, `0x0064E840` | `sim/path-placement.md` §4, §7, §8 |
| floor tile records (water placement) | `0x00619660` | `monsters/population.md` §9.2 |
| room "populated" bit, restore instead of populate | active room +0x34 bit 0 | `drlg/rooms.md` §5 step 3, `sim/tick.md` §4 |

**Which rooms are populated, and in which order** (consequences of the
owners above; reproduce them, do not model them separately):

1. Only active rooms are populated, each the first time the tick room
   pass meets it (`monsters/population.md` §1 rule 1). A room becomes
   active only through a build: a client's room change builds every
   room of the new room's rooms-near array, depth first
   (`drlg/rooms.md` §4.1); streaming builds one room (`drlg/rooms.md`
   §4.3: unit placement at coordinates, the §10 spawn-room choice, and
   so also the kind-11 query of §11.5 item 4 made during population).
2. Each build prepends its active room to the act room list
   (`drlg/rooms.md` §5 step 5) and the room pass walks the list from the
   head, so rooms built in one burst are populated in the reverse of
   their build order (recorded: `monsters/population.md` §1 rule 3).
3. Coordinate indexes come from the level counter at each room's build
   (§11.3 steps 3, 5, 7; one-record rooms reset it, §11.2 step 2), and
   `0x0061B130` compares indexes across a room and its adjacent rooms.
   A host that builds rooms in another order gets other indexes and so
   accepts or rejects other spawn points (`monsters/population.md` §8
   step 2.3, §9.3 step 3.2.2).
4. Entering a level builds only the rooms-near closure of the arrival
   room (rule 1); populating every room of a level in level-list order
   right after creation is not an order the original produces. The
   density draw and every unit allocation use the game seed
   (`monsters/population.md` §3.2, §9.6), shared with every other
   game-seed user between creation and that room pass (`sim/rng.md`
   §5.2), so a population result depends on everything drawn before it.

### 12. Level connections: Act I, Act III, Act V

Owner of the per-level connection lists below (REC-230; Act I
dungeons, the Docks waypoint and the summit units REC-249, REC-246). It adds no
rule: it applies §7, `drlg/outdoor.md` §2.3 / §2.7,
`drlg/preset.md` §6, `drlg/rooms.md` §3.3 / §9.5.1,
`sim/path-placement.md` §12 and `drlg/maze.md` §6 to the 1.14d data.
Data: vis/warp from patch_d2 `levels.txt` (equal to the live
`leveldefs.bin` +0x48 / +0x68 for all 137 rows); lvlwarp rows from
d2exp `LvlWarp.txt` (equal to the live `lvlwarp.bin`, 88 rows of 0x30
bytes); exit cells read from the DS1 files (wall cells with
orientation 10 or 11, style = bits 20–25, the `preset.md` §6 step 7
marker test). Cell positions are DS1 tile coordinates inside the
preset. patch_d2.mpq replaces none of these DS1 files except
`Expansion\Town\townWest.ds1`, which has no exit cell in either version.

#### 12.1 Connection kinds

| Kind | Vis slot i | Made by | How the player crosses |
|---|---|---|---|
| walk | warp −1 | Act I (and II, IV): the link driver, both ways (`outdoor.md` §2.3 step 4). Act III, V: adjacency warps, a shared edge (`outdoor.md` §2.7) | No tile and no lvlwarp row. Boundary rooms get flag 0x10 << i: outdoor cells through the link vis flag (`outdoor.md` §5.5, only for levels with neighbour entries), preset areas (towns, outdoor stamps) on every cell (`preset.md` §6 step 3). The rooms-near build then appends the other level's flagged rooms within the 6-tile gap (`rooms.md` §3.3, W = −1), so the player walks across. |
| tile | leveldefs `Warp`i = lvlwarp `Id` ≥ 0 | A stamped preset whose DS1 has an exit cell with style i. This sets flag 0x10 << i on its room (`preset.md` §6 step 7). | Tile unit with class = `Id`. Its row letter is 'l' for orientation 10 and 'r' for 11; 'b' rows match both (§7 rule 4, `sim/path-placement.md` §12.1). Room link: `rooms.md` §3.3. Warping in: `sim/path-placement.md` §12.2. |
| maze | the far side of a tile in a maze level | `maze.md` §6 special cells | as tile |

Quest and town portals are not level connections (they use no vis or
warp slot). Examples: the Cairn Stones to Tristram, the red portals
from Harrogath.

#### 12.2 Act I fields (no draw decides any slot)

The vis and warp arrays are fixed once Act I is created. They are
leveldefs plus the link-driver rows in table order. §7 rule 3 puts each
new id in the first slot with vis 0 and warp −1, so: A1W Stony–Cold,
Blood Moor–Cold, Rogue Encampment–Blood Moor, Burial–Cold; A1M
Tamoe–Monastery Gate, Black Marsh–Tamoe, Dark Wood–Black Marsh.

| Level | Vis0..7 | Warp0..7 | Walk to | Tile slots → level (lvlwarp `Id`) |
|---|---|---|---|---|
| 1 Rogue Encampment | 2, 0, 0, 0, 0, 0, 0, 0 | all −1 | 2 | — |
| 2 Blood Moor | 3, 1, 0, 8, 8, 8, 8, 0 | −1, −1, −1, 0, 1, 2, 3, −1 | 3, 1 | 3..6 → 8 Den of Evil (0..3) |
| 3 Cold Plains | 4, 2, 17, 9, 9, 9, 9, 0 | as 2 | 4, 2, 17 | 3..6 → 9 Cave 1 |
| 4 Stony Field | 3, 0, 0, 10, 10, 10, 10, 0 | as 2 | 3 | 3..6 → 10 Underground Passage 1 |
| 5 Dark Wood | 6, 0, 0, 10, 10, 10, 10, 0 | as 2 | 6 | 3..6 → 10 Underground Passage 1 |
| 6 Black Marsh | 7, 5, 20, 11, 11, 11, 11, 0 | −1, −1, 10, 0, 1, 2, 3, −1 | 7, 5 | 2 → 20 Forgotten Tower (10); 3..6 → 11 Hole 1 |
| 7 Tamoe Highland | 26, 6, 0, 12, 12, 12, 12, 0 | as 2 | 26, 6 | 3..6 → 12 Pit 1 |
| 17 Burial Grounds | 18, 19, 3, 0, 0, 0, 0, 0 | 6, 7, −1, −1, −1, −1, −1, −1 | 3 | 0 → 18 Crypt (6); 1 → 19 Mausoleum (7) |
| 26 Monastery Gate | 7, 27, 0, 0, 0, 0, 0, 0 | all −1 | 7, 27 | — |
| 10 Underground Passage 1 (maze) | 4, 5, 0, 0, 14, 0, 0, 0 | 4, 4, −1, −1, 5, −1, −1, −1 | — | 0 → 4, 1 → 5 (4 "Cave Up"); 4 → 14 (5 "Cave Down"); exit cells `maze.md` §6 (cave_prev, cave_next, cave_down) |

So Dark Wood does not touch Cold Plains or Stony Field. The way from
Stony Field to Dark Wood is through Underground Passage 1: Stony
Field's cave tile links to its slot-0 room, Dark Wood's to its slot-1
room (`rooms.md` §3.3: the (c+1)-th match fails, then the first slot
whose vis holds the level).

Each field level 2..7 gets exactly one cave mouth. `outdoor.md` §7.2
and §7.3 set flag 0x40 on the first one placed. Its preset decides the
slot:

| Placed by | lvlprest, file | Exit cell (x, y), orientation, style | Slot, lvlwarp row |
|---|---|---|---|
| §7.2 step 2 (cliff-marked levels 4..7 only; §7 step 1 skips 2, 3, 17) | 25 Wild Cliff Cave Left, `clfcave.ds1` | (5, 2), 10, 3 | 3, `Id` 0 "Cliff L" |
| same | 24 Wild Cliff Cave Right, `clfcave2.ds1` | (6, 5), 11, 4 | 4, `Id` 1 "Cliff R" |
| §7.2 step 3 or §7.3 step 3, levels 3..7 | 51 Cave Entrance, file 1 `CaveDr1.ds1` / file 2 `CaveDr2.ds1` (F −1: build-list roll, `outdoor.md` §5.1) | (3, 4), 10, 5 / (3, 4), 10, 6 | 5, `Id` 2 "Floor L" / 6, `Id` 3 "Floor R" |
| same, level 2 | 52 DOE Entrance, `DenEnt.ds1` / `DenEnt2.ds1` | (3, 4), 10, 5 / (5, 4), 10, 6 | 5 / 6 |
| §7.4 S 163, level 6 | 163 Tower 1, `Tower1.ds1` | (3, 3), 10, 2 | 2, `Id` 10 |
| §7.4 stamp 108, level 17 | 108 Graveyard, `gravey.ds1` | (12, 27), 10, 0; (11, 6), 10, 1 | 0, `Id` 6; 1, `Id` 7 |

Rows `Id` 0..3, 6, 7 and 10 are all direction 'b', so the cell's
orientation does not change the row.

#### 12.3 Act III

Three things depend on the DRLG seed (`drlg/outdoor-act3-act5.md` §2):
which jungle levels touch, the vis slot numbers in 76..79, and the
jungle clearing files. Nothing else does. Adjacency (§2.7 of
`outdoor.md`) runs i = 75..83 ascending, and each level's record gets
its neighbours in ascending id. Kurast's vis slots 0..3 are already
taken, so its new ids go to the first free slots.

| Level | Walk to (vis slots) | Tile slots → level (lvlwarp `Id`) |
|---|---|---|
| 75 Kurast Docks | 76 only (slot 0) | — |
| 76 Spider Forest | 75 (slot 2), 77 (always), 78 (by seed) | 0 → 84 Spider Cave, 1 → 85 Spider Cavern (51 both) |
| 77 Great Marsh | 76 (always), 78 (by seed) | — |
| 78 Flayer Jungle | 76 and/or 77 (by seed, at least one), 79 (always) | 0 → 86 Swampy Pit 1 (53), 1 → 88 Flayer Dungeon 1 (54) |
| 79 Lower Kurast | 78, 80 (slots 0, 1) | — |
| 80 Kurast Bazaar | 79, 81 (slots 4, 5) | 0, 1 → 92 Sewers 1 (57); 2 → 94 Ruined Temple, 3 → 95 Disused Fane (61) |
| 81 Upper Kurast | 80, 82 (slots 4, 5) | 0, 1 → 92 (57); 2 → 96 Forgotten Reliquary, 3 → 97 Forgotten Temple (61) |
| 82 Kurast Causeway | 81, 83 (slots 0, 1) | 2 → 98 Ruined Fane, 3 → 99 Disused Reliquary (61) |
| 83 Travincal | 82 (slot 1) | 0 → 100 Durance of Hate 1 (64) |
| 100 Durance 1 (maze) | — | 0, 1 → 101 (67, 68 "Down L/R"); 2, 3 → 83 (65, 66 "Up L/R") |
| 101 Durance 2 (maze) | — | 0, 1 → 102 (67, 68); 2, 3 → 100 (65, 66) |
| 102 Durance 3 (preset) | — | 2, 3 → 101 (65, 66) |

Geometry, from `outdoor-act3-act5.md` §2.2 and `outdoor.md` §9.2:

- **Docks and 76.** Placement record 0 sits on the docks' top edge and
  always becomes 76. Every other jungle's rect ends at least 64 tiles
  above the docks.
- **79 touches only 78.** 78 has the smallest y of the three jungles,
  and the Kurast chain is stacked above it. 79's x edges (78.x − 8,
  78.x + 72) are off the 64-tile jungle grid, and no jungle has 78's top
  y in an adjacent column (no placement case gives a (±64, 0) offset).
- **80..83** lie wholly above 78's top and touch only their chain
  neighbours.

An enumeration of the jungle placer (the `d2-sim` port, run outside
the repo) over 200,000 seeds (init x·2654435761, two steps, then
`0x00677880`) found:

- adjacency sets {76–77, 76–78} 21.4 %, {76–77, 77–78} 48.3 %, all
  three 30.3 %;
- 75 touched only 76 and 79 touched only 78, every time;
- no fatal.

The recorded vector (`outdoor-act3-act5.md` Test vectors: 76 (1000,
808), 77 (936, 744), 78 (1000, 616)) is the all-three case. Its 76 vis
is 84, 85, 75, 77, 78; 77 is 76, 78; 78 is 86, 88, 76, 77, 79.

Exit cells (all orientation 10 except Travincal's):

| Level | Preset (how stamped) | Exit cell (x, y), style |
|---|---|---|
| 76 | clearing Webby 575–584: file 1 `Spid<dir>.ds1`, file 2 `Spid<dir>2.ds1`, file 3 none | style 0 in file 1, style 1 in file 2 (e.g. `SpidE` (15, 14), `SpidE2` (10, 10)) |
| 77 | clearing Boggy 585–594 | no file has an exit cell |
| 78 | clearing Pygmy 595–604: `Pyg<dir>.ds1` / `Pyg<dir>2.ds1` / file 3 none | style 0 / 1 (e.g. `PygE` (12, 12), `PygE2` (12, 19)) |
| 80 | 629 Burbs Sewer: stamp F 0 at cell (3, 3) `BurbsSewer0.ds1`, F 1 at (X, 3) `BurbsSewer1.ds1`; S(630, 0) `BurbsTemple2.ds1`, S(630, 1) `BurbsTemple3.ds1` (`outdoor.md` §9.4) | sewers (4, 3) style 0 / 1; temples (6, 4) style 2, (4, 7) style 3 |
| 81 | 646 Metro Sewer F 0 / F 1; S(647, 0) `MetroTemple2.ds1`, S(647, 1) `MetroTemple3.ds1` | sewers (4, 3) style 0 / 1; temples (4, 4) style 2, (4, 6) style 3 |
| 82 | 652 `Bridge.ds1`, stamp F 0 at (0, 0) | (26, 8) style 2; (5, 7) style 3 |
| 83 | 654 `TravN.ds1` (stamp at (2, 0)) | (15, 17), orientation 11, style 0 |

Jungle stamping picks clearing c's file as F := G[3r + c]
(`outdoor-act3-act5.md` §3). With 2 clearings r < 2, so the files are
{0, 1}. With 3 clearings G's row uses all of 0, 1, 2. Either way
slots 0 and 1 each get exactly one tile. Every enumerated seed gave
2 or 3 clearings per jungle; the counts were 76: 21.5 % / 78.5 %,
77: 3.5 % / 96.5 %, 78: 10.3 % / 89.7 %. A level with fewer than 2
clearings would miss a cave mouth. The data has no other source for
slots 0 and 1, and none was seen.

The jungle head 573 and tail 574 (`TransL/U`, `TravL/U`), the docks
(`DockTown3.ds1`) and the Kurast border, gate and filler presets have
no exit cell. Their crossings are all walk.

**Docks waypoint** (REC-246 (b)). The Kurast Docks waypoint is a preset
unit of `DockTown3.ds1` (lvlprest 529, v18, DS1 act 2): object record
id 1 → object-preset table [2][1] = object 237 (`act3waypoint`, a
waypoint row), DS1 sub-tile (159, 49), mode 0, kept by every filter
(`preset.md` §5.3, §7). With level 75 at tile (1000, 1000) it stands at
sub-tile (5159, 5049): room (1024, 1008) (column 3, row 1 of the 8-tile
grid), (39, 9) from that room's sub-tile origin. It is the DS1's only
waypoint unit. Its index (`world/waypoints.tsv`) is not a DRLG fact.

#### 12.4 Act V

The vis arrays are fixed once Act V is created. No table warps are set
in Act V. The adjacency order is 111..112, 110..111, then 109..110
(`outdoor.md` §2.1). The B1 and B2 placements always share an edge
with their parent (`outdoor.md` §2.4: 111's right edge is 110's left,
y spans overlap; every B2 offset puts 112 on an edge of 111). 117 is
placed alone (BD) and is never adjacency-tested.

| Level | Vis0..7 (Warp) | Walk to | Tile slots → level (lvlwarp `Id`) |
|---|---|---|---|
| 109 Harrogath | 110 (−1) | 110 | — |
| 110 Bloody Foothills | 111, 109 (−1, −1) | 111, 109 | — |
| 111 Frigid Highlands | 112, 110 (−1, −1) | 112, 110 | — |
| 112 Arreat Plateau | 111, 0, 113 (−1, −1, 71) | 111 | 2 → 113 Crystalized Cavern 1 (71) |
| 113 Crystalized Cavern 1 (maze) | 112, 115, 114 (73, 74, 75) | — | 0 → 112, 1 → 115, 2 → 114 Cellar of Pity |
| 114 Cellar of Pity | 113 (73) | — | 0 → 113 |
| 115 Crystalized Cavern 2 (maze) | 113, 117, 116 (73, 74, 75) | — | 0 → 113, 1 → 117, 2 → 116 Echo Chamber |
| 116 Echo Chamber | 115 (73) | — | 0 → 115 |
| 117 Frozen Tundra | 115, 0, 118 (72, −1, 71) | — | 0 → 115 (72); 2 → 118 Glacial Caves 1 (71) |
| 118 Glacial Caves 1 (maze) | 117, 120, 119 (73, 74, 75) | — | 0 → 117, 1 → 120, 2 → 119 |
| 119 Glacial Caves 2 | 118 (73) | — | 0 → 118 |
| 120 Arreat Summit (preset) | 118, 128 (79, 80) | — | 0 → 118, 1 → 128 |
| 128, 129, 130 Worldstone Keep 1–3 (maze) | previous, next (81, 82) | — | 0 → previous (120, 128, 129), 1 → next (129, 130, 131) |
| 131 Throne of Destruction | 130, 132 (81, 82) | — | 0 → 130, 1 → 132 |
| 132 Worldstone Chamber | 131 (81) | — | 0 → 131 |

So the walk chain is 109 – 110 – 111 – 112. Every later link is a
tile. 114, 116 and 119 are dead ends off 113, 115 and 118, and 115
leads straight to 117.

Exit cells of the outdoor and preset levels (cave presets:
`outdoor.md` §11 step 5. A tall level (w < h) stamps P tall at (2, 0),
or at (2, gh − 2) when side = 1. A wide level stamps P wide at (0, 2),
or at (gw − 2, 2).):

| Level, row | Preset | Exit cell (x, y), orientation, style → row |
|---|---|---|
| 112 (112, 0, 0, 913, 914) | tall: 913 `NorthEntrance_Dirt.ds1`; wide: 914 `WestEntrance_Dirt.ds1` | (17, 9), 11, 2 → `Id` 71 'r'; (9, 12), 10, 2 → 71 'l' |
| 117 (117, 0, 1, 983, 984) | 983 `NorthExit_Snow.ds1` / 984 `WestExit_Snow.ds1`, at the far end | (14, 4), 10, 0 / (3, 14), 10, 0 → `Id` 72 'b' |
| 117 (117, 0, 0, 985, 986) | 985 `NorthEntrance_Snow.ds1` / 986 `WestEntrance_Snow.ds1` | (12, 11), 11, 2 → 71 'r' / (10, 12), 10, 2 → 71 'l' |
| 120 | 1089 `MtnTop.ds1` | (11, 25), 10, 0 → 79; (9, 5), 10, 1 → 80 |

The ice-cave and Worldstone files follow `maze.md` §6. Their exit
cells use style 0 for back, 1 for ahead and 2 for down in the ice
caves (`ice<dir>back01`, `ahead01`, `down01`), and 0 for up and 1 for
down in the Worldstone Keep (`baal<dir>up0n`, `down0n`). Harrogath
(`townWest.ds1`), the siege strips 865–879 and 880
`siege2barricade.ds1` have no exit cell, and nor do the Dirt "From
Cave" presets 911 and 912, which no 1.14d table row stamps.

**Arreat Summit** (REC-246 (c)). Level 120 is one preset map,
`MtnTop.ds1` (lvlprest 1089, v18, DS1 act 4, 20 × 28 tiles), at the
level rect (2000, 2508), multi-room (8-tile grid: columns at x 0, 8, 16;
rows at y 0, 8, 16, 24). Its exit cells are in wall layer 1, both
hidden, orientation 10:

| Cell | Style → slot | Row (`Id`, offsets) | Room (DS1 tiles) | Tile sub-tile from the room origin |
|---|---|---|---|---|
| (11, 25) | 0 → 118 | 79 'b' (2, 1) | (8, 24) | (17, 6) |
| (9, 5) | 1 → 128 | 80 'b' (2, 1) | (8, 0) | (7, 26) |

Its preset units (DS1 records → `preset.md` §5.3; no row is filtered
by §7). Positions are DS1 sub-tiles; add (10000, 12540) for the level's
sub-tile frame:

| DS1 record | Unit | Sub-tile |
|---|---|---|
| object 142 | object-preset [4][142] = 564 summit door | (57, 127) |
| object 128 | [4][128] = 547 ancients door ("To The Worldstone Keep Level 1") | (48, 25) |
| object 127 | [4][127] = 546 ancients altar | (48, 78) |
| monster 24 | monpreset Act V row 24 = monstats 537 `ancientstatue1` → object 476 | (48, 67) |
| monster 25 | row 25 = 538 `ancientstatue2` → object 475 | (37, 77) |
| monster 26 | row 26 = 539 `ancientstatue3` → object 474 | (58, 77) |

So the summit door stands on the exit cell toward 118 and the ancients
door on the one toward 128. The DS1 has no unit for object 561 (the
invisible Ancient) or monsters 540–542 (the Ancients). Those come from
quest code (`world/quests-act5*.md`), and the exits' closed state from
`world/quests.md` §8.2 (`0x0058D090`), not from the DRLG.

#### 12.5 Act I dungeons (REC-249)

No draw decides any slot here either: the vis/warp arrays are
leveldefs (no link-driver row names these levels, except 26 in §12.2),
and each exit cell's style is its slot. Draws pick only which maze cell
carries a stamp, its N/E/S/W variant (`maze.md` §6) and its file
(`maze.md` §9 step 2: the stamp defs below lie outside every rotation
range, so the file is the cell map's own `roll(Files)` on the level
seed). Every lvlwarp row of Act I (`Id` 0–18) is direction 'b'.
Positions: §6 rule 1 (`OffsetX/Y`; 27 and 33 by `Depend`), 28 by
`maze.md` §7.1.

| Level | Kind (DrlgType, LevelType) | Built from | Vis (Warp), non-zero slots | Walk to | Tile slots → level (lvlwarp `Id`) |
|---|---|---|---|---|---|
| 8 Den of Evil | maze, cave (1, 3) | cave_prev, cave_doe | 0: 2 (4) | — | 0 → 2 (4 "Cave Up"). The Den has no way on. |
| 9 Cave Level 1 | maze, cave | cave_prev, cave_down, cave_coldcrow | 0: 3 (4); 4: 13 (5) | — | 0 → 3 (4); 4 → 13 (5 "Cave Down") |
| 10 Underground Passage 1 | maze, cave | cave_prev, cave_down, cave_next | 0: 4, 1: 5 (4, 4); 4: 14 (5) | — | 0 → 4, 1 → 5 (4); 4 → 14 (5) |
| 11 Hole 1 | maze, cave | cave_prev, cave_down | 0: 6 (4); 4: 15 (5) | — | 0 → 6 (4); 4 → 15 (5) |
| 12 Pit 1 | maze, cave | cave_prev, cave_down | 0: 7 (4); 4: 16 (5) | — | 0 → 7 (4); 4 → 16 (5) |
| 13–16 Cave Level 2, Underground Passage 2, Hole 2, Pit 2 | preset (2, 3) | lvlprest 104–107 `CaveRoom2..5.ds1` (one file each) | 1: 9 / 10 / 11 / 12 (4) | — | 1 → parent (4) |
| 18 Crypt, 19 Mausoleum | maze, crypt (1, 4) | crypt_prev, then crypt_bonebreak (18) / crypt_chest (19) | 0: 17 (8) | — | 0 → 17 (8 "Crypt Up") |
| 20 Forgotten Tower | preset (2, 2) | 164 `Tower2.ds1` | 0: 6 (11), 1: 21 (12) | — | 0 → 6 (11), 1 → 21 (12) |
| 21–24 Tower Cellar 1–4 | maze, crypt | crypt_prev, crypt_next | 0: id − 1 (8), 1: id + 1 (9) | — | 0 → previous (8), 1 → next (9 "Crypt Down") |
| 25 Tower Cellar 5 | preset (2, 4) | 159 `CryptCountess1/2.ds1` (Files 2) | 0: 24 (8) | — | 0 → 24 (8) |
| 26 Monastery Gate | preset (2, 5) | 165 `facade1.ds1` (`Scan` 0) | 0: 7 (−1, §12.2), 1: 27 (−1) | 7, 27 | — |
| 27 Outer Cloister | preset (2, 6) | 166 `CourtW/CourtN/…` (Files 3; `preset.md` §3.1) | 0: 26, 1: 28 (−1, −1) | 26, 28 | — |
| 28 Barracks | maze, barracks (1, 7) | ring(2), grow tree, `maze.md` §7.1 (forge and next) | 0: 27 (−1), 1: 29 (14) | 27 | 1 → 29 (14 "Jail Down") |
| 29 Jail 1 | maze, jail (1, 8) | jail_prev, jail_waypoint, jail_next | 0: 28 (13), 1: 30 (14) | — | 0 → 28 (13 "Jail Up"), 1 → 30 (14) |
| 30 Jail 2 | maze, jail | jail_prev, jail_pitspawn, jail_next | 0: 29 (13), 1: 31 (14) | — | 0 → 29, 1 → 31 |
| 31 Jail 3 | maze, jail | jail_prev, jail_cath | 0: 30 (13), 1: 32 (13) | — | 0 → 30 (13), 1 → 32 (13) |
| 32 Inner Cloister | preset (2, 6) | 256 `Cat_Court.ds1` | 0: 31 (14), 1: 33 (−1) | 33 | 0 → 31 (14) |
| 33 Cathedral | preset (2, 9) | 257 `Cathy3.ds1` | 0: 32 (−1), 1: 34 (15) | 32 | 1 → 34 (15) |
| 34 Catacombs 1 | maze, catacombs (1, 10) | start cell 290 (Prev NSEW), catacombs_next | 0: 33 (16), 1: 35 (18) | — | 0 → 33 (16), 1 → 35 (18 "Catacombs Down") |
| 35, 36 Catacombs 2, 3 | maze, catacombs | start cell 288 (Prev EW) or 289 (Prev NS), catacombs_next (+ waypoint in 35) | 0: id − 1 (17), 1: id + 1 (18) | — | 0 → previous (17 "Catacombs Up"), 1 → next (18) |
| 37 Catacombs 4 | preset (2, 10) | 299 `Andy3.ds1` | 0: 36 (17) | — | 0 → 36 (17) |
| 38 Tristram, 39 Moo Moo Farm | preset (2, 11) / outdoor (3, 2) | 300 `Tri_Town4.ds1` / `outdoor.md` | none | — | — (portals only, §12.1) |

So the dungeon tree is: Blood Moor → Den; Cold Plains → Cave 1 → Cave
2; Stony Field and Dark Wood → Underground Passage 1 → 2; Black Marsh →
Tower → Cellars 1–5 and Black Marsh → Hole 1 → 2; Tamoe → Pit 1 → 2;
Burial Grounds → Crypt, Mausoleum; Tamoe – Monastery Gate – Outer
Cloister – Barracks by walking, then tiles Barracks → Jail 1–3 → Inner
Cloister, walking Inner Cloister – Cathedral, then tiles Cathedral →
Catacombs 1–4. The maze builders' unconditional stamps for 13–16, 25, 37
(`maze.md` §6) never run: those levels are presets.

**Walk links inside the monastery.** 27 hangs on 26 (`Depend` 26,
offset (0, −40): rect (3000, 960, 56, 40) on 26's (3000, 1000, 64, 18)),
and 33 on 32 (offset (−4, −34): (3996, 966, 28, 34) on (4000, 1000,
18, 20)). Both pairs share the edge y = 1000. 28 is shifted against 27
(`maze.md` §7.1). `preset.md` §6 step 3 runs for every map built through
`0x00667ED0`, maze cells included (`maze.md` §9 step 3), so every room
of 26, 27, 28, 32 and 33 carries flag 0x10 << i for each walk slot i.
Rooms-near then appends the other level's rooms within the 6-tile gap
(`rooms.md` §3.3, W = −1).

**Exit cells and tile places.** Scan of every Act I lvlprest DS1 (Defs
53–310; wall layers, orientation 10/11, style ≤ 7, sub 0/4 or bit 31).
These are all the exit cells in the stamps used above. The coldcrow,
Den, bonebreak, chest, waypoint, pitspawn, forge, court-connect and
plain cells, 26, 27 and 38 have none. A tile unit is placed per
`sim/path-placement.md` §12.1 at (5·lx + `OffsetX`, 5·ly + `OffsetY`)
from the origin of the 8-tile room holding the cell. Cells of 12 × 12 or
smaller build one room (`maze.md` §9 step 3), so (lx, ly) is the cell
itself.

| Stamp (Def, file) | Cell (x, y), orientation, style | Row | Room in the map | Tile (sub-tiles) |
|---|---|---|---|---|
| Cave Prev W/E/S/N 83–86 (files 1, 2 equal) | (3, 5) / (7, 3) / (3, 3) / (4, 5), 10, 0 | 4 (2, 5) | (0, 0) | (17, 30) / (37, 20) / (17, 20) / (22, 30) |
| Cave Next W/E/S/N 87–90 (files equal) | same cells, style 1 | 4 | (0, 0) | same |
| Cave Down W 91, file 1 / 2 | (14, 10) / (13, 20), 10, 4 | 5 (1, 3) | (8, 8) / (8, 16) | (31, 13) / (26, 23) |
| Cave Down E 92, file 1 / 2 | (10, 9) / (10, 6), 10, 4 | 5 | (8, 8) / (8, 0) | (11, 8) / (11, 33) |
| Cave Down S 93 / N 94 (files equal) | (9, 6) / (3, 18), 10, 4 | 5 | (8, 0) / (0, 16) | (6, 33) / (16, 13) |
| `CaveRoom2` / `3` / `4` / `5` (levels 13–16) | (4, 5) / (10, 9) / (18, 9) / (4, 5), 10, 1 | 4 | (0, 0) / (8, 8) / (16, 8) / (0, 0) | (22, 30) / (12, 10) / (12, 10) / (22, 30) |
| Crypt Prev W/E/S/N 139–142 | (3, 1) / (1, 2) / (3, 1) / (2, 4), 11, 0 | 8 (5, 1) | cell | (20, 6) / (10, 11) / (20, 6) / (15, 21) |
| Crypt Next W/E/S/N 143–146 | (4, 3) / (0, 4) / (1, 4) / (1, 4), 10, 1 | 9 (1, 1) | cell | (21, 16) / (1, 21) / (6, 21) / (6, 21) |
| `CryptCountess1` / `2` (25) | (26, 12) / (13, 25), 11, 0 | 8 | (24, 8) / (8, 24) | (15, 21) / (30, 6) |
| `Tower2` (20) | (0, 1), 10, 0; (0, 2), 10, 1 | 11 (4, −2); 12 (2, 3) | (0, 0) | (4, 3); (2, 13) |
| Barracks Next W/E/S/N 198–201 (10 × 14: two room rows) | (6, 7) / (4, 3) / (5, 4) / (5, 6), 10, 1 | 14 (−1, 3) | (0, 0) | (29, 38) / (19, 18) / (24, 23) / (24, 33) |
| Jail Prev W/E/S/N 236–239 | (5, 2) / (5, 2) / (5, 2) / (3, 6), 11, 0 | 13 (5, 1) | cell | (30, 11) ×3 / (20, 31) |
| Jail Next W/E/S/N 240–243 | (7, 3) / (4, 4) / (6, 6) / (7, 7), 10, 1 | 14 | cell | (34, 18) / (19, 23) / (29, 33) / (34, 38) |
| Jail Cath W/E/S/N 244–247 | (5, 2) / (5, 2) / (5, 5) / (5, 5), 11, 1 | 13 | cell | (30, 11) / (30, 11) / (30, 26) / (30, 26) |
| `Cat_Court` (32) | (2, 3), 10, 0 | 14 | (0, 0) | (9, 18) |
| `Cathy3` (33) | (21, 10), 11, 1 | 15 (3, 1) | (16, 8) | (28, 11) |
| Catacombs Prev NSEW 290 | (5, 7), 11, 0 | 16 (7, 1) | cell | (32, 36) |
| Catacombs Prev EW 288, files 1–4 | (5, 1) / (5, 2) / (5, 6) / (5, 2), 11, 0 | 17 (5, 1) | cell | (30, 6) / (30, 11) / (30, 31) / (30, 11) |
| Catacombs Prev NS 289, files 1–4 | (3, 10) / (3, 10) / (5, 7) / (7, 7), 11, 0 | 17 | cell | (20, 51) / (20, 51) / (30, 36) / (40, 36) |
| Catacombs Next W 291, files 1 / 2 | (7, 4) / (7, 6), 10, 1 | 18 (−1, 3) | cell | (34, 23) / (34, 33) |
| Catacombs Next E 292 | (4, 4) / (5, 5), 10, 1 | 18 | cell | (19, 23) / (24, 28) |
| Catacombs Next S 293 (files equal) / N 294 | (6, 3) / (6, 6), (6, 7), 10, 1 | 18 | cell | (29, 18) / (29, 33), (29, 38) |
| `Andy3` (37) | (18, 22), 11, 0 | 17 | (16, 16) | (15, 31) |

Outdoor mouths, same rule (the stamp is one 8 × 8 room, `outdoor.md`
§12.1; cells §12.2): `DenEnt` (3, 4) style 5 → row 2 (−1, 1) → (14, 21);
`DenEnt2` (5, 4) style 6 → row 3 (1, −1) → (26, 19); `CaveDr1` (3, 4)
style 5 → (14, 21); `CaveDr2` (3, 4) style 6 → (16, 19); `clfcave`
(5, 2) style 3 → row 0 (1, −1) → (26, 9); `clfcave2` (6, 5) style 4 →
row 1 (3, 1) → (33, 26); `Tower1` (3, 3) style 2 → row 10 (−4, 0) →
(11, 15). `gravey.ds1` (24 × 32): (12, 27) style 0 → row 6 (5, 0),
room (8, 24) → (25, 15); (11, 6) style 1 → row 7 (5, 2), room (8, 0) →
(20, 32).

Hidden cells (Cave, Barracks Next, Jail Next, Catacombs Next,
`Cat_Court`, the outdoor mouths, `gravey`) place the tile through
`rooms.md` §9.5.1 step 3; the others have sub 0 and place it through the
wall warp tiles of step 6. Either way exactly one tile per exit cell.

Den of Evil, worked: Blood Moor's cave mouth is stamp 52 file 1 or 2
(`outdoor.md` §5.1). File 1 sets flag 0x10 << 5 on the stamp's room
and a tile of class 2 at (14, 21) from its origin; file 2 sets 0x10 << 6
and class 3 at (26, 19). In the Den, the cave_prev cell (variant N/E/S/W
by the stamp row) carries flag 0x10 and a class-4 tile at (22, 30) /
(37, 20) / (17, 20) / (17, 30). Linking the Den's room (W = 4): Blood
Moor's first slot holding 8 is 3, and no room has flag 0x80. Slots 3..6
are then tried in order; the mouth's slot (5 or 6) links (`rooms.md`
§3.3). Arrival uses the far side's record and its tile
(`sim/path-placement.md` §12.2).

## Constants & data dependencies

| Constant | Value | Use |
|---|---|---|
| level seed | `init_low(dwStartSeed + id)` | §4.3, §5.1 |
| tomb base level | 66 (7 tombs, `mod 7`) | §3 |
| town level ids | 1, 40, 75, 103, 109 (`0x006E7D1C`, `0x006426A0`) | §2, §6.3 |
| act boundaries | `0x006EB2F0`: 1, 40, 75, 103, 109, 1024 | §6.3 |
| level inactivity reset | 10 passes of tick step 10 | §9 |
| vis/warp slots | 8 | §7 |
| warp-room centre slots | 9 (unchecked) | §5.4 |
| spawn-tile class table | `0x006EED88`, 14 pairs | §10.2 |
| logical-room fill tables | T1 `0x006EEEA0` (20), T2 `0x006EEE38` (26), direction offsets `0x006EEE14` (4 pairs); values in §11.3 step 6 (equal to D2MOO's) | §11.3 |
| logical-room flags | visited 0x10000000, node 0x20000000, index mask 0x0FFFFFFF | §11.3 |

Data: lvlprest `Logicals` (§11.2).

Data: leveldefs `LevelType`, `DrlgType`, `SizeX/Y` (per difficulty),
`OffsetX/Y`, `Depend`, `Vis0..7`, `Warp0..7`, `Position`; lvlwarp `Id`,
`Direction` (first match); objects.txt subclass (waypoint bit).

## Randomness

Per act creation, in order (DRLG seed unless noted):

1. Step → `dwStartSeed`.
2. Act II: pairs of `lo' mod 7` until different. Act III: one step,
   `lo' & 1`.
3. The outdoor placer's draws (`drlg/outdoor.md`; most on a copy, see
   §3.7) and the draws of
   level allocations it triggers (preset file choice on each preset
   level's seed, `drlg/preset.md`).
4. Town generation (server): level seed and room seeds of the town.

Later, on demand: generation of other levels (their level seeds and room
seeds), spawn-room choice (`roll` on the level seed, §10; also reached
from monster population through the kind-11 query, §11.5), and room
activation draws (`drlg/rooms.md`). The DRLG seed is not drawn after act
creation by any code in this spec.

## Edge cases & original bugs

1. Lookups by id allocate (§4.2): the activity counter (§9.1) and the free
   test (§9.3) allocate vis levels that were never needed, changing the
   level list order (prepend); a point outside every level allocates level
   0 (§8.1). Reproduce: list order decides iteration in §9.2 and §8.1.
2. `0x00642920`: if no slot matches and no free slot exists, the slot
   index stays −1: vis goes to the record's level-id field and warp to
   vis[7]. D2MOO asserts. Unreachable with 1.14d data (open question 2,
   answered).
3. Warp-room centres have 9 slots and no bound check: a 10th qualifying
   room overwrites y[0] and the count (§5.4).
4. Spawn-tile pick with no match reads record 0. The pick walk cannot
   run out in 1.14d (k ≤ n matches, §10 step 2); a level without records
   reads record 0 (zeroed: position (0, 0)).
5. A level freed and regenerated keeps its id, seed and populated-room
   bits, but allocations made since (vis levels) stay in the list.
6. Logical rooms (§11.3): every grid-built room's list ends with a
   zeroed record (index 0, boxes zero); one-record rooms reset the level
   counter to 1; region numbers are consumed by blocked start cells;
   orientations 0 and 13 read rule values from before T2; the main-index
   test ignores bits 20 and 25. All reproduced.
7. The kind-11 location query (§11.5 item 4) streams a room and can draw
   on the level seed each time population asks it.

## Test vectors

Synthetic (from the rules and `rng.md`; CI-safe):

| Input | Expected | Source |
|---|---|---|
| init seed 644409375 | DRLG seed {644409375, 666}; `dwStartSeed` 4014346869; DRLG seed after {4014346869, 268778232} | §3 |
| same, act index 1 | tries: (5, 4) → staff tomb 71, boss tomb 70, 2 draws | §3.4; recorded (`pc2rec-d1-rng`, TestSor, `-seed` 644409375, sha256 8ac70b456cea9732…): `0x00642E73` lo' 1406222081 (mod 7 = 5), `0x00642EA6` lo' 3154683627 (mod 7 = 4), one try; drlg +0x94 = 71, +0x484 = 70 |
| same, act index 2 | jungle bit 1 (`lo'` 1406222081) | §3.4; recorded (`pc2rec-d1-rng`): one draw `0x00642EFF` lo' 1406222081, +0x474 = 1; Act IV no draw; every act's `dwStartSeed` 4014346869; server (seq 36430, 62907) and client copy (48902, 65279) equal |
| start 4014346869, level 1 | level seed {4014346870, 666}; first room seed {2928842600, 666}, its `dwInitSeed` 4134077858; second room seed {1513463342, 666} | §4.3, `rooms.md` §2 |
| level ids 39, 40, 109, 1024 | acts 0, 1, 4, 0 | §6.3 |
| lvlwarp request (id 0, 'b') | row "Act 1 Wilderness to Cave Cliff L" (first row with Id 0) | §7.4 |
| spawn-tile request 0 / 5 / 11 against record indexes 0..13 | 0 matches 0–4; 5 matches 5–9; 11 matches 11 only | §10 step 2 |
| one-record room, rect (10, 20, 8, 8), level counter 7 | counter 1; one record index 1, node 0, box and clipped box (10, 20, 18, 28); `0x0061AD30` at subtile (52, 103) → it | §11.2, §11.4 |
| grid build, rect (10, 20, 2, 1), no blockers, floor cells 0, counter 0 | start 1; one region index 2 over all 6 cells; records allocated 2; counter 4; list: {index 2, node 0, box (10, 20, 13, 22), clipped (10, 20, 12, 21)}, then the zeroed record | §11.3 |
| same rect, counter 3, blockers at (1, 0), (1, 1) with orientation 1, floor (2, 0) = 0x01E00002 | regions: (0, 0), (0, 1) → index 4; (1, 0) starts index 5 (rule 23 from d −1: mark, fill +x, +y, −y) and reaches (1, 1), (2, 0), (2, 1); node 0 (the start cell decides); records allocated 3; counter 8; list: {5, box (11, 20, 13, 22), clipped (11, 20, 12, 21)}, {4, box (10, 20, 11, 22), clipped (10, 20, 11, 21)}, zeroed | §11.3 |
| grid build, rect (0, 0, 1, 1), blocker at (0, 0) with orientation 0, counter 0 | rule 0xFFFFFFFF at d −1: one region index 2 over all 4 cells; list {2, box (0, 0, 2, 2), clipped (0, 0, 1, 1)}, zeroed | §11.3 step 6.3 |

The §11 vectors come from a simulation of the §11.3 text (scratch
script, not committed); the queued check is a recording of a Logicals
level (the Act 1 crypt rows, e.g. lvlprest Def 109 "Act 1 - Crypt W",
have `Logicals` 1; caves and towns have 0): dump
DRLG room +0x64 lists after activation and compare (open question 7).

Recorded (`20261005-232125-rng.jsonl`, seq numbers; labels from state
chaining):

| Seq | Event | Rule |
|---|---|---|
| 2417 | DRLG seed `init_low(644409375)` at `0x00642E02` | §3.2 |
| 2418 | step at `0x00642E09`, `lo'` 4014346869 | §3.3 |
| 2419–2424 | six steps on a copy of the DRLG seed in the Act 1 placer (`0x00676165`…); 2439 the one real DRLG-seed step (`0x006774DB`, `lo'` 1406222081) | §3.7 |
| 2425–2452 | level seeds set in this order: 4, 3, 2, 1, 17, 39, 26, 7, 6, 27, 5, 8, 9, 10, 11, 12, 13, 14, 15, 16 (each `start + id`), with 2 more copy steps after 17, one real DRLG-seed step (`0x006774DB`) after 27 and preset-file rolls on 26, 27, 13, 14, 15, 16 | §4.3 |
| 2454 | town (level 1) generation re-seeds {4014346870, 666} at `0x006424BF` | §3.8, §5.1 |
| 2561, 6896 | levels 2 and 3 generated on demand (warp links) | §5.5 |
| 13366–13403 | the client's DRLG copy: same seeds, same level order | §2.3 |

Resulting server level list (head first) after act creation: 16, 15, 14,
13, 12, 11, 10, 9, 8, 5, 27, 6, 7, 26, 39, 17, 1, 2, 3, 4.

Recorded spawn-room choice (§10; `pc2rec-d1-rng`, TestSor, `-seed`
644409375, sha256 8ac70b456cea9732…, server only):

| Arrival | Draws in `0x0066AC00`–`0x0066AD80` | Rule |
|---|---|---|
| game start, Rogue Encampment (1) | one at `0x0066ACB3` (seq 2546) on the level seed, {1946398892, 1447840265} → {2374762085, 811828125}: tile index ≠ 13 with n > 0 | §10 step 2 |
| waypoint to Lut Gholein (40), Kurast Docks (75), Pandemonium Fortress (103) | none: tile index 13, waypoint room | §10 steps 2, 4 |

Comparison (exact): for each created act, the DRLG seed state after
creation, `dwStartSeed`, the act choices, and the level list (ids, head
first) with each level's seed state, equal the recorded game.

Connections (§12, derived from the rules and the 1.14d data; CI-safe
with the tables as fixtures). After act creation, the vis/warp arrays
(drlg +0x90 records, or leveldefs where no record exists) of Act I
levels 1–7, 17 and 26 and of Act V levels 109–112 equal the §12.2 and
§12.4 tables for every seed. For the recorded Act III seed (`-seed`
644409375), 75's vis is 76; 76's is 84, 85, 75, 77, 78; 77's is 76,
78; 78's is 86, 88, 76, 77, 79; 79's is 78, 80. Kurast 80–83 are as
§12.3.

Warp tile places (§12.5, `sim/path-placement.md` §12.1; game-file
tests over the user's DS1 and lvlwarp files, `#[ignore]`):

| Map, exit cell | Level / slot | Tile unit (type 5) |
|---|---|---|
| `DenEnt.ds1` stamped in the Blood Moor, (3, 4) style 5 | 2 / 5 | class 2 at (14, 21) from the stamp room's sub-tile origin |
| `DenEnt2.ds1`, (5, 4) style 6 | 2 / 6 | class 3 at (26, 19) |
| Den of Evil, Cave Prev N `CaveNPre1.ds1`, (4, 5) | 8 / 0 | class 4 at (22, 30) from the cell's first room |
| Cave Level 1, Cave Down W `CaveWDown2.ds1`, (13, 20) | 9 / 4 | class 5 at (26, 23) from the room at cell tile (8, 16) |
| Cathedral `Cathy3.ds1`, (21, 10) orientation 11 | 33 / 1 | class 15 at (28, 11) from the room at level tile (16, 8) |
| Catacombs 2, Prev NS file 4 `CatNSUp4.ds1`, (7, 7) | 35 / 0 | class 17 at (40, 36) from the cell's room |
| Arreat Summit `MtnTop.ds1`, (11, 25) / (9, 5) | 120 / 0, 1 | class 79 at (17, 6) from room (8, 24); class 80 at (7, 26) from room (8, 0) |

Every row of the §12.5 tile table is a vector of the same form; a test
over the Act I lvlprest DS1s of Defs 53–300 finds exactly those exit
cells, the §12.2 ones of 108 and 163, and one in the unused 103
`CaveRoom1.ds1` ((6, 18), style 1; no level row names Def 103). Preset units: Kurast Docks object 237 at DS1
sub-tile (159, 49); Arreat Summit as the §12.4 unit table.

## Provenance

- **1.14d `Game.exe`**: every address above read from the disassembly
  `re/exports/all.asm` and the Ghidra decompile (register arguments
  from the disassembly). Struct sizes from the allocation calls
  (`0x0040B430` size argument). Tables `0x006E7D1C`, `0x006EB2F0`,
  `0x006EED88`, `0x006EEE14`–`0x006EEEEC` read from the file image.
  §11: `0x0066D110` and its steps `0x0066C7F0`, `0x0066C870`,
  `0x0066C580`, `0x0066C3D0`, `0x0066CA50`, `0x0066C9C0`, `0x0066D040`,
  `0x0066CF60`, `0x0066C770`; `0x0066CCB0` (one record, callers
  `0x00666DC8`, `0x0067D794`); `0x0066C6E0` (free); lookups `0x0066CF30`,
  `0x0066CEB0`, `0x0066CE30` and wrappers `0x0061AD50`, `0x0061AD30`,
  `0x0061B130`; `0x0066BB20`, `0x00642BE0`, `0x00642380`, `0x0066B2B0`
  via `0x00619E50`; the caller `0x0054DB50` for argument order; §11.6
  `0x0061A1B0` → `0x0066BAB0`, `0x00619730` (copies active room
  +0x4C..+0x68). lvlprest
  `Logicals` counted in patch_d2 `lvlprest.txt`. Callers found by scanning the
  disassembly for direct calls.
- **D2MOO** (1.10f) `D2Common/src/Drlg/DrlgDrlg.cpp` (`DRLG_AllocDrlg`,
  `DRLG_AllocLevel`, `DRLG_InitLevel`, `DRLG_FreeLevel`,
  `DRLG_UpdateAndFreeInactiveRooms`, `sub_6FD745C0`, coordinates,
  warps), `DrlgDrlgWarp.cpp`, `D2Dungeon.cpp` (`DUNGEON_AllocAct`):
  same rules; differences in 1.14d: tile library loaded after the act
  draws (§3.5), the level allocation silently ignores unknown DRLG types,
  the spawn-tile and set-warp code has no asserts, act creation passes
  the arena's level in arena games. `DrlgDrlgLogic.cpp` (logical rooms):
  same algorithm and tables; 1.14d's node test masks 0x01E0FF00 (main
  index bits 21–24 only), and the merge looks records up by tile
  coordinates.
- **§12 data** (2026-10-08, REC-230):
  - Tables: patch_d2 `levels.txt` vis/warp compared with the live
    `traces/raw/20261006-115547-tables/leveldefs.bin` (+0x48, +0x68,
    0x9C-byte rows), 0 differences in 137 rows. d2exp `LvlWarp.txt`
    compared with the live `lvlwarp.bin` (0x30-byte rows; `Id` +0x00,
    `LitVersion` +0x24, `Tiles` +0x28, `Direction` +0x2C). lvlprest file
    names from patch_d2 `lvlprest.txt`.
  - DS1 exit cells: d2data.mpq / d2exp.mpq files extracted with
    `mpq-tool extract`, scanned with a scratch script (wall layers,
    orientation 10/11, style ≤ 7, sub 0/4 or bit 31). The patch_d2.mpq
    overrides were checked by hash lookup of every listed name: only
    `townWest.ds1` is patched, and it has no exit cell.
  - §12.5 and the §12.3 / §12.4 unit paragraphs (2026-10-08, REC-249,
    REC-246): DrlgType / LevelType / vis / warp / offsets from patch_d2
    `levels.txt` (= leveldefs, above); lvlmaze rows from patch_d2
    `lvlmaze.txt`; lvlwarp offsets (`OffsetX` +0x1C… as read by
    `0x0066E1C0`) from d2exp `LvlWarp.txt`. DS1s read from the user's
    MPQs in Patch_D2 > d2exp > d2data order (`d2-formats` `ArchiveSet`
    and `Ds1::parse`, scratch program outside the repo), Defs 51–310,
    529 and 1089, with the §12 marker test; unit records mapped with
    `preset.md` §5.3 through `preset-tables.tsv` (objpreset [2][1],
    [4][127], [4][128], [4][142]) and patch_d2 `monpreset.txt` (Act V
    rows 24–26). Tile places computed from `sim/path-placement.md`
    §12.1 and the room grid of `preset.md` §6 step 10 / `maze.md` §9
    step 3. The cave, crypt, jail, catacombs and barracks stamp defs
    are those of `maze-specials.tsv`.
  - Jungle statistics: the `d2-sim` port of `0x00677880` (tested
    against the recorded Act III vector) run outside the repo over
    200,000 seeds.
- **Recorded**: `20261005-232125-rng.jsonl` (RNG hooks at all 846 inline
  sites and the helpers): DRLG seed, start seed, level seed order and
  values, room seed values (§Test vectors).

## Open questions

1. Act II tomb choice and Act III jungle bit: record entering Act 2 and
   Act 3 (draws at `0x00642E73`–`0x00642ED5`, `0x00642F04`; read drlg
   +0x94, +0x484, +0x474) to confirm the computed vectors.
   *Answered (2026-10-08, recorded: `pc2rec-d1-rng`)*: both vectors
   observed as computed (Test vectors, act index 1 / 2); the jungle-bit
   draw site is `0x00642EFF`. `check_drlg_acts` D1–D7 0 errors on the
   trace and on 12 more seeds × Acts I–III (`pc2rec-d2-sweep.log`, 36
   records, 271 level seeds).
2. *Answered* (static): `0x00642920` never overflows in 1.14d. Its only
   callers are the link driver (`0x006772C0`, four sites: both link
   columns, both ways; skipped in Act V) and the adjacency warps
   (`0x006775C0`, `outdoor.md` §2.7). Free slots (vis 0 and warp −1 in
   1.14d `levels.txt`) versus the most new vis ids a level can receive:
   Act I/II/IV links give at most 3 (Cold Plains: 4, 2, 17; 4 free),
   Black Marsh 2 (3 free), every other level ≤ 2 with ≥ 4 free; Act V
   adjacency gives one id per call (110, 111: two calls, 8 free). Act III
   adjacency (75..83): every id in the range is new (no 1.14d vis value
   lies in 75..83); jungle rects are 64 × 192 on a 64-tile x grid from
   the docks, the Kurast chain is stacked above 78 and centred on it
   (79–81 x edges at 78.x − 8 / + 72, never on the jungle grid; 82 lies
   inside 78's column), so a chain level touches a jungle only through a
   horizontal edge and 83 only through a side. Bounds: 80, 81 ≤ 4 (two
   chain neighbours + two jungles; 4 free), 82 ≤ 4 (6 free), 76/77/78
   ≤ 6 (75, the two other jungles, 79 or 81 by a horizontal edge, 83 by
   a side; 6, 8, 6 free), 75 touches only the jungle directly on top of
   it. Edge case 2 is therefore unreachable.
3. Does the server call the spawn-room choice (§10) with index ≠ 13 for
   levels with `Position` ≠ 0 (town arrivals)? Record a town arrival and
   an act change: draws at `0x0066ACB0`–`0x0066ACE0` on the level seed.
   *Answered (2026-10-08, recorded: `pc2rec-d1-rng`)* for game start
   and waypoint arrivals (Test vectors, "Recorded spawn-room choice"):
   the game-start town arrival uses an index ≠ 13 (one `roll(n)` draw
   at `0x0066ACB3`), waypoint arrivals use 13 (no draw). An act change
   by NPC (Warriv) is not recorded.
4. *Answered:* warp-room centres (+0x1E0) are read only through
   `0x0061AC10` (§11.5 item 3), whose only caller is `0x0054DB50`, called
   only from `0x0054DC40` (`monsters/population.md` §8, owner of the
   `WarpDist` test).
5. *Answered* (static): game +0x7C has three writers. `0x0052C2E3` and
   `0x0052C2FD` in game creation (`0x0052C280`: time value, or the
   `-seed` value, `rng.md` §5.2), and `0x00532A45` in the player-join
   routine `0x00532690` (only caller `0x00534080`): it copies the
   32-bit field at +0x7E of the 0x80-byte join record (copied to the
   stack at entry) into +0x7C when game +0x6A = 3, game +0x84 = 0 (no
   fixed seed) and the record's difficulty (high nibble of record byte
   +0x58) equals game +0x6D. The write precedes the player's unit
   allocation (`0x00555230`, same routine); act creation (§2) reads +0x7C
   when the act is made, and the recorded single-player DRLG seed equals
   the map ID, so the first act is created after this write. That the
   record field is the `.d2s` map ID is observed (`rng.md` §5.4); a
   brand-new character's value is `rng.md` OQ 2.
6. Can a 1.14d level have 10 or more warp-room centre rooms (§5.4)?
   Generate every level of the five acts and count rooms with a
   waypoint flag or a warp flag whose warp id ≠ −1; a 10th corrupts the
   count, an 11th writes past the level.
7. Logical rooms (§11.3) are read from the binary only: record a crypt
   level (`Logicals` 1) and an outdoor level, dump DRLG room +0x64 info
   and its records (boxes, node, index, order) after activation, and
   compare with a simulation of §11.3 on the same tiles.
8. *Answered* (`impl-room-population` §3 Q1): orientations above 19
   read the file-name string after T1 (not reproducible) and are never
   reached by 1.14d DS1 data (§11.3 step 6).
9. *Answered* (`impl-room-population` §3 Q2): the blocker grid is
   (W+1) × (H+1), row-major, in fixed 1,024-cell / 256-row buffers that
   1.14d rooms never overflow (§11.3 step 4).
10. *Answered* (`impl-room-population` §3 Q3): out-of-grid record
    lookups read the record-grid block as §11.4's table; reachable only
    from `0x005B2A00`'s spread search.
11. *Answered (2026-10-08, static and data; REC-230)*: how the Act I
    fields, Act III and Act V levels connect, and where their warp
    tiles come from, is §12. Walk links come from the link driver or
    adjacency (no tile). Tile links come from the vis slot's lvlwarp
    `Id` and a DS1 exit cell whose style is the slot. No recording is
    needed. Optional confirmations: after entering Act III, dump the
    drlg +0x90 vis/warp records of 75–83 (§Test vectors); after
    entering Act V, the records of 109–112 and the warp units of 112
    and 117.
12. *Answered (2026-10-08, static and data; REC-249, REC-246 (b), (c))*:
    the Act I dungeons' kinds, slots, lvlwarp rows and tile places are
    §12.5; the Kurast Docks waypoint unit is §12.3, the Arreat Summit's
    exits and preset units §12.4. No recording is needed. Optional
    confirmation (R-A1DUNGEON): walk Blood Moor → Den, Cold Plains →
    Cave 1 → Cave 2, Black Marsh → Tower → Cellar 5, Monastery Gate →
    Catacombs 4 and record each created type-5 unit (level, room sub-tile
    origin, class, position) and the drlg +0x90 vis/warp records of the
    levels entered.
