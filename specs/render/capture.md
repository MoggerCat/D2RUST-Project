# Spec: Render — Frame capture from 1.14d (design of link 1)

- **Status:** draft (2026-10-06). Recorder `tools/trace-recorder/record_frames.py`
  0.3.0 writes `frames-raw-3` (§5; first run 2026-10-08 under Wine, q-cloud-game:
  134 of 134 tile draws named their DT1 file and index, 50 of 53 cel draws
  their cel header, every checked DC6 header equal to the file's frame
  header); `--selftest` passes (PNG round trip,
  hash sensitivity, 13 state fields each follow exactly their source bytes,
  stability counts only repeated keys and reports exactly the changed key).
  Two `frames-raw-1` runs (2026-10-06, 15,934 in-game frames) confirmed the
  camera fields of §3.1 (every frame equals `camera.md` §1–§3) and found
  the animated sources of §7; `frames-raw-2` has not run yet.
- **Target version:** 1.14d
- **Crate/module:** `tools/trace-recorder/record_frames.py` (capture),
  `d2-client::verify` case kind `scene` (comparison)
- **Related specs:** `render/composition.md` (framebuffer, frame cycle,
  palette), `render/camera.md` (state recorded with each frame),
  `render/unit-composite.md` and `render/draw-order.md` (meaning of the
  raw unit and tile fields of §3.5), `client/render-pipeline.md` §A10
  (link 1), `client/ui.md` §B6 (cursor owner to write), `traces/FORMAT.md`
  §Render captures (committed format), `sim/tick.md` (server tick hook),
  `sim/rng.md` (seed step)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 48–58 |
| Inputs | 59–66 |
| Outputs / state changes | 67–72 |
| Rules | 73–74 |
|   1. Configuration | 75–81 |
|   2. Hooks | 82–98 |
|   2a. Front end (`record_frames.py --front-end`) | 99–121 |
|   3. What is read | 122–317 |
|   4. Tie to ticks | 318–330 |
|   5. Raw format `frames-raw-2` (and `frames-raw-3`) | 331–366 |
|   6. Hashes and the comparison | 367–388 |
|   7. Stability first | 389–426 |
|   8. Capture cases | 427–442 |
| Constants & data dependencies | 443–447 |
| Randomness | 448–454 |
| Edge cases & original bugs | 455–466 |
| Test vectors | 467–477 |
| Provenance | 478–497 |
| Open questions | 498–555 |
<!-- /index -->

## Summary

A capture is the exact index frame 1.14d presented, read from process
memory at the in-game `EndScene` call, with its palette, the server tick it
follows and every input it was drawn from that the original does not
derive from ticks: camera, level, the mouse cursor, the player's client
seed, light quality and weather state, and on selected frames the full
list of draw calls (tiles, units, UI cels, primitives). Images stay local
(Blizzard art); the repo keeps hashes and state. Stability is proven
first: frames drawn from equal recorded state must be identical (§7).

## Inputs

| Name | Type | Source |
|---|---|---|
| reference `Game.exe` | 1.14d, SHA-256 checked | `traces/reference-install.toml` |
| game arguments | `-w -ns` | GDI driver (`composition.md` §1) |
| player actions | per capture case (§8) | the person at the game |

## Outputs / state changes

`traces/raw/<time>-frames.jsonl` (gitignored, format `frames-raw-2`, §5),
`game/captures/<time>/frame-<seq>.png` (gitignored), and, after
selection, committed traces under `traces/render/` (`FORMAT.md`).

## Rules

### 1. Configuration

`Game.exe -w -ns`: display type 1 (GDI, `[0x007C8CB0] = 1`), resolution
800 × 600 (`[0x007C9138] = 800`, `[0x007C913C] = 600`; the in-game
resolution option must be 800 × 600). Frames in any other configuration
are logged as refused, never captured.

### 2. Hooks

| Hook | Address (first bytes) | When | Reads |
|---|---|---|---|
| frame start | in-game draw `0x0044C990` entry (`55 8B EC 83 EC 1C`) | before any draw of the frame | cursor (§3.3), player seed |
| frame end | `EndScene` `0x004F6190` entry (`55 8B EC 83 EC 10`), return address `[ESP] = 0x0044CB4F` (the call at `0x0044CB4A`) | every draw of the frame done, present (`Blit`, slot `+0x20`) not run (`composition.md` §3) | framebuffer, palette, §3.1–§3.4 |
| cel file loaded | `0x00478946` in the loader `0x004788B0` (`8B 45 FC`) | after each cel file load by path | path at `EBP − 0x108`, cel file pointer `[EBP − 4]` (§3.6) |
| unit draw | `0x00471EC0` entry (`55 8B EC 83 EC 70`) | selected frames only | §3.5 |
| draw calls | the D2GFX wrappers of §3.5 (`55 8B EC`) | selected frames only | §3.5 |
| cel rasterizer (raw-3) | `0x006014C0` entry (`55 8B EC 51 8B 4D 08`), stdcall, `[ESP+4]` = cel context | selected frames only | the cel the call draws: context `+0x3C` (§3.5) |
| component path (raw-3) | in `0x005FE610` right after the path `sprintf`: `0x005FE77C` (`.dcc`, `83 4E 44 01`) and `0x005FE7A8` (`.dc6`, `B8 01 00 00 00`) | every component file path built | path at `EBX`, composed name at `[EBP+8]` (§3.6) |

