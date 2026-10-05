# Spec: Formats — DCC (compressed animations)

- **Status:** implemented. All 21,717 `.dcc` files in 1.14d decode, with
  every sub-stream exactly consumed and fewer than 8 leftover PCD bits per
  direction (`mpq-tool formats`). Version 6 throughout. No frame has
  bottom-up = 1. Visually confirmed on large animations (`DiabloLightning`,
  `MissileWarHammer`, `poisonNova` via `mpq-tool render`).
- **Target version:** 1.14d
- **Crate/module:** `d2-formats::dcc`
- **Related specs:** `specs/formats/cof.md`, `specs/formats/palette.md`

## Summary

DCC stores the animations of units, missiles and many objects: `D`
directions × `F` frames of 8-bit indexed images. Each direction is one
bit-packed stream. Frames are divided into roughly 4×4-pixel cells. A cell
is either unchanged from the last time that area was drawn, or coded with up
to four palette colors plus 1–2-bit indices per pixel.

## Inputs

| Name | Type | Source |
|---|---|---|
| file bytes | `.dcc` | MPQ, e.g. `data\global\chars\...\*.dcc` |

## Outputs / state changes

Per direction: the frame headers, the direction bounding box, and per frame
a `width × height` image of palette indices (0 = transparent), cropped to
the frame's own box.

## Rules

### Bit reading

Directions are LSB-first bit streams (the first bit is bit 0 of the first
byte). An **unsigned n-bit field** is n bits read LSB-first. A **signed
n-bit field** is the same value, sign-extended from bit n−1 (n = 0 gives 0).

`ENCODED_BITS = [0, 1, 2, 4, 6, 8, 10, 12, 14, 16, 20, 24, 26, 28, 30, 32]`
maps a 4-bit width code to a field width.

### File header (little-endian bytes)

| Offset | Size | Field |
|---|---|---|
| 0 | u8 | signature: must be 0x74 |
| 1 | u8 | version (6 in 1.14d; recorded) |
| 2 | u8 | directions `D` |
| 3 | u32 | frames per direction `F` (must be ≤ 256) |
| 7 | u32 | tag (recorded) |
| 11 | u32 | final DC6 size (recorded) |
| 15 | u32 × D | direction offsets (absolute) |

Direction `d`'s data runs from offset `d` to offset `d+1` (or end of file
for the last). Offsets must increase and lie inside the file.

### Direction header (bits)

| Width | Field |
|---|---|
| 32 | outsize coded (recorded) |
| 2 | compression flags: bit 0 = raw pixel encoding present, bit 1 = equal-cells stream present |
| 4 | variable0 width code |
| 4 | width code |
| 4 | height code |
| 4 | x offset code |
| 4 | y offset code |
| 4 | optional bytes code |
| 4 | coded bytes code |

Then `F` frame headers, each made of fields whose widths come from the
codes via `ENCODED_BITS`:

| Field | Kind |
|---|---|
| variable0 | unsigned |
| width | unsigned |
| height | unsigned |
| x offset | signed |
| y offset | signed |
| optional bytes | unsigned |
| coded bytes | unsigned |
| bottom-up | 1 bit |

If any frame has optional bytes: skip to the next byte boundary, then read
each frame's optional bytes in frame order. Bit reading continues right
after them.

Then:
- 20 bits equal-cells stream size, only if flag bit 1 is set (else 0)
- 20 bits pixel-mask stream size
- 20 bits encoding-type stream size and 20 bits raw-pixel stream size, only
  if flag bit 0 is set (else 0)
- 256 bits **pixel values**: for i = 0..255, if the bit is 1, palette index
  i is appended to the list `PV`. A pixel code `c` means palette index
  `PV[c]`. A code ≥ len(PV) is an error.

The sub-streams follow, back to back, starting at the current bit: equal
cells, pixel mask, encoding type, raw pixels (each exactly its size in
bits). Everything after them, to the end of the direction data, is the
**pixel-code-and-displacement (PCD) stream**. Reading past the end of any
stream is an error.

### Boxes

For each frame: `x_min = x_offset`, `x_max = x_min + width − 1`. If
bottom-up: `y_min = y_offset`, `y_max = y_min + height − 1`; else
`y_max = y_offset`, `y_min = y_max − height + 1`. The **direction box** is
the union of the frame boxes, with `dir_w = x_max − x_min + 1` and `dir_h`
likewise. Frame positions below are relative to the direction box:
`fx = x_min(frame) − x_min(dir)`, `fy` likewise. A width or height of 0, or a
direction box larger than 16M pixels, is an error.

### Cells

The direction is divided into 4×4 **direction cells**,
`cells_w = ceil(dir_w / 4)` across.

