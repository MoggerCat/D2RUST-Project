# Spec: DRLG — Preset levels, DS1 maps and preset units

- **Status:** draft. Every rule below was read from the 1.14d `Game.exe`
  disassembly (addresses in Provenance); the file draw, its seed and the
  35-room town split are confirmed by recording `20261005-232125-rng`;
  table counts are measured on the extracted 1.14d tables and DS1 files.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::drlg::preset`
- **Related specs:** `drlg/levels.md` (level allocation and generation,
  level seed, DRLG struct, Vis/warps, act layout), `drlg/rooms.md` (DRLG
  room allocation and room seed, activation, tile grids and tile choice,
  door tiles), `drlg/maze.md` and `drlg/outdoor.md` (callers that place
  preset maps inside maze and outdoor levels; lvlsub uses of DS1 tag
  layers and groups), `formats/ds1.md` (byte layout), `sim/rng.md`
  (generator, `roll`), `data/fields.tsv` (lvlprest, monpreset layouts),
  `data/runtime-maps.md` §8 (monpreset act ranges). Sibling table:
  `drlg/preset-tables.tsv`.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 47–62 |
| Inputs | 63–74 |
| Outputs / state changes | 75–85 |
| Rules | 86–87 |
|   1. Records (1.14d layout, for recorders and checks) | 88–103 |
|   2. Finding lvlprest rows | 104–114 |
|   3. DrlgType 2 levels | 115–171 |
|   4. Allocating a preset map (`0x00666ED0`; all level types) | 172–188 |
|   5. Loading a DS1 (`0x00665F40`, parser `0x00665950`) | 189–271 |
|   6. Building a preset area (`0x00667ED0`, scan `0x00667970`) | 272–352 |
|   7. Unit filter (`0x00667620`) | 353–377 |
|   8. First activation of a preset room (`0x00667890`) | 378–414 |
|   9. Preset room grids and unit transfer (`0x006667D0`) | 415–444 |
|   10. Tile fill switches (`0x00666AC0`) | 445–457 |
|   11. Door preset units (`0x0066D9E0`) | 458–482 |
|   12. Pops at run time (presentation) | 483–495 |
|   13. lvlprest columns (1.14d use) | 496–516 |
| Constants & data dependencies | 517–548 |
| Randomness | 549–571 |
| Edge cases & original bugs | 572–589 |
| Test vectors | 590–615 |
| Provenance | 616–657 |
| Open questions | 658–699 |
<!-- /index -->

## Summary

A *preset map* is one lvlprest row placed at a tile rectangle of a level:
it chooses one of the row's DS1 files, loads that DS1 (shared, cached),
copies the DS1's object/monster records into the map as *preset units*
(with a seeded random filter), optionally scans the DS1's wall layers for
warp markers, "pop" (roof) regions and special tiles, and splits the
rectangle into DRLG rooms of at most 8×8 tiles. Preset maps are used by
all three level types: a DrlgType 2 level is exactly one preset map
covering the level; maze and outdoor levels place preset maps through
the same functions (`drlg/maze.md`, `drlg/outdoor.md`). When a preset
room is built, this spec also gives how its tile grids are cut from the
DS1 layers and how preset units move from the map to the room. It stops
at the room's preset-unit record; spawning units is
`claude/phase3-monsters`.

## Inputs

| Name | Type | Source |
|---|---|---|
| lvlprest rows | 432-byte records, 1,091 rows | `lvlprest.bin` (`data/fields.tsv`) |
| DS1 files | bytes | MPQ `DATA\GLOBAL\TILES\...` (path from lvlprest `File1`–`File6`, fixed up per `data/fixups.md` §12) |
| monpreset act ranges | per act: first row, count | `data/runtime-maps.md` §8 |
| monstats / superuniques / objects counts and objects `SubClass` | | loaded tables |
| object-preset table, door-unit table | constants | `drlg/preset-tables.tsv` |
| level seed, DRLG room seed | 8-byte seeds | `sim/rng.md` §5.4 |
| level id, level rectangle, level flags | | `drlg/levels.md` |

## Outputs / state changes

- Level preset info (level +0x14, 8 bytes): map pointer, *direction*
  (picked file index, −1 = none yet).
- Preset map record (0x58 bytes, pushed on the level's map list at
  level +0x1B0).
- Shared DS1 file record (0x5C bytes) in a global cache.
- Preset units on the map, later moved to rooms (room +0x5C).
- DRLG rooms of type 2 with preset room data (0xF8 bytes, room +0x20).
- Pop regions on the map; tile-info entries on the level; room flags.

## Rules

### 1. Records (1.14d layout, for recorders and checks)

| Record | Size | Fields (offset: meaning) |
|---|---|---|
| level preset info | 8 | +0 map, +4 direction |
| preset map | 0x58 | +0x00 lvlprest index (Def), +0x04 picked file, +0x08 lvlprest record, +0x0C DS1 file record, +0x10/+0x14/+0x18/+0x1C x, y, width, height (tiles), +0x20 "has link grid" pointer (freed as a vertex list, `0x0067D1E0`) and +0x24 per-8×8-cell link grid (20-byte grid; never filled in 1.14d, §6 step 10), +0x38 preset-unit list, +0x3C "hardcoded units pending" (1 at alloc), +0x40 pop count, +0x44/+0x48/+0x4C/+0x50 pop group, pop sub index, pop timer, pop rectangle (16 bytes), +0x54 next map |
| DS1 file record | 0x5C | +0x00 tag type, +0x04 file buffer, +0x0C width−1, +0x10 height−1 (as stored in the DS1), +0x14 wall count, +0x18 floor count, +0x1C..+0x2B orientation layers 0–3, +0x2C..+0x3B wall layers 0–3, +0x3C/+0x40 floor layers 0–1, +0x44 shadow layer, +0x48 tag layer, +0x4C group count, +0x50 groups (24 bytes each), +0x54 preset-unit list |
| preset unit | 0x20 | +0x00 mode, +0x04 class id, +0x08 x, +0x0C next, +0x10 path, +0x14 unit type, +0x18 y, +0x1C flags |
| path | 8 | +0 point count, +4 points (12 bytes each: action, x, y) |
| preset room data | 0xF8 | +0x00 lvlprest index, +0x08 map, +0x0C preset room flags (1 = single room), +0x10.. grids, +0xEC link, +0xF0/+0xF4 tombstone list and count |

Below, "the DS1 width/height" means the stored `width_minus_1` /
`height_minus_1` values: a DS1 with stored width `W` has `W+1` cells per
row and covers `W` tiles. The extra row and column are the shared edge
with the next map (`drlg/rooms.md` §9.5).

### 2. Finding lvlprest rows

1. **By level id** (`0x0061F0E0`): the first row, in table order, whose
   `LevelId` equals the level id; none → no row.
2. **By index** (`0x0061F0B0`): row number `i` (0-based, 0 ≤ i < row
   count), else none. Callers pass a row's `Def` value as the index; in
   1.14d `Def` equals the row number for all 1,091 rows, so the two are
   interchangeable.
3. Rows marked `Expansion` are skipped at load when d2exp is absent
   (`data/loading.md` §7); this spec assumes the expansion install.

### 3. DrlgType 2 levels

#### 3.1 At level allocation (`0x00667430`, from `0x00642AE0`)

Runs right after the level seed is set to `{dwStartSeed + level id, 666}`
(`drlg/levels.md` §4):

1. Find the level's lvlprest row by level id (§2.1). None → fatal error
   (assert at `Preset.cpp` line 0xAFB; "level labeled as preset but no
   preset claims it").
2. Allocate the level preset info: map = none.
3. If `Files` ≠ 0: direction := `roll(Files)` on the **level seed**
   (`0x0066749F`). Else direction := −1 (no draw).
4. Then the level position/size code runs (`0x00642D10`,
   `drlg/levels.md` §6).

The act-layout code may overwrite the direction later, before generation
(`0x006772C0`, owner `drlg/levels.md` / `drlg/outdoor.md`): for level 1
(Rogue Encampment) and level 40 (Lut Gholein), both with `Files` = 0, it
writes the layout's town orientation; for level 27 (Outer Cloister) it
may replace the `roll(3)` result after one drlg-seed draw (`0x006774DB`).

#### 3.2 At level generation (`0x00668100`, from `0x006424A0`)

`0x006424A0` first resets the level seed to `{dwStartSeed + id, 666}`
(`drlg/levels.md` §5). Then:

1. Find the row by level id; allocate the preset map (§4) with index =
   the row's `Def`, rectangle = the level rectangle (level +0x1C..+0x28),
   seed = the level seed. This draws `roll(Files)` again from the reset
   seed, so the result equals §3.1's draw.
2. If direction = −1: direction := the map's picked file. Else the map's
   picked file := direction. (Towns 1 and 40: `Files` = 0, so the map's
   draw is skipped and returns 0; the layout's direction wins.)
3. Build the map's area (§6) with no extra flags, multi-room mode.
4. Automap (presentation; needs the DRLG's automap callbacks, which only
   the client DRLG has — `drlg/levels.md` §1):
   - Levels 40, 103, 109: if the town-automap callback (drlg +0x488)
     exists and lvlprest `AutoMap` ≠ 0: call it with (level id, picked
     file, level x + width/2, level y + height/2).
   - Any other preset level: if the automap callback (drlg +0x454) exists
     and `AutoMap` ≠ 0: for each Vis slot 0–7 with a level id, get that
     level and **generate it if it has no rooms** (`0x006424A0`; its own
     seed, its own draws); then for each room of this level in list
     order: initialize/stream the room (`0x0061B730`, `drlg/rooms.md`
     §4.3, which draws room seeds and adds preset units, §8) and pass its
     active room to the callback.
   In 1.14d only Defs 1, 301, 529, 797, 863 have `AutoMap` = 1 (levels 1,
   40, 75, 103, 109), so the generic branch runs for levels 1 and 75.

#### 3.3 Freeing

`0x00642010` (`drlg/levels.md` §9) frees the level's map list
(`0x006683D0`: per map, its link grid and vertices, its DS1 reference
(§5.1), its preset units and paths, its pop arrays) and resets the
preset info (`0x006674D0`: map := none; info freed unless kept).

### 4. Allocating a preset map (`0x00666ED0`; all level types)

Arguments: level, lvlprest index, rectangle (x, y, w, h in tiles), seed.

1. New zeroed map; record := row by index (§2.2); index stored.
2. Picked file := `roll(Files)` on the given seed (inline, `0x00666F33`;
   `Files` ≤ 0 → 0 without a draw).
3. x, y := rectangle x, y. If the row's `SizeX` and `SizeY` are both
   non-zero: w, h := `SizeX`, `SizeY`; else the rectangle's w, h.
4. Hardcoded-units-pending := 1. Push the map on the level's map list
   (head insert).

Maze and outdoor callers use the level seed too (`drlg/maze.md`,
`drlg/outdoor.md`); outdoor code then overwrites the picked file
(`0x00666EC0`) but the draw has already happened. Recorded: 48 such
draws per build for level 2 and 61 for level 3 (site `0x00666F33`).

### 5. Loading a DS1 (`0x00665F40`, parser `0x00665950`)

#### 5.1 Cache

DS1 files are cached process-wide in one list (global head `0x0096D60C`,
lock `0x0096D610`), keyed by the exact path string (byte compare, case
sensitive). Hit: reference count + 1, share the record. Miss: new
record (zeroed, 0x5C bytes), path copied (0x104-byte buffer), count 1,
head insert, then parse. Release (`0x00668300`): count − 1; at ≤ 0 the
entry is unlinked and the buffer, groups and the record's unit list are
freed. The parsed record is never modified after parsing, so sharing is
safe; d2rs may cache by path the same way.

#### 5.2 What the parser keeps

Byte layout: `formats/ds1.md`. The 1.14d parser reads the whole file
into a buffer of file size + 0x320 bytes (`0x00517079`) and walks it
**without bounds checks**. It keeps:

1. Version, stored width and height.
2. Act: 0 if v < 8; else the stored value, and values > 4 become 4
   (signed compare; negative values stay).
3. Tag type: 0 if v < 10, else stored.
4. Tile-file names: skipped (the level type's DT1 list is used instead,
   `drlg/rooms.md` §9.3).
5. Layers: pointers into the buffer, `(W+1)·(H+1)` u32 each. v < 4: wall
   count 1, **floor count stays 0** (the floor block is skipped and never
   used), and the stream blocks are wall 0, floor 0, orientation 0, tag,
   then shadow. v ≥ 4: wall and floor counts as stored, no range check.
6. v < 7: every orientation cell of every wall layer is replaced through
   the 1.14d table at `0x006EEF20` (42 entries; the first 25 equal
   `ds1.md`'s `ORIENTATION_LOOKUP`; entries 25–41 are 0, 0, 1, 2, 5, 5,
   7, 9, 11, 13, 14, 15, 16, 17, 18, 19, 20; no range check beyond).
7. Tag layer pointer only if tag type is 1 or 2 (or v < 4).
8. **Objects** (v ≥ 2): §5.3.
9. **Groups** (v ≥ 12 and tag type 1 or 2): v ≥ 18 skips 4 bytes; count;
   per group x, y, width, height and, if v ≥ 13, one more value (record
   +0x14). Used only by tile substitution (`drlg/outdoor.md`); the preset
   path never reads tag layer or groups.
10. **Paths** (v ≥ 14 only; the count is read unconditionally): per path
    point count n, x, y. If n ≠ 0, find the first unit in the file's unit
    list (§5.3; newest first) whose x and y equal the path's x, y. Found:
    give it a new path of n points: x, y and action (action = stored
    value if v ≥ 15, else 1); a unit that already has a path gets a new
    one (last path wins). Not found: skip n·8 bytes, plus n·4 if v ≥ 15.
11. Anything after the last section is ignored.

#### 5.3 DS1 object → file preset unit (`0x00665B20`–`0x00665D98`)

For each object record (type, id, x, y, flags (v ≥ 6, else 0)):

| DS1 type | Mode | Class id |
|---|---|---|
| 1 monster | 1 | v ≤ 4: unit dropped. v ≥ 5: see below |
| 2 object | 0 | v ≥ 6: id < 150 → object-preset table[act][id] (`preset-tables.tsv`, absent entries are 0); id ≥ 150 → id − 150. v ≤ 5: id as stored, except 573 → −1 |
| 4 item | 3 | v ≥ 5: code table entry `id` (only entry 0 exists: `HDM`) lower-cased, space-padded to 4 (`hdm `), looked up as an item code (`0x00633640`). v ≤ 4: id as stored |
| other | 0 | id as stored |

Monster ids (v ≥ 5), with M = monstats count (734), S = superuniques
count (66): if the DS1 act has a monpreset range and id < its count, take
monpreset row `first + id`: kind 0 (monplace) → `place + S + M`; kind 1
(monstats) → `place`; kind 2 (superunique) → `place + M`; other kind →
−1. Otherwise the id stays as stored. So preset monster ids form one
space: `[0, M)` monstats, `[M, M+S)` superuniques, `[M+S, …)` monplace
rows. Then, for ids in `[0, M)`:

| DS1 act | Monster id | Becomes |
|---|---|---|
| 2 (Act III) | 297 natalya | object 382, mode 0 |
| 2 | 366 compellingorb | object 404, mode 0 |
| 4 (Act V) | 514 nihlathak | object 461, mode 0 |
| 4 | 537, 538, 539 ancientstatue1–3 | object 1013 − id (476, 475, 474), mode 0 |

The unit is kept if class id ≥ 0 and (type ≠ 1 or v > 4); it is pushed
at the **head** of the file's unit list (so the list is in reverse file
order) with flags := the DS1 flags.

Measured on the 2,043 DS1 files named by lvlprest: all are v12–18; only
types 1 (2,267 records) and 2 (14,105) occur; one record has flags ≠ 0
(value 1); object-preset ids ≥ 573 occur (580: 46, 581: 135, 582: 24
records): they are kept as class ids (their meaning is the spawner's,
`claude/phase3-monsters`).

### 6. Building a preset area (`0x00667ED0`, scan `0x00667970`)

Arguments: level, map, room flags F, single-room mode. Callers: level
generation (§3.2: F = 0, multi-room), `drlg/maze.md` (`0x00673A60`),
`drlg/outdoor.md` (`0x006750F0`).

1. If lvlprest `Outdoors` ≠ 0: F |= 0x80000.
2. A cell grid of `(w/8 + 1) × (h/8 + 1)` (single mode: one cell is
   used) is created.
3. **Warp flags from Vis:** for i = 0..7, if the level's Vis slot i is
   set and the warp lookup for slot i (`0x0066AF30`, `drlg/levels.md`
   §7) returns −1: F |= 0x10 << i. Every cell := F.
4. If `Scan` = 0 and `Pops` = 0: stop scanning (no DS1 load now; it
   happens at first room activation, §8).
5. Otherwise: load the DS1 of the picked file (§5), run the unit filter
   (§7) with the **level seed** (level +0x1C4), then require DS1 width =
   map w and DS1 height = map h — a mismatch is a **fatal error** in
   1.14d (asserts at lines 0x8B9 / 0x8BA; D2MOO only warns). Measured:
   all 2,043 lvlprest DS1s match their row's `SizeX`/`SizeY` (1,054 rows
   set them) and every DrlgType 2 level's size.
6. If `Pops` ≠ 0: four arrays of `Pops` entries, zeroed.
7. **Scan**, for each wall layer i in order, rows y = 0..h−1, columns
   x = 0..w−1 (the DS1 edge row/column is not scanned): o := orientation
   cell, v := wall cell, style := (v >> 20) & 0x3F, sub := (v >> 8) &
   0xFF. Only cells with o = 10 or o = 11 count:
   - **Warp marker** (`Scan` ≠ 0, style ≤ 7, and sub = 0 or sub = 4 or
     v bit 31 set): cell (x/8, y/8) (single mode: cell 0) |= 1 << (style
     + 4), i.e. the "has warp *style*" room flag.
   - **Pop marker** (`Pops` ≠ 0, 8 ≤ style ≤ 29): if an entry with this
     style exists, its second corner := (x, y); else a new entry: group
     := style, sub index := sub, first corner := (x, y), second corner
     stays (0, 0). No capacity check (measured: no lvlprest DS1 has more
     distinct pop styles than its row's `Pops`).
   - **Tile info** (`Scan` ≠ 0, 30 ≤ style ≤ 33): append to the level's
     tile-info list (level +0x2C, 12 bytes each, count at level +0x1D8,
     no capacity check; consumer `drlg/levels.md`): x := map x + x, y :=
     map y + y, value := sub (style 30), sub + 5 (31), 10 (32), 11 (33).
8. **Pops finish** (`0x00666E00`), per entry: rectangle := from
   (min x, min y) to (max x, max y) of the two corners, stored as
   (map x + min x, map y + min y, width, height) in tiles; group :=
   style / 4 − 1 (styles 8–11 → 1, …, 28–29 → 6). A style seen only once
   keeps corner 2 = (0, 0), so its rectangle spans from the DS1 origin to
   the marker (reproduced original bug).
9. **Waypoints** (`Scan` ≠ 0): for each unit in the **file's** list
   (unfiltered, DS1 coordinates): type 2, class id < 573 and objects
   `SubClass` bit 0x40 (record +0x167): cell (x/5/8, y/5/8) (single
   mode: cell 0) |= 0x30000. Both divisions truncate toward zero; the
   cell is element cy·(w/8 + 1) + cx of the grid (row offsets r·(w/8 +
   1), `0x0067CBF0`), written through `0x0067C4F0` with **no bound
   check**: cx past the row width writes into the next row's cells, a
   row outside 0..h/8 reads past the row-offset array (open question
   7).
10. **Rooms:**
    - Multi-room: for tile row Y = map y, map y + 8, … < map y + h (outer)
      and column X = map x, map x + 8, … < map x + w (inner): a room of
      (X, Y, min(8, rest w), min(8, rest h)) with room flags := the
      cell's value, preset room flags 0, link := the map's link-grid
      value for the cell (0 if the map has no link grid).
    - Single-room: one room covering the map, flags := cell 0, preset
      room flags 1, link 0.
    Each room (`0x00666680`, inlined in single mode): allocate a type-2
    DRLG room (`0x0066B3E0`: **one level-seed step** plus its room seed,
    `drlg/rooms.md` §2); room flags |= F-derived cell value; DT1 mask :=
    lvlprest `Dt1Mask`; rectangle; preset data: map, lvlprest index,
    preset room flags, link (if non-zero, link +0x0C |= 1; preset room
    data +0xEC := link); if
    `Populate` = 0: room flag 0x800000; add the room to the level
    (`0x0066B970`: **head insert**: room next (+0x24) := level first
    room (+0x10), level first room := room, level room count (+0x08) +=
    1). So the level list holds the rooms in reverse creation order
    (multi-room: the last row's last column first). BuildArea returns
    the last room.
    The link is always 0 in 1.14d: `0x00667ED0` reads the map's link
    grid (+0x24) only when map +0x20 is non-zero (`0x00668071`–
    `0x0066808A`), and nothing writes map +0x20 after the allocation's
    zeroing memset (`0x00666F09`; the writes of +0x20 in the DRLG code
    `0x00665000`–`0x00682000` target rooms, levels, tile grids, outdoor
    room data and path nodes). So no room gets a link, "+0x0C |= 1" never
    runs and preset room data +0xEC stays 0; no link-record type is
    needed.

### 7. Unit filter (`0x00667620`)

Arguments: map, seed. Origin (sx, sy) := (map x · 5, map y · 5). Walk the
DS1 file's unit list (reverse file order). For each unit decide keep or
skip; a kept unit is copied (`0x00667510`: same type, class, mode,
flags; x + sx, y + sy; path copied with every point + (sx, sy)) and
pushed at the head of the map's list (so the map list is in file order).

| Unit | Draw (on the given seed) | Kept when |
|---|---|---|
| monster 204, 205 (act2vendor1/2), 371 (lightningspire), 372 (firetower) | one step | `lo' mod 3 = 0` |
| monster `M+S+33` (monplace `place_group25`) | one step | `lo' & 3 ≠ 0` |
| monster `M+S+34` (`place_group50`) | one step | `lo' & 1 ≠ 0` |
| monster `M+S+35` (`place_group75`) | one step | `lo' & 3 = 0` |
| object 196, 261 (floor traps) | one step | `lo' & 1 = 0` |
| object 581 | one step | `lo' & 3 ≠ 0` |
| anything else | none | always |

