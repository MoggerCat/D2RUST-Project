# Spec: Render — Sprite placement (cel and tile pixels from a draw position)

- **Status:** draft (2026-10-06, RE on 1.14d `Game.exe`; no capture yet).
  Every rule names its 1.14d address; the pixel proof is the capture case
  of §Test vectors (unverified until it runs).
- **Target version:** 1.14d
- **Crate/module:** `d2-client::frames` (offsets), `d2-client::composite`
  and `world_view` (`place` hooks), `d2-client::scene` (`DrawItem.x/y`)
- **Related specs:** `formats/dc6.md`, `formats/dcc.md`, `formats/dt1.md`
  (file layouts), `render/camera.md` (where the draw position comes from),
  `render/composition.md` (framebuffer, clear, domain),
  `client/render-pipeline.md` §A2/§A3 (d2rs `IndexFrame`, `DrawItem`),
  `render/map-preview.md` (Phase 1b preview, simplified)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 38–47 |
| Inputs | 48–56 |
| Outputs / state changes | 57–62 |
| Rules | 63–64 |
|   1. The cel draw path | 65–77 |
|   2. Placement (orientation bit 0 clear: the normal case) | 78–92 |
|   3. Where the cel fields come from | 93–118 |
|   4. Orientation bit set (top-down cels) | 119–129 |
|   5. Clipping | 130–146 |
|   6. Transparency | 147–158 |
|   7. DT1 tiles | 159–174 |
|   8. d2rs mapping (answers the `place` hooks) | 175–193 |
| Constants & data dependencies | 194–198 |
| Randomness | 199–202 |
| Edge cases & original bugs | 203–226 |
| Test vectors | 227–241 |
| Provenance | 242–262 |
| Open questions | 263–323 |
<!-- /index -->

## Summary

Every sprite in 1.14d is drawn by one call: "draw this frame at (X, Y)".
This spec owns how (X, Y) and the frame's own offsets give the screen
pixels the frame covers, for DC6 frames, DCC frames and DT1 tile blocks,
and how that maps onto d2rs's `IndexFrame` offsets and `DrawItem` top-left.
Where (X, Y) comes from is not here: world units and tiles in
`camera.md`, UI panels in the `ui/` specs, per-unit extra offsets in
`unit-composite.md`.

## Inputs

| Name | Type | Source |
|---|---|---|
| draw position | (X, Y) integer screen pixels | caller (`camera.md` §4–§6, UI specs) |
| cel | `w`, `h`, `xoff`, `yoff`, orientation bit, encoded rows | DC6 frame header; DCC frame converted (§3) |
| tile | DT1 tile: blocks with `(bx, by)`, format, pixels | `formats/dt1.md` |
| frame clip | W × H frame, column clip `[L, R)` | `composition.md` §2, §1 |

## Outputs / state changes

The set of framebuffer pixels written and which source pixel lands on each.
How each source index is shaded and blended is `shading.md` /
`blend-modes.md`; the write itself is `composition.md` §5.

## Rules

### 1. The cel draw path

All software cel draws go through one path: `D2GFX_DrawCelContext`
`0x004F6480` → driver slot `+0x84` (DirectDraw `0x00511FB0`, GDI
`0x006C84B0`, identical bodies) → common rasterizer `0x006014C0` → row
drawer `0x00608540` (only entry of the drawer table `0x006E3688` used for
DC6 encoding 0 and for DCC cels; every live DC6 file has encoding 0, see
§Provenance counts). The cel ("cell") fields read are, in the
frame header layout of `dc6.md` §Frame: `+0x00` orientation word (only bit 0
is tested, `0x00608540`), `+0x04` w, `+0x08` h, `+0x0C` xoff, `+0x10` yoff,
then the encoded rows. Accessors: `0x006018C0` (w), `0x006018F0` (h),
`0x00601920` (xoff), `0x00601950` (yoff).

### 2. Placement (orientation bit 0 clear: the normal case)

A cel drawn at (X, Y) covers

- columns `X + xoff` … `X + xoff + w − 1`,
- rows `Y + yoff − h + 1` … `Y + yoff` (both ends inclusive).