A frame start without a frame end exists: when `0x004F6070` returns
non-zero the in-game draw jumps to `0x0044CB4F` without drawing; the next
frame start replaces the pending state. The memory read at the frame end
equals the presented frame.

### 2a. Front end (`record_frames.py --front-end`)

Out-of-game screens present through the same `EndScene` `0x004F6190`
(`composition.md` §3: every out-of-game `StartDraw` caller), so in this
mode every `EndScene` entry is a frame, whatever its caller; the in-game
frame start `0x0044C990` is not used. A frame is everything drawn between
two `EndScene` calls: a shot arms the draw hooks of §3.5 at one
`EndScene` and captures the next, so a draw before the frame's
`StartDraw` (the loading screen's `ClearScreen`) is in the log.

| Item | Where | Notes |
|---|---|---|
| frame index | `present`: `EndScene` calls since launch | no server tick before a game (`f` null) |
| caller | `ret` = `[ESP]` at `EndScene` entry; `in_game` = (`ret` = `0x0044CB4F`) | measured 2026-10-09 (Wine): menus `0x004F9A67` (after `StartDraw` `0x004F9934` of the D2Win control drawer `0x004F98E0`), loading `0x004567D4` (in `0x004565E0`, `frontend-loading.md` L4) |
| launcher mode | `[0x0074C704]` (4 menu, 1 client: `tools/original-hooks.md` §5.4) | |
| D2Win cel files | `D2Win_LoadCelFile` `0x004FA9B0` (`ui/text.md` §3): path in `ECX` at entry (found per call: the first of `ECX`, `EDX`, `[ESP+4]`, `[ESP+8]` pointing at path text; 112 of 112 loads used `ECX`), cel file = `EAX` at the return address | `celfile` records with `"via": "0x4fa9b0"`; the menus load no cel through `0x004788B0`. Entry bytes (measured) `55 8B EC 81 EC 08 01 00`, checked at start like the other hooks |

Game state (§3.1–§3.4) is read only for in-game frames; a front-end frame
has none, and `facts_render.py` writes those `frame.tsv` keys as `-` and
`tick` as `?` (`tools/facts-render.md` §1 r2). The menus draw animated
fire, snow and cursor on wall-clock time: front-end stability (§7) is not
established.

### 3. What is read

#### 3.1 Frame and camera (frame end)

| Item | Where | Notes |
|---|---|---|
| index frame | `W × H` bytes at `[0x007C9154]` | row-major, top row first, stride W |
| palette | color table `0x00989C40`, 256 × (B, G, R, 0) | stored as R, G, B (768 bytes) |
| tick | frame number `f` of the last server tick hook `0x0052D870` (`record_tick.py`) before this frame | §4 |
| draw counter | `[0x007A0494]` | not unique per frame (§4) |
| client updates | `[0x007A0498]` | +1 per client update `0x0044C790`; gates the weather update |
| player | unit `[0x007A6A70]`: type `+0x00`, mode `+0x10`, frame `+0x44` (8.8 fixed: drawn frame = value >> 8, `0x00621810`); path `+0x2C`: fixed `+0/+4`, client `+8/+0xC` = `(px, py)` of `camera.md` §2 | confirmed on 15,934 frames |
| tile origin, view rect | view `[0x007A0640]` `+0x24/+0x28`, `+0x04..+0x10` | `camera.md` §1, §3 |
| unit origin | `[0x007A520C]`, `[0x007A5208]` | `camera.md` §3 |
| panel | open mode `[0x007A5210]`, shift `[0x007A5214]` | `camera.md` §1 |
| shake | peak `[0x007B9534]`, offsets `[0x007B9538]`, `[0x007B8D20]` | `camera.md` §8: the offsets keep their last value when a frame's amplitude is 0, so only the origins show what was applied |
| post-draw clear counter | `[0x0070F2C0]` | `composition.md` §3 |
| player seed | unit `+0x20/+0x24`, at frame start and frame end | consumed by shake, cursor and weather in one frame (§7) |

#### 3.2 Level and act (frame end)

| Item | Where |
|---|---|
| level id | player path `+0x1C` → room; room `+0x10` → `+0x58` → level; level `+0x1D0` (the chain `0x004646A0` → `0x00620BB0` → `0x0061A1B0` → `0x0066BAB0` the frame uses for BlankScreen, `composition.md` §3). Static units use path `+0x00` for the room |
| act | client act `[0x007A0634]`, byte `+0x14` (written by `0x006194A0` from S→C 0x03 byte 1) |
| environment | act `+0x04` → `+0x0C` intensity (`0x0061C0E0`), `+0x18..+0x1A` R, G, B (`0x0061C0B0`) |

The act palette (`pal.pl2`, `composition.md` §4) and the level's tile files
follow from level id and act through the data tables; the recorded
palette hash checks the first.

#### 3.3 Mouse cursor (frame start)

The cursor is drawn into the framebuffer at the end of the UI pass
(`0x004684C0`, order: `draw-order.md`) and animates on wall-clock time.
Rule owner: `ui/panels-3.md` §23 (move / down / up transitions
`0x00468840`, `0x00467F20`, `0x00467FA0`; item and shop cursors
`0x00468070`, `0x00468010`, `0x00468040`; the dead copy `0x004685C0`;
the idle cursor steps the client player seed on wall-clock time). This
section keeps the recorded fields.

| Global | Meaning |
|---|---|
| `[0x007A6B08]` | cursor drawn (≠ 0) |
| `[0x007A6AB0]`, `[0x007A6AAC]` | mouse x, y in frame pixels |
| `[0x007A6AB4]` | x adjust `adj` (set by the init `0x004680B0` from ECX) |
| `[0x007A6ADC]` | cursor type 0–6 |
| `[0x007A6AE0]` | frame, 8.8 fixed |
| `[0x007A6AF0]` | animation state |
| `[0x007A6AEC]`, `[0x007A6AE8]` | last step time, idle start time (`GetTickCount` ms) |
| `[0x007A6ABC]` | item unit on the cursor (0 = none) |
| `[0x007A6AC0 + 4 × type]` | cel file of each type: `DATA\GLOBAL\UI\CURSOR\<name>` (`0x004680B0`) |

Type table `0x00712010 + 0x1C × type`: `+0x00` animated, `+0x04` loops,
`+0x08` frames, `+0x0C` step (1/256 frame), `+0x14` draw function,
`+0x18` name:

| Type | Name | Animated | Loops | Frames | Step |
|---|---|---|---|---|---|
| 0 | Gaunt | 0 | 0 | 1 | 0 |
| 1 | grasp | 1 | 0 | 8 | 0 |
| 2 | ohand | 1 | 0 | 8 | 64 |
| 3 | orotate | 1 | 1 | 8 | 32 |
| 4 | ppress | 1 | 1 | 8 | 64 |
| 5 | protate | 1 | 1 | 8 | 64 |
| 6 | buysell | 0 | 0 | 1 | 0 (own draw function `0x00468460`) |

Draw (`0x004683C0`, types 0–5, no item on the cursor): `x = clamp(mx +
adj, adj, W − adj − 1)`, `y = clamp(my, 0, H − 1)`; one `CelDraw` of
frame `frame >> 8` (arithmetic) of the type's cel file at (x, y), light
`0xFFFFFFFF` (none), draw mode 5, no palette. With an item unit (type 4)
on the cursor and state < 6 the item's cel is drawn instead (`0x004684C0`;
owner `ui/inventory.md`).