Monster ids ≥ M whose offset from M is < S (superuniques) are kept
without a draw; negative monster ids are treated as −1 (kept). All draws
are inline steps (`0x00667687`, `0x006676D0`, `0x00667721`,
`0x00667794`, `0x006677B8`). Measured occurrences in lvlprest DS1s:
monsters 204: 2, 205: 2 (both `Act2/Town/LutW.ds1`), 371: 26, 372: 8;
monplace 33: 4, 34: 20, 35: 36; objects 196: 52, 261: 53, 581: 135.

### 8. First activation of a preset room (`0x00667890`)

Called by room activation (`drlg/rooms.md` §4, §9.2 step 2) for a type-2
room without flag 0x2000000:

1. If the map has no DS1 yet (`Scan` = 0 and `Pops` = 0, §6 step 4):
   load it (§5) and run the unit filter (§7) with **this room's DRLG
   room seed** (room +0x14). So for 777 of 1,091 lvlprest rows the
   filter draws come from the first-activated room of the map, not the
   level seed.
2. If the map's hardcoded-units-pending is 1: clear it, then by lvlprest
   index d:
   - d = 4–7 (`Act 1 - Wild Border 1–4`), level id 2 (Blood Moor) and
     picked file 3 (`File4`, e.g. `Bord1o.ds1`; set by outdoor code):
     add a monster unit, class 266 (navi; −1 if monstats has ≤ 266 rows),
     mode 1, flags 0, at sub-tile ((map x + w/2)·5, (map y + h/2)·5)
     (integer halves) to the map list.
   - d = 1, 3, 26, 27, 28 or 300 (`Town 1`, `Town 1 Transition S`,
     `River Upper`, `River Lower`, `Bridge`, `Tristram`), only if the
     room has flag 0x40000 (set at room allocation when level flag 0x10
     is set, which the client DRLG sets: `drlg/levels.md` §1): river
     objects, below.
