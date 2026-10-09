# Spec: Tools — Rendering facts (`facts/render/`) and the scene compare

- **Status:** implemented (2026-10-08): the d2rs exporter (`d2-client play
  --dump-draws`) and `d2-client facts-compare` follow §1–§6; format tests
  on hand-made TSVs pass. `facts_render.py` 0.2.0 writes §1–§4 from a
  `frames-raw-3` capture (selftest; first real capture converted under Wine
  2026-10-08, q-cloud-game). Comparison on real facts: see the scene
  handoffs; unverified (M02) until one passes.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::facts` (format, export, compare);
  `tools/trace-recorder/facts_render.py` (the 1.14d converter, PC 1)
- **Related specs:** `render/capture.md` (§3.5 draw log, §5 `frames-raw-2`,
  §6 hashes), `render/sprite-placement.md` §8 (cel offsets → draw
  position), `render/camera.md` §1–§3, §6–§7 (origins, tile handed
  position), `render/blend-modes.md` (draw modes), `client/assets.md` §A1
  (canonical paths), `client/render-pipeline.md` §A3 (d2rs draw items)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 40–51 |
| Inputs | 52–58 |
| Outputs / state changes | 59–65 |
| Rules | 66–67 |
|   1. Files and header | 68–89 |
|   2. `draws.tsv` | 90–133 |
|   3. `frame.tsv` | 134–166 |
|   4. `sprites.tsv` | 167–182 |
|   5. d2rs export | 183–261 |
|   6. Comparison | 262–284 |
|   7. Requests | 285–296 |
| Constants & data dependencies | 297–300 |
| Randomness | 301–304 |
| Edge cases & original bugs | 305–312 |
| Test vectors | 313–321 |
| Provenance | 322–326 |
| Open questions | 327–341 |
<!-- /index -->

## Summary

A rendering fact set describes one frame of one scene as plain text: the
ordered list of draw calls (`draws.tsv`), the state the frame was drawn
from and the hashes of what it presented (`frame.tsv`), and the decoded
size and offsets of every sprite frame drawn (`sprites.tsv`). 1.14d's
facts are produced by `facts_render.py` from a `frames-raw-2` capture
(`render/capture.md` §5); d2rs writes the same files for one frame of its
own renderer. `facts-compare` reads both and reports the first difference,
cause before effect: frame inputs, then sprites, then the draw list, then
the presented hashes.

## Inputs

| Name | Type | Source |
|---|---|---|
| 1.14d capture | `frames-raw-3` JSON lines + its `celfile` and `compfile` records (`frames-raw-2` reads with the raw-3 cells `?`) | `record_frames.py --draws-every N` (PC 1, or the cloud under Wine: `tools/cloud-game/`) |
| d2rs frame | the world view's built, sorted draw list (`WorldFrame`) | `d2-client play --dump-draws DIR --at-tick N` |

## Outputs / state changes

`facts/render/scenes/<scene>/draws.tsv`, `.../frame.tsv`,
`facts/render/sprites.tsv` (1.14d, committed: measurements only, rule 1);
the same three files in the d2rs dump directory (not committed); the
compare's report and exit code.

## Rules

### 1. Files and header

1. Each file is UTF-8 text, lines ending in `\n`, fields separated by one
   tab. Line 1 is the header line
   `# facts v1; tool: <name version>; command: <exact command>; game: <game>`
   with `<game>` = `1.14d` for the original and `d2rs <version>` for an
   export. Line 2 is the column-name row of §2–§4, exactly as listed and
   in that order. Every later line is one row with exactly that many
   fields. Any other shape is an error (exit 3), never skipped.
2. Values: decimal integers (signed, no leading `+`), lowercase hex with
   `0x` where a column says hex, lowercase 64-digit hex for SHA-256, or
   text without tabs. Two markers: `-` = not applicable to this row's
   kind (both sides must agree), `?` = applicable but not measured by
   this producer (never equal to anything; §6 r3).
3. Paths are canonical (`client/assets.md` §A1): lowercase ASCII, `/`
   separators, no leading `/`, with the extension (`.dc6`, `.dcc`,
   `.dt1`). 1.14d's `DATA\GLOBAL\UI\CURSOR\orotate` becomes
   `data/global/ui/cursor/orotate.dc6`.
4. A scene directory holds `draws.tsv` and `frame.tsv`; `sprites.tsv` is
   one file for all 1.14d scenes (`facts/render/sprites.tsv`) and one per
   d2rs dump directory.

### 2. `draws.tsv`

