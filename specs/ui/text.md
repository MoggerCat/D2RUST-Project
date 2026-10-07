# Spec: UI — Text rendering (fonts, layout, color codes, wrap)

- **Status:** draft (2026-10-06, RE on 1.14d `Game.exe` and the 1.14d font
  and palette files; no capture yet). Metrics in the test vectors are
  measured on the 1.14d files; pixel results are unverified until the
  capture cases of §Test vectors run.
- **Target version:** 1.14d, English install (`data\local\use` = 0)
- **Crate/module:** `d2-client::ui::text` (`TextRules`, `TextOpts`,
  `GlyphLookup`), `d2-client::world_view::ui_bind` (`TextHooks`),
  `d2-data::strings` (lookup by id)
- **Related specs:** `client/ui.md` §A3 / §B3 (d2rs design this answers),
  `formats/font-tbl.md` (font `.tbl` layout), `formats/dc6.md`,
  `formats/tbl.md` (string table layout), `data/field-types.md` (string
  ids), `formats/palette.md` (PL2 layout), `render/sprite-placement.md`
  (cel pixels from a draw position), `render/composition.md` §5 (one pixel
  write), `render/blend-modes.md` (draw modes; to write),
  `client/assets.md` §B (archive order), `text-fonts.tsv` (font table,
  this spec)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 48–60 |
| Inputs | 61–71 |
| Outputs / state changes | 72–75 |
| Rules | 76–77 |
|   1. Fonts and locale | 78–117 |
|   2. Strings: decoding and lookup by id | 118–135 |
|   3. Glyph lookup | 136–150 |
|   4. Glyph pixels | 151–188 |
|   5. Color codes | 189–224 |
|   6. Measuring | 225–244 |
|   7. The draw call | 245–270 |
|   8. Framed text (hover boxes) | 271–291 |
|   9. Variants of the draw call | 292–331 |
|   10. Word wrap | 332–369 |
|   11. Alignment | 370–377 |
|   12. Clipping (decision CG2) | 378–388 |
|   13. d2rs answers (hooks in `d2-client`) | 389–403 |
| Constants & data dependencies | 404–419 |
| Randomness | 420–423 |
| Edge cases & original bugs | 424–446 |
| Test vectors | 447–482 |
| Provenance | 483–513 |
| Open questions | 514–555 |
<!-- /index -->

## Summary

All UI text in 1.14d is drawn by D2Win: a current font (one of 14 DC6 +
`.tbl` pairs) is selected, then a UTF-16 string is drawn glyph by glyph at
a pen position. Each glyph is the DC6 frame its font record names, drawn
as an ordinary cel (`sprite-placement.md`) at the pen, recolored by a PL2
text-color map; the pen moves right by the record's advance. A line feed
moves the pen **up** one line step. `ÿc` codes switch the color. There is
no pixel clip besides the frame. This spec owns: font files and selection,
string decoding and lookup by id, glyph lookup, the text pixel write, color
codes, the measuring functions, the draw call and its variants, framed
hover text, word wrap, and the clipping answer (decision CG2).

## Inputs

