# Spec: Render — Composition domain, framebuffer and present (1.14d reference renderer)

- **Status:** draft (2026-10-06, RE on 1.14d `Game.exe`; no capture yet).
  Every rule names its 1.14d address; unverified until the capture cases
  of `capture.md` run.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::scene` (`cpu`, `item::BlendOp`),
  `d2-client::render` (GPU compositor, present)
- **Related specs:** `client/render-pipeline.md` §A4/§A5/§A8/§A9 (d2rs
  design this answers), `render/capture.md` (reading frames from 1.14d),
  `render/sprite-placement.md`, `render/camera.md`,
  `render/shading.md` / `render/blend-modes.md` (meaning of each table; to
  write), `formats/palette.md` (`.dat` / `.pl2` layouts)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 37–48 |
| Inputs | 49–57 |
| Outputs / state changes | 58–62 |
| Rules | 63–64 |
|   1. Renderers in 1.14d and the reference | 65–104 |
|   2. Framebuffer | 105–113 |
|   3. Frame cycle | 114–143 |
|   4. Palette (one per presented frame) | 144–161 |
|   5. One pixel write (index domain) | 162–191 |
|   6. d2rs answers | 192–205 |
|   7. DirectDraw (display type 3) differences | 206–217 |
| Constants & data dependencies | 218–223 |
| Randomness | 224–227 |
| Edge cases & original bugs | 228–237 |
| Test vectors | 238–249 |
| Provenance | 250–265 |
| Open questions | 266–282 |
<!-- /index -->

## Summary

1.14d draws every frame in software into an 8-bit framebuffer of palette
indices; translucency and lighting are 256-entry and 256×256 index tables;
the finished index frame is turned into colors only when presented,
through one 256-color palette. The reference is the GDI (windowed)
driver, which shares that rasterizer with the default DirectDraw driver.
This spec owns: which renderer is the reference and why, the framebuffer,
the frame cycle (clear, draw, present), the palette used to present, and
the domain mechanics of one pixel write. What each table means and which
draw uses which are `shading.md` and `blend-modes.md`.

## Inputs

| Name | Type | Source |
|---|---|---|
| draws of one frame | cel/tile draws in draw order | `draw-order.md`, `sprite-placement.md` |
| index tables | light maps, blend tables, remaps | act `pal.pl2` (§4) |
| palette | 256 × (R, G, B) | act `pal.pl2` (§4) |
| level flag | `Levels.txt` BlankScreen of the player's level | data tables |

## Outputs / state changes

The presented frame: W × H palette indices plus the palette, i.e. an RGB
image. The index framebuffer persists between frames (§3).

## Rules

### 1. Renderers in 1.14d and the reference

The driver table `0x0072DA80` (indexed by display type) holds four
drivers: 1 GDI (`0x0074C4A8`), 3 DirectDraw (`0x0072F6D0`), 4 Glide
(`0x0072F178`), 6 Direct3D (`0x0074BF28`); types 2, 5, 7 are empty. Each
is the 54-slot interface of D2MOO's `D2GraphicsInterfaceStrc`; the active
one is `[0x007C8CC0]`, the type `[0x007C8CB0]`. Selection (`0x00405C30`,
command-line table at `0x0070509C`, 0x5C-byte records):

| Command line | Config byte | Display type |
|---|---|---|
| `-3dfx` | `+0x0A` | 4 Glide |
| `-w`, `-window`, `-windowed` | `+0x08` | 1 GDI |
| `-d3d` | `+0x0D` | 6 Direct3D |
| none of these | — | 3 DirectDraw (default) |

`-opengl` sets `+0x0B` but no OpenGL driver exists. Perspective (sprite
scaling) is enabled only for types above 3 (`0x00405C30`; `0x004F6670`
requires type 4–7).

GDI and DirectDraw draw cels and tiles with the same software code
(`0x006014C0`, `0x00608540`, DT1 helpers `0x0072DA60`) into an 8-bit index
buffer. Glide and Direct3D draw GPU textures in RGB with driver-dependent
results and perspective; they cannot be reproduced exactly and are not a
reference.

**The reference is the GDI driver** (display type 1, `Game.exe -w`):

1. its index framebuffer is produced by the same rasterizer as the default
   DirectDraw driver, so drawn pixels are identical;
2. it is the configuration the trace recorder already runs (`-w -ns`), and
   its framebuffer is plain process memory, readable at any time
   (`capture.md`);
3. it presents the palette unmodified (DirectDraw applies a gamma ramp, §7).

The DirectDraw differences are listed in §7. Answers
`render-pipeline.md` §A5: the framebuffer domain is **indexed** (u8 per
pixel, palette at present); the `Rgb` op is not needed. It also answers
§A3/OQ3: the reference never scales a sprite.

### 2. Framebuffer

GDI (`0x006C7C10` → `0x006C7B00`): an 8-bit DIB section with
`biHeight = −H` (top row first), W × H by resolution mode (0: 640 × 480,
3: 1344 × 700, otherwise 800 × 600; d2rs uses 800 × 600, `camera.md` §1),
row stride W, pixels at `[0x007C9154]`, W in `[0x007C9138]`, H in
`[0x007C913C]`. Cel column clip `[0, W)` (`0x006C82D0`). Pixel `(x, y)`
is byte `y × W + x`.

### 3. Frame cycle

The in-game frame (`0x0044C990`), once per client tick (`camera.md` §9):

1. Camera origins and shake (`camera.md` §3, §8).
2. `StartDraw(bClear, 0, 0, 0)` (`0x004F60F0` → slot `+0x18`) with `bClear`
   = BlankScreen of the player's current level (`Levels` record
   `+0x218` via `0x0061DB70`). GDI (`0x006C7D80`): if `bClear`, rows
   `0 … H − 48` are set to index 0 (`0x006C9220` with partial = 1: the first
   `(H − 47) × W` bytes); rows `H − 47 … H − 1` are not cleared. If
   `bClear = 0` nothing is cleared. All 137 rows of the live
   `patch_d2` `levels.txt` have BlankScreen = 1.
3. World (`0x00476BC0`, skipped in screen open mode 3), then UI, cursor
   and overlays (`0x00456EE0`, `0x004F98E0`, `0x00468820`, `0x004684C0`,
   `0x00477980`); order is `draw-order.md`.
4. If the counter `[0x0070F2C0]` is above 0, `ClearScreen(0)` (slot
   `+0xC4`, GDI `0x006C9270` → `0x006C9220(0)`: all W × H bytes set to 0)
   runs **after** all drawing, and the counter is decremented: that frame
   presents all index 0. `0x0044E100` sets the counter to 1.
5. `EndScene` (`0x004F6190`): frame pacing (if less than 5 ms passed since
   the last call, `Sleep` up to 5 ms; no pixel effect), `EndDraw` (slot
   `+0x1C`, GDI `0x006C7DF0`: leave the draw lock), then `Blit` (slot
   `+0x20` = **present**; GDI `0x006C7E30`: `SetDIBColorTable` with all 256
   entries of `0x00989C40`, `StretchBlt` of the W × H DIB to the window's
   client rectangle).

The presented frame is exactly the framebuffer at `EndScene` entry.
Pixels not written in a frame keep the previous frame's index (the
uncleared bottom 47 rows, or everything when `bClear = 0`).

### 4. Palette (one per presented frame)

The framebuffer holds indices, so a presented frame has exactly one
palette; there are no per-region palettes (answers `render-pipeline.md`
§A4 "palettes per screen region": one). The palette comes from the act's
`pal.pl2`: the D2Win loader `0x004FB1E0` (paths from callers such as
`0x004FB500`, default `palette\act1`) copies the **first 1,024 bytes of
`pal.pl2`** (256 × 4 bytes) to `0x0081E668`, sets each entry's 4th byte to
5, builds the index tables (§5) and calls `SetPalette` (`0x004F63D0` →
slot `+0x70`). The GDI `SetPalette` (`0x006C7FA0`) reads the entries as
`PALETTEENTRY` (byte 0 red, 1 green, 2 blue) into the color table
`0x00989C40` (blue, green, red, 0) and into `0x007C8CC8`. The `.dat` file
named next to it is loaded by `0x004FB1E0` but its colors are not used.

So index `i` presents as `(pl2[4i], pl2[4i + 1], pl2[4i + 2])`, index 0
included (`formats/palette.md` OQ1: the base palette's order is R, G, B and
it is the palette used).

### 5. One pixel write (index domain)

The row drawer `0x00608540` writes each non-transparent source index `s`
(`sprite-placement.md` §6) onto the destination index `d` with up to three
tables, passed by the driver's cel draw (`0x00511FB0` / `0x006C84B0`):

| Table | Size | Chosen by (owner) |
|---|---|---|
| `P` remap | 256 | the draw call's palette argument (`unit-composite.md`, `shading.md`) |
| `L` light | 256 | light byte `v`: none if `v = 0xFF`, else the table at pointer `+0x0C + 4 × (v >> 3)` of the palette-table block (`SetPaletteTables`, `0x005102D0`; filled by `0x004FB010`); draw mode 7 uses pointer `+0x118` instead (`shading.md`) |
| `T` blend | 256 × 256 | draw mode via `0x00511D70` (GDI `0x006C8250`); none for mode 5 (`blend-modes.md`) |

Read in `0x00608540` (each table may be absent; an absent step is skipped):

- no `T`: `d' = L[P[s]]`
- `T`, no `L`: `d' = T[256 × d + P[s]]` — **row = destination, column =
  source**.