Each frame is divided into **frame cells**, per axis. For the x axis (y is
identical with fy and height):
- `first = 4 − (fx mod 4)`
- If `width − first ≤ 1`: one cell of width `width`.
- Otherwise, with `rem = width − first`: cells are `first`, then 4-wide
  cells, then the remainder:
  - `rem mod 4 == 0`: `rem/4` cells of 4
  - `rem mod 4 == 1`: `rem/4 − 1` cells of 4, then one of 5
  - otherwise: `rem/4` cells of 4, then one of `rem mod 4`

Frame cells are visited in row-major order. Frame cell `(cx, cy)` belongs
to direction cell `(fx div 4 + cx, fy div 4 + cy)`.

### Stage 1: cell colors (all frames, in order)

Each direction cell remembers the 4 codes of the last **cell entry**
written for it (initially none). For each frame `f` and each frame cell `c`
(direction cell `d`):

1. If `d` already has an entry: read 1 bit from the equal-cells stream if
   that stream exists, else use 0. If the bit is 1, the cell is **equal**:
   no entry is made, continue with the next cell. Otherwise read a 4-bit
   **pixel mask** from the pixel-mask stream.
   If `d` has no entry: pixel mask = 0xF.
2. `n` = number of set bits in the mask. If `n > 0` and the encoding-type
   stream exists, read 1 bit `raw`, else `raw = 0`.
3. Decode up to `n` codes into a stack: `last = 0`. Repeat `n` times:
   - if `raw`: `code` = 8 bits from the raw-pixel stream
   - else: `code = last`; repeat { `disp` = 4 bits from PCD; `code += disp` }
     while `disp == 15`. The code is kept modulo 256.
   - If `code == last`, stop early. Otherwise push `code` and set
     `last = code`.
4. New entry values `v[0..4]`: for i = 0..3, if mask bit i is set, pop the
   most recently pushed code (or 0 if the stack is empty). Otherwise copy
   `v[i]` from `d`'s previous entry.
5. Record the entry as (frame `f`, cell `c`, `v`) in an ordered queue, and
   as `d`'s latest entry.

### Stage 2: building frames (all frames, in order, after stage 1)

State: a persistent direction-sized pixel buffer `B` (initially 0), and for
each direction cell its last drawn rectangle (initially none).

For each frame `f`: start an output image `O` (direction size, all 0). For
each frame cell `c` at rectangle `R = (x, y, w, h)` (relative to the
direction box) with direction cell `d`:
- If the next queue entry is for (`f`, `c`): take it (advance the queue).
  - If `v[0] == v[1]`: fill `R` in `B` with `v[0]`.
  - Else: `bits = 1` if `v[1] == v[2]`, else `2`. For each pixel of `R` in
    row-major order, read `bits` bits `i` from PCD and set the pixel to
    `v[i]`.
  - Copy `R` from `B` into `O`.
- Else (equal cell): if `d`'s last rectangle exists and has the same `w`
  and `h` as `R`, copy that rectangle of `B` into `R` of `B`, reading the
  whole source before writing (overlap-safe), then copy `R` from `B` into
  `O`. Otherwise set `R` in `B` to 0 (`O` stays 0 there).
- Set `d`'s last rectangle to `R`.

Frame `f`'s image is `O` cropped to `(fx, fy, width, height)`, with codes
mapped through `PV`.

### End checks

After both stages, the equal-cells, pixel-mask, encoding-type and
raw-pixel streams must be exactly consumed. The PCD stream may have
leftover bits (see survey).

## Constants & data dependencies

`ENCODED_BITS` above.

## Randomness

None.

## Edge cases & original bugs

- The "equal cell" copy reads from wherever that direction cell was last
  drawn, which may be several frames earlier. That's why the buffer `B` is
  persistent and not cleared between frames. Riiablo's newer decoder copies
  from the previous frame's output instead, which differs whenever a frame
  skips a cell. This spec follows the older codec and the community
  documentation.

## Test vectors

| Input / seed | Expected output | Source |
|---|---|---|
| frame cells for width 1, fx 0 | one cell of width 1 | §Cells |
| width 9, fx 0 | 4, 5 | §Cells |
| width 10, fx 2 | 2, 4, 4 | §Cells |
| width 7, fx 3 | 1, 4, 2 | §Cells |
| width 2, fx 3 | 2 (single cell: width − first = 1) | §Cells |
| synthetic one-frame direction (built by the test) | decodes to the expected 4-color image | §Stage 1/2 |
| every `.dcc` in the 1.14d archives | decodes; sub-streams exactly consumed | survey |

## Provenance

Paul Siramy's DCC decoding documentation (community), cross-checked
against Riiablo (Apache-2.0) `codec/DCC.java` (frame-cell sizes,
persistent frame buffer, cell copy rule) and `file/Dcc.java` (header and
stream layout, `ENCODED_BITS`). No Blizzard code or decompiler output was
consulted.

## Open questions

1. ~~Frames with bottom-up = 1~~: none exist in 1.14d, so the question
   doesn't arise for the original data.
2. Exact screen placement (the same one-row question as DC6): rendering
   spec.
3. Meaning of variable0 and the optional bytes.
