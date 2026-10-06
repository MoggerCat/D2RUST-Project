# Spec: Render — Shading (PL2 index maps, light-map selection, colormaps, highlight)

- **Status:** draft (2026-10-06, RE on 1.14d `Game.exe` plus measurements
  on the five act `pal.pl2` files of `d2data.mpq`; no pixel capture of a
  shaded draw compared yet). Every rule names its 1.14d address.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::scene` (`MapTable`, `ShadeChain`),
  `d2-client::composite` (`ComponentResolver::shade`), `d2-client::world_view`
- **Related specs:** `formats/palette.md` (PL2 file layout), `render/composition.md`
  §4–§5 (palette and the pixel write), `render/blend-modes.md` (draw modes,
  blend tables), `render/unit-composite.md` §7 (which colormap source a
  component uses), `ui/text.md` §4 (text-color maps), `render/lighting.md`
  (light values; to write), `client/render-pipeline.md` §A4, §B3

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 40–52 |
| Inputs | 53–63 |
| Outputs / state changes | 64–67 |
| Rules | 68–69 |
|   1. The palette-table block | 70–100 |
|   2. Map semantics | 101–109 |
|   3. Light map of a cel draw | 110–123 |
|   4. Light maps of DT1 tile blocks | 124–171 |
|   5. Selected-unit highlight | 172–187 |
|   6. Remap tables (`P`) | 188–219 |
|   7. Mapped index 0 | 220–229 |
|   8. Tables loaded but not drawn by GDI | 230–241 |
|   9. Palettes per screen region | 242–247 |
|   10. d2rs answers | 248–259 |
| Constants & data dependencies | 260–267 |
| Randomness | 268–272 |
| Edge cases & original bugs | 273–280 |
| Test vectors | 281–304 |
| Provenance | 305–322 |
| Open questions | 323–341 |
<!-- /index -->

## Summary

1.14d shades a drawn pixel only with 256-entry index maps: a **remap** `P`
chosen by the caller (unit colormap, item color, text color) and a
**light map** `L` chosen from a light value. Both come from tables the
palette loader builds once per act palette, almost all copied unchanged
from the act `pal.pl2`; two are computed from the palette. This spec owns
what each table means, how a draw's light value selects a light map (per
cel, per wall pixel, per floor pixel), the selected-unit highlight table,
the unit and item colormap tables, and the rule for a mapped index 0. How
`P`, `L` and the blend table `T` combine in one pixel write is
`composition.md` §5; which draw mode a draw uses is `blend-modes.md`.

## Inputs

| Name | Type | Source |
|---|---|---|
| act `pal.pl2` | 439,808 + 259·T bytes | `formats/palette.md`; act by `composition.md` §4 |
| light value of a cel draw | byte `v` (0xFF = unlit) | caller; for units `lighting.md` |
| light values of a wall/roof block | 4 ints `c0…c3` | caller's light record (`lighting.md`) |
| floor light grid | 12-byte cells, 8 per row | `lighting.md` (`0x00477730` grid) |
| remap request | unit palette index, monster shift, item, text color | `unit-composite.md` §7, `ui/text.md` §5 |
| `items\Palette\*.dat`, monster `palshift.dat`, `RandTransforms.dat`, `GreenBlood.dat` | index maps | archives (§6) |

## Outputs / state changes

The 256-byte maps `P` and `L` passed to each pixel write; no game state.

## Rules

### 1. The palette-table block

The D2Win loader `0x004FB1E0` (`composition.md` §4) copies tables out of
the act `pal.pl2`, computes two more, and `0x004FB010` hands the driver a
block of 0x48 pointers (`0x004F6410` → `SetPaletteTables`; the driver keeps
it at `[0x007C9150]`; the DT1 tile helpers keep a copy at `0x007D5458`
and their own copy of the 49 maps at `0x007D2348`, `0x004F8E40`). Offsets
are PL2 file offsets (`formats/palette.md` Layout).

| Block | Table | PL2 offset | Meaning (§2) |
|---|---|---|---|
| `+0x00`, `+0x04`, `+0x08` | alpha blend levels 0, 1, 2 (65,536 each) | `0x3500`, `0x13500`, `0x23500` | `blend-modes.md` §2 |
| `+0x0C + 4k`, k = 0…31 | light map `k` | `0x400 + 256k` | §3 |
| `+0x8C + 4k`, k = 0…15 | inventory color variation `k` | `0x2400 + 256k` | §8 |
| `+0xCC` | selected-unit shift | `0x3400` | §8 |
| `+0xD0 + 4k`, k = 0…12 | text-color map `k` | `0x6B627 + 256k` | `ui/text.md` §4 |
| `+0x104` | additive blend | `0x33500` | `blend-modes.md` §2 |
| `+0x108` | multiplicative blend | `0x43500` | `blend-modes.md` §2 |
| `+0x10C` | max-component blend | `0x5B500` | `blend-modes.md` §2 |
| `+0x110` | darkened color shift | `0x6B500` | §8 |
| `+0x114` | text colors (13 × RGB) | `0x6B600` | not used for drawing (`ui/text.md` §4) |
| `+0x118` | highlight map `H` (computed) | — | §5 |
| `+0x11C` | red map `R` (computed) | — | §8 |

Outside the block, `0x004FB1E0` copies the 128 maps at PL2 `0x53500`
(the 111 hue variations, the red, green and blue tones and the 14
unknown variations, in file order) to `0x007D6468`; `0x004FB0C0(i)`
returns map `i` of them (§6). Only the first 1,024 bytes (the palette)
and the tables above are read; every table is used as stored, never
transposed (`composition.md` §5).

### 2. Map semantics

A 256-byte map `M` sends a source index `i` to `M[i]`. The maps are data:
d2rs reproduces them by reading the act PL2 (never by a color formula).
Measured on the act 1 file (informative): light map 31 is the identity
(all five acts); light map `k < 31` darkens (index 31 = (196, 196, 196)
→ map 0: (8, 4, 4), map 16: (104, 104, 104)); every light map sends 0 to 0
(all five acts).

### 3. Light map of a cel draw

A cel draw (`sprite-placement.md` §1; GDI `0x006C84B0` slot `+0x84`,
`0x006C85A0` slot `+0x88`) takes one light byte `v`:

1. `v = 0xFF`: no `L`.
2. otherwise `L` = light map `v >> 3` (block `+0x0C + 4 × (v >> 3)`), so
   `v` 0…7 → map 0 (darkest), `v` 0xF8…0xFE → map 31.
3. Draw mode 7: `L` = `H` (§5) whatever `v` is.

What `v` a unit, missile, item or overlay receives is `lighting.md`'s
(the unit draw `0x00471EC0` gets it in EDX). The composition with `P` and
`T` is `composition.md` §5.

### 4. Light maps of DT1 tile blocks

Tiles carry no single light byte; each 32-pixel-wide block gets four
corner values. The gradient table `G` (`0x007CA320`, 32 × 32 × 32 bytes,
built by `0x004F7CC0`) is

`G[a][b][x] = ⌊(32·a + x·(b − a)) / 32⌋` for `a, b, x` in 0…31

(the value walks from `a` at `x = 0` toward `b`; floor division).

**Walls and roofs** (lit wall slot `+0x9C` GDI `0x006C94B0`, translucent
wall `+0xA0` `0x006C93A0`; per block helper `0x004F8120` / `0x004F84F0`).
The caller passes `c0, c1, c2, c3` (ints, 0…255, from `lighting.md`).

1. `[0x007D2340]` selects a flat-only path; it has no writer in
   `Game.exe`, so it is 0.
2. Let `Δ = |c1 − c0| + |c3 − c0| + |c2 − c1|`. If the low-quality
   setting (`[0x0072DA50]`, settings `+0x08`, default 0) is 0 and
   `Δ > 9`: **gradient**. Otherwise **flat**.
3. Flat: `c0` in 0xF8…0xFF → the block is copied unlit (no `L`); else
   every pixel uses light map `c0 >> 3` (`0x004F8050`).
4. Gradient (`0x004F71A0`, rows top to bottom, `r` = 0…31): left level
   `a_r = (32·c0 + r·(c3 − c0)) >> 8`, right level
   `b_r = (32·c1 + r·(c2 − c1)) >> 8` (arithmetic shifts); the pixel in
   column `x` of the block uses light map `G[a_r][b_r][x]`. So `c0` is the
   top-left, `c1` the top-right, `c2` the bottom-right, `c3` the
   bottom-left corner.

**Floors** (slot `+0x7C` GDI `0x006C95D0` → helper `0x004F8B80`). The
caller passes the floor light grid: cells of 12 bytes, 8 cells per row,
the cell's byte 0 is its light value; let `e[n]` be the byte of cell `n`.
A block with grid coordinates `(gx, gy)` (block record bytes 6 and 7) has
base `g = gx + 8·gy`.

1. Let `Δ = |e[g+10] − e[g+9]| + |e[g+17] − e[g+9]| + |e[g+18] − e[g+10]|`.
   Low quality set, or `Δ < 10`: **flat** with `v = e[g+9]`: every pixel
   uses light map `v >> 3` (`0x004F8810`; no unlit copy for floors).
2. Otherwise corners (each sum shifted right by 2):
   `c0 = e[g+8] + e[g+9] + e[g+16] + e[g+17]`,
   `c1 = e[g+1] + e[g+2] + e[g+9] + e[g+10]`,
   `c2 = e[g+10] + e[g+11] + e[g+17] + e[g+19]`,
   `c3 = e[g+17] + e[g+18] + e[g+25] + e[g+26]`
   (`c2` uses `e[g+17]`, not `e[g+18]`: reproduce).
3. RLE floor blocks (block flag bit 2, `0x004F8850`): rows `r` = 0…14,
   `a_r = (16·c0 + r·(c3 − c0)) >> 7`, `b_r = (16·c1 + r·(c2 − c1)) >> 7`,
   column `x` uses `G[a_r][b_r][x]`. Isometric (diamond) blocks go to
   `0x004F6BB0` with the same corners: per-pixel rule is Open question 1.

### 5. Selected-unit highlight

The hovered unit is drawn in draw mode 7 (which units: `blend-modes.md`
§3, §4). The GDI cel draw replaces `L` with block `+0x118` = `H` (§3 r3).
`H` is computed by `0x004FB1E0` from the palette (§4 of `composition.md`,
entries `(R, G, B)`):

`H[i] = nearest(min(255, ⌊R·170/100⌋), min(255, ⌊G·170/100⌋), min(255, ⌊B·170/100⌋))`

where `nearest(r, g, b)` (`0x00605210`) is the index `j` in 0…255 with the
smallest `(R_j − r)² + (G_j − g)² + (B_j − b)²`, the lowest `j` on ties
(index 0 included). The caller also doubles the light byte (clamped to
0x40…0xFF) for mode 7; the GDI draw ignores it (§3 r3). The PL2
"selected-unit shift" map is **not** this table (act 1: equal for 63 of
256 entries) and no GDI draw reads it (§8).

### 6. Remap tables (`P`)

The cel draw's palette argument is `P` (`composition.md` §5). The source
of each component's `P` is `unit-composite.md` §7; the tables are:

1. **Unit palette index** (unit `+0x6C`, `p ≠ 0`): hue/tone map `p − 1`
   of §1 (`0x004FB0C0(p − 1)`; callers `0x00471000`, `0x0047200F`). So
   `p` = 1…111 are the hue variations, 112–114 the red, green, blue tones,
   115–128 the unknown variations.
2. **Monster palette shift** (`0x00477530`, monsters only; when the unit
   has no palette index): the class's `palshift.dat` (2,048 bytes = 8
   maps, loaded per monster class into the table `[0x007B9578]`, 8 bytes
   per class) or, for a shift index ≥ 8, map `index − 8` of
   `Data\Global\Monsters\RandTransforms.dat` (30 maps at `0x007B9580`,
   loaded by `0x00476EA0`). The shift index is `0x0046F250` of the unit's
   gfx (+0x54). Details: Open question 2.
3. **Blood map** (`0x00477680`): `Data\Global\Monsters\GreenBlood.dat`
   (one map at `[0x007BB384]`); used for S8 components and missiles with
   `LocalBlood` when `0x0044DC60` (`[0x007A05FC]`) ≠ 0 (Open question 3).
4. **Item color** (`0x0062C100(unit, item, …)` → `0x00600C20(t, c)`):
   map `c` (0…20) of item palette file `t`, stored at
   `0x008ADBB8 + (105·t + c)·256` (`0x006009C0` loads each file's 21 maps,
   5,376 bytes). Files by `t`: 1 `grey`, 2 `grey2`, 3 `gold`, 4 `brown`,
   5 `greybrown`, 6 `invgrey`, 7 `invgrey2`, 8 `invgreybrown`
   (`Data\Global\Items\Palette\<name>.dat`, loader `0x00600B60`).
   `0x00600C20` returns no map for `t` = 0, 3, 4 or ≥ 9, or `c` ≥ 21. In
   the world `t` is the item's `Transform` (items `+0x141`); the first
   source of `c` is an active state of the unit with `itemtrans`
   (states `+0x2C`) < 21 whose `itemtype` (`+0x2A`) the item matches; the
   remaining cases (by item quality and affixes) are Open question 4.
5. **Text color** `k`: `ui/text.md` §4.

### 7. Mapped index 0

Transparency is decided before any map: only pixels the cel's encoding
draws reach the write (`sprite-placement.md` §3, §6; DCC index 0 is
encoded as a skip). A drawn pixel whose `P`, `L` or `T` result is 0 is
**written as index 0**, i.e. opaque palette entry 0 (black in all five
acts, `composition.md` OQ1); dark light maps do send some indices to 0.
Hue maps send index 0 to non-zero values; that matters only if a source
index 0 is ever drawn (DC6 literal runs, `dc6.md`).

### 8. Tables loaded but not drawn by GDI

Copied or computed, but read by no GDI cel or tile path:

| Table | Reader found | Note |
|---|---|---|
| inventory color variations (PL2 `0x2400`) | the client copy of the 49 maps (`0x004FB1C0` → `0x007B5C10`, from `0x00475E5D`) | light byte `v >> 3` never exceeds 31 |
| selected-unit shift (PL2 `0x3400`) | same copy | highlight uses `H` (§5) |
| darkened color shift (PL2 `0x6B500`, block `+0x110`) | none found | Open question 5 |
| `R[i] = nearest(R_i, 0, 0)` (block `+0x11C`) | none found | Open question 5 |
| text RGB (block `+0x114`) | — | `ui/text.md` §4 |

### 9. Palettes per screen region

One palette per presented frame (`composition.md` §4): world, UI and
cursor share the act palette; all per-draw color differences are index
maps.

### 10. d2rs answers

| Hook | Answer |
|---|---|
| `MapTable` contents | the act PL2 maps of §1 unchanged, plus `H` and `R` computed by §5 / §8 at palette load |
| `ShadeChain` of a cel | `[P]` then `[L]` (`L[P[s]]`, `composition.md` §5); `L` from §3; when the draw also has `T`, see `composition.md` §5 (`P` dropped when `L` is present) |
| `ShadeChain` of a tile pixel | `[L]` with `L` from §4 per pixel (flat blocks: one map) |
| `ComponentResolver::shade` | the §6 table selected by `unit-composite.md` §7 |
| `TextHooks::glyph_look` shade | `ui/text.md` §4 |
| mapped result 0 | drawn as index 0 (§7) |
| frame palettes | one (§9) |

## Constants & data dependencies

PL2 offsets of §1 (`formats/palette.md`); highlight factor 170/100; `G`
dimensions 32 × 32 × 32; gradient thresholds `Δ > 9` (walls), `Δ ≥ 10`
(floors); settings struct `0x0072DA48` (`+0x08` low quality, `+0x14`
blended shadows: `blend-modes.md` §5); `Transform` (items `+0x141`),
states `itemtrans` / `itemtype`; files of §6.

## Randomness

None in this spec. (A monster's `RandTransforms` choice is made elsewhere:
Open question 2.)

## Edge cases & original bugs

1. Floor corner `c2` averages `e[g+17]` instead of `e[g+18]` (§4).
2. Item palette files 3 (`gold`) and 4 (`brown`) are loaded but
   `0x00600C20` never returns their maps.
3. The highlight ignores the light byte in GDI (§5).
4. Unit palette index `p` selects map `p − 1`: `p = 0` means "none".

## Test vectors

Real values: `d2data.mpq` `data\global\palette\ACT1\Pal.PL2`, 443,175
bytes, SHA-256 `de848a8dfef15e9d8481af985fdaf02087653916493d2f308a336b70a0eb6549`
(the captured run's presented palette equals its first 1,024 bytes as
R, G, B: `composition.md` §4).

| Input | Expected output | Source |
|---|---|---|
| act 1, cel light `v = 0x7F`, `s = 100` | `L` = map 15, `d' = L[100] = 68` (PL2 byte `0x400 + 15·256 + 100`) | §3, live |
| act 1, `v = 0xFF`, `s = 100`, no `P`, no `T` | `d' = 100` | §3 r1 |
| act 1, `v = 0xF8`, `s = 31` | map 31, `d' = 31` (identity) | §3, live |
| act 1, mode 7, any `v`, `s = 31` | `d' = H[31] = 255`; `H[100] = 168`, `H[172] = 33`, `H[200] = 217`, `H[0] = 0` | §5, live |
| act 1 `R` | `R[255] = 98`, `R[100] = 10`, `R[0] = 0` | §8, live |
| act 1, unit palette index `p = 2`, `s = 100` | hue map 1: `106` (PL2 byte `0x53500 + 256 + 100`) | §6 r1, live |
| act 1, `p = 112` (red tone), `s = 100` | map 111: `33` | §6 r1, live |
| `G[31][0][1]`, `G[31][0][31]`, `G[0][31][31]`, `G[10][20][16]`, `G[20][10][1]` | 30, 0, 30, 15, 19 | §4 |
| wall corners `c0…c3` = 200, 200, 200, 200 | flat, map 25 | §4 r3 |
| wall corners 0xFF, 0xFF, 0xFF, 0xF8 | flat (`Δ` = 7), unlit copy | §4 r3 |
| wall corners `c0 = 0`, `c1 = 255`, `c2 = 255`, `c3 = 0`, row 0 | `a = 0`, `b = 31`, column `x` uses map `⌊31x/32⌋` | §4 r4 |
| floor grid, every cell 0x80 | `Δ` = 0: flat, map 16 | §4 floors r1 |
| `t = 3` (gold), any `c` | no map | §6 r4 |
| drawn `s = 5`, `P[5] = 0`, no `L`, no `T` | index 0 written | §7 |

