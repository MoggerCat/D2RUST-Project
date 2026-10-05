# Spec: Formats — Palettes (.dat) and palette transforms (.pl2)

- **Status:** implemented. All 19 `pal.dat` and 17 `.pl2` files in 1.14d
  parse (`mpq-tool formats`). One PL2 has 12 text colors, the rest have 13.
- **Target version:** 1.14d
- **Crate/module:** `d2-formats::palette`
- **Related specs:** `specs/formats/dc6.md`, `specs/formats/dcc.md` (8-bit
  indexed images that these palettes color)

## Summary

All D2 graphics are 8-bit palette indices. A `.dat` palette maps the 256
indices to colors. A `.pl2` file next to it (same directory, e.g.
`data\global\palette\act1\pal.pl2`) holds precomputed index→index lookup
tables for lighting, blending, item color variations and text colors.

## Inputs

| Name | Type | Source |
|---|---|---|
| `.dat` bytes | 768 bytes | MPQ, e.g. `data\global\palette\act1\pal.dat` |
| `.pl2` bytes | see below | MPQ, e.g. `data\global\palette\act1\pal.pl2` |

## Outputs / state changes

- `Palette`: 256 colors (r, g, b).
- `Pl2`: named lookup tables, each 256 bytes (or 256×256 for the blend
  tables), plus text colors.

## Rules

### .dat palette

Exactly 768 bytes: 256 entries of 3 bytes in the order **blue, green, red**.
Any other length is an error. Index 0 is drawn as fully transparent when
used in sprites. The palette itself stores a color for index 0; transparency
is a rendering rule, not part of the file.

### .pl2 palette transform

Layout, in file order. Every "map" is 256 bytes indexed by a palette index,
giving a palette index.

| Field | Size (bytes) | Count of 256-byte maps |
|---|---|---|
| base palette: 256 × 4 bytes (copy of the `.dat` colors, ignored) | 1024 | — |
| light level variations [32] | 8192 | 32 |
| inventory color variations [16] | 4096 | 16 |
| selected-unit shift | 256 | 1 |
| alpha blend [3][256] (indexed by [level][source index], gives a map over the destination index) | 196608 | 768 |
| additive blend [256] | 65536 | 256 |
| multiplicative blend [256] | 65536 | 256 |
| hue variations [111] | 28416 | 111 |
| red tones | 256 | 1 |
| green tones | 256 | 1 |
| blue tones | 256 | 1 |
| unknown variations [14] | 3584 | 14 |
| max-component blend [256] | 65536 | 256 |
| darkened color shift | 256 | 1 |
| text colors: T entries of 3 bytes (r, g, b) | 3·T | — |
| text color shifts [T] | 256·T | T |

The fixed part (everything before the text colors) is 1024 + 1714 × 256 =
439,808 bytes. The text-color count `T` is derived from the remaining
length: `remaining = 259 × T`. Normally `T = 13`, but some files have
`T = 12`. A remaining length that isn't a multiple of 259 is an error.

The exact rendering use of each table (which light level maps to which
brightness, which blend level means which alpha) is a rendering rule for
Phase 1b/6, not part of the format.

## Constants & data dependencies

- 1714 = 32 + 16 + 1 + 768 + 256 + 256 + 111 + 3 + 14 + 256 + 1 maps.

## Randomness

None.

## Edge cases & original bugs

- PL2 files with 12 text colors instead of 13 (noted by Riiablo for the
  loading-screen palette).

## Test vectors

| Input / seed | Expected output | Source |
|---|---|---|
| 768 bytes `[b0,g0,r0, b1,g1,r1, …]` | color i = (r_i, g_i, b_i) | §.dat |
| 767 or 769 bytes | error | §.dat |
| PL2 of 439,808 + 13 × 259 bytes | parses, T = 13 | §.pl2 |
| PL2 of 439,808 + 12 × 259 bytes | parses, T = 12 | §.pl2 |
| PL2 of 439,808 + 100 bytes | error | §.pl2 |
| every `.dat` under `data\global\palette` in the 1.14d archives | parses | survey |
| every `.pl2` in the 1.14d archives | parses | survey |

## Provenance

Community documentation of the PL2 layout (Phrozen Keep), cross-checked
against Riiablo (Apache-2.0) `codec/PL2.java` (1714 maps, 13 tints of 3 +
256 bytes, skipping the 0x400 header) and `file/Palette.java` (BGR order).
No Blizzard code or decompiler output was consulted.

## Open questions

1. Byte order of the PL2 base palette (it is ignored, so this doesn't block
   anything).
2. Rendering meaning of each table: to be specified in a Phase 1b rendering
   spec, checked visually against the original game.
