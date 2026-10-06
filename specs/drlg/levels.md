# Spec: DRLG — Acts, levels, level seeds and warps

- **Status:** draft (no `d2-sim` code yet); every rule read from the 1.14d
  `Game.exe` code; the seed rules are confirmed on the recording
  `traces/raw/20261005-232125-rng.jsonl` (Act 1 entry, single player:
  DRLG seed, start seed, 21 level seeds and 47 room seeds of the server
  build and the same values in the client's copy).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::drlg::level` (act DRLG, level list, level
  seeds, vis/warp records, spawn rooms)
- **Related specs:** `drlg/rooms.md` (DRLG rooms, rooms-near order,
  activation, active rooms); `drlg/preset.md`, `drlg/maze.md`,
  `drlg/outdoor.md` (what each level type generates, and the act-wide
  placement of outdoor levels); `sim/rng.md` (generator, seed derivation,
  §5.4 DRLG seeds); `sim/tick.md` (steps 3, 9, 10 that drive the room and
  level lifecycle); `sim/unit-order.md` (act room list).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 43–58 |
| Inputs | 59–68 |
| Outputs / state changes | 69–74 |
| Rules | 75–76 |
|   1. Structures (1.14d layout, for recorders and checks) | 77–100 |
|   2. Act creation (server) | 101–115 |
|   3. DRLG creation (`0x00642DA0`) | 116–150 |
|   4. Level list, get-or-allocate | 151–168 |
|   5. Level generation (`0x006424A0`, D2MOO `DRLG_InitLevel`) | 169–192 |
|   6. Level position, size, act number | 193–209 |
|   7. Vis and warp records | 210–233 |
|   8. Coordinates to rooms | 234–247 |
|   9. Level lifecycle: activity and freeing | 248–278 |
|   10. Spawn room in a level (`0x0066B2B0`) | 279–306 |
| Constants & data dependencies | 307–323 |
| Randomness | 324–341 |
| Edge cases & original bugs | 342–356 |
| Test vectors | 357–389 |
| Provenance | 390–409 |
| Open questions | 410–426 |
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
| level | | room count; first room; activity count; inactive frames | +0x08; +0x10; +0x0C; +0x1D4 |
| level | | position x, y; width, height (tiles) | +0x1C, +0x20; +0x24, +0x28 |
| level | | spawn-tile records (x, y, tile index; stride 12); count | +0x2C; +0x1D8 |
| level | | next level; drlg; level type; level seed (lo, hi); level id | +0x1AC; +0x1B4; +0x1C0; +0x1C4, +0x1C8; +0x1D0 |
| level | | warp-room centres x[9], y[9]; count; populated-room memory | +0x1E0, +0x204; +0x228; +0x22C |
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
   not DRLG) and the act's animated-tile cache (`0x00642A30`).

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
   check (9 slots).
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
town arrival, act change: D2MOO `DUNGEON_FindActSpawnLocation`):

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
seeds), spawn-room choice (`roll` on the level seed, §10), and room
activation draws (`drlg/rooms.md`). The DRLG seed is not drawn after act
creation by any code in this spec.

## Edge cases & original bugs

1. Lookups by id allocate (§4.2): the activity counter (§9.1) and the free
   test (§9.3) allocate vis levels that were never needed, changing the
   level list order (prepend); a point outside every level allocates level
   0 (§8.1). Reproduce: list order decides iteration in §9.2 and §8.1.
2. `0x00642920`: if no slot matches and no free slot exists, the slot
   index stays −1: vis goes to the record's level-id field and warp to
   vis[7]. D2MOO asserts. Not known to be reached (open question 2).
3. Warp-room centres have 9 slots and no bound check (§5.4).
4. Spawn-tile pick with no match reads record 0; a loop that runs out
   reads record `count` (one past the last), as in 1.10f.
5. A level freed and regenerated keeps its id, seed and populated-room
   bits, but allocations made since (vis levels) stay in the list.

## Test vectors

Synthetic (from the rules and `rng.md`; CI-safe):

| Input | Expected | Source |
|---|---|---|
| init seed 644409375 | DRLG seed {644409375, 666}; `dwStartSeed` 4014346869; DRLG seed after {4014346869, 268778232} | §3 |
| same, act index 1 | tries: (5, 4) → staff tomb 71, boss tomb 70, 2 draws | §3.4 (computed, not yet observed) |
| same, act index 2 | jungle bit 1 (`lo'` 1406222081) | §3.4 (computed) |
| start 4014346869, level 1 | level seed {4014346870, 666}; first room seed {2928842600, 666}, its `dwInitSeed` 4134077858; second room seed {1513463342, 666} | §4.3, `rooms.md` §2 |
| level ids 39, 40, 109, 1024 | acts 0, 1, 4, 0 | §6.3 |
| lvlwarp request (id 0, 'b') | row "Act 1 Wilderness to Cave Cliff L" (first row with Id 0) | §7.4 |

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

Comparison (exact): for each created act, the DRLG seed state after
creation, `dwStartSeed`, the act choices, and the level list (ids, head
first) with each level's seed state, equal the recorded game.

## Provenance

- **1.14d `Game.exe`**: every address above read from the disassembly
  `re/exports/all.asm` and the Ghidra decompile (register arguments
  from the disassembly). Struct sizes from the allocation calls
  (`0x0040B430` size argument). Tables `0x006E7D1C`, `0x006EB2F0`,
  `0x006EED88` read from the file image. Callers found by scanning the
  disassembly for direct calls.
- **D2MOO** (1.10f) `D2Common/src/Drlg/DrlgDrlg.cpp` (`DRLG_AllocDrlg`,
  `DRLG_AllocLevel`, `DRLG_InitLevel`, `DRLG_FreeLevel`,
  `DRLG_UpdateAndFreeInactiveRooms`, `sub_6FD745C0`, coordinates,
  warps), `DrlgDrlgWarp.cpp`, `D2Dungeon.cpp` (`DUNGEON_AllocAct`):
  same rules; differences in 1.14d: tile library loaded after the act
  draws (§3.5), the level allocation silently ignores unknown DRLG types,
  the spawn-tile and set-warp code has no asserts, act creation passes
  the arena's level in arena games.
- **Recorded**: `20261005-232125-rng.jsonl` (RNG hooks at all 846 inline
  sites and the helpers): DRLG seed, start seed, level seed order and
  values, room seed values (§Test vectors).

## Open questions

1. Act II tomb choice and Act III jungle bit: record entering Act 2 and
   Act 3 (draws at `0x00642E73`–`0x00642ED5`, `0x00642F04`; read drlg
   +0x94, +0x484, +0x474) to confirm the computed vectors.
2. `0x00642920` overflow (edge case 2): can any 1.14d act layout fill all
   8 slots of a record? Check the outdoor placer's calls per act.
3. Does the server call the spawn-room choice (§10) with index ≠ 13 for
   levels with `Position` ≠ 0 (town arrivals)? Record a town arrival and
   an act change: draws at `0x0066ACB0`–`0x0066ACE0` on the level seed.
4. Warp-room centres (+0x1E0) are read only by `0x0054DB50` (server,
   via `0x0061AC10`): a point closer than levels `WarpDist` (squared
   distance) to any centre is rejected; owner: the monster population
   spec (claude/phase3-monsters). Confirm its caller `0x0054DC40` there.
5. When is game +0x7C set to the map ID in single player (`rng.md` OQ 2)?
   Watch writes to game +0x7C during a single-player join.