Step (`0x00468310`, right after each cursor draw): with `now =
GetTickCount()`, an animated type steps only when `now > last + 16`
(then `last = now`):

1. State 1: the local player unit's seed (`+0x20`) takes one step of the
   D2 generator (`sim/rng.md`); if bits 0–5 of the new low 32 bits are
   below 16, `frame += 0x20`. Other states except 3: `frame += step`.
2. When `frame ≥ frames × 256`: a looping type subtracts `frames × 256`;
   else state 2 → state 4, type 3, frame 0; state 5 → state 1, type 5,
   frame 0; any other state is a fatal error.
3. State 3 (`0x004682C0`) runs backwards (`frame −= 0x40`); below 0 a
   non-looping type goes to state 1, type 5, frame 0.
4. Then, in state 1, when `now > idle + 5000`: state 2, type 2, frame 0.

The init sets drawn, state 1, type 5, frame 0, idle = now; the mouse
handlers (`0x00467F20`, `0x00467FA0`) set the position and idle = now.
So an untouched mouse shows `protate` advancing by chance for 5 s, then
`ohand` once (32 steps), then `orotate` looping every 64 steps; at one
draw per tick (draws are more than 16 ms apart) the loop is 64 ticks.

#### 3.4 Light quality and weather (frame end)

| Item | Where | Notes |
|---|---|---|
| light quality | `[0x007B567C]` (0–2) | chosen by `0x00475780` from the measured draw rate (`render/lighting.md` §5) |
| measured draw rate | `[0x007A04A8]` | 25 at game start (`0x0044F100`); recomputed by `0x0044CCE0` (called each client-loop pass, `0x0044F0AE`): once the loop clock `[0x007A048C]` (`GetTickCount` of the pass) is more than 3,000 ms past the last recompute `[0x007A04B8]`, rate := `[0x007A04AC]` / 3 (integer: in-game draws per second), `[0x007A04B0]` := `[0x007A04B4]` / 3, both counters := 0. `[0x007A04AC]` is +1 per in-game draw issued (`0x0044F29C`, after the draw call `0x0044F28B`); `[0x007A04B4]` is +1 per loop pass with a due client tick whose draw is skipped (`[0x007A0704]` ≠ 0). Wall clock, so not reproducible: captures record it |
| quality options | `[0x0072DA50]`, `[0x0072A348]` | raw |
| render kind | `[0x00712CCC]` (`0x00477730`) | wall / roof fade is instant below 4 |
| rain, snow on | `[0x007A8A44]`, `[0x007A8A40]` | the level's Levels record `+0x05`, `+0x06` (`0x0061DBA0`, `0x0061DC20`) |
| lightning frame trigger, frames drawn in the last second | `[0x007A89E8]`, `[0x007BB390]` | `0x00473910` (the flash needs > 9 frames per second, `draw-order-2.md` §11.7) |
| weather update mark | `[0x007A8A0C]` | last client-update count the weather advanced on (`0x00473F50`) |

