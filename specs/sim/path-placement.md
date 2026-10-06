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
|   2. Path records | 100–203 |
|   3. Size, collision pattern, footprint mask | 204–244 |
|   4. Collision queries | 245–325 |
|   5. Footprints | 326–378 |
|   6. Moving a footprint | 379–411 |
|   7. Nearest free point (`0x0064DEA0`) | 412–476 |
|   8. Coarse free-box search (`0x0064E840`) | 477–509 |
|   9. Floor drop placement (`0x00555DA0`) | 510–532 |
|   10. Placing a unit at a point (`0x00554EA0`) | 533–582 |
|   11. Level spawn point (`0x0061B060`) and game entry | 583–624 |
|   12. Warp tiles and warp arrival | 625–685 |
| Constants & data dependencies | 686–704 |
| Randomness | 705–714 |
| Edge cases & original bugs | 715–750 |
| Test vectors | 751–788 |
| Provenance | 789–817 |
| Open questions | 818–885 |
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
| +0x94 | monster re-path budget (u8) | pathing §9.10 |
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
   - missile: masks 0, type 4 through set type (`0x00648CF0`,
     `sim/pathing.md` §2: flags get the table's 0x60000, direction
     offset 0).
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
box (§4 rule 4). The size query `0x0064D9B0` reads exactly these cells
(jump table `0x0064DA2C`: sizes 0 and 1 the point read `0x0064D450`, 2
the plus `0x0064D100`, 3 the 3×3 box `0x0064D4A0`); the size stamp and
clear (§5.1) differ only at size 0, which they skip. A size above 1
never reduces to the single cell.

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
   Split (`0x0064CDF0`; boxes are inclusive {left, bottom, right, top};
   R, T = the room's sub-tile right and top edge, exclusive):
   - bottom > top or left > right, or the room has no collision record
     (`0x0061A010` null; R, T come from it, `drlg/rooms.md` §10) → no
     boxes → the query returns 0x27;
   - box 1 := the box; if right ≥ R: box 1 right := R − 1 and the
     right strip := {R, bottom, right, top} (**full height**, so the
     corner beyond both edges belongs to it);
   - if top ≥ T: box 1 top := T − 1 and the top strip := {box 1 left,
     T, box 1 right (already clipped), top};
   - order: box 1, right strip, top strip. Box 1 is read by
     `0x0064CC30`; each strip by `0x0064CEB0` again with the room found
     for (left, bottom) as the lookup start; results are ORed.
5. Query functions (room, x, y, size or pattern, mask):

| Function | Shape argument | Result |
|---|---|---|
| `0x0064CB30` | — (point) | masked value (rule 2) |
| `0x0064D9B0` | size 0, 1 point; 2 plus; 3 box; other → 0xFFFF | OR of masked values |
| `0x0064D870` | pattern 0 point; 1, 3, 5 plus; 2, 4 box; other 0xFFFF | OR of masked values |
| `0x0064D910` | pattern as above, other → 1 | 1 if any cell collides, else 0 |
| `0x0064DE30` / `0x0064DC00` | sizeX × sizeY box | set / clear bits |

6. **Unit at a point** (`0x00641CB0(room, x, y, accept, arg, r)`, D2MOO
   `D2Common_10407`; callers: missile collision `0x005AE3A4` with r =
   the missile's size, `missiles/missiles.md` §R4 rule 9; `0x00467729`
   with r = 1; `0x00663E75` with r from its caller; `0x004F10A8`,
   `0x004D32EB`). Returns the
   first accepted unit or null:
   1. Room null, or r outside 1..3 → null.
   2. Rooms: the room's adjacency array (`drlg/rooms.md` §6: contains
      the room itself) from index 0 to count − 1 (`0x00619790` reads
      room +0x00 and +0x24). Each room passes a near-rect test
      (`0x00641930`, margin 2 against the sub-tile rect at room +0x4C)
      that, as written, never rejects a room with non-negative width
      and height (it rejects only when x + 2 < left **and** x − 2 >
      right, or the same for y): every adjacent room is searched. A
      room without a unit list head (+0x74) is skipped.
   3. Units: the room unit list from its head (+0x74) along unit +0xE8
      (`sim/unit-order.md` §5 order). Skipped: type 0 (player) in mode
      0 or 17; type 1 (monster) in mode 0 or 12; types 2 (object), 4
      (item) and any type above 4. Missiles (type 3) are candidates,
      the searching missile included (the callback filters).
   4. Unit size s := §3 size (`0x00620510`); s ≤ 0 → skipped; s > 3 →
      3. Unit point (ux, uy) := the dynamic path's sub-tile position
      (path +0x2C null → (0, 0)).
   5. Hit test, d = (|x − ux|, |y − uy|): the query shape of size r
      (1 point, 2 plus, 3 3×3 box) overlaps the unit's shape of size s:

      | r \ s | 1 | 2 | 3 |
      |---|---|---|---|
      | 1 | d = (0, 0) | dx + dy ≤ 1 | dx ≤ 1 and dy ≤ 1 |
      | 2 | dx + dy ≤ 1 | dx + dy ≤ 2 | (dx ≤ 2 and dy ≤ 1) or (dx ≤ 1 and dy ≤ 2) |
      | 3 | dx ≤ 1 and dy ≤ 1 | (dx ≤ 2 and dy ≤ 1) or (dx ≤ 1 and dy ≤ 2) | dx ≤ 2 and dy ≤ 2 |

      (jump table `0x00641EF8`, index s − 1 + 3(r − 1)).
   6. On a hit, `accept(unit, arg)` (fastcall); non-zero → return the
      unit. Otherwise continue with the next unit, then the next room.