The encoded rows run bottom row first; the first encoded row lands on row
`Y + yoff` and each next row one above (`0x00601521`: bottom = Y + yoff;
`0x00601570`: destination = bottom × pitch + X + xoff; `0x00608540`
negates the pitch when bit 0 is clear). So `yoff` is the **bottom row**,
inclusive. This settles the one-row question (`dc6.md` OQ1, `dcc.md` OQ2,
HANDOFF §7 carried-over #3): the vertical extent is
`[offset_y − height + 1, offset_y]`, not `[offset_y − height, offset_y)`.

### 3. Where the cel fields come from

| Source | Cel built by | orientation word | xoff, yoff | rows encoded |
|---|---|---|---|---|
| DC6 frame | none: the file's frame header is the cel (`D2CMP_GetCelFromCelContext` `0x00601840` returns frame `F × dirmap[D][dir] + frame`; the direction map is `unit-composite.md`) | `flip` | `offset_x`, `offset_y` | as stored (`dc6.md` §Pixel decoding) |
| DCC frame | DCC decoder `0x0060BFF0`, one cel per frame | the frame's `variable0` (copied into the first word) | the frame header's `x offset`, `y offset`, unchanged | re-encoded by `0x0060BDB0`: bottom row first (for bottom-up = 0), index 0 → skip, runs ≤ 127 |

Cache path: when the cel context carries no cel file pointer,
`0x006001F0` fetches the frame from a store of earlier decoder output
(built through `0x005FFE90` → `0x005FF760` → `0x005FF6E0` → `0x0060BFF0`)
as a record with the DC6 frame header layout (records chained by `0x23 +
length + [+0x14]`). The getter `0x005FEC50` → `0x005FEB80` returns that
record unchanged as the cel, but only when its orientation word is 0 and
w, h ≤ 256 (else no cel: nothing drawn); the one-time pass `0x005FEC90`
only rewrites `+0x18` (`next_block`, not read by the drawer) and ends the
process with a fatal error (tag `0x58C`, `0x005FEDF4`) on any record
outside those limits. So a cached
cel draws exactly like the decoder's cel.

For DCC frames with bottom-up = 0 (all of 1.14d, `dcc.md` OQ1) the cel
covers exactly the `dcc.md` §Boxes frame box: columns `x_min … x_max`,
rows `y_min … y_max` with `y_max = y offset`, offset by (X, Y). The
decoder's internal direction box puts a top-down frame at rows
`[y offset − h, y offset)` (`0x0060B530`); that is internal and shifts all
frames of a direction alike, so the decoded image is unchanged.

### 4. Orientation bit set (top-down cels)

When bit 0 of the orientation word is set the row drawer walks **down**
from row `Y + yoff` (positive pitch): the cel covers rows `Y + yoff` …
`Y + yoff + h − 1`, first encoded row on top. Clipping is still computed
as if the cel were bottom-up (§5): see Edge cases. In 1.14d data this
applies to the 140 DC6 frames with `flip = 1` (`dc6.md` status line;
`data\global\items\inv*.dc6`) and to no DCC frame: `variable0` is 0 in
all 3,305,132 live DCC frames (and bottom-up is 0 in all of them;
§Provenance counts).

### 5. Clipping

- Rows: the rasterizer clamps the bottom row to `H − 1` (skipping that many
  encoded rows) and stops at row 0 (`0x00601530`–`0x00601559`). Rows
  outside `[0, H)` are never written.
- Columns: each run is cut to `[L, R)` (`0x00608540`, clipped variants
  `0x006077C0` etc.), with `L`/`R` the driver's cel display left/right
  (GDI `0x0098A168`/`0x0098A16C`, DirectDraw `0x0088129C`/`0x008812A0`),
  set to `[0, W)` at surface creation (GDI `0x006C82D0`). A run fully
  inside `[L, R)` is drawn unclipped (same pixels).
- The driver's pre-test (`0x00511FB0`: `X + xoff < W`, `X + xoff + w ≥ 0`,
  `Y + yoff ≥ 0`, `Y + yoff − h < H`) only rejects cels with no visible
  pixel; it never changes which pixels are drawn.

So d2rs's `DrawItem.clip` for a world or UI cel is the frame rectangle
`[0, W) × [0, H)` unless an owner spec narrows it.

### 6. Transparency

A pixel is transparent only through the encoding: DC6 skips (`b & 0x80`),
DCC zeros (turned into skips by `0x0060BDB0`), DT1 RLE skips. Every byte
inside a DC6 copy run, every byte of a DT1 RLE run and every pixel of a DT1
isometric block's diamond is written, value 0 included (through the
shade maps). d2rs's `IndexFrame` treats index 0 as transparent
(`render-pipeline.md` §A2); that is exact for the live data: no DC6 copy
run (88,997,463 bytes), no DT1 RLE run (66,053,175 bytes) and no DT1
isometric diamond (58,110,976 bytes) holds a 0 (§Provenance counts). A
non-live file with such a 0 would need an "opaque zero".