The `L`-and-`T` variants are separate routines (`0x00606E40`,
`0x00607060`, `0x00607970`, `0x00607B90`; Open question 2).

The tables are copied unchanged from `pal.pl2` by `0x004FB1E0` (no
transposition): blend tables from file offset `0x3500` (3 × 65,536 bytes),
light/variation maps from `0x400`, further 65,536-byte tables from
`0x33500`, `0x43500`, `0x5B500`. Hence the PL2 alpha tables are laid out
`[level][destination][source]`. This contradicts `formats/palette.md`
("[level][source index], gives a map over the destination index") and
`render-pipeline.md` §A5 `IndexTable` (`map[base + src][dest]`): both must
read row = destination (hook `scene/item.rs` `BlendOp::IndexTable`).

### 6. d2rs answers

- `scene/cpu.rs` "what the original clears the frame to": the framebuffer
  persists between frames; at the start of each frame, when the level's
  BlankScreen is set, rows `0 … H − 48` become index 0 and the bottom 47
  rows keep their content; §3 step 4 clears everything after drawing. A
  single-frame verify case starts from an all-0 buffer unless it records
  the previous frame.
- `scene/item.rs` domain: indexed, `Rgb` stays out of `BlendOp`.
- `IndexTable`: row = destination (§5).
- RGBA for verify and present: `(R, G, B, 255)` of §4 for every index,
  0 included. `map::cpu::to_rgba` (0 → black) equals this exactly when
  each act's PL2 entry 0 is black (Open question 1).

