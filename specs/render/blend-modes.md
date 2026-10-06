# Spec: Render — Blend modes (draw modes, blend-table orientation, translucency sources, shadows)

- **Status:** draft (2026-10-06, RE on 1.14d `Game.exe`, measurements on the
  five act `pal.pl2` files and the 3,512 COFs of the archives, one capture
  check: the Town Portal of run 2 is consistent with mode 3). Every rule
  names its 1.14d address.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::scene` (`BlendOp`, CPU and GPU compositors),
  `d2-client::composite` (`ComponentResolver::blend`), `d2-client::world_view`
- **Related specs:** `render/composition.md` §5 (the pixel write),
  `render/shading.md` (maps `P`, `L`, `H`; the palette-table block),
  `render/unit-composite.md` §5, §7, §9 (component loop, single cels),
  `render/draw-order.md` §6, §8 (passes, wall fade), `formats/palette.md`,
  `formats/cof.md`, `client/render-pipeline.md` §A5, §B5

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 38–48 |
| Inputs | 49–59 |
| Outputs / state changes | 60–63 |
| Rules | 64–65 |
|   1. Draw modes | 66–90 |
|   2. Blend-table orientation (per drawer) | 91–115 |
|   3. Draw mode of a composite unit component | 116–157 |
|   4. Single-cel units and overlays | 158–169 |
|   5. Shadows (the darkening blend) | 170–201 |
|   6. Translucent walls and roofs | 202–219 |
|   7. d2rs answers | 220–230 |
| Constants & data dependencies | 231–239 |
| Randomness | 240–243 |
| Edge cases & original bugs | 244–254 |
| Test vectors | 255–282 |
| Provenance | 283–300 |
| Open questions | 301–319 |
<!-- /index -->

## Summary

Every cel draw of 1.14d carries a **draw mode** 0–7 that picks at most one
256 × 256 blend table `T` from the act PL2; tiles pick `T` from an alpha
byte. A blend writes `T[256·row + column]` with the destination index and
the (shaded) source index as row and column; which is the row depends on
the drawer, and this spec settles it per drawer from the 1.14d code. It
also owns where each draw's mode comes from (COF layer override, ghostly
monsters, the Fade stat, ethereal and transparent items, missiles,
overlays, the hover highlight) and the two shadow draws.

## Inputs

| Name | Type | Source |
|---|---|---|
| blend tables `A0`, `A1`, `A2`, `ADD`, `MUL`, `MAX` | 65,536 bytes each | act `pal.pl2` via the palette-table block (`shading.md` §1) |
| draw mode | 0–7 (other values possible from data) | §3–§5 |
| COF layer bytes 1, 3, 4 (shadow, override, new level) | u8 | `formats/cof.md` §Layer records |
| unit flags, Fade stat, items `transparent` / `transtbl`, `missiles.Trans`, `overlay.Trans` | data | §3–§4 |
| Blended Shadows setting | 0 / 1 | `[0x0072DA5C]` (§5) |
| wall alpha byte | 0–255 | `draw-order.md` §6, §8 |

## Outputs / state changes

The destination index of every blended pixel. No game state.

## Rules

### 1. Draw modes

The driver's blend getter (GDI `0x006C8250`, DirectDraw `0x00511D70`,
same bodies) maps a draw mode to a table of the palette-table block:

| Mode | `T` | PL2 table | Index-space write (cel drawers, §2) | Act 1 fit (informative) |
|---|---|---|---|---|
| 0 | block `+0x08` | alpha level 2 (`0x23500`) | `d' = A2[256·d + s']` | 25 % source: 378 / 400 |
| 1 | block `+0x04` | alpha level 1 (`0x13500`) | `d' = A1[256·d + s']` | 50 %: 372 / 400 |
| 2 | block `+0x00` | alpha level 0 (`0x3500`) | `d' = A0[256·d + s']` | 75 % source: 369 / 400 |
| 3 | block `+0x104` | additive (`0x33500`) | `d' = ADD[256·d + s']` | per-channel `min(255, d + s)`: 400 / 400 |
| 4 | block `+0x108` | multiplicative (`0x43500`) | `d' = MUL[256·d + s']` | `d·s/255`: 400 / 400 |
| 5 | none | — | `d' = s'` | opaque |
| 6 | block `+0x10C` | max-component (`0x5B500`) | `d' = MAX[256·d + s']` | no simple formula; `MAX[256·d + 0] = d` for all `d` |
| 7 | none | — | `d' = s'` with `L = H` (`shading.md` §5) | highlight |
| any other | none | — | as mode 5 | e.g. `overlay.Trans` 8 |

`s'` is the source after the shade maps of `composition.md` §5 (`P[s]`,
or `L[s]` when lit). The fit column counts, for 400 random pairs, how
often the table entry equals the nearest palette color (`shading.md` §5
metric) of the named formula with `d` = row, `s` = column: it names the
tables, it is not the rule (the rule is the table). The getter also
writes a per-mode value (table `0x0074C5A0`: 2, 2, 2, 1, 1, 0, 1, 0) to
the caller's argument slot; the GDI cel draws ignore it.