| Name | Type | Source |
|---|---|---|
| font id | 0–13 | caller (`text-fonts.tsv`) |
| text | UTF-16 code units, NUL-ended | string tables (§2) or built by the caller |
| pen | (x, y) screen pixels, y = bottom row of the glyph cells | caller (`ui/panels.md` etc.) |
| color | text color index (0 = none) | caller; changed by `ÿc` codes (§5) |
| fonts | `.tbl` + `.dc6` per font | `data\local\font\latin\` (§1) |
| text-color maps | 13 × 256 bytes | act `pal.pl2` (§4) |

## Outputs / state changes

Framebuffer pixels (one cel per drawn glyph); the D2Win current font.

## Rules

### 1. Fonts and locale

1. The language is the first byte of `data\local\use` (`0x00525150`,
   cached; ≥ 14 reads as 0). English 1.14d: 0.
2. The font directory and lookup kind come from the language
   (`0x00502C60`, run once at D2Win start):

   | Language | Directory | Glyph lookup (§3) |
   |---|---|---|
   | 0–5, 12 | `Latin\` | by position |
   | 6 | `JPN\` | by code |
   | 7 | `KOR\` | by code |
   | 8, 9 | `CHI\` | by code |
   | 10 | `LATIN2\` | by code |
   | 11 | `CYR\` | by code |
   | 13 | `Latin\` | by code |

   With language 12 the chat font (13) uses `KOR\` and the lookup by code;
   other fonts use by position (`0x00502EF0`). Only `Latin\` is in the
   1.14d archives.
3. Font `n` is the pair `DATA\LOCAL\Font\<dir><name>.tbl` and the DC6 at
   the same path (`0x00501490` / `0x005013A0`, `D2Win_LoadCelFile`
   `0x004FA9B0` adds the extension), `<name>` from the 14-entry table at
   `0x006DC980`. The table, files, archives and measured metrics are
   `text-fonts.tsv` (§Constants). `Patch_D2.mpq` holds no font file (hash
   probe of all 28 names); `fontformal10.dc6` and
   `reallythelastsucker.dc6` are read from `d2exp.mpq` (the `d2data`
   copies differ), everything else from `d2data.mpq` (archive order:
   `client/assets.md` §B1).
4. Only these 14 names are ever opened by the font path, so
   `DEFAULT.TBL`, `FONTER.TBL`, `DEFAULT.MAP`, `README.TXT` and the
   `font*.txt` files in the same directory are not font inputs (answers
   `tbl.md` OQ2 for the font path).
5. `SetFont(n)` (`D2Win_SetUnicodeTextFont` `0x00502EF0`, n in ECX)
   makes `n` current and returns the previous id. Loading and unloading
   (fonts unused for more than 240 × 1,024 ms are freed, checked at most
   every 600 × 1,024 ms; font 1 is loaded at start) change no pixel.
6. Header field `height` (`.tbl` byte 10) is the only header field the
   text path reads; header `width` (byte 11) is never read.

### 2. Strings: decoding and lookup by id

1. String-table values (`formats/tbl.md`) are decoded as **UTF-8** into
   UTF-16 when the tables load (`0x005259C0` → `0x00526320` →
   `0x00526100`). An invalid UTF-8 sequence ends the string at that point.
   All 9,078 values of the English `string.tbl`, `expansionstring.tbl` and
   `patchstring.tbl` (d2data / d2exp copies) are valid UTF-8 and decode to
   code points ≤ U+00FF; the color-code lead `ÿ` is stored as `C3 BF`.
2. Lookup by id (`D2Lang_GetStringByIndex` `0x00524A30`), ids as assigned
   in `data/field-types.md`:
   1. id ≥ 20,000: if `expansionstring.tbl` is loaded, element
      `id − 20,000` of it; else the id becomes 11,078 and lookup goes on.
   2. id ≥ 10,000 and `patchstring.tbl` loaded: element `id − 10,000`.
   3. Otherwise element `id` of `string.tbl`.
   4. Within a table (`0x00524930`): an element number ≥ `num_elements`
      reads element 500; an element whose slot is out of range, outside the
      file, unused or without a string is a fatal error (process exit).

### 3. Glyph lookup

The record for code unit `c` in the current font:

- **by position** (Latin, `0x00501650`): `c ≤ 0xFF` → record number `c`;
  `c > 0xFF` → record 0. The record's `code` field is not read.
- **by code** (other locales, `0x00501690`): binary search over the records
  by `code`, record count = `.tbl` header u16 at byte 8, at most 1,000
  probes; not found → record 31.

In all 14 Latin fonts record `i` has `code = i` and `frame = i` (256
records each), so both lookups agree for `c ≤ 0xFF`. Of a record the text
path reads only `width` (byte 3, the advance) and the frame as a **u16 at
byte 8** (`formats/font-tbl.md`).

### 4. Glyph pixels

1. A glyph is drawn with the driver's colored cel draw (`0x004F64B0` →
   slot `+0x88`; GDI `0x006C85A0`) at the pen (x, y): DC6 frame = the
   record's frame, direction 0, light byte 0xFF, draw mode 5, palette
   argument = the current color `k`.
2. Placement is `sprite-placement.md` §2. Every frame of the 14 fonts has
   `offset_x = offset_y = 0`, `flip = 0` and one size per font
   (`text-fonts.tsv` `frame_w`, `frame_h`), so a glyph covers columns
   `x … x + frame_w − 1`, rows `y − frame_h + 1 … y`: **the pen y is the
   bottom row of the glyph cell.** Frames are wider than the advance; a
   later glyph overwrites an earlier one where both have pixels.
3. Pixel write (`composition.md` §5): light none (byte 0xFF), blend table
   none (mode 5), remap `P` = text-color map `k` if `k ≠ 0`, none if
   `k = 0` (`0x006C8647`: pointer at palette-table block `+0xD0 + 4k`).
   So a glyph source index `s` writes `s` (color 0) or `map_k[s]`.
   Transparency is the DC6 encoding only; an opaque pixel remapped to
   index 0 is written as index 0.
4. Text-color map `k` (`0x004FB010`, `0x004FB1E0`) is the `k`-th 256-byte
   map of the PL2 "text color shifts" (`palette.md`), file offset
   `439,847 + 256k` of the act `pal.pl2` loaded for the frame
   (`composition.md` §4). The loader always copies 13 maps (3,328 bytes
   from offset `0x6B627`). Map 0 is never used for drawing (all zero in
   every 1.14d PL2). The 13 RGB triples before the maps (offset `0x6B600`)
   are copied to `0x007D563C` and not used by glyph drawing.
5. A `k < 0` (§5 r1 keeps it; units 0x00–0x2F give `k` = −48 … −1)
   reads pointer `+0xD0 + 4k` of the 0x48-pointer block
   (`0x004FB010`; GDI copies the block to `0x0098A040`, `0x006C8000`),
   which stays inside the block: `k` = −1 → the selected-unit shift map
   (`+0xCC`); −2 … −17 → inventory color variations 15 … 0
   (`+0xC8` … `+0x8C`); −18 … −48 → light maps 31 … 1 (`+0x88` …
   `+0x10`) (`render/shading.md` §1). A code's `k ≥ 13` never reaches
   the block (§5 r1 sets 0); a caller's `k` (§7) is not checked: 13 →
   additive blend `+0x104`, 14 → `+0x108`, 15 → `+0x10C`, 16 → darkened
   shift `+0x110`, 17 → the text RGB triples `+0x114`, 18 → `H`, 19 →
   `R` (each used as a 256-byte map: its first 256 bytes); `k` ≥ 20
   reads past the block (GDI: driver globals from `0x0098A160`).

### 5. Color codes

In the draw call (§7) and the mode and horizontal-window variants (§9) a
unit `0xFF` is a color code when the next unit is
`c` (U+0063, case-sensitive; `0x00526540` with `0x008426E0` = "c") **or**
the string ends right after the `ÿ`:

1. If a unit follows the `c`: `k = unit − 0x30`; if `k ≥ 13`, `k = 0`.
   Three units are consumed, nothing is drawn. `k < 0` is kept (§Edge
   cases).
2. If the string ends after `ÿc` (or after `ÿ`): `k = 0`, drawing ends.
3. `ÿ` followed by anything else is an ordinary glyph (record 255); drawing
   goes on with the following unit.
4. The color stays in force across line feeds until the next code or the
   end of the call; each call starts with the caller's color.

| Code | `k` | Act 1 PL2 text color (RGB, informative) |
|---|---|---|
| (none) / `ÿc0` | 0 | no remap (glyph colors as drawn) |
| `ÿc1` | 1 | 255, 77, 77 |
| `ÿc2` | 2 | 0, 255, 0 |
| `ÿc3` | 3 | 105, 105, 255 |
| `ÿc4` | 4 | 199, 179, 119 |
| `ÿc5` | 5 | 105, 105, 105 |
| `ÿc6` | 6 | 0, 0, 0 (map all 0) |
| `ÿc7` | 7 | 208, 194, 125 |
| `ÿc8` | 8 | 255, 168, 0 |
| `ÿc9` | 9 | 255, 255, 100 |
| `ÿc:` | 10 | 0, 128, 0 |
| `ÿc;` | 11 | 174, 0, 255 |
| `ÿc<` | 12 | 0, 200, 0 |
| `ÿc=` and above | 0 | — |

The English string tables use `ÿc0`–`ÿc5`, `ÿc7`, `ÿc9` and one `ÿc` at
a string's end (measured).

### 6. Measuring

`adv(c)` = `width` of the §3 record of `c`. Each measure has its own
treatment of codes (reproduce each as is):

| Measure | Address | Range | `LF` | `ÿ` |
|---|---|---|---|---|
| width A | `0x00501820` (= B over the whole string) | whole string | 0 | counted as a glyph, and so are the two units after it |
| width B (n units) | `0x005017D0` | first `n` units, stopping at a NUL unit (the NUL adds nothing) | 0 (skipped, not record 10) | counted as glyphs |
| width C (n units) | `0x00501730` | first `n` units, stopping at a NUL unit (so `n` past the end adds nothing) | 0 (skipped) | `ÿ`,`c`,`0`–`6` skipped (0) only if the code starts at index `i` with `i + 3 < n`; every other `ÿ` counted as a glyph |
| line width | `0x00501910` | from a start to the first `LF` or the end (index < length) | ends the line | `ÿ` and the next two units: 0, plus `adv('m')` if the unit after `ÿ` is `m` or `M`; the two units are passed over unread, so an `LF` among them does not end the line, and a skip past the end ends the walk |
| max width | `0x00501840` | whole string, per `LF` line (index < length; also stops at a NUL unit) | splits lines | as line width (an `LF` among the two skipped units splits nothing); result = the widest line |
| text height | `0x005019C0` | whole string | counts lines | — |
| font height | `0x00501A40` | — | — | returns header `height` |

Text height = `trunc(height × 16 × lines / 10)`, lines = 1 + number of
`LF` (factor 16 from `0x0072E004`, English). Line step = `trunc(height ×
16 / 10)` (`0x0072E000` = 16): `text-fonts.tsv` `line_step`. Other
languages: §Constants.

### 7. The draw call

`DrawText(text, x, y, k, centered)` (`D2Win_DrawUnicodeText`
`0x00502320`: text ECX, x EDX, then y, k, centered on the stack) →
`0x00501A80`:

1. `W` = max width + 8 if `centered`, else 0.
2. Pen = (x, y); if `centered`, pen x = `x + ((W − line width of the
   first line) >> 1)` (arithmetic shift).
3. For each unit, in order, up to the string length (`0x00526800`):
   - `ÿ` → §5;
   - `LF` (U+000A) → pen x = `x`, or `x + ((W − line width of the next
     line) >> 1)` if `centered`; pen y −= line step (**lines go up**);
   - any other unit → draw its glyph at the pen (§4), pen x += `adv`.
4. Nothing else moves the pen: no kerning, no tab handling, no spacing.

Because line feeds move up, a multi-line string's first line is the
bottom one; callers that want top-down reading build the string last line
first. Centering is per line inside a block of width `max width + 8`
whose left edge is `x`: a one-line string is drawn at `x + 4`.

`D2Client_DrawCenteredUnicodeText` (`0x004A7080`: x1 ECX, y EDX, then x2,
text, k): span = `x2 − x1 + 1`; if width A < span, x = `x1 + ((span −
width A) >> 1)`, else x = `x1`; then `DrawText(text, x, y, k, 0)`.
Width A counts color-code units, so colored strings sit left of center.

### 8. Framed text (hover boxes)

`DrawFramedText(text, x, y, rect_color, rect_mode, k)` (`0x005023B0`,
text ECX, x EDX; 2 call sites in D2Client, `0x004A72EF`, `0x004C0C8E`):

1. `W` = max width + 8; `H` = text height; screen size (Sw, Sh) from the
   driver (`0x004F59B0`; 800 × 600, `camera.md` §1).
2. `x' = max(x, 0)`, then `x' = min(x', Sw − W)`.
3. `y' = y + 2`; bottom `b = max(y', H)`; if `max(y', H) ≥ Sh − 31`,
   `b = Sh − 31`.
4. Rectangle `(x', b − H)`–`(x' + W, b)` with `rect_color`, `rect_mode`
   (`D2GFX_DrawRectangle` `0x004F6300`; its pixels are
   `blend-modes.md`).
5. Text: `0x00501A80` with x = `x'`, y = `b − trunc(3 × height / 10)`,
   block width `W`, color `k`, centered.

Variant `0x00502480` (1 call site, D2Win `0x00500106`): `W` = max width
(no + 8); if `x + W > Sw`, `x = Sw − W`; `y' = min(y + 2, Sh − 31)`; if
`y' − height < 0`, `y' = 2y'`; rectangle `(x, y' − height)`–`(x + W, y')`;
text at y `y' − 2`, block width `W`, centered.

### 9. Variants of the draw call

| Call | Address | Differs from §7 |
|---|---|---|
| draw with mode | `0x00502360` → `0x00501C30` | draw mode is a 6th argument instead of 5 |
| horizontal window | `0x00501FE0` (text ECX, x EDX, y, k, s, w; caller: text-box control `0x004FBF30`) | pen starts at `x + s`; before **every** unit (glyph, `ÿ` code or `LF`) the call stops when pen x > `x + w`; a glyph (and a `ÿ` not followed by `c`, drawn as record 255) is drawn only if pen x > `x` before it (the pen still advances); `ÿc` + unit sets `k` as §5 (`k` > 12 → 0, negative kept), `ÿc` at the end sets `k` 0 and ends; `LF` resets pen x to `x` (not `x + s`) and moves y up by `trunc(16 · h / 10)` (`h` = the font's line-height byte, as §9 details r3) |
| vertical window | `0x00501DF0` (text ECX, x EDX, y, unused, skip, lines; callers in `0x0049D5A0`) | returns at once if `skip < 0`; `ÿc` + 1 unit skipped (no color: glyphs drawn with `0x004F64E0`, draw mode 5, no remap), `ÿ` + any other unit except `m` skips those 2 units; per glyph: if `skip + lines ≤` the cel height, rows `skip`, `lines`, else rows 0, cel height; nothing drawn on display type 6; `ÿm`: details below |
| no color | `0x00502190` (via `0x005023A0`) | every `ÿ` skips 3 units; line step `trunc(−15 × height / 10)`; color 0. Dead code: nothing in `Game.exe` calls, jumps to or holds the address of `0x005023A0` (no rel32, absolute or RVA reference; not among the 24 exports), so d2rs needs no counterpart |

**Vertical window details** (`0x00501DF0`):

1. The fourth argument (`[ebp+0xC]`) is never read.
2. Row window (`CelDrawEx` `0x004F64E0` → driver slot `+0x8C`, GDI
   `0x006C86A0` → `0x00601650`): `lines` = 0 draws nothing. Otherwise
   the same pre-test as the plain cel draw (`render/sprite-placement.md`
   §5), then, with `B = Y + yoff` the cel's bottom screen row
   (`sprite-placement.md` §2) and `H` the surface height: `top = B −
   skip − lines`, `bot = B − skip`; nothing when `bot < 0` or `top ≥ H`;
   `top := max(top, 0)`, `bot := min(bot, H − 1)`; the screen rows `top
   + 1 … bot` are drawn, i.e. the encoded rows `skip … skip + lines − 1`
   counted from the bottom row, cut to the screen. When the window is
   cut at the top, screen row 0 is never drawn (the `top` bound is
   exclusive; reproduce). No light, no remap; the blend table of the
   mode (5: none).
3. `ÿm`: the unit after `m` is looked up as a glyph record of the
   current font (`[0x00841DA0]`); its frame (`+0x08`) of the
   MonsterIndicators cel file (`[0x00841DA4]`) is drawn whole with the
   plain cel draw (`0x004F6480`, light 0xFF, mode 5, no palette) at the
   pen; the pen does not move in x, and y += `trunc(16 · h / 10)` with
   `h` = the font's line-height byte (`+0x0A` of the font record
   `0x00841DB0 + 20 · [0x0072DFFC]`): one line **down**. `LF` resets x
   to the start and moves y by `trunc(−16 · h / 10)` (up), as §7.
4. Caller: `0x0049D5A0` (from `0x004A0770`), a scrolled text panel at
   (`[0x007BF278]` + 16, `[0x007225FC]` + …) whose scroll position is in
   1/1024 pixel and whose lines are 18 pixels apart (0x4800): full lines
   use `DrawText`, the partly visible top line `0x00501DF0(…, skip 0,
   lines = min(visible, 18))` and the bottom ones `0x00501DF0(…, skip,
   lines)`. Which panel this is: the panels owner (`ui/panels*.md`).


### 10. Word wrap

`Wrap(text, M)` (`0x00502970`, text ECX, out-count EDX, max width `M`; 13
call sites) splits a string into lines; widths are width C (§6) of the
candidate span `text[s..=e]` with `n = e − s + 1` units (so `n` may count
the NUL at the end). `L` = length.

1. If width C of the whole string (`n = L`) ≤ M: one line, the whole
   string.
2. Else from `s = 0`, repeat:
   1. `e = L`; while width C(`s`, `e`) > M and `e > s`: `e −= 1`.
   2. If `e > s` (it fits): unless `e = L`, move `e` down until a break is
      allowed after `e` (`0x00526D30`: unit `e + 1` is not white space and
      unit `e` is white space, white space = `< 0x100` and CRT `isspace`:
      0x09–0x0D, 0x20) or `e = s`.
   3. If `e = s` (no fit or no break): `e = L`, then `e −= 1` while width
      C(`s`, `e`) > M and `e > s` (a hard break inside a word; a single
      unit wider than M stays alone).
   4. The line is `text[s..=e]`, minus its first unit if that is white
      space (`0x005028E0`); trailing white space stays in the line.
   5. `s = e + 1`; stop when `s > L`.
3. `LF` is white space here and does not force a break; a line may hold a
   `LF` and is then drawn as two lines by §7.
4. **Empty lines.** Because width C stops at the NUL, a span ending at
   `e = L` measures the same as one ending at `L − 1` (or less, when a
   final `ÿc0`–`ÿc6` is skipped only with the NUL counted), so a fit
   (r2.1) or a break (r2.2) never ends a line at `L − 1`. An empty line
   (an output string of length 0) is produced only when (a) the rest of
   the string is one white-space unit (r2.4 drops it: e.g. `ab ` with
   `M` = width of `ab` gives `ab`, then ``), or (b) a hard break (r2.3)
   leaves `s = L` — the last unit alone is wider than `M` — and the next
   round copies just the NUL. The caller receives the empty line in the
   count and the list (`0x00502970` builds every line before returning).

Languages 6, 8, 9 use a different break test (break before a CJK unit
unless it is one of 11 listed punctuation units or Latin / full-width
alphanumerics); not in scope for English.

### 11. Alignment

The original has no alignment argument: left = the pen at `x`; center =
§7 `centered` (block of max width + 8), `0x004A7080` (span, width A) or
the text-box control's own arithmetic (`0x004FBF30`, flags `0x02` center
/ `0x10` right, widths by width A; owner `ui/controls.md`, OQ 4). There is
no right alignment in the draw call.