### 7. DT1 tiles

A tile drawn at (X, Y) puts block `b`'s pixel `(px, py)` (block-local,
`py = 0` the block's top row, `formats/dt1.md` §Block pixels) at screen
`(X + b.x + px, Y + b.y + py)`: wall drawers `0x005131B0` (DirectDraw)
/ `0x006C94B0` (GDI, slot `+0x9C`, lit) and `0x005130A0` / `0x006C93A0`
(slot `+0xA0`, translucent); floor drawer `0x005132C0` / `0x006C95D0`
(slot `+0x7C`), which first moves X by −80 and by the panel shift
(`camera.md` §5). Floors **and roofs** use the floor drawer (`camera.md`
§6: the roof list `0x004DEA70` calls only `0x004F68E0`). The block pixels go through the DT1 helpers (e.g.
`0x004F7EA0` for RLE blocks: runs copied verbatim, `(0, 0)` = next row).
The wall drawer culls whole blocks (`camera.md` §7); the floor drawer
culls nothing per block. The pixel clip is the frame.

The screen (X, Y) passed for each tile kind is `camera.md` §6.

### 8. d2rs mapping (answers the `place` hooks)

`IndexFrame` keeps each format's own offsets (`frames/mod.rs`). The
`DrawItem` top-left for a draw at (X, Y) is:

| Frame | `IndexFrame (x_off, y_off)` | `DrawItem (x, y)` |
|---|---|---|
| DC6, `flip & 1 = 0` | `(offset_x, offset_y)` | `(X + x_off, Y + y_off − height + 1)` |
| DC6, `flip & 1 = 1` | `(offset_x, offset_y)` | `(X + x_off, Y + y_off)` |
| DCC (bottom-up 0, `variable0` even) | `(x_min, y_min)` | `(X + x_off, Y + y_off)` |
| DT1 tile image (`map-preview.md` §Tile images) | `(x0, y0)` | `(X + x_off, Y + y_off)` |

`flip_x` (`render-pipeline.md` §A3) stays `false`: the software cel path
has one row drawer and its only orientation switch is the vertical bit;
nothing mirrors horizontally. A DC6 frame set must therefore carry each
frame's `flip & 1`: today `IndexFrame` does not, so the builder reads it
from the decoded `Dc6` frame (hook `frames/mod.rs` `TODO(spec:
render/sprite-placement.md)`).

## Constants & data dependencies

Frame size W × H (`composition.md` §1). Block culling margins 32 (`0x005131DE`,
`0x005131EA`). Floor X adjustment −80 (`0x005132DD`, `0x006C95ED`).

## Randomness

None.

## Edge cases & original bugs

1. **Top-down cels clip as bottom-up.** For an orientation-bit-set cel the
  rasterizer still computes the skipped rows and the row count as if rows
  ran up from `Y + yoff` (§5) while the drawer walks down: a top-down cel
  crossing the frame's top or bottom edge is cut wrongly (rows skipped
  from its top, count limited by `Y + yoff + 1`). Reproduce the rows kept
  inside the frame. Rows the original writes past the end of the surface
  (a top-down cel whose first row is near `H − 1`) land outside the
  framebuffer in process memory and cannot be reproduced: d2rs clips them,
  and a capture case containing such a draw does not count. No live case
  is known to cross an edge (the top-down frames are inventory item cels,
  drawn inside panels).
2. **No-clip branch.** If `L = R = 0` the rasterizer requires `X + xoff ≥ 0`
  and `Y + yoff` clamped `≥ 0` instead of clipping columns (`0x0060155C`).
  `R` is never 0 after surface creation, so the branch is dead in play.
3. Zero-size frames draw nothing (`dc6.md` Edge cases).
4. `0x006014C0` rejects a DC6 cel file whose version is not 6 or whose
  flags word has bit 2 (fatal errors `0x452`/`0x453`).