### 2. Blend-table orientation (per drawer)

A PL2 blend table is stored as `T[256·i + j]` (file byte
`table offset + 256·i + j`). The drawers index it as follows (1.14d):

| Drawer | Used by | Write |
|---|---|---|
| cel runs with `P` and `T` (`0x00607970`, column-clipped `0x00607B90`; the store at `0x006079DE` uses `T[(d << 8) + P[s]]`) | every cel draw with a blend mode | `d' = T[256·d + P[s]]` — **row = destination** |
| cel runs with `L` and `T` (`0x00606E40`, `0x00607060`) | lit translucent cels | `d' = T[256·d + L[s]]` — row = destination |
| unit shadow rows (`0x00608D60`) | §5 | `d' = T[256·d + 0]` — row = destination |
| translucent tile, unlit (`0x004F82D0`, DT1 helper `+0x14`) | shadow tiles (§5) | `d' = T[256·d + s]` — row = destination |
| translucent wall, lit (`0x004F84F0`, DT1 helper `+0x18`; the store at `0x004F86A2` uses `T[(L[s] << 8) + d]`) | translucent walls and roofs (§6) | `d' = T[256·L[s] + d]` — **row = source** |

**Verdict:** for cels and shadows the row is the destination, the column
the source; only the lit translucent wall drawer reads the transpose.
Evidence beyond the code: `MAX[256·d + 0] = d` for all 256 `d` (a black
source leaves the destination: the defining property of a lighten blend;
read transposed it holds for 21 of 256), and under this reading modes
0, 1, 2 weight the source 25 %, 50 %, 75 % (the 1.10f D2MOO names
`TRANS25`, `TRANS50`, `TRANS75`, a hint). `ADD` and `MUL` are exactly
symmetric in all five act PL2 files (0 of 65,536 entries differ from their
transpose), so additive and multiplicative draws cannot show the
orientation; the three alpha tables and `MAX` can (act 1 `A0`: 65,066
asymmetric entries).

### 3. Draw mode of a composite unit component

The slot loop `0x00470EC0` (`unit-composite.md` §5) gives each drawn
component `c` (not 14) a mode. Inputs:

- `g` **ghostly**: the unit is a monster whose monster-data flags
  (`+0x16`) have bit 0x40 (`0x004AE340`; passed by the unit draw entry
  `0x004DC7B0` as the unit draw's flag a; 0 for every other unit type).
- `r` **unit override** (`0x004DB360`, first match wins; draw type by
  `0x0046D9F0`):
  1. player: stat 181 `fade` ≠ 0 → mode 1;
     monster: `fade` in 1…15 → mode 1; `fade` > 15 → mode 1 if some
     item in the monster's inventory list has item flag 0x400000
     (ethereal), otherwise **no override and stop**;
  2. the component's item (the request's item record) has `transparent`
     (items `+300`) ≠ 0 → mode = `transtbl` (`+301`);
  3. player only, the item on component 5 (RH) or 6 (LH) is ethereal →
     mode 1; on component 7 (SH) → mode 2.
- `h` **hovered**: `0x00464370`: the unit is the hover target
  (`0x00467A10`), except an object whose `objects.SubClass` (`+0x167`) has
  bit 0x80.