3. Room flag |= 0x2000000 (in every case).

**River objects** (`0x006663C0`, client DRLG only). G := the DS1's floor
layer 0 (stride map w + 1). Helper *sounds(cx)*: object 65, mode 0,
flags 1, at sub-tile (cx·5, sy) for sy = map y·5, map y·5 + 40, … <
(map y + DS1 height)·5. Helper *strip(c)*: for row r = 0..map h−1: sub-tile
x0 := (map x + c)·5 − 5, y := (map y + r)·5; objects 40 at x0, 41 at
x0+5, x0+10, x0+15, 42 at x0+20 (mode 0, flags 1); then if G(max(c, 0),
r) has style 4 and sub ∈ {0, 4, 8, 16, 29, 39}: r += 3; r += 1.
- d = 27: sounds(map x), strip(−1).
- Otherwise for columns c = 0..map w−1 of G's row 0: if the cell has
  style 2 and sub 24: sounds(map x + c + 1), strip(c), c += 4; c += 1.
All these units go to the map list (head insert). They are visual and
sound objects; the server DRLG never creates them.

### 9. Preset room grids and unit transfer (`0x006667D0`)

Called when the room's tiles are built (`drlg/rooms.md` §9.2 step c,
after the room-seed reset). Sub-rectangle of the DS1: origin (room x −
map x, room y − map y), size (room w + 1) × (room h + 1), DS1 stride
map w + 1.