### 12. Clipping (decision CG2)

No text call takes a clip rectangle. Glyph pixels are clipped only like any
cel: rows to `[0, H)`, columns to the driver's cel column clip `[0, W)`
(`sprite-placement.md` §5). The only restrictions are per glyph and not
pixel clips: the horizontal window (§9: a glyph is drawn whole or not at
all, so its pixels may pass `x + w`) and the vertical window (§9: rows
of each glyph). Hence `TextRequest.clip` for UI text is the frame
`[0, 800) × [0, 600)`, and `TextOpts` carries the call kind and its
arguments (§13), not a rectangle.

### 13. d2rs answers (hooks in `d2-client`)

| Hook | Answer |
|---|---|
| `ui::text::TextOpts` (CG2) | the §7–§9 arguments: `centered: bool`, block width (derived, §7), draw mode (default 5), window `Horizontal { s, w }` / `Vertical { skip, lines }` / none; no clip rect |
| `TextRules` (`NoTextRules`) | §5–§7 (and §9 for the window kinds) |
| `GlyphLookup::record` / `MissingGlyph` / `AmbiguousGlyph` | Latin lookup is by position (§3): record `c`, or record 0 for `c > 0xFF`; never an error, never ambiguous |
| `TextStyle.font` | font id 0–13 (`text-fonts.tsv`) |
| `TextStyle.color`, `TextHooks::glyph_look` | color index `k`; shade = PL2 text-color map `k` (none for 0), no light, blend none (draw mode 5) |
| `TextHooks::text_font` | `text-fonts.tsv` row `font` |
| `Label` (where text sits in the rect) | the caller's pen is the bottom row of the first-drawn line (§4.2); position per panel is `ui/panels.md` |
| `TextInput` caret | OQ 3 |
| `client/assets.md` §B2 (locale font directory) | §1: `latin` for English |
| `formats/tbl.md` OQ2 (`DEFAULT.TBL`, `FONTER.TBL`) | not opened by the font path (§1.4) |