### 7. DirectDraw (display type 3) differences

Same rasterizer and same index tables; differences, all outside the GDI
reference:

| Step | DirectDraw | Address |
|---|---|---|
| surface | 8-bit fullscreen display mode W × H × 8; render target an 8-bit system-memory offscreen surface `0x00881280`, locked in `StartDraw` (pointer `[0x007C9154]`, pitch `[0x007C9138]`) | `0x00511210`, `0x00511720` |
| clear | every `StartDraw` fills the whole surface with 0 (`Blt` color fill; on failure `memset`), whatever `bClear` | `0x00511720` → `0x00512ED0` |
| palette | `SetPalette` passes the entries through a gamma ramp (`0x006BE190`, setting `0x0072F7C8`, default 100, slot `SetOption` 11) into `0x007C8CC8`; `Blit` uploads entries 1–255 only, entry 0 stays black from creation | `0x00510160`, `0x00511870`, `0x005115E0` |
| present | `BltFast` render surface → back buffer, `Flip` (or `BltFast` to the primary when flipping is unavailable) | `0x00511870` |

## Constants & data dependencies

W × H (`camera.md` §1); uncleared bottom band 47 rows; palette = first
1,024 bytes of the act `pal.pl2`; table offsets in §5; Levels BlankScreen
(record `+0x218`).