1. For each wall layer i: grids from wall layer i and orientation layer
   i. If there is a wall layer: OR 0x84 into the border cells (first and
   last row and column) of wall grid 0. For i ≥ 1: OR `i << 18` into
   every cell of wall grid i.
2. For each floor layer i: a grid, every cell OR `i << 18`.
3. Shadow grid.
4. OR 0x84 into the border cells of every floor grid, then of the shadow
   grid.
These ORs write into the shared DS1 buffer (`drlg/rooms.md` §9.5).

**Unit transfer** (`0x00666710`): R := the room's sub-tile rectangle
(room x·5, room y·5, room w·5, room h·5). Walk the map list from the
head; each unit with R.x ≤ x < R.x + R.w and R.y ≤ y < R.y + R.h is
unlinked, gets x −= room x·5, y −= room y·5 (path points are **not**
shifted: they stay absolute level sub-tiles), and is pushed at the head
of the room's preset-unit list. So a room's list is in reverse map order
for its units. Units outside every room of the map stay on the map list
for good (measured: 98 lvlprest DS1 records lie at x ≥ 5·W or y ≥ 5·H).

The room's preset-unit record is complete here: type, class id, mode,
room-relative sub-tile x, y, flags (bit 0 = "spawned"/fixed, from the DS1
flags or set by §8), optional path. Placement: `claude/phase3-monsters`.