## Provenance

1.14d `Game.exe`: palette loader `0x004FB1E0` (copies and the `H` / `R`
computation, nearest color `0x00605210`), block builder `0x004FB010`,
`0x004F6410`, DT1 helper copies `0x004F8E40`, getters `0x004FB0C0`,
`0x004FB1C0`; GDI cel draws `0x006C84B0` / `0x006C85A0` (light byte and
mode 7); tile helpers `0x004F8120`, `0x004F8050`, `0x004F71A0`,
`0x004F84F0`, `0x004F8B80`, `0x004F8810`, `0x004F8850`, gradient table
`0x004F7CC0`; settings `0x0072DA48` (driver init slot `+0x04`,
`0x004F52E0`); colormaps `0x00477530`, `0x00476EA0`, `0x00477680`,
`0x0062C100`, `0x00600C20`, `0x006009C0`, `0x00600B60`; unit palette index
use `0x00470EC0` (`0x00471000`) and `0x00471EC0` (`0x0047200F`).
Measurements: scratch scripts over the five act PL2 files and
`items\Palette\*.dat` (5,376 bytes each) extracted with `mpq-tool
extract`; field offsets from `specs/data/fields.tsv` (items `Transform`
`+0x141`, states `itemtype` `+0x2A`, `itemtrans` `+0x2C`). `D2MOO`
not used.

## Open questions

1. Per-pixel light of isometric floor blocks (`0x004F6BB0`, from the four
   corners of §4): Ghidra read; a capture of a lit floor at night.
2. Monster palette shift: what `0x0046F250` returns (the shift index),
   the `+0x804` second set in `0x00477530` (when `0x0044DC60` ≠ 0 and the
   `+0x11C` byte = 2), the class rule for 0x16B / 0x16C and the
   `[0x007BB380]` gate. Ghidra read.
3. `0x0044DC60` (`[0x007A05FC]`): which option turns the green-blood map on.
   Ghidra read of its writers.
4. Item color `c` beyond the state rule: the quality / affix branches of
   `0x0062C100` (`colors.txt` codes in magic, unique and set rows). Ghidra
   read; a capture of a colored item on the ground.
5. Readers of the darkened shift (block `+0x110`) and of `R` (`+0x11C`)
   outside the GDI cel and tile paths (D2Win, automap, UI). Ghidra xref
   on `[0x007C9150]` users.
6. The light values themselves (unit `v`, wall `c0…c3`, floor grid,
   player light flicker, `capture.md` findings): `render/lighting.md`.