### 5. Footprints

#### 5.1 Stamp and clear primitives

Set ORs the mask into each cell of the shape; clear ANDs its
complement; cells without a room are skipped. Every cell (and the
marker cell) is looked up separately from the **room argument** (§4
rule 1), not from the centre's room; plus order (x−1, y), (x, y),
(x+1, y), (x, y−1), (x, y+1). Pattern 0 stamps and clears **nothing**
(its jump-table entry is empty; the pattern query still tests the
point), and patterns above 5 do nothing. Pattern stamp
`0x0064EA90(room, x, y, pattern, mask)` and pattern clear `0x0064EC10`
apply the §3 cells with the mask, then, **only when the mask is not 0**,
the marker (set: OR the marker; clear: AND it out). Clear with a null
room does nothing. Size stamp `0x0064EA00` / clear `0x0064EBA0`: size 1
cell, 2 plus, 3 box, others nothing (no marker). The clear tests the
room first (null → nothing) and dispatches through `0x0064EBFC`
(size 0 → return; 1 → `0x0064DB70` one cell; 2 → `0x0064DA40` plus;
3 → `0x0064DBC0` 3×3 box); the stamp tests the size by subtraction
(1, 2, 3; anything else returns).

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
| 2 object | force, or `HasCollision[mode]` ≠ 0 (`0x006219C0`: objects byte +0x120 + mode, no bound; `ObjMode.txt` has modes 0–7, a mode above 7 would read the next fields, `IsAttackable0`, `Start0`…) | box |
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
2. **Forced move** (`0x0064EFA0(room1, old, room2, new, pattern,
   foot)`): clear at old looked up from room1, stamp at new looked up
   from room2, no test; room1 null → nothing (no stamp either). The
   footprint move of `sim/pathing.md` §9.6 passes the path's room as
   both.
3. **Missile move** (`0x0064ED20`, size shapes): clear at old, query at
   new; stamp at new unless the result has 0x1 or 0x4, else at old;
   returns the result.
4. **Teleport** (`0x00650910(path, room, x, y)`, D2MOO
   `sub_6FDAD5E0`), always succeeds:
   - missile: (0, 0) → clear (size, path room), collided mask := 0;
     else flag 0x8 := 1 if the new cell differs from the old cell, else
     0; `0x0064EE70`: clear (size) at old from the path's room (if any),
     collided mask := size query at new from the **destination room**
     with the move mask, stamp at new from the destination room without
     condition; saved steps := {(x, y)};
   - others: (0, 0) → clear (pattern) from the path's room; else forced
     move (rule 2) with room1 = the path's room, room2 = the
     **destination room** argument (a warp to a room not adjacent to the
     old one still stamps at the destination);
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
   tests use the room the search passed, not a per-cell room: the
   candidate cell's room (the room rule 1's lookup just found, which is
   also the new hint; `0x0064DEA0` → `0x0066A670`).

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
   For n + 2 ≤ 1 the value is the cell's grid value **masked** with
   `mask`, read from the room the cell lookup finds from the cell room
   (no room or no grid → 0x27 unmasked). "Inside the rows / columns" is
   half-open: rect x ≤ x < rect x + w (likewise y).
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