## Constants & data dependencies

- `text-fonts.tsv` (code consumes it): one row per font id. Columns: `id`,
  `name` (as in `0x006DC980`), `tbl_path`, `dc6_path` (lowercase archive
  paths), `dc6_archive` (archive the DC6 is read from), `height` (`.tbl`
  byte 10), `line_step` (`trunc(height × 16 / 10)`), `frame_w`, `frame_h`
  (size of every DC6 frame of the font). All 14 `.tbl`: version 1, header
  bytes 6–7 = 0, bytes 8–9 = 256, 256 records with `code = frame = index`,
  `unknown1 = 0`, `unknown2 = 1`, `unknown3 = 0`, `unknown4 = 0`,
  `unknown5 = 0`; record `height` = header `height`.
- Line factors (tenths of `height`; draw step `0x0072E000` / text height
  `0x0072E004`, set by `0x00502C60`): languages 0–5 16 / 16; JPN 14 / 15;
  CHI 14 / 14; LATIN2 13 / 13; KOR, CYR, 12 and 13 16 / 17 (non-English
  rows out of scope).
- Text-color maps: `pal.pl2` offset 439,847 + 256k, k = 1–12.

## Randomness

None.

## Edge cases & original bugs

Reproduced by default.

- Width A / B count `ÿcN` as three glyphs; §7 centering ignores codes; wrap
  (width C) skips only `ÿc0`–`ÿc6` and only away from the measured span's
  end. Centered colored text, wrapped colored text and text-box alignment
  therefore differ from what the visible glyphs suggest.
