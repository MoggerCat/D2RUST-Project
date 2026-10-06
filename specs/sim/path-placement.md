# Spec: Simulation — Unit position, collision footprints and placement

- **Status:** draft: every rule read from the 1.14d `Game.exe` code
  (addresses below; D2MOO 1.10f D2Common `Path*`, `D2Collision.cpp` and
  D2Game `SUnit.cpp` used as a map, differences noted); constant tables
  in `sim/path-tables.tsv` equal the executable's bytes
  (`py tools/trace-recorder/path_tables.py`: 582 rows, `--selftest` 582
  perturbations reported); `ExpField.D2` measured on d2data.mpq.
  Recorded vectors: arrival positions in
  `20261006-015956-packets.jsonl` / `20261006-022633-packets.jsonl`
  (Test vectors R1–R3). No position trace exists yet (open question 1).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::path` (path records, coordinates, collision
  queries, footprints, free-point searches, unit placement and warps)
- **Related specs:** `sim/pathing.md` (walk/run: path finding, per-tick
  movement; owns the dynamic path's movement fields); `drlg/rooms.md`
  §1 (active room layout), §6 (adjacency arrays), §10 (the collision grid
  and the tile bits); `drlg/levels.md` §7 (warp records), §10 (spawn
  room of a level); `sim/units.md` (unit record, lifecycle);
  `sim/unit-order.md` §5–§6 (room unit lists, update queue);
  `monsters/population.md` §8–§9 (monster spawn search: its own ring
  search, which calls the collision queries of §4 here);
  `world/waypoints.md` §7 (waypoint travel calls §10–§11 here);
  `sim/intents-events.md` (message transport; S→C layouts in
  `sim/server-messages.tsv`).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 54–68 |
| Inputs | 69–77 |
| Outputs / state changes | 78–83 |
| Rules | 84–85 |
|   1. Coordinates | 86–99 |
|   2. Path records | 100–200 |
|   3. Size, collision pattern, footprint mask | 201–237 |
|   4. Collision queries | 238–269 |
|   5. Footprints | 270–313 |
|   6. Moving a footprint | 314–337 |
|   7. Nearest free point (`0x0064DEA0`) | 338–400 |
|   8. Coarse free-box search (`0x0064E840`) | 401–429 |
|   9. Floor drop placement (`0x00555DA0`) | 430–448 |
|   10. Placing a unit at a point (`0x00554EA0`) | 449–483 |
|   11. Level spawn point (`0x0061B060`) and game entry | 484–506 |
|   12. Warp tiles and warp arrival | 507–545 |
| Constants & data dependencies | 546–564 |
| Randomness | 565–574 |
| Edge cases & original bugs | 575–601 |
| Test vectors | 602–634 |
| Provenance | 635–663 |
| Open questions | 664–688 |
<!-- /index -->

## Summary

Every unit has a position in sub-tiles (5 per tile) held by a path
record: players, monsters and missiles a dynamic path (16.16 fixed-point
position, movement state), objects, items and tiles a static path
(integer position). Units occupy the room collision grids (`drlg/rooms.md`
§10) with a footprint: a point, a plus-shaped 5-cell pattern or a 3×3
box, stamped with a per-kind bit. Placement asks the grid for a free spot
near a point: a growing square ring search (`0x0064DEA0`), a coarse
2-step box search (`0x0064E840`), or, for floor drops, the ring search
plus a walk-back test through the `ExpField.D2` direction field. A unit
is put at a spot by `0x00554EA0` (search, move the footprint, change
room, notify); level entry and warps compute the spot from the level's
spawn room or the destination warp tile.

## Inputs

| Name | Type | Source |
|---|---|---|
| active rooms | sub-tile rect, collision grid, adjacency array | `drlg/rooms.md` §1, §6, §10 |
| unit | type, class, mode, path record | `sim/units.md` §2 |
| tables | monstats, monstats2, objects, missiles, charstats, lvlwarp | `data/fields.tsv` |
| `data\global\ExpField.D2` | direction field (§7.3) | d2data.mpq |

## Outputs / state changes

Unit positions and rooms, collision grid bits (footprints), room unit
lists and update queue entries (`sim/unit-order.md`), unit flags 2
(+0xC8) bits 0x10000 / 0x800, S→C 0x07 and 0x0D, one type-14 timer.

## Rules

### 1. Coordinates

1. Sub-tile coordinates (x, y) are the game's unit positions; tile
   coordinates × 5 = sub-tile (`0x00643560`).
2. A dynamic path holds the position as two u32 16.16 values ("precise"):
   high 16 bits = sub-tile, low 16 bits = fraction. A unit at rest sits on
   a cell centre: fraction 0x8000 (`PATH_ToFP16Center`).
3. Client coordinates (for drawing; stored, never read by game logic
   here): dynamic path from the precise values, a = px >> 11, b = py >> 11
   (arithmetic), client x = (a − b) >> 1, client y = (a + b) >> 2
   (`0x00643290`); static path from sub-tiles, client x = (x − y) · 16,
   client y = (x + y) · 8 (`0x00643260`).
4. Square distance helper `0x006492A0`: dx² + dy² (no root).

### 2. Path records

#### 2.1 Which kind has which

Unit +0x2C points to the path. Types 0 player, 1 monster, 3 missile: a
dynamic path. Types 2 object, 4 item, 5 tile: a static path. Every unit
position getter (D2MOO `UNITS_GetXPosition`) follows this split: static
x/y at path +0x0C/+0x10, dynamic x/y = the sub-tile words at +0x02/+0x06
(`0x006488C0`, `0x00648900`); a type 0/1/3 unit without a path reads
(0, 0). Room of a unit (`0x00620BB0`): static path +0x00, dynamic path
+0x1C.

#### 2.2 Static path (0x20 bytes)

| Offset | Field |
|---|---|
| +0x00 | room |
| +0x04, +0x08 | client x, y |
| +0x0C, +0x10 | sub-tile x, y |
| +0x1C | direction (u8) |
| +0x1D | room-changed flag (u8; cleared by the setter) |

Set (`0x00620AE0(unit, room, x, y)`): room, x, y, client coordinates;
+0x1D := 0.

#### 2.3 Dynamic path (0x200 bytes, zeroed at allocation)

Fields this spec and `sim/pathing.md` use (D2MOO `D2DynamicPathStrc`
names; every offset read in the 1.14d functions named):

| Offset | Field | Notes |
|---|---|---|
| +0x00, +0x04 | precise x, y (16.16) | §1 |
| +0x08, +0x0C | client x, y | §1 rule 3 |
| +0x10, +0x12 | target x, y (u16) | `0x00648AD0` sets them and clears +0x58 |
| +0x14, +0x16 | previous target | pathing |
| +0x18, +0x1A | final target | pathing |
| +0x1C | room | |
| +0x20 | previous room | set on every room change |
| +0x24, +0x28 | current point index, point count | pathing |
| +0x30 | owner unit | |
| +0x34 | flags | table below |
| +0x38 | reset to 15 by a velocity change, 0 by a new path | not read in this spec |
| +0x3C, +0x40 | path type, previous path type | pathing §2 |
| +0x44 | unit size | §3 |
| +0x48 | collision pattern | §3 |
| +0x4C | footprint mask | §3 |
| +0x50 | move-test mask | §3 |
| +0x54 | collided-with mask (u16) | pathing |
| +0x58, +0x5C, +0x60 | target unit, its type, its GUID | `0x00648B90` |
| +0x64, +0x65, +0x66 | direction, new direction, turn step (u8) | pathing §8 |
| +0x68 | target lead (u8) | pathing §3.4 |
| +0x6A, +0x6E | direction vector x, y (i32, length 4096) | pathing §8 |
| +0x72, +0x76 | velocity vector x, y (i32, 16.16 per tick) | pathing §8 |
| +0x7C, +0x80, +0x84, +0x88, +0x8C | velocity, saved velocity, max velocity, acceleration, acceleration counter | pathing §7 |
| +0x90, +0x91, +0x92, +0x93 | distance budget, max path distance, IDA* start score, stop distance (u8) | pathing |
| +0x98 | direction offset of the path type | pathing §2 |
| +0x9C | points: 78 × {u16 x, u16 y} | pathing |
| +0x1D4, +0x1D8 | saved-step count, 10 × {u16 x, u16 y} | §6, pathing §9 |

Flags (+0x34): 0x1 a path point (or the destination of a move) lies
outside the current room; 0x2 room changed since the room-change
messages (§10 rule 6, pathing §9); 0x4 move footprints without testing;
0x8 the last step crossed at least one cell; 0x10 set by the allocation
argument, keeps the target; 0x20 a path is active; 0x200 face away (−32
on the computed direction); 0x800, 0x1000, 0x2000, 0x4000, 0x8000,
0x10000, 0x20000 path-type flags (pathing §2); 0x40000 missile path.

#### 2.4 Dynamic path allocation (`0x00649D00(pool, flag, x, y, unit, set0x10)`)

1. Allocate 0x200 bytes, zero them; unit +0x2C := path; owner := unit.
2. Size := §3 size; pattern := §3 pattern of that size.
3. Position := (x, y) cell centres; velocity (+0x7C) := 0x800; room
   (+0x1C) := the room argument; saved-step count := 1, saved step 0 :=
   (x, y).
4. Per type:
   - player: footprint mask 0x80, move mask 0x1C09, path type 7
     (`0x00648CF0`), max distance 73, IDA* score 70;
   - monster: footprint mask 0x100; flags &= ~0x7FF00, type 2, direction
     offset 0; monstats `BaseId` row 38 (wraith1): pattern 5, move mask
     0x804; else move mask from `0x00648480`: monstats `flying` → 0x1804,
     else `opendoors` → 0x3401, else 0x3C01; max distance 14. (A previous
     type 11 or 8 is a fatal assert.)
   - missile: masks 0, type 4.
5. If a room was given: stamp the footprint (§5.2) and put the unit in
   the room's unit list (`0x0064C350`, `sim/unit-order.md` §5).
6. Client coordinates; if `set0x10`: flags |= 0x10.

#### 2.5 Adding a unit to the world (`0x00554850`, D2MOO `SUNIT_Add`)

Unit, x, y, game, room, flag. Act := act of the room's level (unit
+0x18, +0x1C). Then per type: player and monster: dynamic path
allocation (§2.4, pool game +0x1C); monster: `0x005735A0`, then
`0x00573780` or `0x00553220` (monster spec); object: static set, the
footprint only when objects `HasCollision[mode]` (+0x120 + mode) ≠ 0,
room list insert; missile: dynamic path allocation; item: only in mode 3
(on the floor): static set, footprint, room list insert without queueing
(`0x0064C2C0`); tile: static set, footprint, room insert. Then hash
insert `0x00553060` (`unit-order.md` §3.1), queue for update if the unit
has a room, and unless `flag`: room-changed flag set (`0x00620FA0`).

### 3. Size, collision pattern, footprint mask

Size (`0x00620510`) and footprint mask (`0x006209D0`):

| Unit type | Size | Footprint mask |
|---|---|---|
| 0 player | 2 | path +0x4C (0x80) |
| 1 monster | monstats2 `SizeX` (+0x08, signed) | path +0x4C (0x100) |
| 2 object | objects `SizeX` (+0xD0); size Y `SizeY` (+0xD4) | `IsDoor` = 0: `SubClass` & 4 → 0x8000, else 0x400, + 4 if `BlockMissile`; `IsDoor` ≠ 0: `BlocksVis` → 0x806, else `BlockMissile` → 0x804, else 0x400 |
| 3 missile | missiles `Size` (+0x18A) | path +0x4C (0) |
| 4 item | 1 | 0x200 |
| 5 tile | 0 | 0x1 |

Pattern from size (`0x00648580`, table `pattern_of_size`): size 0 → 0,
1 → 1, 2 → 1, 3 → 2, any other (negative included, unsigned compare) →
1. A monster that can be in town (`0x0063E860`: monstats `npc` or
`inTown`, or unit flags bit 31) and whose monstats `interact` bit is
clear gets 1 → 3 and 2 → 4.

| Pattern | Cells | Extra marker on stamp |
|---|---|---|
| 0 | the cell | — |
| 1 small unit | plus: (x,y), (x±1,y), (x,y±1) | 0x1000 (NO_PATH) on (x,y) |
| 2 big unit | 3×3 box | 0x1000 on the five plus cells |
| 3 small pet | plus | 0x2000 (PET) on (x,y) |
| 4 big pet | 3×3 box | 0x2000 on the plus cells |
| 5 small, no presence | plus | — |

Size-based shapes (missiles, items, tiles, and the size queries of §4):
0 or 1 → the cell; 2 → plus; 3 → 3×3 box; objects use a sizeX × sizeY
box (§4 rule 4).

Collision bits: `drlg/rooms.md` §10.6. Masks used here: 0x1C09 player
move / placement (WALL, NOPLAYER, OBJECT, DOOR, NO_PATH); 0x3C01 monster
move (WALL, OBJECT, DOOR, NO_PATH, PET); 0x3E01 item floor (WALL, ITEM,
OBJECT, DOOR, NO_PATH, PET); 0x801 walk-back field (WALL, DOOR).

### 4. Collision queries

1. **Cell lookup** (`0x00463740(room, x, y)`): the room itself if its
   sub-tile rect (room +0x4C x, +0x50 y, +0x54 w, +0x58 h) contains the
   cell; else the first room of its adjacency array (`drlg/rooms.md` §6
   order) that contains it; else none. A null room gives none.
2. **Cell value**: grid of the found room (`drlg/rooms.md` §10.3 index);
   no room or no grid → the value **0x27**, returned unmasked (so it
   collides with any non-zero mask).
3. **Plus and point queries** start from the cell's room (rule 1 from
   the given room) and look each neighbour up from that room when it
   lies on the room's edge (D2MOO `COLLISION_CheckCollisionMaskWith
   AdjacentCells`); the result equals the OR of the five cells' masked
   values with rule 2 per cell. A centre whose room is missing gives
   0x27 / "collides".
4. **Box queries** (3×3, objects' X×Y): box left = x − sx/2, bottom =
   y − sy/2 (unsigned halving), right = left + sx − 1, top = bottom + sy −
   1. The room of (left, bottom) is looked up; none → 0x27 / collides.
   The box is clipped at that room's right and top edges into up to three
   boxes (the inside one, the strip right of the room, the strip above
   it at the inside box's width); each outside strip is queried again
   from that room (recursively). The inside box ORs its cells.
5. Query functions (room, x, y, size or pattern, mask):

| Function | Shape argument | Result |
|---|---|---|
| `0x0064CB30` | — (point) | masked value (rule 2) |
| `0x0064D9B0` | size 0, 1 point; 2 plus; 3 box; other → 0xFFFF | OR of masked values |
| `0x0064D870` | pattern 0 point; 1, 3, 5 plus; 2, 4 box; other 0xFFFF | OR of masked values |
| `0x0064D910` | pattern as above, other → 1 | 1 if any cell collides, else 0 |
| `0x0064DE30` / `0x0064DC00` | sizeX × sizeY box | set / clear bits |

### 5. Footprints

#### 5.1 Stamp and clear primitives

Set ORs the mask into each cell of the shape; clear ANDs its
complement; cells without a room are skipped. Pattern stamp
`0x0064EA90(room, x, y, pattern, mask)` and pattern clear `0x0064EC10`
apply the §3 cells with the mask, then, **only when the mask is not 0**,
the marker (set: OR the marker; clear: AND it out). Clear with a null
room does nothing. Size stamp `0x0064EA00` / clear `0x0064EBA0`: size 1
cell, 2 plus, 3 box, others nothing (no marker).

#### 5.2 Per unit kind

Add (`0x00649400(unit)`), at the unit's position and room, with its
footprint mask:

| Type | Shape |
|---|---|
| 0, 1 | pattern (path +0x48) |
| 2 | sizeX × sizeY box (`0x0064DE30`) |
| others | size (§3) |

Remove (`0x00649560(unit, force)`), returns whether it cleared:

| Type | Cleared when | Shape |
|---|---|---|
| 0 player | force, or mode not 0 (DT) and not 17 (DD) | pattern |
| 1 monster | force, or mode not 0 (DT) and not 12 (DD) | pattern |
| 2 object | force, or `HasCollision[mode]` ≠ 0 | box |
| others | always | size |

#### 5.3 Changes of shape or mask

1. Footprint mask change (`0x00648C30(path, mask)`): clear the old
   footprint with the **current** pattern and old mask (missiles: size),
   store the mask, stamp it again.
2. Pattern set (`0x00649190`) writes path +0x48 without restamping.
3. Dead body (allocation of a dead player or monster, `sim/units.md`
   §3.1 step 8, and death code): remove (force), pattern := 5, mask :=
   0x8000 (rule 1 restamps the plus with 0x8000; pattern 5 adds no
   marker).
4. Unit removal clears with force (`0x00649F50`).

### 6. Moving a footprint

1. **Try move** (`0x0064EDA0(room, old, new, pattern, foot, test)`):
   clear the footprint at old; r := pattern query (`0x0064D870`) at new
   with the test mask; r ≠ 0 → stamp at old again and return r; r = 0 →
   stamp at new, return 0. One room (the path's) serves both points;
   cells are looked up from it (§4 rule 1).
2. **Forced move** (`0x0064EFA0`): clear at old, stamp at new, no test;
   room null → nothing.
3. **Missile move** (`0x0064ED20`, size shapes): clear at old, query at
   new; stamp at new unless the result has 0x1 or 0x4, else at old;
   returns the result.
4. **Teleport** (`0x00650910(path, room, x, y)`, D2MOO
   `sub_6FDAD5E0`), always succeeds:
   - missile: (0, 0) → clear (size); else flags 0x8 := moved, collided
     mask := size query at new with the move mask, footprint moved
     without condition, saved steps := {(x, y)};
   - others: (0, 0) → clear (pattern); else forced move (rule 2);
   - flags |= 0x1 if the room differs; position := the cell centre with
     the room recache of `sim/pathing.md` §9.6 (destination room as the
     hint); a non-zero point without a room is a fatal assert;
   - movement reset (pathing §9.7). `0x00650BE0` wraps it and sets the
     point count to 0.

### 7. Nearest free point (`0x0064DEA0`)

#### 7.1 Arguments and wrappers

`0x0064DEA0(room, &point, field origin or null, size, mask, field mask,
allow-fallback, max distance, step)`; the point is updated in place; the
result is a room or null.

| Wrapper | Field | Max distance | Step | Fallback | Callers (1.14d) |
|---|---|---|---|---|---|
| `0x0064E7B0(room, &pt, size, mask, fallback)` | none | 50 | 1 | argument | §10, §11, §12; objects `0x00582A30`, `0x00584870`, `0x00584D00`, `0x005A6EB0`; skills `0x005C5430`, `0x005D89B0`, `0x005D9A10`, `0x005D9D50`, `0x005F3170`, `0x005FD0F0`; `0x0059F580`; `0x00645CA0` |
| `0x0064E7E0(room, &pt, size, mask, step)` | none | 50 | argument | 0 | `0x005352C0`, `0x0053ACC0`, `0x0056CF40`, `0x0059DFD0`, `0x005EE040` |
| `0x0064E810(room, &pt, &origin, size, mask, fmask, fallback)` | origin | 50 | 1 | argument | §9 and `items/treasure.md` §7 |

#### 7.2 Search

With (X0, Y0) the requested point, k the step, D the max distance:

1. "Free" at (x, y): the cell has a room (§4 rule 1, looked up from the
   **hint room**), the size query (`0x0064D9B0`) with `mask` is 0, and,
   with a field, the walk-back test (§7.3) passes. The hint starts as the
   argument room; every successful lookup replaces it.
2. (X0, Y0) free → return its room; the point is unchanged.
3. If D > 1, rings r = 1, 2, …: with L = X0 − (r−1)k, R = X0 + (r−1)k,
   T = Y0 − 1 − (r−1)k, B = Y0 + 1 + (r−1)k, s = 2 + 2(r−1)k:
   1. Columns: for y = T, T+k, … ≤ B; for x = L − 1, then L − 1 + s
      (= R + 1) — the two side columns, top to bottom, left before right.
   2. Rows: for x = L, L+k, … ≤ R; for y = T, then T + s (= B) — the top
      and bottom rows between the columns, left to right, top before
      bottom.
   3. Each free candidate has d = |x − X0| + |y − Y0|; it replaces the
      kept point only when no point is kept in this ring or d < the kept
      d (first found wins ties).
   4. A ring that kept a point ends the search: the point is written.
      Otherwise the next ring runs while rk + 1 < D (k = 1, D = 50: rings
      1–49).
4. Result: found → the room at the written point (looked up from the
   hint). Not found → with fallback: the room at the **unchanged** point
   (may be one that collides); without: null.

With k = 1, ring r is the square of Chebyshev radius r; with k > 1 the
rings skip cells.

#### 7.3 Walk-back field (`ExpField.D2`)

1. File `data\global\ExpField.D2` (d2data.mpq, 65,546 bytes, measured):
   u16 at +0 (0x010A, unused), u32 height at +2 (256), u32 width at +6
   (256), then height × width bytes, row-major. Loaded by `0x0066A2B0`;
   centre = (width >> 1, height >> 1) = (128, 128); the row stride used
   by the readers is a fixed 256.
2. Each byte is a direction 0..8 toward the centre: offsets (dx, dy) =
   0 (0,−1), 1 (1,−1), 2 (1,0), 3 (1,1), 4 (0,1), 5 (−1,1), 6 (−1,0),
   7 (−1,−1), 8 (0,0) (tables `0x00749780` / `0x007497A4`,
   `field_dx`/`field_dy`). Only the centre holds 8 (measured: 1 cell);
   following the directions from every cell reaches the centre without
   leaving the grid (measured, all 65,536 cells).
3. Test (`0x0066A670(field, room, x, y, fmask)`): room null → passes.
   The point test (`0x0064CB30`, from `room`) at (x, y) ≠ 0 → fails.
   Then repeat: step (x, y) by the direction stored at (x − ox + 128,
   y − oy + 128) (origin (ox, oy)); stop with "passes" when the byte at
   the new cell is 8; the new cell's point test ≠ 0 → fails. All point
   tests use the room the search passed, not a per-cell room.

### 8. Coarse free-box search (`0x0064E840`)

`0x0064E840(room, &point, n, mask, &out room)`, D2MOO `D2Common_10136`:

(X0, Y0) = the point on entry. "Rect" = the sub-tile rect last read
(initially `room`'s; each cell visit below reads its row room's rect).

1. Passes j = 1, 2, …, 49. Pass j visits dy = −j, −j + 2, … while
   dy < j (rows), and in each row dx = −j, −j + 2, … while dx < j; pass 1
   visits only (X0 − 1, Y0 − 1), pass 2 the offsets {−2, 0}², and so on
   (odd passes odd offsets, even passes even offsets).
2. Row: y = Y0 + dy is written to the point. Row room := `room` if y is
   inside the rect's rows, else the cell lookup from `room` at (the
   point's current x — the previous cell's x, X0 before any — , y). No
   row room → next row.
3. Cell: x = X0 + dx is written to the point; read the row room's rect;
   cell room := the row room if x is inside its columns, else the cell
   lookup from the row room at (x, y). With a cell room: box query
   (`0x0064CEB0`, (n + 2) × (n + 2) box centred on (x, y) by §4 rule 4;
   n + 2 < 2 → the cell's grid value, 0x27 without a grid) with `mask`
   = 0 → out room := the cell room and stop; the point holds (x, y).
4. After pass 49: out room := null; the point holds the last written
   coordinates.

Later passes revisit the earlier offsets of the same parity; the first
free cell in scan order wins.
Callers: `0x005424F0` (inactive unit restore), `0x00582420` (objects),
`0x005A09E0` (monsters).

### 9. Floor drop placement (`0x00555DA0`)

The rule items use to put a dropped item on the floor; 20 call sites
(`0x0055957x`–`0x005AF474`, item and object code).
`0x00555DA0(room, &from, &out, size, fallback)`:

1. Start := (x + 2, y + 3) if a cell lookup from `room` at (x + 2, y + 3)
   finds a room, else (x, y), where (x, y) = `from`.
2. `0x0064E810(room, &start, field origin = from, size, mask 0x3E01,
   field mask 0x801, fallback)` (§7): the ring search around the start,
   each candidate also needing a WALL/DOOR-free walk back to `from`
   through the field.
3. Out := the written point; result := the room (null: no place; with
   fallback the unchanged start's room).

`items/treasure.md` §7 step 2 calls `0x0064E810` with the same
arguments directly (size 1, fallback 1). Item size is 1, so the size
query is a single cell against 0x3E01.

### 10. Placing a unit at a point (`0x00554EA0`)

`0x00554EA0(game, unit, room, x, y, exact, alt flag)` (D2MOO
`sub_6FCBDFE0`), result 1 placed / 0 not:

1. Unit null or without path → fatal assert.
2. Room null: cell lookup with a null hint (always none, dead code), then
   from the unit's current room; none → 0.
3. Unless `exact`: free point (§7, `0x0064E7B0`, size of the unit, mask
   0x1C09, no fallback) from that room; none → 0.
4. Teleport (§6 rule 4) to the point (cannot fail).
5. Not a player: queue for update (`0x0064C040`); unit flags 2 (+0xC8)
   |= 0x10000 (`alt` = 0) or 0x800 (`alt` ≠ 0); room-change messages
   (`sim/pathing.md` §9.8); result 1.
6. Player: client of the player (`0x005531C0`); S→C 0x07 (MapReveal:
   tile x, tile y of the destination room, its level id; builder
   `0x0053BC50`); queue for update; flags 2 as in rule 5; room-change
   messages; player data +0x148 / +0x14C := x, y; host-only position
   history (rule 7); timer event 14 at frame + 50 with callback
   `0x00554570` (`sim/units.md` §6); pets follow (`0x005754B0`, the
   pet/mercenary spec); result 1.
7. Position history (player data +0xA0 index u8, +0xA4 `GetTickCount`,
   +0xA8 + 8·i: 20 × {x, y}): written here and in the walk step
   (`sim/pathing.md` §9.2) from wall-clock time; nothing in the
   simulation reads it (open question 4). d2rs keeps it out of `d2-sim`.

The unit's +0xC8 flags drive S→C 0x15 in the update pass
(`sim/pathing.md` §10.3). Callers: level warp `0x0053AEC0` (§11), warp
arrival `0x005550B0` (§12.2), C→S 0x5F handler `0x0054CC40` (exact 0,
alt 1), waypoint travel (`world/waypoints.md` §7), town portal and
object code `0x00581B50`, `0x00584870`, `0x00584D00`, monsters
`0x0059F580`, skills `0x005C5430`, `0x005CA360`, `0x005CB170`,
`0x005CCC80`, `0x005D1AB0`, `0x005D6020`, `0x005D7850`, `0x005E3930`,
`0x005E3EA0`, `0x005ED9E0`, `0x005EF710`, `0x005F2FC0`.

### 11. Level spawn point (`0x0061B060`) and game entry

`0x0061B060(act, level, tile index, &x, &y, size)`:

1. Act null → fatal. Spawn room and position: `drlg/levels.md` §10
   (`0x0066B2B0`, may draw on the level seed). None → 0.
2. Position × 5 (sub-tiles), then **+3 on both axes**.
3. Free point (§7, `0x0064E7B0`, mask 0x1C09, no fallback) from the
   spawn room; none → **fatal assert**.
4. Out := the point; result := the cell lookup there from the found room.

Callers: game/act entry `0x005394A0` (tile index 0; level = the act's
start level, act +0x08) and its follower placement (`0x005352C0`),
level warp `0x0053AEC0` (same act; the tile index from the caller; an
act change goes to `0x00537340` + `0x0053ACC0`, the act-change spec),
`0x0053ACC0`, `0x0056CF40`, `0x00584870`, `0x00584D00`, `0x0059DFD0`.

Game entry (`0x005394A0`, player not yet placed): spawn point as above;
S→C 0x07 for the spawn room; `0x00554850` puts the player in the world
(§2.5); S→C 0x15 (`0x0053BC10`: type, GUID, x, y, flag 1) at once.
Level warp `0x0053AEC0` places with `0x00554EA0(exact 0, alt 0)`, so the
free search runs twice (the second finds the same point).

### 12. Warp tiles and warp arrival

#### 12.1 Warp tile preset (`0x0066E1C0`, the `LevelTypes::warp_unit` seam)

Called by the tile grid fill (`drlg/rooms.md` §9.5.1 step 3) for a
hidden exit cell of type t (10 or 11) at world tile (wx, wy) with packed
value v, DRLG room R:

1. Direction letter: t = 11 → 'r', else (10) 'l' (read at
   `0x0066E1C4`–`0x0066E1D5`; D2MOO `DRLGROOMTILE_AddWarp` agrees: the
   right exit type takes 'r'). Earlier text had the letters swapped. Warp slot = main index
   (v bits 20–25). lvlwarp record: `drlg/levels.md` §7 rule 4 for R's
   level, that slot and letter (no record → fatal).
2. Local tile (lx, ly) = (wx − R tile x, wy − R tile y). If lx = R tile
   width **or** ly = R tile height (the shared far edge), nothing.
3. Else add a preset unit to R's list (`0x0066BF30`, prepended):
   type 5 (tile), class = the record's `Id`, mode 0, position (5·lx +
   `OffsetX`, 5·ly + `OffsetY`) relative to the room. Result 1.

#### 12.2 Walking into a warp (`0x005550B0(game, player, tile)`, D2MOO `SUNIT_WarpPlayer`)

1. Game or player null → fatal. Destination tile and its lvlwarp record:
   `0x006195A0(room of the tile, tile class)`; none → nothing.
2. Point := the destination tile's position; free point (§7,
   `0x0064E7B0`, player size, 0x1C09, **fallback 1**) from the
   destination tile's room; none → nothing.
3. Quest gate: destination level 73, 100, 118, 128 or 132 and
   `0x00545B80(source level, destination level)` ≠ 0 → nothing.
4. `0x00554EA0(destination tile room, x, y, exact 0, alt 0)`; failure →
   nothing.
5. Walk-out: target := (x + `ExitWalkX`, y + `ExitWalkY`) (lvlwarp
   +0x14, +0x18); player mode request: walk (mode 2) to the target
   (`0x005809D0(no skill, 2, target, 0)`, `sim/pathing.md` §1).
6. S→C 0x0D (`0x0053B4B0`): unit type, GUID, 1, target x, target y, 0,
   life percent (`0x00621F20`).

Callers: `0x00548C32` (C→S 0x13 on a warp tile), `0x00581F3A`,
`0x00582027`, `0x0059D9EF`.

## Constants & data dependencies

| Constant | Value | Where |
|---|---|---|
| player size / masks | 2 / footprint 0x80, move 0x1C09 | `0x00620510`, `0x00649D00` |
| missing-room value | 0x27 | `0x0064CB30` and the other point readers |
| free search | max distance 50, step 1 | `0x0064E7B0`, `0x0064E810` |
| floor drop | start (+2, +3), mask 0x3E01, field mask 0x801 | `0x00555DA0` |
| coarse search | 49 rings, step 2, box n + 2 | `0x0064E840` |
| spawn offset | +3, +3 sub-tiles | `0x0061B060` |
| type-14 timer | frame + 50 | `0x00554EA0` |

Tables: `pattern_of_size`, `field_dx`, `field_dy` in `sim/path-tables.tsv`
(columns documented in `sim/pathing.md` Constants). Data columns:
monstats `BaseId`, `flying`, `opendoors`, `npc`, `inTown`, `interact`;
monstats2 `SizeX`; objects `SizeX`, `SizeY`, `HasCollision0..7`,
`IsDoor`, `BlocksVis`, `SubClass`, `BlockMissile`; missiles `Size`;
lvlwarp `Id`, `ExitWalkX/Y`, `OffsetX/Y`, `Direction`.

## Randomness

No function of this spec draws. Draws on the paths that use it belong
to their owners: the spawn tile pick of `drlg/levels.md` §10 (level
seed), the mode request of `sim/pathing.md` §1.3 (unit seed, state 42),
the 0x5F handler `0x0054CC40` (`roll(100)` on the player's seed for its
state-108 timer; owner: the C→S 0x5F spec, open question 6). Ring order
(§7.2) and the scan order (§8) are deterministic and must be followed
exactly: they decide which free cell wins.

## Edge cases & original bugs

Reproduced by default.

1. A missing room reads 0x27 regardless of the mask, so placement never
   chooses a cell outside every active room, and a query with mask 0
   still "collides" there.
2. §7: a ring stops at the first ring with any free cell even when a
   cell of the next ring is closer in Manhattan distance (ring 5 corner
   d = 10 beats ring 6 side d = 6, vector P1).
3. §7 with fallback returns the room at the unchanged point, which may
   be blocked (warp arrival §12.2 uses fallback 1).
4. §7.3 walks the field with the search's room for every point test, so
   a cell of the walk outside that room and its neighbours counts as
   blocked (0x27).
5. §8 rings restart from the centre each time (ring j rescans ring j−1's
   cells); the out point holds the last tested cell on failure.
6. §11 rule 3: a spawn with no free cell within 49 rings is a fatal
   assert.
7. §10 rule 2: the first room lookup passes a null hint and can never
   succeed.
8. §5.3 rule 1 clears the old footprint with the current pattern; after
   rule 2 changed the pattern, the old shape is not what is cleared
   (dead-body sequence §5.3 rule 3 is safe because it removes first).
9. §12.1: an exit cell on the room's far column or far row adds no warp
   tile (only one of the two needs to match).

## Test vectors

Synthetic vectors: one room covering sub-tiles [0, 20) × [0, 20) (no
neighbours), all masks 0 except the listed cells set to 0x1 (WALL).
Expected values come from the rules above (scratch model
`model.py`); they are the unit tests for `d2-sim::path`.

| Id | Grid | Call | Expected |
|---|---|---|---|
| P1 | walls on x 5..14, y 5..14 | §7, (10,10), size 2, 0x1C09, k 1 | (15, 15) (ring 5; the plus of every ring-5 side cell touches a wall) |
| P1b | same | size 1 | (15, 10) (ring 5; (10, 15) has the same d, found later) |
| P2 | walls (10,10), (9,10), (11,10), (10,9), (10,11), (9,9), (11,11) | §7, (10,10), size 2 | (12, 9) |
| P2b | same | size 1 | (11, 9) |
| P3 | as P1 | §7, k = 2 | (15, 15) |
| P4 | empty | §7, (10,10) | (10, 10), unchanged, ring loop not entered |
| P5 | empty, point (−1, 5) outside the room | §7, size 1 | (0, 5) (ring 1, d = 1) |
| P5b | same | size 2 | (1, 5) (ring 1 has no free plus; ring 2, d = 2) |
| F1 | `ExpField.D2` | bytes at (dx, dy) = −1..+1 around the centre, rows dy = −1, 0, +1 | `3 4 5` / `2 8 6` / `1 0 7` (measured) |
| F2 | `ExpField.D2`, empty grid | walk-back from (103, 98) to origin (100, 100) | (102, 99), (101, 100), (100, 100): passes |
| F3 | same | from (96, 104) | (97, 103), (98, 102), (99, 101), (100, 100): passes |
| D1 | 20×20, walls x = 12 (all y) | §9 drop from (10, 10), size 1 | start (12, 13) is a wall; ring 1: (11, 12) d 2 kept, (13, 12) fails the walk-back, (11, 13) d 1 wins → (11, 13) |
| D2 | 20×20 empty | §9 drop from (10, 10) | (12, 13) (start free, walk-back passes) |
| D3 | empty, item bit 0x200 at (12, 13) | §9 drop from (10, 10) | (11, 13) |

Real vectors (recordings, `traces/raw/`, reproducible once the DRLG of
the recorded game is regenerated from its seeds):

| Id | Recording | Event | Expected |
|---|---|---|---|
| R1 | `20261006-015956-packets.jsonl` frame 1 | game entry §11, S→C 0x15 | `15 00 01000000 ff12 1516 01`: player GUID 1 at (4863, 5653), Rogue Encampment |
| R2 | `20261006-022633-packets.jsonl` frame 1 | same | `15 00 01000000 4112 c411 01`: (4673, 4548); preceded by ten 0x07 for the spawn room and its neighbours |
| R3 | `20261006-022633-packets.jsonl` frames 132–133 | waypoint travel (`world/waypoints.md` §7) through §11 and §10 | 0x07 `07 d003 e003 03` (tile 976, 992, level 3), 0x0D at (4896, 4996), next tick 0x15 at (4893, 4993) flag 1 |

## Provenance

- 1.14d functions read (decompile exports and `tools/ghidra/disasm.py`):
  path records `0x00649D00`, `0x00620AE0`, `0x006488C0`, `0x00648900`,
  `0x00620BB0`, `0x00648C30`, `0x00649190`; size and masks `0x00620510`,
  `0x006205A0`, `0x006209D0`, `0x00648580`, `0x00648480`, `0x0063E860`;
  queries `0x00463740`, `0x0064CB30`, `0x0064CA50`, `0x0064D100`,
  `0x0064D450`, `0x0064D4E0`, `0x0064D870`, `0x0064D910`, `0x0064D9B0`,
  `0x0064CEB0`; stamps `0x0064EA90`, `0x0064EC10`, `0x0064EA00`,
  `0x0064EBA0` (jump tables `0x0064EB88`, `0x0064ED00`); moves
  `0x0064EDA0`, `0x0064EFA0`, `0x00650910`, `0x00650BE0`; footprints
  `0x00649400`, `0x00649560`, `0x00649F50`; searches `0x0064DEA0`,
  `0x0064E7B0`, `0x0064E7E0`, `0x0064E810`, `0x0064E840`, field
  `0x0066A2B0`, `0x0066A5D0`, `0x0066A670`; placement `0x00554850`,
  `0x00554EA0`, `0x00555DA0`, `0x0061B060`, `0x005394A0`, `0x0053AEC0`,
  `0x005550B0`, `0x0066E1C0`, `0x0066BF30`; builders `0x0053BC50`,
  `0x0053BC10`, `0x0053B4B0`. Caller lists from `disasm.py xref`.
- D2MOO 1.10f as a map: `PATH_AllocDynamicPath`,
  `PATH_Add/RemoveCollisionFootprintForUnit`,
  `COLLISION_GetFreeCoordinatesImpl`, `D2Common_10136`,
  `D2Common_11099` (field), `SUNIT_WarpPlayer`, `sub_6FCBDFE0`.
  1.14d differences: the size → pattern lookup treats sizes > 3 (and
  negative) as pattern 1; monster pattern changes for town units test
  `npc`/`inTown`/flag bit 31 and the `interact` bit; the first room
  lookup of `0x00554EA0` has a null hint.
- Measured: `ExpField.D2` (d2data.mpq via `mpq-tool extract`): size,
  header, byte histogram (8 only at the centre), reachability of the
  centre; charstats / monstats rows quoted from the live patch_d2 tables.

## Open questions

1. No recording logs unit positions: record per-tick path state (see
   `sim/pathing.md` open question 1) and the placement calls (`0x0064DEA0`
   entry/exit, `0x00554EA0` arguments/result) to check §7 and §10 on
   live data.
2. §4 rule 4 sub-box recursion: confirm on a box straddling two room
   edges (corner) that the strip above uses the inside box's clipped
   width (D2MOO's reading; `0x0064CDF0`/`0x0064CC30` not traced
   line by line). Settle: Ghidra on `0x0064CDF0`, or a recorded drop at a
   room corner.
3. `0x006195A0` (destination warp tile of a source tile) is named but
   not specified: which tile and lvlwarp record it returns for a level
   with several warps. Settle: Ghidra `0x006195A0` (with
   `drlg/levels.md` §7).
4. Player position history (§10 rule 7): which code reads player data
   +0xA0..+0x14C (anti-cheat, C→S 0x5F?). Settle: xref the readers.
5. `0x00545B80` quest warp gate (§12.2 rule 3): owner is the quests
   spec; its result for each level pair is not specified.
6. C→S 0x5F handler `0x0054CD50` → `0x0054CC40`: the state-108 timer and
   its `roll(100)` table `0x006E1064` are not specified (owner: a client
   resync spec). Settle: Ghidra `0x0054CD50`; record a 0x5F.
7. Pets following a teleport (`0x005754B0`): owner is the pet /
   mercenary spec.
