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
|   3. Frame cycle | 114–151 |
|   4. Palette (one per presented frame) | 152–199 |
|   5. One pixel write (index domain) | 200–238 |
|   6. d2rs answers | 239–257 |
|   7. DirectDraw (display type 3) differences | 258–269 |
| Constants & data dependencies | 270–275 |
| Randomness | 276–279 |
| Edge cases & original bugs | 280–289 |
| Test vectors | 290–301 |
| Provenance | 302–328 |
| Open questions | 329–362 |
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
   `patch_d2` `levels.txt` have BlankScreen = 1. "The player's current
   level" is the level of the local player unit's current room
   (`0x0044CA8F`–`0x0044CAC8`: `0x004646A0` → `0x0061A1B0` level id →
   `0x0061DB70` record; the chain `capture.md` §3 records as "level id").
   No room (player not yet placed) → `bClear` = 0, nothing cleared; a
   level id without a Levels record is fatal (error 0x5DE).
3. World (`0x00476BC0`, skipped in screen open mode 3), then UI, cursor
   and overlays (`0x00456EE0`, `0x004F98E0`, `0x00468820`, `0x004684C0`,
   `0x00477980`); order is `draw-order.md`.
4. If the counter `[0x0070F2C0]` is above 0, `ClearScreen(0)` (slot
   `+0xC4`, GDI `0x006C9270` → `0x006C9220(0)`: all W × H bytes set to 0)
   runs **after** all drawing, and the counter is decremented: that frame
   presents all index 0. `0x0044E100` sets the counter to 1 when it
   replaces the client act; its only caller is the S→C 0x03 handler
   `0x0045C8E0` (entry 3 of the handler table `0x007114D0`, size 12): the
   first in-game frame after each act load is black.
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