### 10. Tile fill switches (`0x00666AC0`)

The fill itself is `drlg/rooms.md` §9.5; the preset switches:

| Column | Effect in 1.14d |
|---|---|
| `KillEdge` | if set: kill-X := room right edge = map right edge; kill-Y := room bottom edge = map bottom edge |
| `FillBlanks` | passed for floor layer 0 only |
| `Animate` | animated tiles for floors, walls (with their orientation grids) and shadow; animation speed argument = lvlprest +0x24, which is 0 in every 1.14d row (no column fills it) |
| `Logicals` | set: logical coordinate lists from the orientation, floor and wall grids (`0x0066D110`); clear: default lists (`0x0066CCB0`) (`drlg/rooms.md`) |
| lvlprest index 1 or 108 (`Town 1`, `Graveyard`) | one extra zero-filled grid of (w+1)×(h+1) is counted and filled (adds nothing) |
| level id 17 (Burial Grounds) | tombstones (`0x00666990`), once per room: scan wall grid 0, rows < room h, columns < room w; cells with style 10 and 23 ≤ sub ≤ 27, at most 6: sub-tile ((room x + x)·5 + 2, (room y + y)·5 + 2). Read through `0x00666A80` (type-2 rooms only, else none) |

### 11. Door preset units (`0x0066D9E0`)

