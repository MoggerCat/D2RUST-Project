# Spec: Formats — DC6 sprites

- **Status:** implemented. All 1,657 `.dc6` files (29,117 frames) in 1.14d
  decode (`mpq-tool formats`). 140 frames have `flip = 1`. Termination bytes
  seen: `EE×4` (1,195 files), `CD×4` (400), `00×4` (62).
- **Target version:** 1.14d
- **Crate/module:** `d2-formats::dc6`
- **Related specs:** `specs/formats/palette.md`

## Summary

DC6 holds 8-bit indexed images, mostly UI panels, fonts, inventory item
graphics and loading screens. A file has `D` directions × `F` frames per
direction. Each frame is a run-length encoded bitmap.

## Inputs

| Name | Type | Source |
|---|---|---|
| file bytes | `.dc6` | MPQ |

## Outputs / state changes

- Header fields, and for each (direction, frame): the frame header and a
  `width × height` array of palette indices stored top row first, where 0
  means transparent.

## Rules

All integers are little-endian.

### File header (24 bytes)

| Offset | Size | Field | Notes |
|---|---|---|---|
| 0x00 | i32 | version | must be 6 |
| 0x04 | u32 | flags | informational |
| 0x08 | u32 | encoding | informational |
| 0x0C | 4 bytes | termination | informational (usually EE EE EE EE or CD CD CD CD) |
| 0x10 | u32 | directions `D` | |
| 0x14 | u32 | frames per direction `F` | |

Then `D × F` u32 frame pointers (absolute file offsets), in direction-major
order: frame `f` of direction `d` is entry `d × F + f`. The pointer table
must fit in the file, and `D × F` must not exceed 0x10000.

### Frame

At each frame pointer `p` (which must leave room for the 32-byte header):

| Offset | Size | Field |
|---|---|---|
| 0x00 | u32 | flip |
| 0x04 | u32 | width |
| 0x08 | u32 | height |
| 0x0C | i32 | offset_x |
| 0x10 | i32 | offset_y |
| 0x14 | u32 | unknown (informational) |
| 0x18 | u32 | next_block (informational; the pointer table is authoritative) |
| 0x1C | u32 | length: bytes of encoded pixel data that follow |

The encoded data is the `length` bytes after the header, and must lie
inside the file. Three termination bytes usually follow; they are ignored.
`width × height` must not exceed 0x100_0000 (16M pixels), and the frames
of the whole file together must not exceed 0x400_0000 (64M pixels)
(implementation limits: an empty encoding is a whole transparent frame,
and frame pointers may repeat; see Open questions).

### Pixel decoding

Start with every pixel 0 (transparent). Row order: if `flip == 0`, the
first encoded row is the **bottom** row (y = height − 1) and rows go up. If
`flip != 0`, the first row is the top row (y = 0) and rows go down. Begin
at x = 0 on the first row. (1.14d's drawer tests bit 0 only,
`render/sprite-placement.md` §4; the two rules agree on the live values
0 and 1; other values are counted by the C52 game-file check.)

Read bytes until `length` bytes are consumed:
- `0x80`: end of row. x = 0, move to the next row.
- `b & 0x80` (b ≠ 0x80): skip `b & 0x7F` transparent pixels: x += b & 0x7F.
- otherwise: the next `b` bytes are pixel indices written at x, x+1, ….
  x += b.

Errors: a copy that runs past the encoded data; pixels written outside
`[0, width) × [0, height)` (a skip that passes the row end is allowed, but
pixels must stay inside). Moving to a row past the last is allowed as long
as no pixel is written there.

### Placement (informational)

`offset_x`, `offset_y` position the frame relative to the sprite's anchor.
See Open questions.

## Constants & data dependencies

None.

## Randomness

None.

## Edge cases & original bugs

- Zero-size frames (width or height 0) decode to empty images (d2rs
  rule). None occurs in the 1.14d files: 0 in the 1,651 DC6 listed by
  `d2exp.mpq` (367) and `d2data.mpq` (1,284) (ignored game test
  `dc6_zero_size_frames`, 2026-10-06; `patch_d2.mpq` has no `(listfile)`
  and is not enumerated). The earlier "they occur" was unmeasured.

## Test vectors

| Input / seed | Expected output | Source |
|---|---|---|
| frame 3×2, flip 0, data `02 05 06 80 82 01 07 80` | rows top→bottom: `[0,0,7]`, `[5,6,0]` | §Pixel decoding |
| frame 2×1, data `03 01 02 03` | error (pixel outside the row) | §Pixel decoding |
| frame 2×1, data `02 01` | error (copy runs past the data) | §Pixel decoding |
| every `.dc6` in the 1.14d archives | decodes | survey |

## Provenance

Community documentation of DC6 (Phrozen Keep), cross-checked against
Riiablo (Apache-2.0) `file/Dc6.java`, `file/Dc6Decoder.java`,
`codec/DC6.java`. No Blizzard code or decompiler output was consulted.

## Open questions

1. ~~Exact screen placement~~ (Riiablo's codecs disagree by one row).
   1.14d draws `[offset_y − height + 1, offset_y]`: owner
   `render/sprite-placement.md` §2.
2. ~~Orientation of `flip = 1` frames.~~ Confirmed visually: the 140
   flipped frames (all in `data\global\items\inv1x1/1x2/2x2/2x3.dc6`)
   render upright when decoded top-down (`mpq-tool render`).
3. Whole-file pixel limit (64M, §Frame) is an implementation limit, not
   observed original behavior: confirm every 1.14d `.dc6` stays under it
   (`mpq-tool formats`).
   *Answered* (game-file read, 2026-10-07, the 1,651 DC6s d2data and
   d2exp list; `Patch_D2.mpq` unlisted): the largest file's frames total
   3,190,514 pixels, far under 64M; every file has `encoding` 0.