Columns: `i op file dir frame tile x y w h xoff yoff mode light pal unit at`.
One row per draw call of the frame, in call order (1.14d: the order of the
frame record's `draws`, `capture.md` §3.5; d2rs: §5).

1. `i`: the row number, from 0.
2. `op`: the D2GFX wrapper name of `capture.md` §3.5 (`StartDraw`,
   `ClearScreen`, `FloorTileDraw`, `TileDrawLit`, `TileDrawTrans`,
   `ShadowTileDraw`, `CelFlatSpriteDraw`, `CelDraw`, `CelDrawColor`,
   `CelDrawEx`, `CelDrawClipped`, `CelDrawShadow`, `CelDrawHilight`,
   `UtilDiamond`, `UtilRect`, `DrawBox`, `DrawBoxAlpha`, `DrawLine`) or
   `unit` for the unit draw `0x00471EC0`.
3. Cel ops (`Cel*`): `file` = the cel file: the `celfile` path of the
   context's `+0x34` pointer; when that pointer is null (unit
   components), the `compfile` path whose name the context's five tokens
   compose (`capture.md` §3.6); else `?`. `dir` = context `+0x40`,
   `frame` = context `+0x00`, `tile` = `-`; `x`, `y` = the call's X, Y
   arguments; `w h xoff yoff` = that sprite's row of `sprites.tsv` (§4;
   `?` when it has none); for `CelDraw` (arguments context, X, Y, light,
   mode, palette; the cursor vector of `capture.md` §Test vectors)
   `mode` = the draw-mode argument (`blend-modes.md`), `light` = the
   light argument as hex, `pal` = `0` when the palette argument is null,
   else `?` (a pointer; which table it is: Open question 2). The other cel
   wrappers' argument positions are not specified: `mode light pal` = `?`.
4. Tile ops: `file` = the DT1 path and `frame` = the tile's index in it
   (`capture.md` §3.5 lookup; `?` until the recorder resolves it, Open
   question 3), `dir` = `-`, `tile` = `orientation.main.sub.rarity` of
   the tile header, `x`, `y` = the X, Y arguments, `w h xoff yoff` = `-`,
   `mode` = the alpha argument for `TileDrawTrans` else `-`, `light` =
   the light digest the recorder logged (16 hex digits, no `0x`), `pal` =
   `-`.
5. `unit`: `unit` = `type:guid` of the unit (decimal), `x`, `y` = the
   first two stack arguments (client x, y), `light` = EDX as hex; every
   other column `-`.
6. Primitives (`Util*`, `DrawBox*`, `DrawLine`, `StartDraw`,
   `ClearScreen`): `x`, `y` = the rectangle's left, top (`Util*`) or
   the first two arguments (`ClearScreen` has one: `y` = `-`); `mode` =
   the color argument (`Util*`: the second; `DrawLine`, `DrawBox`: the
   fifth, `blend-modes.md` §8 r1–r2; `DrawBoxAlpha`: not specified, `?`;
   `StartDraw`, `ClearScreen`: `-`); every other column `-`.
7. `at`: the call site as hex (1.14d) or `-` (d2rs). Information only:
   never compared.

### 3. `frame.tsv`

Columns: `key value`. One row per key, in this order:

<!-- rows -->
| Key | 1.14d source (`frames-raw-2` frame record) | Role |
|---|---|---|
| `seq` | `seq` | info |
| `tick` | `f` | input |
| `w` | `w` | input |
| `h` | `h` | input |
| `act` | `level.act` | input |
| `level` | `level.level_id` | input |
| `player_x` | `player` path client x | input |
| `player_y` | `player` path client y | input |
| `tile_origin_x` | `tile_origin[0]` | input |
| `tile_origin_y` | `tile_origin[1]` | input |
| `unit_origin_x` | `unit_origin[0]` | input |
| `unit_origin_y` | `unit_origin[1]` | input |
| `open_mode` | `open_mode` | input |
| `shift_x` | `shift_x` | input |
| `light_quality` | `light.quality` | input |
| `rain` | `weather.rain` | input |
| `snow` | `weather.snow` | input |
| `draws` | number of rows of `draws.tsv` | output |
| `index_sha256` | `index_sha256` | output |
| `palette_sha256` | `palette_sha256` | output |

1. Every key is present exactly once, in the table's order; a missing,
   repeated or unknown key is an error.
2. `index_sha256` and `palette_sha256` are `capture.md` §6's hashes
   (index bytes row-major, top row first; 768 bytes R, G, B).

### 4. `sprites.tsv`

Columns: `file dir frame w h xoff yoff`. One row per distinct (`file`,
`dir`, `frame`) drawn by a cel op, sorted by those three (path bytewise,
then numbers), no duplicates.