- Line width and max width treat `ÿ` + any two units as zero width but
  add `adv('m')` for `ÿm` / `ÿM`, while the draw call draws `ÿ`
  followed by a non-`c` as glyphs: such strings are mis-centered.
- `ÿc` + a unit below `0` gives a negative `k`; the remap pointer is then
  read from palette-table block `+0xD0 + 4k` (another table, §4 r5).
  No English string does this.
- A caller color ≥ 13 is not range-checked (only codes are) (§4 r5).
- `ÿ` as the last unit is a color code (the NUL compares equal to `c`):
  color 0, nothing drawn.
- The `loading` palette PL2 has 12 text colors; the loader still copies 13
  maps, so map 12 holds bytes past the file's maps (OQ 5).
- Code units above 0xFF draw record 0 in Latin fonts.
- Framed variant `0x00502480` doubles `y'` when the box would start above
  row 0.

## Test vectors

Metrics from the 1.14d font files (`text-fonts.tsv`; probe of
2026-10-06). Positions are pen positions (bottom row of the cell).

| Input | Expected output | Source |
|---|---|---|
| Font16 `Stash` | advances 7, 9, 10, 7, 7; width A 40 | §6, font16.tbl |
| Font8 `ÿc4Gold` | width A 49; width C (n = 7) 28; line width 28; max width 28 (`Gold` = 28) | §6 |
| Font8 `ÿc9Rare` | width C (n = 7) 50; line width 29 | §6 (codes 7–12 not skipped by C) |
| Font8 `ÿmX` | line width 12 (= `adv('m')`) | §6 |
| Font16 `AB⏎C`, DrawText at (100, 200), centered | max width 19, W 27; `A` (104, 200), `B` (116, 200), `C` (109, 184); text height 32 | §7 |
| Font16 `Aÿc1B⏎C` at (10, 50), not centered | `A` (10, 50) k 0; `B` (22, 50) k 1; `C` (10, 34) k 1 | §5, §7 |
| Font16 `aÿxb` at (0, 20) | `a` 0, `ÿ` 10, `x` 20, `b` 30 (all y 20, k 0) | §5.3 |
| Font16 `A` at pen (104, 200) | pixels in columns 104–117, rows 185–200 | §4.2 |
| Font16 `AB⏎C`, DrawFramedText x 790, y 100, 800 × 600 | rectangle (773, 70)–(800, 102); `A` (777, 99), `B` (789, 99), `C` (782, 83) | §8 |
| FontFormal10 `The quick brown fox jumps over the lazy dog`, M 100 | `The quick brown ` (94), `fox jumps over ` (87), `the lazy dog` (68) | §10 |
| same, M 60 | `The quick `, `brown fox `, `jumps `, `over the `, `lazy dog` | §10 |
| FontFormal10 `Supercalifragilistic`, M 50 | `Supercal` (49), `ifragilisti` (50), `c` | §10.2.3 |
| synthetic font, every advance 5: `aa bb`, M 15 | `aa `, `bb` | §10 |
| synthetic, every advance 5: `ab  cd`, M 15 | `ab `, `cd` (no empty third line: width C(4, 6) = 10 counts no NUL) | §10 r4 |
| synthetic, every advance 5: `ab `, M 10 | `ab`, `` | §10 r4 (a) |
| synthetic, advance 5, `W` 20: `abW`, M 15 | `ab`, `W`, `` | §10 r4 (b) |
| synthetic, every advance 5: `ab cd`, M 10 | `ab`, `c`, `d` (the break after `ab ` leaves `e = s`: hard break `␣c` minus its first unit) | §10 r2.3 |
| synthetic, every advance 5: line width of `xÿ⏎yz` / max width | 5 + 0 + 5 = 10 (the `LF` inside the skip is passed over) / 10 | §6 |
| horizontal window, advance 5, x 0, s 0, w 7: `abc` | `a` not drawn (pen 0 is not > x = 0), `b` drawn at 5, stop before `c` (pen 10 > 7) | §9 |
| synthetic, every advance 5, height 10: `ab⏎cd` at (0, 100) | `a` (0, 100), `b` (5, 100), `c` (0, 84), `d` (5, 84) | §7 |
| `ÿc=` / `ÿc<` | k 0 / k 12 | §5 |
| Latin lookup of U+20AC / U+0041 | record 0 / record 65 | §3 |
| string id 20,000, expansion loaded / not loaded | `expansionstring` element 0 / `patchstring` element 1,078 | §2.2 |
| capture case `text-0001`: main menu (Font30 / FontExocet10 buttons) | CPU reference with this spec equals the captured index frame | capture, queued |
| capture case `text-0002`: an item hover box with a magic (blue) item and a multi-line description | same | capture, queued |
| row window: cel bottom row `B` = 100, skip 3, lines 10, `H` = 600 | screen rows 88 … 97 (encoded rows 3 … 12 from the bottom) | §9 details r2 |
| row window: `B` = 5, skip 0, lines 18 | `top` = −13 → 0, rows 1 … 5 only (row 0 lost) | §9 details r2 |
| `ÿc!` (`k` = −15) | remap = inventory color variation 2 (`+0x94`) | §4 r5 |