#### 3.5 Draw log (frames selected by `--draws-every N`)

Every pixel-writing driver slot is reached only through these D2GFX
wrappers (stdcall, all arguments on the stack; slot names from D2MOO
`D2GraphicsInterfaceStrc`, 1.10f, whose 54-slot layout 1.14d keeps,
`composition.md` §1). Each entry records the call site (return address −
5) and the arguments in order:

| Wrapper | Slot | Args | Recorded beyond the arguments |
|---|---|---|---|
| `0x004F60F0` StartDraw | `+0x18` | 4 | — |
| `0x004F63B0` ClearScreen | `+0xC4` | 1 | — |
| `0x004F68E0` FloorTileDraw (floors and roofs) | `+0x7C` | 9: tile, light grid, X, Y, world x, world y, alpha, open mode, data | tile header; light grid (8 × 8 × 12 bytes) digest or bytes |
| `0x004F6920` TileDrawLit (walls) | `+0x9C` | 5: tile, X, Y, light, open mode | tile header; light (8 × 4 bytes) |
| `0x004F6950` TileDrawTrans | `+0xA0` | 6: as lit + alpha | same |
| `0x004F6980` ShadowTileDraw | `+0xA4` | 5 | tile header |
| `0x004F6450` CelFlatSpriteDraw, `0x004F6480` CelDraw, `0x004F64B0` CelDrawColor, `0x004F64E0` CelDrawEx, `0x004F6510` CelDrawClipped, `0x004F6540` CelDrawShadow, `0x004F6570` CelDrawHilight | `+0x80` … `+0x98` | 7, 6, 6, 6, 5, 3, 4: cel context, X, Y, … | cel context (0x48 bytes) |
| `0x004F6280` UtilDiamond, `0x004F62A0` UtilRect | `+0xA8`, `+0xAC` | 2: RECT*, color | the RECT |
| `0x004F6300` DrawBox, `0x004F6340` DrawBoxAlpha, `0x004F6380` DrawLine | `+0xB8` … `+0xC0` | 6 | — |

- **Unit draw** `0x00471EC0`: ECX unit, EDX light (`b << 24 | g << 16 | r
  << 8 | intensity`, from the light map at the unit's list cell,
  `0x004DF1C0` → `0x00475AA0`), stack: client x, client y, two flags. It is
  reached from the world unit draw `0x004DC7B0` only for units that passed
  the visibility test `0x004DC710` (`camera.md` §7); other callers:
  `0x004C9106`, `0x004C9B03`. Recorded unit fields (raw; meaning:
  `unit-composite.md`): type `+0x00`, class `+0x04`, GUID `+0x0C`, mode
  `+0x10`, sequence present `+0x30 ≠ 0` and its mode `+0x40`, frame
  `+0x44`, frame count `+0x48` (both 8.8), flags `+0xC4`, `+0xC8`; types 0,
  1, 3: path fixed `+0/+4`, client `+8/+0xC`, direction byte `+0x64`
  (`0x006487F0`); types 2, 4, 5: path client `+4/+8`, subtile
  `+0xC/+0x10`, direction byte `+0x1C` (`0x00620100`); the extra offsets
  of `0x004DA0B0 / 0x004DA0D0 / 0x004DA0F0` = record `[[unit + 0x54] +
  0x30]` fields `+0x34` (x), `+0x38`, `+0x3C` (both y) (`0x0046F060`,
  `0x00463E10`; 0 when either pointer is 0).