The room argument of rule 2 is the caller's `room` unchanged
(`0x00555DEC`), not the room rule 1's lookup found. `items/treasure.md`
§7 step 2 (`0x0055A550`) is one of the callers: room of the dropper
(`0x00620BB0`), its position, size 1, fallback 1. The inventory drops
(`items/inventory.md` §9.1, §9.3) also pass size 1, fallback 1; the gold
piles (§10.2 there) call `0x0064E810` directly with fallback 0. Item size is 1, so the size
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
   messages; player data +0x148 / +0x14C := x, y; position history
   (rule 7); timer event 14 at frame + 50 with callback
   `0x00554570` (`sim/units.md` §6); pets follow (`0x005754B0`, the
   pet/mercenary spec); result 1.
7. Position history (player data +0xA0 next index u8, +0xA4 time of the
   last write (`GetTickCount`), +0xA8 + 8·i: 20 × {u32 x, u32 y}, a
   ring; `0x006221A0` gives the player data):
   - here (`0x00554FD0`): entry[index] := (x, y) unconditionally, +0xA4
     := now, index := index + 1, 20 → 0;
   - walk step (`0x00580C20`, `sim/pathing.md` §9.2, after the step):
     only when now > +0xA4 + 25 ms (unsigned); with p := index − 1 (0
     → 19), when the squared distance (`0x006492A0`: dx² + dy²) from
     the player's sub-tile position to entry[p] is > 45: entry[index]
     := position, index + 1 (20 → 0), +0xA4 := now.
   **It is read by the simulation**: monster AI helpers `0x005E3930`
   and `0x005E3EA0` (called from many AI functions) walk the target
   player's ring backwards from the newest entry to pick a past
   position as a move target (owner: `monsters/ai.md`). So the ring
   belongs in `d2-sim`. The only wall-clock input is the 25 ms gate of
   the walk-step write; the walk step runs at most once per player per
   tick and ticks are 40 ms apart at normal speed, so d2rs reads the
   gate as always open (deviation only when the original server
   catches up several ticks within 25 ms; not recorded).

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
On every 1.14d waypoint level the spawn search of waypoint travel ends at
the level's waypoint room (`drlg/levels.md` §10 rule 4: the first room
with a waypoint flag, no draw) for both `Position` values, so the second
search of `world/waypoints.md` §7 rule 7 (`0x00619E50`) returns the same
room as the placement's; the room roll (`0x0066AE70`) is never reached
there.

Game entry (`0x005394A0`, player not yet placed): spawn point as above
with size = the unit's size (`0x00620510`, 2 for a player) and tile
index 0; no spawn room → fatal assert; S→C 0x07 for the spawn room;
`0x00554850(flag 0)` puts the player in the world (§2.5; the
room-changed flag is set); S→C 0x15 (`0x0053BC10`: type, GUID, x, y,
flag 1) at once. Level warp `0x0053AEC0` (same act) passes the unit's
size too; no spawn room → nothing (the player stays); else it places
with `0x00554EA0(exact 0, alt 0)`, so the free search runs twice (the
second finds the same point).