- `ov`: the COF layer record of `c` has override byte (byte 3) ≠ 0
  (`0x004DB050`); `lv` its new level (byte 4, `0x004DB140`).

Decision (first that applies):

| `ov` | Mode |
|---|---|
| set | 1 if `g`; else `r` if any; else `lv` (the highlight is never applied to such a layer) |
| clear | 1 if `g`; else `r` if any; else 7 if `h` (light byte doubled, clamped to 0x40…0xFF; GDI ignores it, `shading.md` §5); else 5 |

The draw goes to slot `+0x84` (`0x004F6480`) with the component's
colormap (`unit-composite.md` §7), except when perspective is on
(`0x004F51D0`, never in GDI: `composition.md` §1).

Live data (3,512 COFs of `d2char`, `d2data`, `d2exp`): 618 files have at
least one override layer; override layers' levels: 3 (651), 6 (29), 1
(15), 0 (10), 2 (3), 4 (1); `chars\am\cof\amblxbow.cof` has four
malformed records (override bytes 87, 124, 104, 131 with levels 82, 213,
151, 100: modes outside 0–7 draw as mode 5). Items: only `misc` Torch has
`transparent` = 1 (`transtbl` 3).

### 4. Single-cel units and overlays

| Draw | Mode | Colormap | Address |
|---|---|---|---|
| missile | `missiles.Trans` (`+0x18D`) 1 → 3, 2 → 4, else 5; 7 if it is the hover target (`0x00467A10`, plain equality) | blood map for `LocalBlood` (`shading.md` §6 r3), else the unit map | `0x00472130`–`0x00472196` |
| item | 5; 7 if hover target | item color (`shading.md` §6 r4), else unit map | `0x004721A9` |
| overlay | `overlay.Trans` (`+0x7C`) | **none**: the blood map is fetched for `LocalBlood` (`+0x81`) and dropped | `0x0046E502`–`0x0046E539` |

A hovered missile or item also has its light byte doubled (same clamp as
§3). Live `missiles.Trans`: 1 in 456 rows, 2 in 4, empty in 224;
`overlay.Trans`: 3 in 284, 5 in 5, 0 in 3, 8 in 1.

### 5. Shadows (the darkening blend)

**Unit shadows** (shadow pass kind 2, `draw-order.md` §6 r3; `0x00471620`):
every COF layer of the unit whose shadow byte (byte 1, `0x004DB090`) is
non-zero is drawn through slot `+0x90` (`0x004F6540`; GDI `0x006C87E0` →
`0x00601730` → `0x00608D60`) at the unit's shadow position (Open
question 1):

1. Pixel: for every opaque source pixel, `d' = A0[256·d + 0]` when
   Blended Shadows is on, else `d' = 0`. The source index and all maps
   are ignored.
2. Shape, for a cel `(w, h, xoff, yoff)` drawn at (X, Y): `y0 = Y +
   trunc(yoff / 2)`, `x0 = X + xoff + trunc(yoff / 2)` (truncation toward
   zero). Drawn row `k` = 0, 1, … is source row `2k` counted from the
   bottom, placed on screen row `y0 − k` starting at column `x0 − k`
   (each row one pixel up and one pixel left of the one below). At most
   `⌊h / 2⌋` rows; rows below `H − 1` are skipped (with their sheared
   column offset), drawing stops at row 0, columns are clipped to the cel
   clip `[L, R)`.

Blended Shadows is the settings word `[0x0072DA5C]` (settings struct
`0x0072DA48` `+0x14`), default 1, set by `0x004F5200` from the registry
value `Blended Shadows` under `Diablo II` (`0x0047D040`), read by GDI
through `0x006C8A50`.

**Shadow tiles** (shadow pass kind 1, `0x004F6980` → slot `+0xA4`, GDI
`0x006C9290`): blended → helper `0x004F82D0` with alpha 0xC0, i.e.
`d' = A0[256·d + s]`; not blended → helper `0x004F7EA0`, an opaque copy
`d' = s`. The draw-mode argument (4) the caller passes is not read by
GDI; the last argument limits the tile to the left (1) or right (2) half
of the view.