- **Tile header** (layout `formats/dt1.md` §Tile header; `+0x04` read at
  `0x004DEBA6`, block count `+0x50` and block pointer `+0x54` at
  `0x005131B0`): roof height `+0x04` (u16), orientation `+0x14`, main
  `+0x18`, sub `+0x1C`, rarity `+0x20`. The DT1 file is found from the
  loaded-library list: the loader `0x00600790(&out, path)` keeps one
  0x110-byte record per loaded DT1 (looked up by path first, `0x00600710`)
  in a list headed by `[0x008ADBB4]`: `+0x000` the path as passed (260
  bytes, e.g. `data\global\tiles\act1\barracks\warp.dt1` from
  `0x0044DB60`), `+0x104` the library, `+0x108` the second loader
  result, `+0x10C` next record. A library keeps the DT1 file header
  layout with the tile count at `+0x10C` and the tile array pointer at
  `+0x110`, tiles 0x60 bytes each (`0x00609E90`). So a drawn header `t`
  belongs to the record whose library satisfies `tiles ≤ t < tiles +
  0x60 × count`, tile index `(t − tiles) / 0x60` (resolved by recorder 0.3.0,
  raw-3: `dt1`, `index`). Which tile list each call serves follows from the
  call site (`camera.md` §6; order: `draw-order.md`).
- **Cel header** (raw-3): every software cel draw reaches the common
  rasterizer `0x006014C0` (`sprite-placement.md` §1) with the cel context
  as its first stack argument; the rasterizer reads the cel it draws from
  context `+0x3C` (orientation word `+0x00`, w `+0x04`, h `+0x08`, xoff
  `+0x0C`, yoff `+0x10`: the `dc6.md` §Frame layout). The recorder reads
  those five values there and attaches them to the cel op just logged when
  the context pointer is the same; any other rasterizer call is counted in
  the frame's `raster_other`. A cel op with no rasterizer call (nothing to
  draw, e.g. an empty component, or a wrapper that does not reach it,
  `CelDrawEx` in the first run) has no header.
- **DT1 file and tile index** (raw-3): at the frame's first tile draw the
  recorder reads the loaded-DT1 list above once (path, tile array, count
  per record); each tile draw's header pointer gives `dt1` (the path as
  passed) and `index`, or both `null` when no library contains it.
- **Cel context** (0x48 bytes, built by `0x004DBB50` / `0x004DB7B0` for
  units, by the caller for UI): recorded raw, plus frame `+0x00`, cel file
  pointer `+0x34`, direction `+0x40` and the 4-byte tokens at `+0x18`,
  `+0x1C`, `+0x20`, `+0x24`, `+0x28` (component, armor class, mode and
  weapon class texts for unit components; meaning: `unit-composite.md`).

So the four items the scene source needs per frame are: (1) act and level
(§3.2); (2) the map tiles drawn: the tile draw entries (header, X, Y,
world position, alpha, light); (3) the client units drawn: the unit draw
entries and the cel draws after each; (4) the UI drawn: the remaining cel
and primitive draws, named through §3.6.

#### 3.6 Cel file names

Every load through `0x004788B0` (UI and cursor files, 69 call sites)
emits a `celfile` record: full path (as built: base, name, extension) and
the cel file pointer, so cel contexts in the draw log name their file.
Unit component files load elsewhere (Open question 4, answered): from raw-3
every component path built by `0x005FE610` (`unit-composite.md` §6 r1–r2)
emits `{"k": "compfile", "name", "path"}` (name as composed, path with
`.dcc` or `.dc6`). A component cel context has a null `+0x34`; its five
tokens `+0x18`–`+0x28` (unit, component, armor class, mode, weapon class)
compose the same name (each token up to 3 characters, to the first
space), which names its `compfile` record.

### 4. Tie to ticks

In single player the client loop runs server tick, client update and
draw in one pass, one draw per tick (`camera.md` §9). A capture's `tick` is
the `f` of the most recent server tick of the recorded game that started
before the `EndScene` hit (`null` before the first tick). Measured on the
first runs (run 1: 14,823 frames, run 2: 1,111): 13,701 / 625 frames
follow 1 tick, 1,003 / 466 follow 2 ticks (a draw was skipped), 118 / 19
follow 0 ticks; the draw counter `[0x007A0494]` did not advance on 56 / 18
frames. Frames are therefore numbered by the recorder's own sequence
`seq` (1, 2, … per captured frame), never by the draw counter. Every frame
carries its own tick and state, so skipped draws lose no comparison.

### 5. Raw format `frames-raw-2` (and `frames-raw-3`)

