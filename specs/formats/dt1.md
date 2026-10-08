# Spec: Formats — DT1 (map tiles)

- **Status:** implemented. All 256 live `.dt1` files in 1.14d (names
  case-insensitive, `mpq-tool formats` 2026-10-08) are read: 250 decode
  and 6 unused version-4 leftovers are refused (see Edge cases). Block
  formats seen: 0x0001 iso (226,996), 0x1001 (108,905) and 0x2005 (15,712),
  both RLE. (The earlier 254 files / 110,259 0x1001 blocks counted names
  case-sensitively.)
- **Target version:** 1.14d
- **Crate/module:** `d2-formats::dt1`
- **Related specs:** `specs/formats/ds1.md` (maps that reference tiles)

## Summary

A DT1 is a library of map tiles: floors, walls, roofs, shadows. Each tile
has metadata (orientation, main/sub index, rarity, 5×5 sub-tile flags) and
is drawn from blocks of 8-bit indexed pixels. A block is either a 32×15
isometric diamond or a 32×32 RLE image.

## Inputs

| Name | Type | Source |
|---|---|---|
| file bytes | `.dt1` | MPQ, `data\global\tiles\...` |

## Outputs / state changes

Header, tile headers, and for each tile its blocks with position, format and
decoded pixels.

## Rules

All integers are little-endian.

### File header (276 bytes)

| Offset | Size | Field |
|---|---|---|
| 0 | i32 | major version: must be 7 |
| 4 | i32 | minor version (6 in 1.14d; recorded) |
| 8 | 260 bytes | reserved |
| 268 | u32 | tile count |
| 272 | u32 | offset of the first tile header |

### Tile header (96 bytes each, consecutive)

| Offset | Size | Field |
|---|---|---|
| 0x00 | u32 | light direction |
| 0x04 | u16 | roof height |
| 0x06 | u16 | material flags |
| 0x08 | i32 | height |
| 0x0C | i32 | width |
| 0x10 | i32 | unknown (height to bottom) |
| 0x14 | u32 | orientation (tile type) |
| 0x18 | u32 | main index |
| 0x1C | u32 | sub index |
| 0x20 | u32 | rarity / frame |
| 0x24 | u32 | unknown (transparent color) |
| 0x28 | 25 bytes | sub-tile flags, 5×5, stored as in the file |
| 0x41 | 7 bytes | padding |
| 0x48 | u32 | block headers offset (absolute) |
| 0x4C | u32 | block data length |
| 0x50 | u32 | block count |
| 0x54 | 4 bytes | unused |
| 0x58 | u16 | unknown |
| 0x5A | u16 | unknown (D2MOO: cache index; replaced at load, §Unknown and runtime fields) |
| 0x5C | u32 | unknown |

### Block header (20 bytes each, at the tile's block headers offset)

| Offset | Size | Field |
|---|---|---|
| 0x00 | i16 | x (pixel offset in the tile) |
| 0x02 | i16 | y |
| 0x04 | u16 | unknown |
| 0x06 | u8 | grid x |
| 0x07 | u8 | grid y |
| 0x08 | u16 | format |
| 0x0A | u32 | data length |
| 0x0E | u16 | unknown |
| 0x10 | u32 | data offset, **relative to the tile's block headers offset** |

All offsets and lengths must stay inside the file. The block counts of all
tiles together must not exceed `file length / 20` (one 20-byte header per
block; tiles sharing block headers could otherwise decode them once per
tile, quadratic in the file size; see Open questions).

### Block pixels

**Format 1: isometric**, 32 wide × 15 tall, exactly 256 bytes of pixels. Row
`r` (0..14) starts `SKIP[r]` pixels in and holds `RUN[r]` pixels:

```
SKIP = [14, 12, 10, 8, 6, 4, 2, 0, 2, 4, 6, 8, 10, 12, 14]
RUN  = [ 4,  8, 12, 16, 20, 24, 28, 32, 28, 24, 20, 16, 12, 8, 4]
```

A data length other than 256 is an error.

**Any other format: RLE**, 32 wide × 32 tall. Read byte pairs `(skip,
count)` until the data length is consumed:
- `(0, 0)`: next row, x = 0.
- otherwise: x += skip, then copy `count` pixel bytes at x. x += count.

Pixels must stay inside 32 × 32. Moving past the last row is allowed if no
pixel is written there. Truncated pairs or pixel runs are errors.

Unwritten pixels are 0 (transparent). Assembling blocks into a tile image
(using block x, y) and the meaning of orientation values are rendering /
map rules.

### Unknown and runtime fields

Game-file read 2026-10-08 (the 245 version-7 DT1s of `d2exp.mpq` then
`d2data.mpq` by name, 15,637 tiles, 345,923 blocks; scratch program):