Called by the tile fill for door cells (orientation 8 or 9) and from the
build step (`drlg/rooms.md` §9.5.1; that spec owns the call order and the
RNG draw). Lookup (`0x0066D960`): the level-id entry of the door table
(`preset-tables.tsv`, table `door`), then its first row with main =
cell style, sub = cell sub and right = (orientation = 9). If found and
the door cell has no flag 0x20: sub-tile position := ((cell x − room x)·5
+ dx, (cell y − room y)·5 + dy); outside [0, room w·5) × [0, room h·5) →
nothing. Unit type 1: class id range-checked (out of range → −1), mode 1.
Type 2: mode 0; object ids 91 and 92 only: `roll(3)` on the room seed,
result 0 → no unit. The unit goes straight to the room's preset-unit
list (head insert), flags 0; then the door record gets flag 0x20.

Arguments (`0x0066D9E0`): room, cell world tile (wx, wy), packed cell
v, the door's tile record or none, and the cell orientation. With a
record: a record that already has flag 0x20 is skipped entirely (no
lookup, no draw); right = (record type = 9). Without a record (hidden
door cell, `drlg/rooms.md` §9.5.1 step 3): right = (orientation = 9);
the orientation argument is read only in this case. **Flag 0x20** is set
on the record (when there is one) after a unit is added **and** when
`roll(3)` gave 0 (no unit, the draw is still spent); it is not set when
no table row matches or the position is outside the room. So a door
record draws `roll(3)` at most once over all calls.

### 12. Pops at run time (presentation)

`0x006671E0` (from `0x0061ACD0`, player position in a room): if the
room is type 2, find the first pop entry of its map whose rectangle
(x·5, y·5, w·5 + `PopPad`, h·5 + `PopPad`) contains the sub-tile
position; its group is the current group (else 0). For every near room
of type 2 with an active room, for each pop entry of its map: entries of
the current map and group are revealed, others hidden, by fading the
room's wall tiles whose DT1 style equals the entry's sub index (or that
carry tile flag 0x200), timed with `GetTickCount` + 500 ms. It changes
only tile drawing state; it uses wall-clock time, so it must never feed
the simulation. Exact fade values: render spec.

### 13. lvlprest columns (1.14d use)

| Column | Use |
|---|---|
| `Def` | row index (§2.2) |
| `LevelId` | DrlgType 2 lookup (§2.1) |
| `Populate` | 0 → room flag 0x800000 (no population) |
| `Logicals`, `Animate`, `KillEdge`, `FillBlanks` | §10 |
| `Outdoors` | room flag 0x80000 (§6) |
| `SizeX`, `SizeY` | map size if both ≠ 0 (§4); `0x00666FD0` / `0x00666FE0` return them to outdoor code |
| `AutoMap` | §3.2 step 4 |
| `Scan` | DS1 loaded at build; warp markers, tile info, waypoints (§6) |
| `Pops`, `PopPad` | pop entries (§6), pop rectangle padding in sub-tiles (§12) |
| `Files`, `File1`–`File6` | file choice (§3, §4); path of the picked file: record + 0x44 + 60·file (`0x00667010`) |
| `Dt1Mask` | room DT1 mask (`drlg/rooms.md` §9.3) |
| `Beta` | not in the .bin |
| `Expansion` | load filter (§2.3) |

Files beyond `Files` are only reachable through outdoor code setting the
picked file (82 rows name such files, e.g. Defs 4–7 `File4`/`File5`).

## Constants & data dependencies

- Object-preset table (`0x00748AD8`, 5 acts × 150, u32) and door table
  (level entries `0x006EEFC8`, 37 × {level, first row, last row}; rows
  `0x006EF188`, 34 × {main, sub, right, class, type, dx, dy}):
  `drlg/preset-tables.tsv`. Columns `table a b c d e f g h`:
  - `objpreset`: a = DS1 act (0–4), b = DS1 object id (0–149), c =
    object class id. Only non-zero entries are listed (580 rows);
    missing ids map to 0. Act counts before the first 0: 113, 135, 116,
    66, 150. `0x00665860` (count per act) returns those values (150 for
    act 4; D2MOO returns 0 there).
  - `door`: a = level id, b = main (style), c = sub, d = right flag,
    e = unit type, f = class id, g = dx, h = dy; rows of one level in
    table order (121 rows, expanded from the level ranges).
- Item code table (`0x00748AD4`): one pointer, to `HDM`.
- Orientation table (`0x006EEF20`): §5.2 step 6.
- Object ids: 40, 41, 42 (river strips), 65 (river sound). Monster 266
  (navi). Thresholds: object class < 573 for the waypoint scan.
- Room flags: 0x10 << i (warp i), 0x30000 (waypoint), 0x40000 (client
  river gate), 0x80000 (Outdoors), 0x800000 (Populate = 0), 0x2000000
  (preset units added).
- lvlprest measurements (1,091 rows): `Files` 0: 37, 1: 627, 2: 187,
  3: 127, 4: 80, 5: 21, 6: 12. `Scan` 1: 292; `Pops` ≠ 0: 35 rows
  (values 1–4); `PopPad` −4: 130, −5: 1; `Animate` 1: 33 (Defs 836–862,
  1053–1058); `AutoMap` 1: 5; `Populate` 0: 62.