1. `w`, `h`: the decoded cel's width and height; `xoff`, `yoff`: its
   offsets as 1.14d's cel holds them (1.14d: the `hdr` the recorder read
   at the rasterizer, `capture.md` §3.5 raw-3; a cel op without one adds
   no row) (`sprite-placement.md` §3, §8): for
   a bottom-up cel (DCC; DC6 with `flip & 1 = 0`) `yoff` names the
   bottom row, for a top-down DC6 cel the top row.
2. Two producers' rows for the same key must be equal; the 1.14d file
   merges every scene, so a key present twice with different values is
   an error of the producer.

### 5. d2rs export

The exporter reads the world view's built frame after every pass has
joined it (map, units, UI, ground items, missiles, automap) and before
composition, through `d2-client` only (game logic untouched).

1. Rows come from the sorted `DrawItem` list. Consecutive items with the
   same tag, frame and position (the per-block draws of one tile,
   `client/render-pipeline.md` §A3) are one row. Before the first item of
   each run of items tagged with the same unit, one `unit` row.
2. A frame of a DT1 part is a tile op; of a direction part a cel op;
   `file` / `dir` / `frame` come from the frame store's owner of the
   item's frame id.
3. PROVISIONAL: `op` of a tile item is `FloorTileDraw` in the floor and
   roof passes, `ShadowTileDraw` in the shadow pass, `TileDrawTrans`
   when its blend reads the table transposed, else `TileDrawLit`; of a
   cel item `CelDrawShadow` in the shadow pass, else `CelDraw` (because
   d2rs draw items do not keep the wrapper; settled by REC-295, the
   first compared scene).
4. Cel `x`, `y`: the inverse of `sprite-placement.md` §8: `X = left −
   x_off`, `Y = top − yoff + h − 1` for a bottom-up cel, `Y = top − yoff`
   for a top-down cel. The exporter checks that placing the cel at
   (X, Y) gives the item's top-left again; if not (a clipped top-down
   cel), `x`, `y` are `?`.
5. PROVISIONAL: tile `x`, `y` are the handed position of `camera.md` §6:
   the block origin for walls; for floors and roofs the block origin
   `+ 80 − view.left` (because the wrapper's X, Y are taken to be the
   values before the drawer's own −80 and panel shift; settled by
   REC-296).
6. `w h xoff yoff` of a cel row and its `sprites.tsv` row: the frame
   store's frame, with `yoff` converted to §4 r1's meaning (a DCC frame
   box keeps its top row: `yoff = y_off + h − 1`).
7. Columns d2rs does not measure are `?`: the cel `mode`, `light`, `pal`
   (d2rs keeps a shade chain and blend op, not the call's arguments:
   REC-297), the tile `tile`, `mode` and `light`, the unit `light`, and in
   `frame.tsv` `player_x`, `player_y`, `light_quality`, `rain`, `snow`.
8. `frame.tsv` `tick` is the bridge's server tick count; `index_sha256`
   is the CPU reference composition of the frame onto the persistent
   framebuffer (`composition.md` §3), the frame d2rs presents.
9. The frame cycle's own calls frame the item rows, as in 1.14d's draw
   log (`a1-town-arrival-ama` row 0: `StartDraw 1 0`, at `0x0044CAD5`;
   a `ClearScreen 0` row after every draw, cursor included, on frames
   whose post-draw counter is above 0): first a `StartDraw` row with
   `x` = BlankScreen of the player's level (`bClear`,
   `render/composition.md` §3 step 2) and `y` = 0; last, when the frame's
   plan clears after drawing (§3 step 4), a `ClearScreen` row with `x` =
   0 (its one argument; `y` = `-`). Every other column `-`.
10. Pass 9's calls (`draw-order-2.md` §11.7: the particle lines or the
   lightning flash; `WorldFrame::sky`) are one row each, in call order,
   in place of the 1×1 line-pixel and flash items d2rs composes them
   with (those items write no row): `DrawLine` for a line, `DrawBox` for
   the flash (`DrawRectangle` `0x004F6300`), `x`, `y` = x0, y0 and `mode`
   = the color (`blend-modes.md` §8 r1–r2), every other column `-`. The
   rows stand where the first such item is, else (every pixel
   off-screen) before the first item of a later pass, else at the end.
   Other primitives (`DrawBox`, `DrawBoxAlpha`, `Util*`) are written only
   for primitives d2rs draws; it draws no other yet (the 1.14d arrival
   scene also has the stamina bar box `0x0046EFE9` of
   `ui/control-panel.md` §4 and a hover-label box `0x005031C1`), so those
   rows are real divergences, not export gaps.