**Which act's palette** (frame-cycle FC1). `0x004FB480(a)` loads
`DATA\GLOBAL\palette\act<n>\pal.pl2` (and `pal.dat`) with `n = a + 1`,
`n` outside 1…5 → 1, then `SetPalette`. In game it is called by the
client loop at game start with `a = 0` (`0x0044F2DC`: act 1) and by the
client unit room change `0x004654C0` when the moved unit is the local
player (`[0x007A6A70]`) and the Levels `Pal` byte (`+0x02`; not `Act`,
+0x03, which differs for levels 125–127 and 133–136) of the new room's
level differs from the old room's (`0x00465603`–`0x0046562A`), with `a`
= the new level's `Pal` (`client/msg-units.md` §3 rule 4.4); the first placement (no old room) does
not switch. So while playing, the presented palette is that of the act of
the local player's current room's level, switched on the room change that
crosses acts. A game that starts in another act gets its act palette
from the act load: S→C 0x03 (`0x0044E100`, `client/model.md`) stores the
act byte in `[0x007A288C]` (`0x00454790`) and draws the loading screen
(`0x004565E0` → `0x00456550`, which loads the loading-screen cel into
`[0x007A2888]`). The next client frame `0x0044C990` first calls
`0x004547B0`: when a loading-screen cel is held it is freed and
`0x004FB480([0x007A288C])` loads that act's palette, then the act
set-ups run. So the first in-game frame is drawn with the loaded act's
palette, and the later first room placement (no old room) needs no
switch. The other two callers are not game start: `0x0044D100` is the
out-of-game "betascreens" slideshow (`DATA\GLOBAL\ui\betascreens\screen01`
… `screen10`, 13-byte entries at `0x0070F238`, count `[0x0070F024]` = 10,
each shown 10,000 ms with its entry's act byte, 0 = act 1 for all ten);
`0x00482EF0` plays a video (`%s\video\%s`, 640 × 292 or 640 × 146) and
then loads act 5's palette (`a` = 4 at `0x00483283`); the state loop
`0x0044F360` calls it with video 5 when `[0x007A0604]` ≠ 0 (set by
`0x0044EC80`) and with video 7 when `[0x007A0628]` ≠ 0, and S→C 0x61
(`0x0045E660` → `0x004B9320`) calls it with the video id u8@1.

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
- `T` and `L`: `d' = T[256 × d + L[s]]` — **`P` is not applied**, whether
  or not the draw passed one.

The table-carrying runs go to one routine per case (no clip / column
clip): `L` and `T` → `0x00606E40` / `0x00607060`, `P` and `T` →
`0x00607970` / `0x00607B90`, `L` and `P` → `0x006072F0` / `0x00607480`;
each `T` routine applies a single 256-byte map before `T`, and the
dispatcher picks the `L` routine whenever `L` is present. The inline
path (`[0x008F03C8] ≠ 0`) computes the same values. Translucency with a
remap therefore loses the remap when the draw is lit; `blend-modes.md`
owns which draws pass which tables.

The tables are copied unchanged from `pal.pl2` by `0x004FB1E0` (no
transposition): blend tables from file offset `0x3500` (3 × 65,536 bytes),
light/variation maps from `0x400`, further 65,536-byte tables from
`0x33500`, `0x43500`, `0x5B500`. So for the cel drawers the PL2 blend
tables read `[level][destination][source]` (`render-pipeline.md` §A5
`IndexTable`, hook `scene/item.rs` `BlendOp::IndexTable`). Which of the
two indices is the destination is a property of each drawer, not of the
file: the lit translucent wall drawer reads the transpose
(`blend-modes.md` §2, owner of the orientation).

### 6. d2rs answers

- `scene/cpu.rs` "what the original clears the frame to": the framebuffer
  persists between frames; at the start of each frame, when the level's
  BlankScreen is set, rows `0 … H − 48` become index 0 and the bottom 47
  rows keep their content; §3 step 4 clears everything after drawing. A
  single-frame verify case starts from an all-0 buffer unless it records
  the previous frame.
- `scene/item.rs` domain: indexed, `Rgb` stays out of `BlendOp`.
- `scene/item.rs` table chain (`triage-game-findings` Q12): when a draw
  has both a blend table `T` and a light map `L`, its remap `P` is
  dropped (`d' = T[256·d + L[s]]`, §5); the chain `P` → `L` → `T` is only
  for draws without one of them (`L[P[s]]` without `T`, `T[256·d +
  P[s]]` without `L`).
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

1. The bottom 47 rows are never cleared in GDI; any pixel there not drawn
  by the control panel shows an older frame. Reproduce (persistent
  framebuffer).
2. The `[0x0070F2C0]` clear happens after drawing, so that frame is black
  (index 0) although everything was drawn.
3. Frame pacing (§3 step 5) uses wall-clock time; it changes only timing,
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
loader `0x004FB1E0`, `0x004FB010`, row drawer `0x00608540` (dispatch of
the table cases to `0x00606E40`, `0x00607060`, `0x006072F0`,
`0x00607480`, `0x00607970`, `0x00607B90`), blend getter `0x00511D70`,
act load `0x0045C8E0` → `0x0044E100`. Display-type names from `refs/1.14d-notes` (`VideoMode`) and
D2MOO `DisplayType.h`. Levels BlankScreen counted in
`game/extracted/patch_d2/data/global/excel/levels.txt` (137 × 1). No
capture yet. Frame-cycle follow-ups (FC1, FC2): BlankScreen level chain
read at `0x0044CA8F`–`0x0044CAD5`; act palette loader `0x004FB480`
(format `%s\palette\act%d\%s`), callers `0x0044F2DC`, `0x0046562A`
(room change in `0x004654C0`: old room `0x00620BB0`, new room
`0x00465420`), `0x0044D1A5`, `0x00483293`.
Ghidra backlog (2026-10-06): act palette at game start from
`0x0044E100` → `0x00454790`, `0x004565E0`/`0x00456550`, frame
`0x0044C990` → `0x004547B0`; the other `0x004FB480` callers
`0x0044D100` (betascreens table `0x0070F238` read from the file),
`0x00482EF0`, `0x004F8FE0`.

## Open questions

1. For every act palette: does the PL2 base palette (R, G, B at bytes
   `4i…4i + 2`) equal the `.dat` palette (B, G, R at `3i…`), and is entry 0
   (0, 0, 0)? Game-file check (`mpq-tool` extension); decides whether
   d2rs may keep reading `.dat`. Each capture also holds the presented
   palette (PNG `PLTE`, `capture.md` §5): comparing it with the act's
   `pal.pl2` first 1,024 bytes and its `.dat` settles §4 on live frames.
2. ~~Write order when both `L` and `T` are present~~: answered in §5
   (`T[256 × d + L[s]]`, `P` dropped; dispatcher `0x00608540`). The
   identification is now read from the caller too: the GDI cel draw
   `0x006C84B0` pushes, as the last three arguments of `0x006014C0`
   (stdcall, `[ebp+0x28]`, `+0x2C`, `+0x30`, passed through unchanged to
   the row drawer table `0x006E3688` → `0x00608540`), the light table
   chosen by the light byte (`0x006C8545`, or `+0x118` for mode 7), the
   blend table of `0x006C8250`, and the draw call's palette argument, in
   that order; `0x00608540` uses the first as the outer table of `L[P[s]]`
   and the one kept with `T`. A capture of a lit, remapped, translucent
   draw remains a pixel check, not an open rule.
3. Out-of-game screens (menus, loading screens, cut-scenes) use other
   callers of `StartDraw` (`0x0044CB60`, `0x0044D100`, `0x0044E770`,
   `0x004565E0`, `0x00460190`, `0x004F98E0`): their clear arguments, for
   `ui/` capture cases.
4. ~~What sets the post-draw clear counter~~: the act load (S→C 0x03,
   §3 step 4).
5. ~~Act palette at game start outside act 1~~: answered in §4 (the act
   load's `[0x007A288C]`, applied by `0x004547B0` at the first frame;
   `0x0044D100` and `0x00482EF0` are not game start). Capture
   confirmation: the first frames after loading a character saved in
   act 2.
6. ~~d2rs input for §3 step 2 and §4~~: answered in `client/model.md`
   §11 (act from S→C 0x03, level from the local player's room placed by
   0x15; no message carries the level) and §12 (room of a point).