- DrlgType 2 levels (35, each claimed by exactly one lvlprest row): 1,
  13–16, 20, 25–27, 32, 33, 37, 38, 40, 50, 73, 75, 90, 91, 93–99, 102,
  103, 109, 120, 121, 124, 131, 132, 136. Their `Files`: 0 for levels 1,
  40; 2 for 25, 95–99; 3 for 27, 94; 4 for 124; 6 for 90, 91; 1 for the
  rest. All have `SizeX` = `SizeY` = 0 (size = level size) and `Scan` =
  1 except level 26 (Def 165: `Scan` 0, `Pops` 0).

## Randomness

In order, for one preset map:

1. **Level allocation** (DrlgType 2 only): `roll(Files)` on the level
   seed (`0x0066749F`); none if `Files` = 0.
2. **Map allocation** (every preset map, any level type): `roll(Files)`
   on the seed the caller gives — always the level seed in 1.14d
   (`0x00666F33`, inline); none if `Files` ≤ 0. For a DrlgType 2 level
   this is the first draw after the generation reset.
3. **Unit filter at build** (only if `Scan` or `Pops`): one level-seed
   step per filtered unit (§7), in reverse DS1 order.
4. **Rooms**: one level-seed step per room (`drlg/rooms.md` §2),
   row-major (rows outer).
5. **Later, at the first activation of a room of a map without `Scan`
   and `Pops`**: the unit filter draws from that room's DRLG room seed
   (§8), before that room's tile draws (which restart from the room seed
   reset, `drlg/rooms.md` §9.2).
6. Door units: `roll(3)` on the room seed for objects 91/92 (owner
   `drlg/rooms.md` §9.5.1).
7. Client only: §3.2 step 4 may generate other levels and stream rooms
   (their own draws).

## Edge cases & original bugs

1. DS1 size ≠ map size is fatal only when the DS1 is loaded at build
  (`Scan` or `Pops`); the lazy load of §8 does not check it.
2. A pop style seen once gets a rectangle from the DS1 origin (§6 step 8):
  `Act3/Kurast/MetroTemple2.ds1` (Def 647) style 9, and
  `Act3/Kurast/Metro08x16_2.ds1` (Def 649) style 8.
3. The DS1 parser has no bounds checks (§5.2); groups and paths can read
  the 0x320 slack bytes after the file (contents not cleared).
4. The level tile-info list and pop arrays have no capacity checks.
5. Units outside the map's rooms are never transferred (§9).
6. Path points stay absolute while unit x, y become room-relative (§9).
7. v < 4 DS1s lose their floor layer (floor count 0).
8. Item units with DS1 id ≥ 1 would read beyond the one-entry code table
  (no lvlprest DS1 has item units).
9. The navi unit requires picked file 3, which `roll(3)` never gives:
  only the outdoor code's explicit choice reaches it.

## Test vectors

| Input / seed | Expected output | Source |
|---|---|---|
| level 26 allocated, seed `{4014346895, 666}`, `Files` 1 | one `roll(1)` at `0x0066749F` → 0 | rec. seq 2433–2434 |
| level 27, seed `{4014346896, 666}`, `Files` 3 | `roll(3)`: lo' 2260552554 → 0 | rec. seq 2437–2438 |
| levels 13, 14, 15, 16, `Files` 1 | one `roll(1)` each → 0 | rec. seq 2446–2453 |
| level 1 (`Files` 0) allocation and generation | no file draw; the first level-1 draw after the reset (seq 2454) is room allocation, lo' 2928842600 = first step of `{4014346870, 666}` | rec. seq 2454–2455 |
| level 1 generation, Def 1, DS1 56×40 | 35 rooms (7 columns × 5 rows), 35 level-seed steps at `0x0066B42E`; no filter draws (town DS1s hold no filtered unit) | rec. seq 2455–2559 (35 × 3 records) |
| outdoor maps of levels 2, 3 | 48 / 61 `roll(Files)` steps per build at `0x00666F33` | rec. (96 / 122 for two builds) |
| level 90 (`Files` 6) with dwStartSeed 4014346869 | `roll(6)`: lo' 3449482213 → 1 → `Act3/Jungle/DungRm2A.ds1` | derived (rng.md) |
| level 124 (`Files` 4), same start seed | lo' 4227474959 → 3 → `NihlW.ds1` | derived |
| level 94 (`Files` 3), same start seed | lo' 2025139961 → 2 → `Temple2.ds1` | derived |
| map 84×84 (level 124) | 121 rooms, last row/column 4 tiles | §6 |
| map 8×8 (level 20, `Tower2.ds1`) | 1 room 8×8 | §6 |
| `TownN1.ds1` scanned at map origin (X, Y) | tile info, in order: (X+26, Y+7, 0), (X+28, Y+7, 10), (X+30, Y+14, 11) | survey of the DS1 (§6 step 7) |
| `Act2/Town/LutN.ds1` (Def 301, `Pops` 3) | pops: style 8 sub 8 group 1 rect (24, 3, 7×7); style 13 sub 13 group 2 rect (10, 27, 4×5); style 12 sub 13 group 2 rect (15, 33, 3×3) (DS1-relative); 5 tile-info entries | survey |
| `MetroTemple2.ds1` | style 8: corners (2,2),(6,11) → rect (2, 2, 5×10); style 9 seen once at (11, 6) → rect (0, 0, 12×7); both group 1 | survey |
| synthetic unit filter, unit monster 371, seed lo' = 9 | kept (9 mod 3 = 0); lo' = 10 → skipped | §7 |
| synthetic: object 581, lo' & 3 = 0 | skipped; otherwise kept | §7 |
| synthetic DS1 v18, act 4, monster id k with monpreset row kind 1 place 537 | object 476, mode 0 | §5.3 |
| synthetic DS1 v5, object id 573 | dropped (class −1) | §5.3 |
| synthetic DS1 v14, two paths for the same unit position | the unit keeps the second path; points have action 1 | §5.2 |
| synthetic room (8, 0, 8, 8) (sub-tiles x 40–79, y 0–39), units at (79, 39) and (80, 39) | first moved, room-relative (39, 39); second not moved | §9 |
| synthetic unit filter over file list [A, B, C] (file order), none filtered | map list A, B, C; room list C, B, A | §5.3, §7, §9 |