5. **Orientation bit only.** The drawer tests bit 0 of the orientation word
  (§1, §4); a DC6 `flip` of 2 would draw bottom-up. Live data has DC6
  `flip` 0 (26,177 frames) or 1 (140) and DCC `variable0` 0 only; d2rs
  refuses other values (not live data).

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| DC6 cel w 3, h 2, offset (5, 10), flip 0, drawn at (100, 200) | columns 105–107, rows 209–210; `DrawItem (105, 209)` | §2, §8 |
| same, flip 1 | rows 210–211; first stored row on 210; `DrawItem (105, 210)` | §4 |
| DCC frame w 8, h 20, x offset −4, y offset −1 (box x −4…3, y −20…−1) at (400, 292) | columns 396–403, rows 272–291; `DrawItem (396, 272)` | §2, §3 |
| DC6 cel h 5, yoff 0 at Y = 2 | rows 0–2 drawn (bottom 3 stored rows), rows −2…−1 cut | §5 |
| DC6 cel h 4, yoff 0 at Y = H + 1 (bottom row H + 1) | 2 bottom stored rows skipped, rows H − 2 … H − 1 drawn | §5 |
| DC6 run byte 0 inside a copy run | written (as map[0]), not transparent | §6 |
| DT1 RLE block at b = (32, −64), tile at (X, Y) = (0, 300) | block top-left at (32, 236) | §7 |
| 1.14d end-game screen, expansion (`0x0044E770`, `[0x007A04F4]` ≠ 0): `ui\MENU\EndGame2.dc6` (12 frames, all offsets (0, 0); widths 256, 256, 256, 32; heights 256 × 8, 88 × 4) drawn at X = 0, 256, 512, 768 and Y = 256 (frames 0–3), 512 (4–7), 600 (8–11); 800 × 600 | rows 1–256, 257–512, 513–599; frame row 600 (each bottom-row frame's first encoded row) is clipped (§5); row 0 is never drawn (it keeps the `StartDraw(1, …)` clear, index 0) | §2, §5 |
| same, classic (`[0x007A04F4]` = 0): `ui\MENU\EndGame.dc6` (8 frames, offsets (0, 0), widths 256, 64, heights 256 / 224) at X = 0, 256, W/2, W/2 + 256 and Y = 256 (frames 0, 1, 4, 5) or H − 1 (2, 3, 6, 7); 640 × 480 | top frames rows 1–256, bottom frames rows 256–479: row 256 is drawn twice and the bottom frame (drawn later) wins; row 0 not drawn | §2 |
| capture case `placement-0001`: player standing still in a cleared area, frame captured (`capture.md`) | CPU reference with §2/§8 and `camera.md` equals the captured index frame | capture, queued |

## Provenance

1.14d `Game.exe`: driver table entries at `0x0072F6D0` (DirectDraw) and
`0x0074C4A8` (GDI) give the cel/tile draw slots; `0x006014C0` and
`0x00608540` read for placement, clip and run handling; `0x0060BFF0`,
`0x0060B530`, `0x0060BDB0` for the DCC → cel conversion;
`0x00601840` for DC6 frame selection; `0x005131B0`, `0x005132C0`,
`0x006C95D0`, `0x004F7EA0` for tiles. Game-file counts (2026-10-06,
scratch Python over the files `mpq-tool extract` wrote from `d2data.mpq`,
`d2exp.mpq`, `d2char.mpq`, plus the two DC6 files `patch_d2.mpq` holds
under a listed name, `items\invrVex.dc6` and
`ui\BIGMENU\PlayerProfilebckg.dc6`, read by name): 1,653 DC6 files, all
encoding 0, flags 1, 26,317 frames, 0 zero bytes in copy runs; 250 used
DT1 files (the 6 known-unused ones of `formats/dt1.md` excluded), 0 zero
bytes in RLE runs or iso diamonds; 21,717 DCC files, 3,305,132 frames,
`variable0` 0 and bottom-up 0 in every frame. Driver struct layout from D2MOO
`D2Gfx.h` (`D2GraphicsInterfaceStrc`, 1.10f), confirmed on 1.14d: the
1.14d tables have exactly its 54 slots and the cel draw wrapper calls slot
`+0x84`. Riiablo's two DC6 codecs (`[y − h, y)` vs `[y − h + 1, y]`) were
the two candidates; 1.14d code picks the second. No capture yet.

## Open questions

1. Do live DC6 copy runs or DT1 block pixels contain index 0 (§6)? Settle
   with a game-file count (`mpq-tool formats` extension: count zero bytes
   inside DC6 runs, DT1 RLE runs and DT1 iso diamonds). If any exist,
   `IndexFrame` needs an "opaque zero" representation.
2. Is `variable0` of every live DCC frame even (§3, §4)? Same tool, one
   count. An odd value would draw that frame top-down.
3. Is the DC6 `encoding` field 0 in every live file? `0x006014C0` indexes
   the drawer table `0x006E3688` by it and only entry 0 is a row drawer.
   Same tool.
4. Real-data vector: `offset_x/offset_y` of the `data\global\ui\menu\
   endgame*.dc6` frames and of the control panel frames, to pin one
   full-screen layout to exact rows (game-file read).
5. Whether the DCC cache path (`0x006001F0` when the cel file pointer is
   null) can hand the drawer a cel built some other way than `0x0060BFF0`
   (e.g. a cached copy with changed fields). A Ghidra read of
   `0x005FEC90`/`0x005FEC50` settles it.
   *Answered* (static): no. A DCC pool block is filled only by the
   loader callback `0x005FF6E0`, the single caller of `0x0060BFF0`
   (`0x005FF71E`). Both cache paths (`0x006001F0` at `0x006003A2`,
   `0x005FFE90` at `0x005FFF7C`) run `0x005FEC90` once per block (block
   +0x28 = 0): it walks the decoder's cels (cel size 0x23 + cel +0x14 +
   cel +0x1C), checks first dword = 0, w ≤ 256, h ≤ 256 (else a fatal
   error) and stores in cel +0x18 a pointer to a zeroed 0x2C-byte side
   entry. Then `0x005FEC50` → `0x005FEB80` returns a pointer to the
   chosen cel. No field §1 reads (+0x00 … +0x10, rows) is changed, so
   the drawer sees exactly the `0x0060BFF0` cel (also §3, cache path).
   Open: when `0x006001F0` takes the cache branch, and whether the 37
   live DCC frames wider or taller than 256 (`GTTRLITA1HTH.dcc` 20
   frames 345 × 324, `GTTRLITNUHTH.dcc` 4, `THS1LITDTHTH.dcc` 12 with
   heights 257–274, `RedemptionGhost Big.dcc` 1 at 257 × 214) ever reach
   it (the getter would return no cel, the one-time pass a fatal error).
   A Ghidra read of `0x005FE990` / `0x0060ACE0` (cache lookup) plus a
   capture of one of these monsters (`GT`, `THS1`) settles it.