12. A tile drawer call that puts no pixel in the frame is a row like any
   other: a floor or roof that passes the whole-tile test of
   `camera.md` §7 whose blocks all lie outside the frame (1.14d's
   `a1-town-arrival-ama` log has floors handed at X = −80, a whole
   off-frame column), and a wall whose drawer culls every block. d2rs
   keeps such a tile as a draw with an empty clip
   (`TileDraw::is_call_only`, `WorldFrame::calls`), never composed; the
   export merges them with the items by draw key.
11. `play --input SCRIPT` brings a d2rs scene where a 1.14d scene's
   `autostart.py --input` brought it: steps separated by `;`, `wait N`
   (N server ticks; autostart's `wait` counts seconds), `move X Y`,
   `click X Y`, `rclick X Y` (800 × 600 frame pixels; a click is cursor,
   press, and the release on the next tick). The steps are the window's
   pointer events (the window's own pointer is ignored while a script
   runs), delivered once per new server tick. Scenes match by place, not
   by timing: a script waits until the walk is over before the dumped
   tick.

### 6. Comparison

`d2-client facts-compare <original dir> <d2rs dir> [--ignore COL,...]`.
The original's `sprites.tsv` is `<original dir>/sprites.tsv`, else
`<original dir>/../../sprites.tsv` (`facts/render/sprites.tsv`).

1. Order, first difference wins: (a) the `frame.tsv` input keys in table
   order; (b) every d2rs `sprites.tsv` row against the original's row of
   the same key; (c) `draws.tsv` row by row, columns left to right; (d)
   the `frame.tsv` output keys.
2. Draw lists: rows are matched by position. A difference is the first
   row and column where both values are measured and differ, or the
   first row present on one side only (the other list ended).
3. A cell with `?` on either side is not compared and counted as
   unmeasured; so is a d2rs sprite whose key the original lacks, and a
   missing `sprites.tsv`. Info columns (`at`, `seq`) and `--ignore`d
   columns or keys are not compared and not counted.
4. Outcome and exit code: 0 match (no difference, nothing unmeasured);
   1 diverged (prints the stage, the row number, the column, and both
   full rows); 2 partial (no difference, some cells unmeasured: prints
   the counts per column); 3 error (a file missing or malformed: prints
   which and why).

### 7. Requests

`facts/requests.tsv` lists captures the existing facts do not answer.

1. Line 1 is `# requests v1; …`, line 2 the columns `scene`, `what to
   capture`, `asked by`, `status`; one tab-separated row per request.
2. `scene` is the scene directory name the facts will go in
   (`facts/render/scenes/<scene>/`); `what to capture` names the
   character, seed, input script or place and the frame; `asked by` the
   session or branch; `status` is `open` when added and `done <commit>`
   when PC 1 pushed the facts (`docs/handoff/pc1-data.md` step 3).

## Constants & data dependencies

Format version `v1` (header line). Column lists of §2–§4.

## Randomness

None.

## Edge cases & original bugs

- A frame captured without a draw log has no `draws.tsv`: not a fact set.
- The 1.14d draw log includes UI and cursor draws; d2rs's preview does
  not draw the cursor, so a scene with the cursor shown diverges at its
  row. Captures for compare keep the cursor off the window or accept that
  row as a known divergence.

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| identical hand-made fact sets | exit 0 | format test |
| one draw cell changed | exit 1, that row and column | format test (M08) |
| a `?` cell, otherwise equal | exit 2 | format test |
| a column row in the wrong order | exit 3 | format test |

## Provenance

Columns derived from what `record_frames.py` 0.2.0 logs (`capture.md`
§3.5, §5) and from d2rs's `DrawItem` and frame store. Our own format.

## Open questions

1. Which 1.14d frame (and server tick) matches d2rs's `--at-tick N` for
   a scene: tick numbering of the bridge vs `f` (`game +0xA8 + 1`). The
   first compared scene settles it (REC-298); `--ignore tick` compares
   the rest meanwhile.
2. The cel palette argument: which remap table each pointer is
   (recorder: name the PL2 / palshift table by address range).
3. ~~Tile file and index~~: answered, `frames-raw-3` (`capture.md` §3.5).
4. ~~The 1.14d cel's width, height and offsets~~: answered (REC-299), the
   rasterizer hook of `capture.md` §3.5 raw-3; checked against 12 DC6
   frame headers of the real files (all equal).
5. `DrawBoxAlpha`'s color argument (`0x004F6340`): not specified; its
   `mode` is `?`. (`DrawLine` and `DrawBox`: `blend-modes.md` §8.)