## Provenance

1.14d `Game.exe` (Ghidra exports in `re/exports/`, register use read from
the disassembly). Function map (D2MOO 1.10f names as hints):

| 1.14d | Role (D2MOO name) |
|---|---|
| `0x00665950` | DS1 parser (`DRLGPRESET_ParseDS1File`) |
| `0x00665F40` / `0x00668300` / `0x00668240` | cache load / release / free record |
| `0x00665860`, `0x006658E0` | object-preset count / lookup |
| `0x006660B0`, `0x00667510`, `0x006675D0` | path copy, unit copy, unit free |
| `0x006661A0`, `0x00666220`, `0x006663C0` | river sounds, river strip, river scan |
| `0x006664A0` | navi unit |
| `0x00666630`, `0x00666680` | preset room data alloc, room init |
| `0x00666710`, `0x006667D0` | unit transfer, room grids |
| `0x00666990`, `0x00666A80` | tombstones, tombstone getter |
| `0x00666AC0` | room tile fill (`DRLGPRESET_AddPresetRoomMapTiles`) |
| `0x00666E00` | pops finish |
| `0x00666ED0`, `0x00666EC0` | map alloc (+ file draw), set picked file |
| `0x00666FD0`, `0x00666FE0`, `0x00667010` | SizeX, SizeY, picked file path |
| `0x00667060`, `0x006671E0` | pop toggle, pop update |
| `0x00667430`, `0x006674D0`, `0x00668100` | level init (file draw), reset, generate |
| `0x00667620` | unit filter |
| `0x00667890` | first activation (`SpawnHardcodedPresetUnits`) |
| `0x00667970`, `0x00667ED0` | scan (`BuildPresetArea`), build area |
| `0x006683D0` | free map list |
| `0x0066D960`, `0x0066D9E0` | door unit lookup / placement |

- D2MOO `DrlgPreset.cpp` gave the structure; every rule was re-read on
  the 1.14d code. 1.14d differences: size mismatch is fatal (§6); the
  river sound step is 40 sub-tiles; object-preset count returns 150 for
  a full act; the per-room link is a value from the map's link grid
  (D2MOO passes the grid flags); D2MOO's item-code path is a placeholder,
  1.14d reads the one-entry table; anim speed is lvlprest +0x24 (0).
- RNG: recording `traces/raw/20261005-232125-rng.jsonl` via
  `drlg_rng.py` (sites `0x0066749F`, `0x00666F33`, `0x0066B42E`).
- Measurements: lvlprest/levels/monpreset/objects/monstats from
  `game/extracted/patch_d2`; Game.exe static tables read from the file;
  DS1 survey over the d2data and d2exp copies of the 2,043 files named
  by lvlprest (the patch MPQ has no listfile, so its DS1 overrides were
  not re-read: Open question 5).

## Open questions

1. What reads the room flag `0x10 << style` from warp markers and the
   level tile-info list (warp placement, `drlg/levels.md`)? Needed to
   confirm the scan output is complete.
   *Answered* (static, `all.asm` scan for tests of room +0x28 against
   0xFF0 or a register mask built as `0x10 << i` / `1 << (i + 4)`):
   five readers, all already specified: warp-room centres `0x006423D0`
   (`levels.md` §5 r4), the spawn-room fallback `0x0066B1F0`
   (`levels.md` §10), the rooms-near build `0x0066C370` with the warp
   links `0x0066C220` / `0x0066BE80` (`rooms.md` §3 r3) and the level
   free test `0x00643060` (`levels.md` §9 r3). Each tests only whether
   bit `0x10 << i` is set (with the slot's warp id); none reads which
   cell or tile set it. `0x0066B030` also builds the mask but has no
   caller. So the scan's per-room flag set is the whole output the
   readers need.
2. 1.14d content of a truncated group's missing fields (trees.ds1 group
   14): the bytes come from the uninitialized 0x320 slack after the file
   buffer (`0x00517079`, Fog allocator, no clear). d2rs uses 0; settle by
   checking whether lvlsub ever picks that group (`drlg/outdoor.md`).
3. Order of §8 (hardcoded units added to the map list at activation)
   versus §9 (unit transfer at tile build) for rooms built before the
   first activation of their map: confirm with a recording that the
   river/navi units reach their rooms.
4. Meaning of object ids 580–582 (beyond objects.txt) for the spawner
   (`claude/phase3-monsters`).
5. The ~94 DS1s that `Patch_D2.mpq` overrides were not re-surveyed
   (no listfile): re-run the size/pops/unit counts with `mpq-tool
   formats`-style name lists to confirm the measurements.
6. *Answered* (static): never in 1.14d. `0x0061EBB0`'s only caller is
   the load-all routine `0x00619300` (call `0x0061941D`), which passes
   its input 2 in EDX; the preload loop calls `0x00665F40` only when
   that value is non-zero (test at `0x0061EFAD`), and the routine's only
   caller (`0x0044B937`) sets input 2 = 0 (`data/loading.md` OQ 9).
   DS1s are loaded on first use (§5.1).
7. Does any lvlprest DS1 with `Scan` ≠ 0 have a waypoint object outside
   its map (§6 step 9 writes without a bound check)? Scan the waypoint
   objects (objects `SubClass` bit 0x40) of those DS1s against w, h.
8. *Answered* (`impl-drlg-act3-5` Q12, link bit 0): the §6 step 10 link
   is always 0 in 1.14d (map +0x20 is never set), so the bit is never
   written and needs no record type (§6 step 10).