Recipients: every message of §10–§12 goes to the moving player's own
client. The other 0x07 seen at game entry come from the first
per-client update's room switch (`sim/tick.md` §6 rule 5, `0x00537B50`):
for each room of the new room's adjacency array that was not in the old
one's (all of them at game entry) `0x0053A8E0` sends 0x07 for the room,
adds the client to the room, and sends the add messages (`0x00571F90`)
of every unit in it but the player (R2: one 0x07 from this section,
nine from the switch).

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
   `0x006195A0(room, class, &record)` (room null → fatal):
   1. Via `0x0066AB00` on the room's DRLG room S: in S's warp-link list
      (+0x4C, list order, next +4; `drlg/rooms.md` §3 rule 3 prepends)
      the first link whose lvlwarp record (+0x10) has `Id` = class
      gives the destination DRLG room D (link +0x00); in D's warp-link
      list the first link whose room is S gives the destination record
      R. Either not found → fatal assert (line 0x71). So with several
      warps in one room the tile class picks the link, and the
      destination is always D's link back to S.
   2. record := R (so rule 5's `ExitWalkX/Y` are the destination
      side's record); D's active room (+0x30), activated by
      `0x0061B730` when D has none.
   3. Result: the first unit of type 5 (tile) with class = R's `Id` in
      that active room's unit list (head +0x74, next +0xE8); none →
      null.
2. Point := the destination tile's position; free point (§7,
   `0x0064E7B0`, player size, 0x1C09, **fallback 1**) from the
   destination tile's room; none → nothing.
3. Quest gate: destination level 73, 100, 118, 128 or 132 and
   `0x00545B80(source level, destination level)` ≠ 0 → nothing
   (the checks: `world/quests.md` §8.2).
4. `0x00554EA0(destination tile room, x, y, exact 0, alt 0)`; failure →
   nothing.
5. Walk-out: target := (x + `ExitWalkX`, y + `ExitWalkY`) (lvlwarp
   +0x14, +0x18); player mode request: walk (mode 2) to the target
   (`0x005809D0(no skill, 2, target, 0)`, `sim/pathing.md` §1).
6. S→C 0x0D (`0x0053B4B0`): unit type, GUID, 1, target x, target y, 0,
   life percent (`0x00621F20`).