Answers 1–4 (game-file read, 2026-10-07: every `.dt1`, `.dc6` and `.dcc`
that `d2data.mpq`, `d2exp.mpq` and `d2char.mpq` list, extracted with
`mpq-tool extract` and counted by a scratch script; `Patch_D2.mpq` has no
listfile and was not read; the 6 known-unused DT1s of `formats/dt1.md`
excluded):

- 1 *Answered*: no. 0 zero bytes in the copy runs of 1,651 DC6 files, in
  the RLE runs or iso diamonds of the 397,668 blocks of 251 DT1 files
  (the six unused DT1s do contain zeros: 2,845 blocks). `IndexFrame`
  needs no opaque zero for 1.14d data (§6).
- 2 *Answered*: yes. `variable0` is 0 in all 3,305,132 frames of the
  21,717 DCC files (no frame has optional bytes or the bottom-up bit;
  §4).
- 3 *Answered*: yes, `encoding` = 0 in all 1,651 DC6 files (§1).
- 4 *Answered*: `ui\MENU\EndGame.dc6` (8 frames: 256 × 256, 64 × 256,
  256 × 224, 64 × 224, twice), `EndGame2.dc6` (12 frames, widths 256,
  256, 256, 32, heights 256, 256, 88), `endgameok.dc6` (2 × 96 × 32),
  `ui\PANEL\ctrlpnl7.DC6` (117 × 104, 128 × 55, 128 × 55, 54 × 55,
  117 × 104, 128 × 55), `800ctrlpnl7.dc6` (117 × 104, 128 × 55 × 3,
  86 × 55, 117 × 104, 128 × 55) and `ctrlpnl_popbelt.DC6` (125 × 32)
  all have offset_x = offset_y = 0, so §2 draws each frame on rows
  `y − height + 1 … y` of its draw call's y (the UI code supplies the
  bottom row; owner of the layout: `client/ui.md`). End-game rows are in
  §Test vectors (draw positions read at `0x0044E8C5`–`0x0044EB2E`).