JSON lines, in this order: header `{"k": "header", "format":
"frames-raw-2", "tool", "date", "game_exe_sha256", "args", ...}`; then
`{"k": "capture", "images", "every", "draws_every", "state_key"}`
(`images` = directory name under `game/captures/`); then, interleaved:
base-recorder `game` and `tick` records (`record_tick.py`), `celfile`
records `{"ptr", "path"}`, and per captured frame `{"k": "frame", "seq",
"f", "video_type", "w", "h", "index_sha256", "palette_sha256", "draw",
"client_update", "player", "view_rect", "tile_origin", "unit_origin",
"open_mode", "shift_x", "shake", "clear_counter", "res_mode", "level",
"cursor", "cursor_key", "seed_start", "seed_end", "light", "light_key",
"weather", "image"?, "draws"?}` or the frame head with `"refused"`; footer
with counts and notes. `image` is `frame-<seq, 7 digits>.png`, an 8-bit
palettized PNG (color type 3) whose IDAT holds the index bytes unchanged
and whose PLTE holds the 768 palette bytes. `draws` is the ordered list of
§3.5 entries (`{"op", "at", "a", …}`; `op` = wrapper name or `unit`).

Changes from `frames-raw-1`: images named by `seq` (raw-1 named them by
the draw counter, so 56 images of run 1 and 18 of run 2 were overwritten);
the `capture` record; fields `seq`, `client_update`, `level`, `cursor`,
`cursor_key`, `seed_start`, `seed_end`, `light`, `light_key`, `weather`,
`draws`; `celfile` records. A raw-1 reader must refuse raw-2 and back.

Changes from `frames-raw-2` to `frames-raw-3` (record_frames 0.3.0; every
raw-2 field kept, so a raw-3 reader also reads raw-2 with these fields
absent): `compfile` records (§3.6); in a cel draw's `cel`, `ctx` (the
context pointer, hex) and `hdr` `{"flip", "w", "h", "xoff", "yoff"}` (§3.5
cel header); in a tile draw's `tile`, `dt1` and `index` (§3.5); per frame
with a draw log, `raster_other`; the frame's `player` gains `dir` (path
`+0x64`, the direction byte of §3.5).
With `--front-end` (§2a; same format): the `capture` record gains
`front_end`; a frame gains `present`, `ret`, `in_game`, `launcher_mode`
and, for a shot, `scene`; `celfile` records from `0x004FA9B0` add `via`
and `arg`; a load whose path was not found is a `celload` record.

### 6. Hashes and the comparison

- `index_sha256` = SHA-256 of the W × H index bytes (row-major, top row
  first). `palette_sha256` = SHA-256 of the 768 bytes R, G, B for indices
  0–255.
- Link 1 (`render-pipeline.md` §A10): the CPU reference composes the
  recorded state; the case passes when its index frame is byte-identical
  to the capture's (PNG indices and `index_sha256`) and its palette
  bytes equal the capture's. RGBA equality follows (`composition.md` §6).
  The report gives the mismatched pixel count and the first mismatch;
  `--perturb N` must fail with exactly N (M08).
- Scene source: the recording itself (§3.2–§3.6), not a replay of the
  game's seed through the local server. A frame without a draw log cannot
  be composed.
- Initial framebuffer: the previous captured frame (`seq − 1`) of a
  recording made with `--every 1` (every in-game draw captured); else the
  frame is not composed (the bottom 47 rows and, with BlankScreen 0, every unwritten
  pixel keep older content, `composition.md` §3).
- Outcomes: PASS, FAIL (counts and first mismatch), or NOT WIRED when a
  seam of the comparison is missing (no scene source, no GPU): neither
  pass nor fail, exit code 2, the same for every NOT WIRED kind.

### 7. Stability first

No 1.14d scene is static: the sources below change pixels without any
change of the camera or player state. Each is either part of the recorded
state key or listed as not modelled:

| Source | Clock | Inputs | Handling |
|---|---|---|---|
| mouse cursor (§3.3) | wall clock (> 16 ms per step, 5 s idle) and the player seed | cursor state at frame start | key: `cursor_key` = drawn flag, type, `frame >> 8`, x, y, adj, item present |
| light quality (§3.4) | measured draw rate (wall clock) | `[0x007B567C]` | key: `light_key` |
| unit animations (torches, monsters, objects, missiles) | ticks | unit draw entries | key when the frame has a draw log; else the case must have no animated unit in view |
| rain, lightning | client updates and the player seed | `weather`, seeds | not modelled (Open question 6); stability and placement cases use levels with rain and snow 0 |
| wall / roof fade | wall clock, 500 ms (render kind ≥ 4 only) | — | none on GDI (render kind < 4: fade completes at once) |
| UI animations (automap markers, quest button, globes, stamina) | various | draw log | the stability case keeps the automap and panels closed and stats constant |

Rule: frames whose state key — player record (with the 8.8 frame),
tile origin, unit origin, shake, open mode, palette hash, `cursor_key`,
`light_key`, level — is equal must have equal `index_sha256`, with at
least two keys seen at least twice each (keys seen once never count).
`record_frames.py` prints the number of keys seen at least twice, their
frames, the keys with differing frames and PASS / FAIL / NOT ENOUGH. A
failing key means the capture point or the key misses an input: fix the
spec before any other case. Perturbation: changing one byte of one saved
PNG must make a re-hash differ for exactly that frame; the selftest checks
that one changed hash fails exactly one key.