## Provenance

1.14d `Game.exe`, D2Win text code: font table `0x006DC980`, path builders
`0x005013A0`, `0x00501490`, init `0x00502C60` (language `0x00525150`,
`data\local\use`), `SetFont` `0x00502EF0`, `.tbl` load `0x00501580`,
lookups `0x00501650`, `0x00501690`, measures `0x00501730`, `0x005017D0`,
`0x00501820`, `0x00501840`, `0x00501910`, `0x005019C0`, `0x00501A40`,
draw `0x00502320` → `0x00501A80`, `0x00502360` → `0x00501C30`, windows
`0x00501FE0`, `0x00501DF0`, `0x00502190`, framed `0x005023B0`,
`0x00502480`, wrap `0x00502970` (`0x005028E0`, break test `0x00526D30`;
copy `0x00526790` = bounded copy from the unit after a leading white
space), implementation questions UT1–UT5 (2026-10-06): UT1 §10 r4, UT2
and UT3 §6 table, UT4 §9 horizontal window, UT5 §9 no color (dead code),
string compares `0x00526540`, `0x00526610` (case table `0x00730578`:
`m` and `M` both map to `M`), `"c"` / `"m"` built at `0x00502E9D`;
D2Client `0x004A7080`; GDI colored cel draw `0x006C85A0` (slot `+0x88`
= D2MOO `pfCelDrawColor`, 1.10f slot order confirmed by the `+0x84` /
`+0x88` bodies), palette-table block `0x004FB010`, PL2 copy
`0x004FB1E0`; D2Lang decode `0x005259C0`, `0x00526320`, `0x00526100`,
lookup `0x00524A30`, `0x00524930`. Names `D2Win_*` / `D2Client_*` /
`D2Lang_GetStringByIndex` from `refs/1.14d-notes`. Game files: the 14
font `.tbl` / `.dc6` (`d2data.mpq`, `d2exp.mpq`; `Patch_D2.mpq` checked
by hash lookup), the 17 PL2 files, the English string tables (`d2data`,
`d2exp`), measured with Python probes. No capture yet.
Ghidra backlog (2026-10-06): `0x00501DF0`, `0x004F64E0`, GDI slot
`+0x8C` `0x006C86A0` (vtable `0x0074C52C`) → `0x00601650`; caller
`0x0049D5A0` / `0x004A0770`; block builder `0x004FB010`, GDI copy
`0x006C8000`; `0x005023A0` reference search over the whole image
(rel32, absolute, RVA, export table); `DrawText` call-site scan of the
pushed `k` (214 sites).