## Randomness

None.

## Edge cases & original bugs

- The bottom 47 rows are never cleared in GDI; any pixel there not drawn
  by the control panel shows an older frame. Reproduce (persistent
  framebuffer).
- The `[0x0070F2C0]` clear happens after drawing, so that frame is black
  (index 0) although everything was drawn.
- Frame pacing (§3 step 5) uses wall-clock time; it changes only timing,
  never pixels.

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| framebuffer all 5, BlankScreen 1, nothing drawn, H = 600, W = 800 | rows 0–552 = 0, rows 553–599 = 5 | §3 |
| same, BlankScreen 0 | all 5 | §3 |
| same, counter `[0x0070F2C0]` = 1 | all 0; counter becomes 0 | §3 |
| `s = 7`, `P[7] = 9`, `L[9] = 3`, no `T` | `d' = 3` | §5 |
| `s = 7`, `P[7] = 9`, `T`, `d = 200` | `d' = T[256 × 200 + 9]` | §5 |
| palette bytes `pl2[0..8] = 01 02 03 xx 10 20 30 xx` | index 0 → (1, 2, 3), index 1 → (0x10, 0x20, 0x30) | §4 |
| capture case `composition-0001`: a translucent sprite (e.g. a town portal) on a known floor, frame captured | CPU reference with §5 equals the captured index frame | capture, queued |

## Provenance

1.14d `Game.exe`: driver table `0x0072DA80` and the four driver tables
(slot layout confirmed against D2MOO `D2GraphicsInterfaceStrc`, 1.10f:
54 slots; `0x004F6480` calls `+0x84`, `0x004F6300` `+0xB8`,
`0x004F6190` `+0x1C` and `+0x20`), selection `0x00405C30` and the
command-line records at `0x0070509C`, GDI driver `0x006C7B00`,
`0x006C7C10`, `0x006C7D80`, `0x006C7DF0`, `0x006C7E30`, `0x006C7FA0`,
`0x006C9220`, DirectDraw driver `0x00510160`, `0x00511210`, `0x00511720`,
`0x00511870`, `0x00512ED0`, frame `0x0044C990`, `0x004F6190`, palette
loader `0x004FB1E0`, `0x004FB010`, row drawer `0x00608540`, blend getter
`0x00511D70`. Display-type names from `refs/1.14d-notes` (`VideoMode`) and
D2MOO `DisplayType.h`. Levels BlankScreen counted in
`game/extracted/patch_d2/data/global/excel/levels.txt` (137 × 1). No
capture yet.

## Open questions

1. For every act palette: does the PL2 base palette (R, G, B at bytes
   `4i…4i + 2`) equal the `.dat` palette (B, G, R at `3i…`), and is entry 0
   (0, 0, 0)? Game-file check (`mpq-tool` extension); decides whether
   d2rs may keep reading `.dat`.
2. Write order when both `L` and `T` are present (`0x00606E40`,
   `0x00607060`, `0x00607970`, `0x00607B90`): `T[256 × d + L[P[s]]]` is
   expected; a Ghidra read settles it. Owner of the rule stays here, the
   table choice in `blend-modes.md`.
3. Out-of-game screens (menus, loading screens, cut-scenes) use other
   callers of `StartDraw` (`0x0044CB60`, `0x0044D100`, `0x0044E770`,
   `0x004565E0`, `0x00460190`, `0x004F98E0`): their clear arguments, for
   `ui/` capture cases.
4. What sets the post-draw clear counter (`0x0044E100`, writes 1 at
   `0x0044E1D2`): which event, for scene captures around it.