Evidence (run 1, Den of Evil dead end, player idle, automap and panels
closed): with the `frames-raw-1` key (no cursor), frames f 14,800–15,600
form 16 keys; 772 of 785 same-key pairs differ, and every differing
pixel lies in the 33 × 30 box x 579–611, y 250–279, which is the cursor's
`orotate` cel; same-idle-frame pairs 64 ticks apart are equal in 607 of
737 cases, 16 / 32 / 48 ticks apart in 42 of 785, 10 of 769, 0 of 753
(the 64-step loop of §3.3). In f 13,050–13,426 the cursor rests beside the
player: frames 13,200 and 13,216 (same key) differ in 332 pixels, all on
the cursor. The "±1 step on the dark ramp on and around the player" first
read as a light flicker is the cursor's own shading; no light-map change
was found there (run 1b's left-floor changes: `render/lighting.md` §12).

### 8. Capture cases

Each case: start `py tools/trace-recorder/record_frames.py --seconds S
--draws-every 25` (default arguments `-w -ns`), then in the game window:

| Case | Player does | Proves |
|---|---|---|
| `stability-0001` | Single Player, any character, enter the Den of Evil, kill every monster, walk to a dead end away from doors, stand still 30 s, no panels, automap closed, do not touch the mouse | §7 |
| `placement-0001` | Same place: stand still 5 s, open and close the inventory (right panel), stand 5 s | `sprite-placement.md` §2, §8; `camera.md` §1 modes 0/1 |
| `camera-0001` | Rogue Encampment: walk (not run) in a straight line 5 s, then run 5 s, then stand | `camera.md` §2–§4, §6 |
| `composition-0001` | Rogue Encampment: cast Town Portal (scroll), stand next to it 5 s | `composition.md` §5 (translucent draw) |

The debugger slows the game: draws are skipped (§4) and the measured draw
rate, hence the light quality (§3.4), can differ from normal play; both
are recorded per frame.

## Constants & data dependencies

Addresses in §2–§3; frame size 800 × 600; cursor type table
`0x00712010` (7 records of 0x1C bytes); cel context 0x48 bytes.

## Randomness

None in the capture. Inside a frame the player unit's client seed
(`+0x20`) is stepped by the shake (`camera.md` §8, two draws), the cursor
in state 1 (§3.3, one step per cursor step) and the weather (`0x00473910`,
`0x00473090`); the recorded `seed_start` / `seed_end` bound them.

## Edge cases & original bugs

- The bottom 47 rows carry content from older frames (`composition.md`
  §3): a case's first captured frame may depend on frames before
  recording started (§6, initial framebuffer).
- Frames with `clear_counter > 0` are all index 0.
- The cursor's step and idle timers and the light quality run on wall
  clock; a d2rs frame can match a capture only with the recorded values as
  input. Reproduce the rules; take the clocks from the recording.
- The cursor in state 1 steps the player seed the shake also uses, so the
  shake offsets depend on how many cursor steps ran (`camera.md` OQ6).

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| `record_frames.py --selftest` | "selftest ok" (PNG keeps indices and palette; a 1-byte change changes the hash; 13 state fields each follow exactly their source bytes; stability: two repeated equal keys PASS, one changed hash → exactly 1 failing key, one repeated key NOT ENOUGH) | §5–§7 |
| cursor at mx 580, my 250, adj 0, type 3, frame 0x260 | `CelDraw` frame 2 of `orotate` at (580, 250), light `0xFFFFFFFF`, mode 5 | §3.3 |
| same with mx 900 (W = 800, adj 0) | x = 799 | §3.3 |
| state 4 type 3, frame 0x7E0, one step | frame 0x800 ≥ 8 × 256 → loops to 0 | §3.3 |
| first runs: every frame's path client vs `camera.md` §2 from the fixed position; tile / unit origin and view rect vs §1, §3 | 15,934 of 15,934 equal (modes 0, 1, 2, 3; no shake) | `frames-raw-1` runs 1, 2 |
| `stability-0001` run (`frames-raw-2`) | PASS | §7, queued |

## Provenance

Hook and memory locations from 1.14d `Game.exe`: frame `0x0044C990`
(call `0x0044CB4A`), loop `0x0044EFA0`, counters `0x0044F28B` path and
`0x0044C7F0`; player global read by `0x00463DD0`; level chain `0x004646A0`,
`0x00620BB0`, `0x00648A80`, `0x0061A1B0`, `0x0066BAB0`; act `0x0044E100`,
`0x006194A0`, environment `0x0061AA60`, `0x0061C0B0`, `0x0061C0E0`;
cursor `0x004680B0`, `0x004684C0`, `0x004683C0`, `0x00468310`,
`0x004681C0`, `0x004682C0`, `0x00467E40`, `0x00467E90`, table
`0x00712010`; light `0x00475780`, `0x0044CCE0`; weather `0x00473F50`,
`0x00473910`, `0x00473090`; draw wrappers `0x004F60F0`–`0x004F6980`
(slot offsets read from each wrapper, driver tables `composition.md` §1);
unit draw `0x00471EC0`, `0x004DC7B0`, `0x004DF1C0`; cel loader
`0x004788B0`. D2MOO (1.10f) slot names were a hint; each wrapper's slot
and argument count were read on 1.14d. Evidence of §4 and §7: the
`frames-raw-1` runs `traces/raw/20261006-140102-frames-run1b.jsonl` and
`20261006-141725-frames-run2.jsonl` with their PNGs (local), analysed by
re-hashing and diffing the PNG indices. Recorder design follows
`tools/trace-recorder/record_tick.py`.

## Open questions

1. Whether the client player unit's `+0x44` is its animation frame —
   answered: 8.8 fixed, drawn frame `+0x44 >> 8` (`0x00621810`); in run 1
   it advances 128 per tick in mode 1 (Den) and 80 in mode 5 (town), and
   keys built from it repeat (16 keys in f 14,800–15,600).
2. Whether the debugger makes the client loop skip draws — answered: yes,
   7 % of run 1's frames and 42 % of run 2's follow two ticks (§4);
   `camera-0001` compares per frame with each frame's own tick and state.
3. Which DT1 file a drawn tile header belongs to: the tile library loader
   (D2CMP `LoadTileLibrarySlot` in 1.10f) and where 1.14d keeps the file
   name per tile. Ghidra read of the tile library load path.
   *Answered* (`draw-order.md` OQ 12, `drlg/rooms.md` §9.3 "Entry
   identity"): a record's tile pointer is exactly (file, tile index); a
   recorder maps it through the room's 32 library slots (room +0x68): the
   slot whose file's tile array (+0x110, count +0x10C) contains it. Also
   §3.5 (the loaded-library list `[0x008ADBB4]`, file path per record).
   Done in `record_frames.py` 0.3.0 (`frames-raw-3`, §3.5).
4. Where unit component cel files load (not through `0x004788B0`): the
   composite path `0x004DB7B0` → `0x004DA720` (`unit-composite.md`); a
   `celfile`-style hook there names the DCC files.
   *Answered* (`unit-composite.md` §6 r1–r2): the name is composed by
   `0x005FE2B0` and the path by `0x005FE610` (`%s\%s\%s\%s.dcc`, or
   `.dc6`); a hook on `0x005FE610`'s result names every component file.
5. ~~Light-map rules and the light flicker of run 1b.~~ Answered in
   `render/lighting.md`: build and sources §1–§8, quality and draw rate §5,
   what a capture must record §12 r1–r3 (light-map digest at frame end, not
   the quality alone); run 1b's first still segment §12 r4 (cursor, UI and
   a moving off-screen light; that source is `lighting.md` Open question 9).
6. Weather (now specified in `draw-order-2.md` §11 except its Open question 3) was not modelled: rain particles (`0x00473090`, three draws of
   the player seed per new drop) and lightning (`0x00473910`: countdown
   500 + rnd(1,500), a flash rectangle of index 255, draw mode 5) advance
   in the weather update `0x00473F50`, which the frame calls before
   `StartDraw` and which runs only when the client-update count moved; they
   use the seed the cursor and shake also step. A town case
   needs either the weather rules (owner to name: a weather spec) or a
   mask; until then town frames do not count for §7.
   *Answered* (owner named): the weather rules are `draw-order-2.md`
   §11; what remains is its OQ 1 (a capture with the per-frame player
   seed and weather pools, on the PC 2 recording list).
7. Unexplained small changes in run 1 f 13,486–14,636 (black ↔ index 172
   at x 0–125, y 125–275 — lighting, `render/lighting.md` §12 r5 — and
   x 150–200, y 575–600): rerun with `--draws-every` and read the draw log
   at those pixels.
8. Draws with no server tick between (118 frames of run 1, 19 of run 2)
   while not paused contradict `camera.md` §9 ("passes without a tick do
   not draw"); the `frames-raw-2` `client_update` counter shows whether a
   client update ran.
   *Answered* (static, 2026-10-08, asm `0x0044EFA0`; `camera.md` §9
   cases 1–2): paused passes draw through `0x0044F017` without advancing
   `[0x007A0494]` (a source of the frames where the draw counter did not
   advance), and in the catch-up hold (`[0x007A04BC]`: 10 s after an act
   load, 2 s after the act set-up, 3 s after a local S→C 0x15 room
   move) every pass with ≥ 40 ms on the client clock draws, tick or not.
   Each frame carries its own tick and state (§4), so these frames lose
   no comparison. The recording check that run 1's 0-tick frames fall in
   such windows (or are paused) is on the PC 2 list.