### 6. Translucent walls and roofs

Walls and roofs with alpha byte `a` < 0xFF (`draw-order.md` §6 r1, §8)
are drawn through slot `+0xA0` (GDI `0x006C93A0` → helper `0x004F84F0`):

| `a` | `T` |
|---|---|
| 0xC0…0xFE | `A0` |
| 0x80…0xBF | `A1` |
| 0x40…0x7F | `A2` |
| < 0x40 | block not drawn |

Pixel: `d' = T[256·L[s] + d]` (row = source, §2), with `L` the gradient
light map of `shading.md` §4 r4 always (this helper has no flat or unlit
branch). Measured on act 1, `A0` read this way keeps 25 % of the wall and
`A2` 75 %, so a wall fading from 0xFF toward 0x80 shows 100 % → 25 % →
50 % (Edge case 1).

### 7. d2rs answers

| Hook | Answer |
|---|---|
| `BlendOp::IndexTable(base)` | `d' = map[base + d][s']`: modes 0–4 and 6, unit shadows, shadow tiles |
| translucent walls | the transposed read `d' = map[base + s'][d]` (`render-pipeline.md` §A5 `IndexTableSrcRow`) |
| mode 5, 7, other | `Opaque` (mode 7 with `H` in the chain, `shading.md` §10) |
| unit shadow | chain `[Z]` (`Z[i] = 0` for all `i`) + `IndexTable(A0)` blended, + `Opaque` not blended |
| `ComponentResolver::blend` | §3 decision |
| COF override fields | §3 `ov`, `lv` |

## Constants & data dependencies

Block offsets of `shading.md` §1; getter tables `0x006C8250`
(`0x0074C5A0`); stat 181 `fade`; item flag 0x400000 (ethereal); items
`transparent` `+300`, `transtbl` `+301`; `missiles.Trans` `+0x18D`,
`LocalBlood` `+0x1A2`; `overlay.Trans` `+0x7C`, `LocalBlood` `+0x81`;
monster flag 0x40; `objects.SubClass` `+0x167` bit 0x80; Blended Shadows
`[0x0072DA5C]` (default 1); wall alpha thresholds 0x40 / 0x80 / 0xC0.

## Randomness

None.

## Edge cases & original bugs

1. Translucent walls read their table transposed (§2, §6): reproduce,
   including the non-monotonic fade.
2. A monster with `fade` > 15 and no ethereal item gets no override at
   all: an item `transparent` value is then ignored (§3 r1).
3. Ethereal shields (SH) draw in mode 2, ethereal weapons in mode 1 (§3).
4. Override layers are never highlighted (§3).
5. Overlays lose their blood map (§4).
6. Draw modes outside 0–7 from data draw as mode 5 (§1).

## Test vectors

Real values: act 1 `Pal.PL2` (`shading.md` Test vectors, SHA-256
`de848a8d…`); palette entries 31 = (196, 196, 196), 100 = (204, 152, 80),
172 = (4, 4, 4), 200 = (92, 92, 92), 255 = (255, 255, 255).