| Field | Live values | 1.14d use |
|---|---|---|
| tile 0x10 | 0 in every tile | none needed |
| tile 0x24 | 50 distinct values, 0 in 296 tiles; read as 3 bytes they look like RGB colours (e.g. `0x131C07`) | reader not found (open question 1) |
| tile 0x54 | 0 in every tile | overwritten at load with the block headers' address (tile 0x48 + file base, `0x0060A290`) |
| tile 0x58, 0x5A | both 0 in 14,284 tiles; 13 / 17 distinct values in the rest | the u32 at 0x58 is overwritten at load with a pointer to the library's path (`0x0060A290`, `0x0060A390`): the file values are never read |
| tile 0x5C | 0 in every tile | the header-only load `0x0060A390` ends the process (fatal) when 0x54 or 0x5C ≠ 0 |
| block 0x04, 0x0E | 0 in every block | none needed |
| block 0x0C (upper half of the data length) | 0 in every block | 1.14d reads the data length as u16 at 0x0A (`0x0060A290`, `0x0060A650`) |

A d2rs reader keeps decoding these as stored; no rule depends on them.

## Constants & data dependencies

`SKIP`, `RUN` above.

## Randomness

None.

## Edge cases & original bugs

- Tiles with 0 blocks occur.
- **RLE pair with count 0 and a skip:** `(skip, 0)` with `skip` ≠ 0 only
  moves x. It writes no pixel, so it is not an error even when x passes 32
  or the row passes 31 (found by fuzzing: the reader must not index the pixel
  buffer for a zero-length run). A run of count > 0 outside 32 × 32 is
  an error.
- **Unused version-4 files:** `d2data.mpq` contains six DT1 files in an
  older version-4 layout: `ACT1\BARRACKS\barracks.dt1`,
  `ACT1\BARRACKS\gargtrap.dt1`, `ACT1\CATACOMB\Catacombs.dt1`,
  `ACT1\CATHEDRL\Cathedrl.dt1`, `ACT1\COURT\Court.dt1` and
  `ACT1\OUTDOORS\Outdoor1.dt1` (under `data\global\tiles\`). No level table
  (`LvlTypes`, `LvlPrest`, `LvlSub`, `LvlMaze`, `LvlWarp`, classic or
  expansion) references them. They're pre-release leftovers and are not
  supported.

## Test vectors

| Input / seed | Expected output | Source |
|---|---|---|
| iso block of bytes 0..255 | row 0 = x 14..17 ← 0..3; row 7 = x 0..31 ← 112..143; row 14 = x 14..17 ← 252..255 | §Block pixels |
| RLE `02 03 a b c 00 00 01 01 d` | row 0: x2..4 = a b c; row 1: x1 = d | §Block pixels |
| RLE `1F 02 a b` | error (x 31 + 2 > 32) | §Block pixels |
| every `.dt1` in the 1.14d archives | parses and decodes | survey |

## Provenance

Community documentation of DT1 (Phrozen Keep, Paul Siramy's DT1 notes),
cross-checked against Riiablo (Apache-2.0) `map5/Dt1Decoder7.java` (header
sizes, field order, iso SKIP/RUN tables, RLE pairs). Riiablo reads block data
sequentially after the headers; this spec uses the block's data offset
field, which is authoritative. No Blizzard code or decompiler output was
consulted.

## Open questions

1. Meaning of the unknown fields and of each orientation value: map spec.
   *Partly answered* (game-file read and static, 2026-10-08):
   §Unknown and runtime fields (0x10, 0x54, 0x5C and the block's 0x04,
   0x0C, 0x0E are 0 in all live data; 0x58/0x5A are replaced at load).
   Open: a reader of tile 0x24 (the RGB-looking value); orientation
   values stay with the map specs.
2. Sub-tile flag row order relative to screen and world coordinates: map
   spec (Riiablo reverses rows).
   *Answered* in the owner: `drlg/rooms.md` §10.4 rule 3 (1.14d's
   collision build reads the 25 bytes bottom row first: mask at
   (ox + c, oy + r) |= flags[5·(4 − r) + c]).
3. Whole-file block limit (`file length / 20`, §Block header) assumes
   tiles never share block headers: confirm on every 1.14d `.dt1`
   (`mpq-tool formats`).
   *Answered* (game-file read, 2026-10-07, the 251 DT1s of
   d2data + d2exp by name = 245 version-7 files plus the 6 unused
   version-4 leftovers; the totals quoted then, 15,928 tiles and 397,668
   blocks, are for those 251 files. Version-7 only, re-read 2026-10-08:
   15,637 tiles, 345,923 blocks, see Unknown and runtime fields; `Patch_D2.mpq`
   unlisted): no two tiles' block-header ranges overlap and every file's
   block total is within `length / 20`.