## Open questions

1. ~~Vertical-window variant `0x00501DF0`~~: answered in §9 (row
   window, unused argument, `ÿm`, caller). Open only: which panel
   `0x0049D5A0` is (panels owner).
2. Palette-table slot of `k < 0` / `k ≥ 13`: answered in §4 r5. Partly
   open: of the `DrawText` call sites whose `k` is a pushed constant (66
   sites) all pass 0, 1, 3, 4, 6 or 9; the 148 sites passing a register
   or memory value need a per-site read (or a runtime trace) to exclude
   `k ≥ 13` or a negative `k`.
3. Caret and selection drawing of the D2Win edit box (callers of width B
   `0x005017D0` in `0x004FD…`–`0x004FF…`). Ghidra read; owner of the
   control may be `ui/controls.md`.
4. Text-box control `0x004FBF30` (alignment flags, marquee scroll −2 px
   per draw, selected row `0x005025C0`): belongs to the controls owner;
   listed so it is not lost.
5. Bytes of text-color map 12 under the `loading` palette (12 maps in the
   file): matters only if a loading-screen string uses `ÿc<`. Memory read
   of `0x007D6268` on that screen.
6. ~~Who reaches `0x005023A0` / `0x00502190`~~: nothing (§9: dead
   code).
7. Does any code outside D2Win read the record fields other than `width`
   and `frame` (e.g. `height`, `unknown*`)? Ghidra scan of uses of the
   record pointer returned by `[0x00841DA0]`.
   *Partly answered* (static, `all.asm`): the 20 loads of
   `[0x00841DA0]` are all inside D2Win, in 11 functions from
   `0x00501730` to `0x00502EF0` (`0x00501730`, `0x005017D0`,
   `0x00501840`, `0x00501910`, `0x00501A80`, `0x00501C30`, `0x00501DF0`,
   `0x00501FE0`, `0x00502190`, `0x00502C60`, `0x00502EF0`); no code
   outside D2Win loads the font pointer itself. Still open: whether one
   of these hands a record pointer to an outside caller.
8. CRT `isspace` assumes the "C" locale: confirm no `setlocale` call
   changes it (Ghidra xref of `setlocale`).
   *Answered* (static): nothing can change it. The statically linked
   CRT's locale category table (`LC_ALL` … names at `0x006F3120`–
   `0x006F315C`, table pointer at `0x006F3168`) has no code reference in
   `Game.exe`, and `___get_qualified_locale` (`0x0069FED4`) has no
   caller, so `setlocale` is never reached and `isspace` (`0x00687006`)
   runs in the start-up "C" locale.
9. Pixel proof: capture cases `text-0001` / `text-0002` (§Test vectors)
   against the CPU reference.