Rule 5 uses the point of rule 2, not the point `0x00554EA0` may have
moved the player to (the two are equal unless rule 4's search moves it).
Arithmetic of §12.1 rule 3 and rule 5 is 32-bit two's complement; the
1.14d lvlwarp values (`OffsetX` −4..10, `OffsetY` −6..6, `ExitWalkX/Y`
−5..5, measured on `LvlWarp.txt`) keep every result small.

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
the 0x5F handler `0x0054CC40` (`lo' % 100` on the player's seed for
its state-108 lock; owner: `sim/pathing.md` §1.6). Ring order
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
10. §8: the out room need not hold the out point. The row test reads the
   rect last read (a cell visit's row room) but a row inside it takes
   `room` (the argument) as row room, and a cell inside the row room's
   columns takes the row room without testing its rows (`0x0064E840`).
   Vector C1: the result is A with a point in the room above. Callers
   pass the room on as returned (monster spawn `0x005A09E0` →
   `0x005B30E0`, `monsters/init.md`); a later §7 search from it
   (§10 rule 3) or the first move's room recache (`sim/pathing.md` §9.6)
   finds the holding room.

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
| C1 | room A sub-tiles x 40000..40039, y 40080..40089; room B (adjacent) x 40000..40039, y 40070..40079; wall at (40008, 40078) | §8 from A, (40008, 40080), n = 1, mask 0x1 | pass 1 (40007, 40079) in B: box hits the wall; pass 2 row 40078 is inside B's rect, so row room A; (40006, 40078) free → out room **A**, point (40006, 40078) (in B). Without the wall: out room B, (40007, 40079) |

Real vectors (recordings, `traces/raw/`, reproducible once the DRLG of
the recorded game is regenerated from its seeds). The map seed (game
+0x7C, `drlg/levels.md` init seed) is bytes 2–5 of the S→C 0x03 of each
recording (`0x0053ABE0` → `0x0053B390`: act u8 @1, game +0x7C u32 @2,
start level u16 @6, game +0x80 u32 @8): R1 `03 00 1fe86826 0100 …` =
644,409,375; R2 / R3 `03 00 c4883810 0100 …` = 272,140,484.

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

1. No recording logs unit positions. Design of the recording that
   settles it (recorder `tools/trace-recorder`, one JSONL line per hook
   call, frame = game +0xA8, values as integers):
   - `0x00650840` entry and exit (movement, `sim/pathing.md` §9.4): unit
     type, GUID, class; path +0x00/+0x04 precise x/y; +0x24 index, +0x28
     count, points +0x9C (count × {x, y}); +0x7C velocity, +0x72/+0x76
     velocity vector, +0x6A/+0x6E direction vector; +0x34 flags, +0x3C
     type, +0x54 collided mask, +0x1D4 saved-step count and steps; room
     (+0x1C) as its sub-tile rect; exit: result.
   - `0x00649970` exit (path compute): the same fields plus target
     +0x10/+0x12 and the result.
   - Placement: `0x0064DEA0` entry (room rect, point, size, mask, field
     origin, fallback, max distance, step) and exit (point, result room
     rect); `0x00554EA0` entry (unit GUID, room rect, x, y, exact, alt)
     and exit; `0x00555DA0`, `0x0061B060`, `0x00650910` (teleport) entry
     and exit; unit allocation `0x00555230` exit (type, class, GUID, x,
     y, room rect).
   - Scenarios: game entry; walk and run in the Rogue Encampment and the
     Blood Moor, clicking walls and NPCs; waypoint travel; walking into
     a level warp (cave); dropping items next to walls; a monster
     spawn. The map seed of each run is the S→C 0x03 seed (Test
     vectors). Compare per frame and unit: every field equal.
   The alternative (0xAC / 0x9C position bytes in `server-messages.tsv`)
   sees only spawn points, not paths.
2. *Answered:* §4 rule 4 sub-box recursion (`0x0064CDF0`): the top
   strip uses the inside box's clipped width; the right strip takes the
   corner.
3. *Answered:* `0x006195A0` (destination warp tile and lvlwarp record)
   is §12.2 rule 1.
4. *Answered:* the position history (§10 rule 7) is read by the AI
   helpers `0x005E3930` / `0x005E3EA0`, not by the 0x5F handler.
   Their use of it is `monsters/ai.md`'s to specify.
5. *Answered:* `0x00545B80` quest warp gate (§12.2 rule 3): its
   checks per destination level are `world/quests.md` §8.2.
6. *Answered:* C→S 0x5F handler `0x0054CD50` → `0x0054CC40`, its
   state-108 lock and table `0x006E1064`: `sim/pathing.md` §1.6 (a
   recorded 0x5F would still confirm it on live data).
7. Pets following a teleport (`0x005754B0`): owner is the pet /
   mercenary spec (`world/hirelings.md` §6).

Answered handoff questions (`docs/HANDOFF.md` §7):

- PC1 (and OQ2): §4 rule 4. PC2: §5.1 (cells from the room argument;
  pattern 0 stamps nothing). PC3: §2.4 rule 4 (set type, flags
  0x60000). PC4, WP4: §6 rule 4 (flag 0x8 = cell changed; stamp and
  query from the destination room). PC5: §5.2 (no bound; modes 0–7).
- PF1: §12.2 (32-bit two's complement; 1.14d values small). WP5: §11
  (the waypoint room ends both searches). GX4: Test vectors (S→C 0x03
  seeds 644,409,375 / 272,140,484).
- PP1: §7.3 rule 3 (the candidate's room). PP2, PP3: §8 rule 3 (masked;
  half-open). PP4: §11 (unit size; game entry asserts, level warp does
  nothing). PP5: §11 "Recipients". PP6: §12.2 (rule 2's point). PP7:
  §12.2 rule 1, `world/quests.md` §8.2; §10 rule 7 stays host-only.
- CR1, CR2: open question 1 (recording design). CR3: `combat/vitals.md`
  §5.
- `docs/handoff/prop-wired-path.md` PWQ1: edge case 10, vector C1
  (`0x0064E840` re-read). Its row 4 (warp footprint room): §6 rule 4 as
  written: `0x00650910` clears from the path's room (ECX = path +0x1C)
  and stamps from its destination-room argument (`0x006509FD`–
  `0x00650A0B`), so a warp to a non-adjacent room is stamped. Its row 3
  (room deactivation compress): `sim/units.md` §3.3.
- `docs/handoff/drop-freespot.md` DF1: §9 (the caller's room, i.e. the
  dropper's room). DF2: `items/treasure.md` §7 step 4 (the allocation
  adds the dropped item to the world, path and footprint included).
  DF3: no spec question (provider-off wiring).
