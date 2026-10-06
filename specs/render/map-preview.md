# Spec: Render — DS1 map preview and palette shading (Phase 1b)

- **Status:** implemented. GPU output is byte-identical to the CPU
  reference: full Rogue Encampment (`townN1.ds1`, 32.2M pixels, 3 repeated
  runs), full Lut Gholein (`LutN.ds1`, 37.5M pixels) and off-center crops
  (`d2-client verify`). A deliberately corrupted reference fails with the
  exact pixel count (`--perturb`). Checked visually: CPU renders, GPU
  captures and the `view` window.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::map` (layout, CPU reference renderer),
  `d2-client::render` (Bevy palette material)
- **Related specs:** `specs/formats/ds1.md`, `specs/formats/dt1.md`,
  `specs/formats/palette.md`

## Summary

Draws one preset DS1 map with its DT1 tiles, colored through a `.dat`
palette. This is the Phase 1b "first pixels" view. It is deliberately
**not** the full game renderer: no lighting, no shadows, no blend modes, no
objects or units, no random tile variants, and simplified draw order. Those
come in Phase 6. Anything this spec simplifies is listed under Edge cases.

## Inputs

| Name | Type | Source |
|---|---|---|
| DS1 | `d2_formats::ds1::Ds1` | MPQ, e.g. `data\global\tiles\ACT1\TOWN\townN1.ds1` |
| DT1 files | `d2_formats::dt1::Dt1` | listed in the DS1 (see Tile files) |
| palette | `d2_formats::palette::Palette` | `data\global\palette\act<N>\pal.dat`, N = DS1 act + 1 |

## Outputs / state changes

An ordered list of draw items (tile image + screen position). Drawing them
in order onto a background with index-0 pixels skipped gives the map image.
The CPU reference renderer and the GPU renderer must produce **identical**
pixels for the same view.

## Rules

### Tile files

Each DS1 file entry (e.g. `\d2\data\global\tiles\act1\town\floor.tg1`) maps
to an archive path: take the text from the first case-insensitive
occurrence of `data\` onward, and replace a `.tg1` extension with `.dt1`.
Entries without `data\` are skipped. DT1 files are loaded in the DS1's list
order. A missing file is skipped and reported.

### Tile library and lookup

A tile's key is `(orientation, main_index, sub_index)` from its DT1 header.
Lookup returns the **first** tile with that key, in load order (file order,
then tile order within the file). In the real game the choice among
equal-key tiles is random, weighted by rarity, and belongs to the
simulation (DRLG). The preview takes the first match.

### Tile images

A tile's image is the union bounding box of its blocks. Block `b` occupies
`[b.x, b.x + w) × [b.y, b.y + h)` in tile coordinates (w × h = 32 × 15 for
isometric blocks, 32 × 32 for RLE). Blocks are drawn in block order, and
index-0 pixels don't overwrite. The image keeps its offset `(x0, y0)` = the
bounding box's top-left in tile coordinates. A tile with no blocks has no
image.

### Cells

For a wall, floor or shadow cell value `c`: the cell is empty if `c & 0xFF`
(prop1) is 0. It is **not drawn** if the hidden flag `c & 0x80000000` is
set. Hidden tiles are invisible markers: in `townN1.ds1`, the river banks
carry hidden wall tiles (cell `0x80500081`, orientation 1, main 5) that are
solid blue if drawn. Otherwise sub index = `(c >> 8) & 0xFF` and main index
= `(c >> 20) & 0x3F` (`specs/formats/ds1.md`).

### Screen position

Screen coordinates have x to the right and y down. Cell `(x, y)` has origin

```
sx = (x − y) × 80
sy = (x + y) × 40
```

so a floor tile (blocks spanning 0..160 × 0..80) draws its diamond with the
top vertex at `(sx + 80, sy)`. A tile image is drawn with its top-left at:
- floors (orientation 0): `(sx + x0, sy + y0)`
- walls and other wall-layer tiles: `(sx + x0, sy + y0 + WALL_BASE)`
- roofs (orientation 15): `(sx + x0, sy + y0 + WALL_BASE − roof_height)`

`WALL_BASE` is the distance from a cell's origin to the line wall blocks
stand on (their y = 0). See Open question 1 for how its value is decided.

### What is drawn

- **Floors:** each floor layer, with orientation 0.
- **Walls:** each wall layer `i` with orientation layer `i`'s value `o`:
  - `o` = 0: nothing (an empty orientation).
  - `o` = 10 or 11: nothing (special tiles; invisible in game).
  - `o` = 13: nothing (shadows come from the shadow layer).
  - `o` = 3: the orientation-3 tile, and also the orientation-4 tile with the
    same main/sub, at the same position (the two halves of a corner).
  - otherwise: the tile with orientation `o`.
- **Shadows:** not drawn in 1b (they need a darkening blend; Phase 6).
- **Tag layer, objects, groups, paths:** not drawn.

A cell whose key has no tile is skipped and counted as missing.

### Draw order

Items are drawn in this order; later items paint over earlier ones:
1. Floor layer 0, then floor layer 1. Within a layer, cells in row-major
   order (y outer, x inner).
2. Lower walls (orientations 16–19), ordered as in step 3.
3. All other walls, ordered by `(x + y, x, wall layer, piece)`, where piece
   0 is the main tile and 1 is the orientation-4 half of a corner.
4. Roofs (orientation 15), ordered as in step 3.

### Sprites (demo)

After the map items, two demo sprites are drawn on top, both through the
same palette (act palette; 1b does not use the UI/units palettes):
- **DCC:** `data\global\objects\RB\TR\rbtrlitonhth.dcc` (the Rogue Encampment
  bonfire), direction 0, frame 0. Its anchor is the center cell
  `(width/2, height/2)` diamond center: `origin + (80, 40)`. The frame image
  is drawn at `anchor + (frame.x_min, frame.y_min)`.
- **DC6:** `data\global\ui\PANEL\ctrlpnl7.DC6` (the control panel),
  direction 0, all frames left to right. The first frame's top-left is the
  map bounds' top-left + (32, 32), and each next frame starts where the
  previous one ends (DC6 offsets are ignored).

Map bounds are recomputed to include the sprites.

### Palette shading

Every tile image is an 8-bit index image. A pixel with index 0 is
transparent (not drawn). Any other index `i` is drawn as `palette[i]`
(r, g, b) exactly, with no filtering, blending, tonemapping or dithering.
The background is opaque black.

GPU implementation requirements, so output matches the CPU reference
exactly: index images use an unsigned-integer texture read with
`textureLoad` (no sampler). The palette is a 256×1 sRGB texture, also read
with `textureLoad`. The render target is 8-bit sRGB. MSAA is off and
tonemapping is off. Quads are axis-aligned with edges on pixel boundaries at
1:1 scale.

## Constants & data dependencies

Tile cell size 160 × 80. Orientation meanings as listed above.

## Randomness

None (first-match tile choice).

## Edge cases & original bugs

Simplifications versus the game (all for Phase 6):
- Tile variant choice by rarity (DRLG) → first match.
- Shadows and translucent tiles → not drawn.
- Lighting, PL2 transforms, day/night → none.
- D2's exact isometric draw-order rules for walls versus units → the simple
  depth order above.

## Test vectors

| Input / seed | Expected output | Source |
|---|---|---|
| `\d2\data\global\tiles\act1\town\floor.tg1` | `data\global\tiles\act1\town\floor.dt1` | §Tile files |
| cell (0,0), (1,0), (0,1) | origins (0,0), (80,40), (−80,40) | §Screen position |
| synthetic tile: one iso block at (0,0) and one at (64,32) | image 96 × 47, offset (0,0), blocks at their places | §Tile images |
| synthetic DS1 with orientation 3 | draws the orientation 3 and 4 tiles at the same position, 3 first | §What is drawn |
| ordering of floors, lower walls, walls, roofs | as §Draw order | §Draw order |
| cell `0x80500081` (hidden) | not drawn | §Cells, real value from `townN1.ds1` |
| synthetic sprites | DCC at anchor + frame offset; DC6 frames in a row at bounds + (32, 32) | §Sprites |
| `townN1.ds1`, CPU render | recognizable Rogue Encampment (river, palisade, tents, wagons) | visual check |
| `LutN.ds1`, CPU/GPU render | recognizable Lut Gholein (palace, harbor, market) | visual check |
| `townN1.ds1`, GPU render of the same view | byte-identical to the CPU render | `d2-client verify` |
| reference with N corrupted pixels | verify fails reporting exactly N | `d2-client verify --perturb N` |

## Provenance

Isometric cell size and layout, DS1 tile-file conventions, orientation
meanings (3/4 corner split, 10/11 specials, 13 shadow, 15 roof, 16–19 lower
walls) and roof-height use are community knowledge (Paul Siramy's DS1/DT1
documentation and the DS1 editor's conventions). Block coordinate ranges
were measured from the 1.14d files (floors: blocks x 0..128, y 0..64; walls:
y −256..−32). No Blizzard code or decompiler output was consulted.

## Open questions

1. ~~`WALL_BASE`: 0 or 80.~~ **80.** Rendered both with the CPU reference
   (`townN1.ds1`). At 80, palisade and stone walls stand on the grass. At 0,
   a black gap opens under every wall.
2. Exact D2 draw order and shadow blending: Phase 6 rendering spec
   (order: `render/draw-order.md`; blending: `render/blend-modes.md`).
3. **Unflagged collision tiles.** `act1\outdoors\river.dt1` tile 28 (key
   orientation 1, main 5, sub 0) is a single-color tile (palette index 233,
   dark blue) with every sub-tile flag set to block walking: an invisible
   collision wall. In `townN1.ds1`, 40 cells use it with the hidden flag
   (not drawn) and 8 without it, at cells (44|51, 29|32|33|34), symmetric on
   both river banks. The preview draws those 8 as blue patches. Ruled out as
   the cause: rarity-based choice (it is the only tile with that key), cell
   prop1 bits (visible fences have the same 0x81), and file selection
   (`LvlTypes` "Act 1 - Town" and `LvlPrest` Dt1Mask 959 both include
   `River.dt1`). The client's tile draw path has no such skip
   (`render/draw-order.md` §7, its OQ7 carries the question).