| Input | Expected output | Source |
|---|---|---|
| cel mode 2, no `P`, no `L`, `d = 172`, `s = 255` | `A0[256·172 + 255]` (file byte `0xE1FF`) = **31** (196, 196, 196); the transposed read would give 190 | §1, §2, live |
| cel mode 2, `d = 100`, `s = 200` | `A0[256·100 + 200]` = 207 (transposed: 94) | §2, live |
| cel mode 0, `d = 172`, `s = 255` | `A2[…]` (file `0x2E1FF`) = 190 | §1, live |
| cel mode 1, `d = 172`, `s = 255` | `A1[…]` (file `0x1E1FF`) = 29 | §1, live |
| cel mode 3, `d = 100`, `s = 200` | `ADD` = 170 (symmetric) | §1, live |
| cel mode 4, `d = 172`, `s = 255` | `MUL` = 172 | §1, live |
| cel mode 6, `d = 31`, `s = 0` | `MAX[256·31 + 0]` = 31 (transposed: 30) | §2, live |
| cel mode 5 or 7 or 8, `d = 9`, `s = 100` | `d' = s'` | §1 |
| unit shadow pixel, blended, `d = 31` | `A0[256·31 + 0]` = 183 | §5, live |
| unit shadow pixel, not blended, any `d` | 0 | §5 |
| shadow tile, blended, `d = 100`, `s = 200` | 207 | §5, live |
| translucent wall `a = 0xC0`, `L[s] = 255`, `d = 172` | `A0[256·255 + 172]` = 190 | §6, live |
| translucent wall `a = 0x3F` | nothing drawn | §6 |
| COF layer override 1 level 3, not ghostly, no `r`, hovered | mode 3 | §3 |
| no override, ghostly monster, hovered | mode 1 | §3 |
| no override, player, RH ethereal | mode 1; SH ethereal → 2 | §3 |
| missile Trans 2, not hovered | mode 4 | §4 |
| cel `h = 10`, `yoff = −7`, at (100, 50), shadow | `y0 = 47`, `x0 = 100 + xoff − 3`, 5 rows on screen rows 47…43 from source rows 0, 2, 4, 6, 8, row `k` starting at `x0 − k` | §5 |
| capture run 2 (`20261006-141725`), frames 566 → 590, inventory open, static camera, Town Portal opening (objects `TP`, `TPONHTH.COF`: layer HD override level 3, layer TR none) | 8,641 of the 9,445 pixels that were stable before and changed after hold a value of row `d` of `ADD` (the rest: the opaque TR layer, the portal's light, rain) | §1 mode 3, capture |

## Provenance

1.14d `Game.exe`: blend getters `0x006C8250` / `0x00511D70`; GDI driver
table `0x0074C4A8` (slots `+0x7C`, `+0x84`, `+0x88`, `+0x90`, `+0x9C`,
`+0xA0`, `+0xA4`); cel drawers `0x00607970`, `0x00607B90`, `0x00606E40`,
`0x00607060`, dispatcher `0x00608540`; shadow `0x006C87E0`, `0x00601730`,
`0x00608D60`, `0x00471620`, `0x004DB090`, `0x004F6540`; tile helpers
(table `0x0072DA60`) `0x004F7EA0`, `0x004F82D0`, `0x004F84F0`, GDI
`0x006C9290`, `0x006C93A0`; settings `0x0072DA48`, `0x004F5200`,
`0x0047D040`, `0x006C8A50`; composite `0x00470EC0` (`0x0047123B`–
`0x004713EB`), `0x004DB050`, `0x004DB140`, `0x004DB360`, `0x00464370`,
`0x004AE340`, `0x004AC7E0`, `0x004DC7B0`; single cels `0x00471EC0`;
overlays `0x0046E300`. Field offsets from `specs/data/fields.tsv`. Live
measurements: scratch scripts over the act PL2 files, the COFs (`mpq-tool
extract "*.cof"`), `patch_d2` excel tables, and run 2 of the frame
captures (gitignored; PNG `PLTE` equal to the act 1 PL2 palette). D2MOO
draw-mode names were a hint only.

## Open questions

1. Unit shadow position (`0x00471620`: unit screen position, `−2` in x,
   motion offsets, objects `Xoffset` / `Yoffset`, perspective branch):
   Ghidra read; a capture of the player's shadow on a flat floor.
2. Capture check of the orientation on an asymmetric table: a blended
   unit shadow or a ghostly / Fade / ethereal unit over a known
   background with a static camera (the Town Portal is additive and
   cannot decide it); and of §6 with a wall fading as the player walks
   behind it.
3. What the Blended Shadows registry value was on the recording machine
   (default 1): read `[0x0072DA5C]` in the recorder.
4. Monster inventory test of §3 r1 (`0x0063B2C0`, `0x0063DFD0`,
   `0x0063DFA0`): which list it walks. Ghidra read.
5. Cross-spec: `draw-order.md` §6 r3 says shadow tiles use "draw mode 4";
   GDI ignores that argument (§5). `unit-composite.md` §7 r1 says
   "shift table row +0x6C"; the row is `+0x6C − 1` (`shading.md` §6 r1).
   Both on branch `claude/spec-unit-draw`, edit when merged.
