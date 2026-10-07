# Spec: UI — Automap (cell data, reveal, layers, persistence, modes, drawing, markers)

- **Status:** draft (2026-10-07, RE on the 1.14d `Game.exe` and table
  data already measured by the owners linked below; no capture yet).
  Pixel results are unverified until the capture cases of §Test vectors
  run.
- **Target version:** 1.14d, English install
- **Crate/module:** `d2-client::ui::automap` (cell store, reveal, draw);
  the cell picker uses `d2-data` (`automap_runtime`,
  `automap_level_index`) and the client RNG
- **Related specs:** `client/ui.md` §B7 (this spec answers it);
  `data/runtime-maps.md` §10 (converted `automap.txt` records and
  level-name ranges), `data/loading.md` §8 (name lists A/B);
  `sim/rng.md` §3 (`roll`), §5.5 (automap seed); `drlg/rooms.md` §9.5
  (tile records, flags), `drlg/preset.md` §3.2 step 4 (automap
  callbacks), `drlg/levels.md` §1 (client DRLG callbacks);
  `client/model.md` S→C 0x03 (callbacks registered), §9 (0x07/0x08 room
  sight); `render/draw-order.md` §6 r6 (record flag 0x20000 "drawn"),
  r7 (reveal trigger, summary only), §1 (frame passes);
  `render/camera.md` §1–§4 (W, H, unit origin, `shiftX`, screen
  coordinates); `render/blend-modes.md` §1 (draw modes), §8 (lines);
  `ui/panels.md` §2 (`SetUIState`), §4 (open mode), §5 r3 (where the
  automap draws); `ui/controls.md` §3 (commands 7–11, 38, 45);
  `ui/control-panel.md` §9 (mini-panel Automap button); `ui/text.md`
  (fonts, `DrawText`); `client/msg-units.md` §3 r4 (unit placement),
  §8 (player roster); `sim/server-messages.tsv` 0x90;
  `data/fields.tsv` (`leveldefs` `Layer`/`LevelType`, `monstats`
  `interact`, `monstats2` `automapCel`, `objects` `AutoMap`).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 59–73 |
| Inputs | 74–87 |
| Outputs / state changes | 88–94 |
| Rules | 95–96 |
|   1. Cell store | 97–127 |
|   2. Cell picker (`0x0061FFF0`) | 128–143 |
|   3. Adding a tile record (`0x00457CF0`, room R, record T) | 144–156 |
|   4. Adding units (`0x00458DC0`, room R) | 157–175 |
|   5. Reveal | 176–207 |
|   6. Town art (`0x004591A0`, DRLG callback +0x488) | 208–226 |
|   7. Persistence (`.map`, `.ma0`–`.ma3`) | 227–262 |
|   8. State, keys and options | 263–298 |
|   9. View geometry | 299–321 |
|   10. Cell draw pass | 322–360 |
|   11. Unit markers (`0x0045AC90` → `0x0045A860`) | 361–402 |
|   12. Party roster markers (`0x0045AB60`) | 403–414 |
|   13. Header text | 415–431 |
|   14. Lifetime | 432–438 |
| Constants & data dependencies | 439–452 |
| Randomness | 453–460 |
| Edge cases & original bugs | 461–476 |
| Test vectors | 477–512 |
| Provenance | 513–533 |
| Open questions | 534–552 |
<!-- /index -->

## Summary

The automap is client-only presentation. Per automap **layer** (the
leveldefs `Layer` of a level; several levels may share one) the client
keeps four sorted cell trees: floors, walls, units (objects and
monsters with an automap cel), and town art. A wall or floor tile record
becomes a cell once a frame has drawn it and the player then moves far
enough (§5); preset levels with lvlprest `AutoMap` = 1 are added whole
when their rooms are built. A cell is a frame of an automap DC6 chosen
through `automap.txt` (§2). Only the current layer is in memory; leaving
it appends its new cells to a per-character `.ma0`–`.ma3` file (§7).
State UI 0x0A shows it full screen or as a mini map (§8, §9); the pass
draws cells in (y, x) order with a distance fade (§10), then unit and
party markers (§11, §12) and the header text (§13).

## Inputs

| Name | Type | Source |
|---|---|---|
| tile records of a room (floor, wall arrays) | record flags +0x14, DT1 entry +0x18, type +0x1C, room-relative x/y +0x08/+0x0C | `drlg/rooms.md` §9.5 |
| level of a room → leveldefs `Layer` (+0x08), `LevelType` (+0x34) | u32 | `data/fields.tsv` `leveldefs` |
| converted automap records, level-name ranges | 0x20-byte records; 36 pairs | `data/runtime-maps.md` §10 |
| automap seed | RNG `{lo, hi}` at `0x0096C8C8` | `sim/rng.md` §5.5 |
| local player client position, room, unit origin | ints | `client/model.md`, `render/camera.md` §3 |
| room unit lists (+0x74, next +0xE8), unit flags +0xC4 | — | `sim/units.md` §2 |
| player roster (party id +0x22, level +0x24, x/y +0x28/+0x2C) | — | `client/msg-units.md` §8, S→C 0x90 |
| options (registry `HKCU\…\Diablo II`) | DWORD values (§8) | settings |
| UI state 0x0A, screen open mode | — | `ui/panels.md` §2, §4 |

## Outputs / state changes

Cell trees and layer list (§1); tile record flag 0x40000, unit flag
0x20000000 (§3, §4); `.map` / `.ma0`–`.ma3` files (§7); registry values
(§8); automap seed steps (Randomness); pixels of the UI pass (§10–§13).
No C→S message; the server never sees the automap.

## Rules

### 1. Cell store

1. **Cell** (0x14 bytes, `0x00457C30` pool: blocks of 512 cells, 0x2800
   bytes + next pointer, zero-filled): +0x00 saved flag (1 = came from
   the file, §7), +0x04 i16 cel (frame number), +0x06 i16 x, +0x08 i16
   y (automap units = screen pixels / 10, §3), +0x0A balance, +0x0C
   left, +0x10 right.
2. **Layer** (0x1C bytes, `0x00458CF0`, list head `[0x007A5160]`,
   prepend): +0x00 layer id (leveldefs `Layer`), +0x04 town art kind
   (0 none, 1 Lut Gholein, 2 Pandemonium Fortress, 3 Harrogath; §6),
   +0x08 floor tree, +0x0C wall tree, +0x10 unit tree, +0x14 town tree,
   +0x18 next. The current layer is `[0x007A5164]`.
3. **Switch** (`0x00458D40`, layer id L): find L in the list or create
   it; if it is not the current layer: save the current layer (§7 r3),
   free every cell (`0x00458680`; the four roots := 0), current := L,
   load L's cells from the file (§7 r4). So only the current layer's
   cells are in memory.
4. **Order** (`0x00457B00`, an AVL insert): keys compare y, then x;
   at equal (x, y), let g(c) = group of cel c (table below, else −1):
   the new cell is a **duplicate and is not inserted** when g(new) = −1
   or g(new) = g(old); otherwise the order is by cel number. The
   allocated cell of a refused insert is not returned to the pool
   (counter `[0x007A515C]` still counts it).
5. **Groups** (`[0x007A3150]`, 2048 entries, all −1, then the 49
   (cel, group) pairs of table `0x00711258` at init, `0x0045A4C0`):
   0, 1, 2, 3 → 0; 6, 7, 8 → 1; 11, 12 → 2; 13, 14 → 3; 20, 38 → 4;
   21, 39 → 5; 46, 47, 48, 49 → 6; 51, 52, 53, 54 → 7; 60, 70 → 8;
   61, 71 → 9; 120, 169, 171 → 10; 121, 170, 172 → 11; 257, 258, 259
   → 12; 266, 267 → 13; 337, 338 → 14; 472–475 → 15; 520, 521, 522 →
   16; 533, 534 → 17.

### 2. Cell picker (`0x0061FFF0`)

Arguments: T = leveldefs `LevelType` of the room's level
(`0x00642750`), o = the DT1 tile's orientation, m = main index, s = sub
index (`drlg/rooms.md` §9.3 accessors). `LevelType` is the list-A index
of `data/loading.md` §8 and o the list-B index (`fl` = 0 … `fi` = 19).

1. T = 0, or T's range first (`data/runtime-maps.md` §10) is −1: −1.
   First ≠ −1 with end = −1 is fatal 0x53B (unreachable with the
   loader's ranges).
2. Scan records r = first … end − 1 in order; the first r with
   LevelName = T, TileName = o, (Style = 0xFF or Style = m) and
   (StartSequence = 0xFF or StartSequence ≤ s ≤ EndSequence) wins.
3. Winner: k = `roll(cel count)` on the automap seed (`sim/rng.md` §3;
   count 1 still draws), result Cel[k+1]. No winner: −1, no draw.

### 3. Adding a tile record (`0x00457CF0`, room R, record T)

1. T flag 0x40000 set → nothing. Else set 0x40000 (once per record,
   whether or not a cell results).
2. c := picker (§2) for T's tile; c = −1 → stop.
3. Allocate a cell; (wx, wy) = (R tile x + T.x, R tile y + T.y)
   (room +0x34/+0x38, record +0x08/+0x0C). x = ((wx − wy) · 80) / 10,
   y = ((wx + wy) · 40) / 10 (C division; exact: x = 8(wx − wy),
   y = 4(wx + wy)). If T's type ≥ 16 (lower walls `ld`, `rd`, `fd`,
   `fi`): y += 24.
4. x, y and c must each fit i16 (fatal 0x391, 0x392, 0x393). Insert
   into the tree given by the caller (§5 r2).

### 4. Adding units (`0x00458DC0`, room R)

Walk R's unit list (+0x74, next +0xE8). With A = `[0x007A51A0]` (no
writer in `Game.exe`: 0):

1. A = 0: a unit is processed only when its flags +0xC4 have bit 28
   (0x10000000) and not bit 29 (0x20000000); it then gets bit 29. (A ≠
   0: every unit, bit 29 set.)
2. Monster (type 1): cel = `monstats2` `automapCel` of the class's
   monstats2 row (`monstats` +0x18 index); added only when the chain
   resolves and the cel ≠ 0.
3. Object (type 2): cel = `objects` `AutoMap` (+0x1BC); added when ≠ 0,
   except: object 267 only in a level whose act (`0x006427F0`) is 2 or
   3; object 366 only in mode 2 (+0x10); object 402 only in level 74.
4. Cell (`0x00457E80`): (px, py) = the unit's client position
   (`0x00620650` / `0x006206B0`, `render/camera.md` §2 screen space);
   x = px / 10 + 1, y = py / 10 − 3 (C division); range fatal
   0x3C5–0x3C7; insert into the unit tree. Other unit types: nothing.

### 5. Reveal

1. **Per frame** (`0x00459020`, from the frame `0x0044C7EB`): if the
   countdown `[0x007A51A4]` ≠ 0, decrement it and stop. No local player
   → fatal 0x696. With the player's client position (x, y) and the last
   reveal position (x₀, y₀) (`[0x007A51FC]`, `[0x007A51F4]`, initially
   0), dx = |x₀ − x|, dy = |y₀ − y|, d = (2·max + min) / 2. If d ≥ 0x50:
   (x₀, y₀) := (x, y); P := the player's room (`0x004646A0`); none →
   stop; L := `Layer` of P's level; switch to L (§1 r3); for each room
   of P's near-room list (`0x00619790`) in order whose level has
   `Layer` = L: add the room (r2) with "all" = 0.
2. **Add a room** (`0x00458F40(room, all, layer)`): every floor record
   (array order, `0x00619660`) without flag 0x8 and with flag 0x20000
   (drawn, `render/draw-order.md` §6 r6) or all = 1 or A ≠ 0 → §3 into
   the floor tree; then every wall record (`0x006196A0`) likewise into
   the wall tree; then §4 into the unit tree.
3. **Whole preset levels** (`0x00459150`, DRLG callback +0x454,
   `drlg/preset.md` §3.2 step 4; levels 1 and 75 in 1.14d): remember
   the current layer (−1 if none), switch to the room's level's
   `Layer`, add the room with all = 1, then switch back when there was
   a previous layer.
4. **Countdown**: the unit placement `0x004654C0` sets it to 2
   (`0x00459140`, after the teleport step succeeds,
   `client/msg-units.md` §3 r4), so the first two frames after a
   placement reveal nothing. Every placed unit resets it, not only the
   local player: `0x00459140` is called for any unit that reaches the
   teleport step (units in a death mode, or with flag 0x10000, return
   before it), before the local-player check (`0x00465692`).
5. S→C 0x07 / 0x08 (room sight, `client/model.md` §9) add no cells
   themselves: a room's tiles only become cells after they were drawn
   and r1 runs.

### 6. Town art (`0x004591A0`, DRLG callback +0x488)

Called for levels 40, 103, 109 (`drlg/preset.md` §3.2 step 4) with
(level, picked file f, centre tile (cx, cy)). Remember the current
layer; switch to the level's `Layer`; set the layer's town kind; any
other level id is fatal 0x751. Then (X, Y) = ((cx − cy)·80 / 10,
(cx + cy)·40 / 10) and a grid of cells into the town tree, row-major,
cel id n counting up from n₀ for every grid slot (skipped slots too):

| Level | Kind | Cols × rows | Step (w, h) | First x | First y | n₀ | Skipped n |
|---|---|---|---|---|---|---|---|
| 40 | 1 | 5 × 4 | 160, 100 | X − 453 | Y − 119 | 0 if f = 1, else 20 | 0, 10, 15, 16, 19, 20, 25, 30, 35, 36, 39 (`0x006D6608`) |
| 103 | 2 | 2 × 2 | 136, 90 | X − 133 | Y − 40 | 0 | — |
| 109 | 3 | 3 × 2 | 180, 170 | X − 250 | Y − 15 | 0 | — |

Cell (col, row) is at (first x + w·col, first y + h·row), cel n,
saved flag 0. Range fatal 0x777–0x779. Then switch back when there was
a previous layer.

### 7. Persistence (`.map`, `.ma0`–`.ma3`)

The automap is not in the `.d2s` (`formats/d2s.md`); it lives in files
next to it, `<save dir>[<sub dir>\]<name>.map` and `<name>.ma<k>`
(`0x00457F40`; `<name>` `[0x007A05C4]`, sub dir `[0x007A0500]`, the
plain save dir when the sub-dir file cannot be opened).

1. **Index** (`.map`, 0x18 bytes): u32 version 0xC, u32 next slot, u32
   × 4 slot keys. Key = the first u32 of the act record `[0x007A0638]`
   (S→C 0x03 init seed u32@2, `client/model.md`). Open: a valid header
   (24 bytes read, version 0xC) whose slots hold the key → slot k. Else
   valid header: k := next, slot k := key, next := (next + 1) mod 4,
   `.ma<k>` deleted. Invalid: version 0xC, next 1, slot 0 := key, all
   four `.ma` files deleted, k = 0. The header is rewritten in both new
   cases. The open handle is `.ma<k>`.
2. **Data file** (`.ma<k>`): a 400-byte table of 100 u32 offsets indexed
   by layer id (0 = no records), then records: a 32-byte header of
   eight u32 (layer id, town kind, act record second u32, byte sizes of
   the floor, wall, unit, town blobs, link), then the four blobs. A
   blob is a list of (cel, x, y) i16 triples, 6 bytes each.
3. **Save** (`0x004584C0`, on a layer switch and at teardown, §14): only
   cells with saved flag 0 are written, each tree in in-order (§10 r3);
   a new record is appended and linked from the layer's chain (first
   record: the table entry; later: the previous record's link).
   PROVISIONAL: the link is the header's eighth u32 and the table entry
   holds the first record's offset; settled by automap-0003 (hex dump of
   a `.ma` file after two visits to one layer).
4. **Load** (`0x00458750`, after a switch): follow the layer's chain;
   a record whose layer id ≠ L stops the load and clears the chain at
   that point (`0x004586E0`). Each blob's triples become cells with
   saved flag 1 in the matching tree. A cel ≥ the cel count of the
   file that will draw it (`[0x007A5178]` for floors/walls/units, the
   kind's town file for the town blob) stops that blob and clears the
   chain. The unit blob is read only when the record's third u32 equals
   the current act record's second u32 (S→C 0x03 u32@8), else skipped.

### 8. State, keys and options

| Option (registry value) | Global | Default | Set by | Meaning |
|---|---|---|---|---|
| `AutoMapFade` | `[0x007A51A8]` (+ `[0x007A51AC]`) | 0 | F10 (cmd 9): value := (value + 1) mod 4; options menu (`0x0047D40C`, `0x0047D45B`) | fade v, §10 r4 |
| `AutoMap Centers` | `[0x007113E0]` | 1 | options menu (`0x0047D476`) | re-centre on close / clear screen |
| `AutoMap Party` | `[0x007113E4]` | 1 | F11 (cmd 10) toggle; menu | party markers, §11–§12 |
| `AutoMap Party Names` | `[0x007113E8]` | 1 | F12 (cmd 11) toggle; menu | names, §11–§12 |
| `AutoMap Left` | `[0x007A51E0]` (copies `[0x007A51E4]`, `[0x007A51E8]`) | 1 | V (cmd 45) toggle (`0x00457780`) | mini map side |

1. Init (`0x0045A4C0`, UI init `0x0045699A`) reads the five values;
   only `[0x007A51A8]` is set from `AutoMapFade` (`[0x007A51AC]` stays 0
   until the first write, §10 r4). Each setter writes the registry
   value and the global(s).
2. **Open/close**: UI state 0x0A (`ui/panels.md` §2): Tab or the middle
   button (cmd 7) toggles it, then if it is now closed calls the
   re-centre with force 0; the mini-panel Automap button does the same
   state (`ui/control-panel.md` §9). F9 (cmd 8): re-centre with force 1.
   Space (cmd 38, `0x0044C6B0`): when the clear actually ran, re-centre
   with force 0.
3. **Size** `[0x007A5150]`: 0 full screen, 1 mini. Setter `0x0045A720`
   (options menu `0x0047D38C`, `0x0047D3D9`): on change, scale divisor
   `[0x00711254]` := 20 for mini, 10 otherwise (initial 10), reload the
   cel files (r5), re-centre with force 0.
4. **Re-centre** (`0x00457640(force)`): runs when force ≠ 0 or `AutoMap
   Centers` ≠ 0: recompute the view (§9 r1); full: offset (ox, oy) :=
   (0, 0); mini: (W/3 − mx − 16, H/3 − my − 16) (`[0x007A5188]`,
   `[0x007A518C]`; mx, my of §9 r1). Nothing else writes the offset,
   so the automap cannot be panned.
5. **Cel files** (`0x0045A2B0`, `DATA\GLOBAL\UI\AutoMap\`): full:
   `MaxiMap`, `Act2Map`, `Act4Map`, and `ExTnMap` only in expansion
   (`0x00408F20`); mini: the same names with suffix `S`, `ExTnMapS`
   always. Slots `[0x007A5168]`–`[0x007A5174]`, cel counts
   `[0x007A5178]`–`[0x007A5184]` (`0x006019F0`). Town kind 1 → `Act2Map`,
   2 → `Act4Map`, 3 → `ExTnMap`.

### 9. View geometry

W = `[0x007A5220]` (display width), Hp = `[0x007A521C]` (play height
H − 40), div = `[0x00711254]`; divisions are C signed division.

1. **Marker rectangle** (`0x00457520`, each automap frame): if mini and
   the result of `0x00492C10` changed, or `Left` changed since the last
   frame: store them and re-centre (force 0). Mini origin (mx, my) =
   (2W/3, 78) when `Left` = 0, else (0, 96 if `0x00492C10` ≠ 0 else 0).
   `0x00492C10` is `[0x007BEECC]` ≠ 2: the party-portrait state of
   `ui/messages.md` (portrait pass skipped at 2), so the left mini map
   moves down 96 px whenever the portraits are not hidden.
   Rectangle: mini [mx − 8, W/3 + mx] × [my, H/3 + my] (H = display
   height); full [−16, W] × [−16, H]. (`[0x007A51C8]`–`[0x007A51D4]`.)
2. **Panel side** (`0x00459700`): open mode 1 forces `Left` := 1
   (remembering the old value), open mode 2 forces `Left` := 0, modes 0
   and 3 restore it. Full mode only: s = +W/4 in open mode 1, −W/4 in
   open mode 2, else 0 (`shiftX` sign of `render/camera.md` §1 reversed).
3. **Origin**: with (cx, cy) the unit origin (`render/camera.md` §3,
   `[0x007A520C]`, `[0x007A5208]`): Ax = ox + 40 + (cx / div − W/2 + s),
   Ay = oy + 15 + (cy / div − Hp/2). A cell (cel, x, y) draws at
   X = x·10 / div − Ax, Y = y·10 / div − Ay.

### 10. Cell draw pass

1. **Where**: UI pass step 3 (`ui/panels.md` §5 r3): state 0x0A open and
   open mode ≠ 3, after the whole world draw (`render/draw-order.md`
   §1), before the other panels. Entry `0x0045AD60` needs the client
   act, a local player and its room; order: §9 r1, cells (`0x00459700`),
   unit and party markers (§11, §12), header text (§13).
2. **Trees and clip windows** (block: file, x range, y range, (dx, dy)):

   | Tree | Full | Mini |
   |---|---|---|
   | floors, walls, units | `MaxiMap`, [−16, W + 16], [−32, Hp + 32], (8, 16) | `MaxiMapS`, [mx − 8, W/3 + mx + 8], [my − 16, Hp/3 + my + 16], (4, 8) |
   | town, kind 1 | `Act2Map`, ±160, ±100, (80, 50) | same file slot, [mx − 160, W/3 + mx + 160], [my − 100, Hp/3 + my + 100], (80, 50) |
   | town, kind 2 | `Act4Map`, ±136, ±90, (68, 45) | likewise ±136, ±90, (68, 45) |
   | town, kind 3 | `ExTnMap`, ±180, ±170, (90, 85) | likewise ±180, ±170, (90, 85) |
   | town, kind 0 | as floors | as floors |

   "±a, ±b" = x ∈ [−a, W + a], y ∈ [−b, Hp + b]. Trees draw in the order
   floors, walls, units, town.
3. **Walk** (`0x00459440`): in-order (left, node, right) with pruning:
   descend left only when the node's Y ≥ y min; stop the whole walk at
   the first node with Y > y max; draw a node when y min ≤ Y and
   x min ≤ X ≤ x max. So cells draw in ascending (y, x, cel) order.
4. **Draw**: CelDrawClipped (`0x004F6510`, `render/capture.md`) of
   frame `cel` of the block's file at (X, Y), clip rectangle full
   (0, 0, display W − 1, display H − 1); mini (x0, y0, x0 + 279,
   y0 + 225) with (x0, y0) = (W − 281, 57) when `Left` = 0, else (0,
   (96 if `0x00492C10` ≠ 0 else 0) − 21); draw mode m
   (`render/blend-modes.md` §1) from the fade value v: v is
   `[0x007A51A8]`, but in mini with v = 1 it becomes (`[0x007A51AC]`
   ≠ 0 ? 0 : 2) and is stored (`0x004576C0`).
   - v = 1 (full, block file = `MaxiMap`): (sx, sy) = (X + dx, Y + dy);
     if W/2 + s₂ − 140 ≤ sx ≤ W/2 + s₂ + 140 and Hp/2 − 150 ≤ sy ≤
     Hp/2 + 130 (s₂ = −W/4 in open mode 1, +W/4 in open mode 2, else
     0): e = (2·max + min) / 2 of |W/2 − sx + s₂| and |Hp/2 − 10 − sy|;
     m = 2 if e < 150, 1 if e < 100, 0 if e < 50; otherwise m = 5.
   - v = 2: m = 1. v = 3: m = 1 in mini, 2 in full when the local
     player's byte +0x18 is 0 or 2, else 5. Other v: m = 5.

### 11. Unit markers (`0x0045AC90` → `0x0045A860`)

For each room of the local player's near-room list, each unit of its
list (+0x74, next +0xE8):

1. Skipped when it is a player and `0x00464820` ≠ 0 and it has no state
   7 (`0x00639DF0`); `0x00464820` = "dead": unit +0xC4 bit 16
   (0x10000) set, or a player in mode 0 / 17, or a monster in mode 0 /
   12 (other types: only the flag); skipped when §11 r3 gives no colour.
2. Position: X = px / div − Ax + 8, Y = py / div − Ay − 8 ((px, py) as
   §4 r4); drawn only inside the marker rectangle (§9 r1).
3. **Colour** (`0x00459BC0`; palette indices `nearest(r, g, b)` of
   `0x004FB180`, made at act load `0x0045A620`):

   | Unit | Colour |
   |---|---|
   | player, mode ≠ 17, the local player | B0 = (0, 0, 255) |
   | player, mode ≠ 17, same party id as the local player (≠ −1, `0x00465400`) | B3 = (0, 255, 0) |
   | other player, mode ≠ 17 | B1 = (255, 0, 0) |
   | player, mode 17, whose inventory owner (`0x0063D450`) is itself | B2 = (255, 0, 255) |
   | monster, mode ≠ 12, flag +0xC4 bit 21 clear, without `monstats` `interact`: disguised as a player with state 0x25 and an owner: owner same party → B4, else B1; else by `0x00478D90(owner id)` relation (owner unit +0x0C, −1 without an owner; the client roster list `[0x007BB5BC]`, next +0x30, matched on entry +0x08: none → 0; entry +0x04 outside the levels table or its level flag byte without a `[0x006CE278]` bit → 3; entry +0x0C = the local player's roster +0x0C (`0x00463DD0`) → 1; the two `0x00479BC0` party ids equal and ≠ −1 → 2; else 0; in single player the local player's own pets and hireling give 1): 1 → B4 (the owner flag test `0x00451F30(owner, 0x200)` that follows picks B4 on both branches, so it has no effect) = (0x44, 0x70, 0x74); 2 → B5 = (0x48, 0xA0, 0x34) when `AutoMap Party`; 0, 3 → none | |
   | monster as above with `interact`, class not 537–539 | B6 = (0xF4, 0xF4, 0xF4) |
   | object 59 (town portal) | B8 = (0xF4, 0xF4, 0) |
   | object 60 (portal) unless its target level is 111, 112, 117, 125, 126 or 127 | B8 |
   | object 267 | index 0 |
   | anything else | none |

4. **Shape** (`0x0045A7F0`): 12 opaque lines (`render/blend-modes.md`
   §8 r1) joining 13 points (table `0x006D6638`, ×2): (0,−1), (2,−2),
   (4,−1), (2,0), (4,1), (2,2), (0,1), (−2,2), (−4,1), (−2,0), (−4,−1),
   (−2,−2), (0,−1), each pair (X + 2a, Y + 2b); mini: X − 1, Y + 5.
5. Players: the cross is drawn unless the colour is B3 and `AutoMap
   Party` = 0. A party member (B3) other than the local player gets its
   name (§11 r7) when `AutoMap Party` and `AutoMap Party Names` are on.
6. Monsters: cross; with `AutoMap Party Names` on, an `interact`
   monster's name (`0x00464A60`) in font 6, colour 4, centred at (X,
   Y + 8 − 18); a disguised one as in r5. Objects: object 267 with
   names on → string 0xCF3 centred at (X, Y + 8 − 18) instead of a
   cross; other objects a cross.
7. **Name** (`0x0045A760`): empty → nothing; font 6, colour argument
   (≥ 13 → 0), centred on x, top at y − 10, then the previous font.

### 12. Party roster markers (`0x0045AB60`)

1. S→C 0x90 PartyAutomapInfo (`0x0045E9C0` → `0x0047A100`) stores x, y
   in the roster entry of the GUID (+0x28, +0x2C).
2. After §11, when the local player has a party (`0x00479BC0` ≠ −1, a
   party id shared by ≥ 2 roster entries) and `AutoMap Party` = 1: for
   each roster entry with the same party id and no client unit
   (`0x00463990` = 0): if the act of its level (+0x24) equals the local
   player's act: (x, y) → screen (`0x00643260`, `render/camera.md` §2
   subtile rule), X = x / div − Ax + 8, Y = y / div − Ay − 8, inside
   the marker rectangle: cross in B3; name when `AutoMap Party Names`.

### 13. Header text

Font 1, colour 4, right-aligned at x = W − width − 16; line y starts at
24 and steps 16 (`[0x007A51BC]`). In order, each only when present:

1. Game name: string 4181 (0x1055) + the name (`0x0044B7C0`); password:
   string 4182 + `0x0044B7F0` (both from `0x00459E90`; nothing when
   neither name nor `0x0044B820`).
2. Current level name (`0x00453E70` of the player's room's level).
3. `v 1.14d` (format `v %d.%d%c` with 1, 14, `d`).
4. Difficulty (`0x0044DCD0` = 1 or 2): string 4183 + string 5154
   (Nightmare) or 5155 (Hell); total length > 299 is fatal 0xB2A.
5. Game type 6 or 8 (`0x0044DB30`): `0x0040DF60` text (300 chars).
   Out of scope (Phases 0–6): game types 6 / 8 are TCP/IP games;
   `0x0040DF60` writes the host's IP address text (`gethostbyname`).
6. Expansion: string 22730 (0x58CA).

### 14. Lifetime

1. UI init: §8 r1, groups (§1 r5), cel files (§8 r5).
2. Act load (S→C 0x03, `client/model.md`): colours (§11 r3).
3. Teardown (`0x0045A5C0`, from `0x00456E27`): save the current layer
   (§7 r3), free cells, cel files and layers; current := none.

## Constants & data dependencies

| Item | Value | § |
|---|---|---|
| tile → cell | x = 8(wx − wy), y = 4(wx + wy), +24 for type ≥ 16 | 3 |
| unit → cell | (px / 10 + 1, py / 10 − 3) | 4 |
| reveal distance | (2·max + min) / 2 ≥ 0x50; countdown 2 | 5 |
| scale divisor | 10 full, 20 mini | 8 |
| fade radii | 50, 100, 150 (box −140…+140, −150…+130) | 10 |
| marker polygon | 13 points ×2 | 11 |
| `.map` version | 0xC; 4 slots; table 100 layers | 7 |
| groups | 49 pairs, `0x00711258` | 1 |
| tables read | `automap` (runtime form), `leveldefs` `Layer` +0x08 and `LevelType` +0x34, `monstats` `interact` (flags bit 9), `monstats2` `automapCel` (+0x118), `objects` `AutoMap` (+0x1BC), lvlprest `AutoMap` | 2–6 |

## Randomness

One `roll(n)` on the automap seed `0x0096C8C8` (`sim/rng.md` §5.5)
per §3 call whose picker finds a record (n = its cel count, 1–4). No
other draw. The seed is reset only at table load, so the cel choices
depend on the order in which tiles were revealed over the whole
session; cells restored from the file (§7) draw nothing.

## Edge cases & original bugs

Reproduced:
1. Duplicate cells (§1 r4) leak their pool slot.
2. A record whose picker gives −1 is still flagged 0x40000 and never
   retried (§3 r1).
3. Mini fade reads `[0x007A51AC]`, which init does not load (§8 r1): a
   fresh session in mini with `AutoMapFade` = 1 behaves as fade 2.
4. The fade test in full mode only applies to blocks drawing `MaxiMap`
   (town art never fades under v = 1).
5. §7 r4 clears the chain when a stored cel is out of range, dropping
   the rest of that layer's history.
6. `render/draw-order.md` §6 r7 says "rooms of the player's level"; the
   filter is the leveldefs `Layer` of the player's level (§5 r1). This
   spec owns the rule.

## Test vectors

Synthetic (CI):

| Input | Expected | Source |
|---|---|---|
| tile (wx, wy) = (100, 50), type 1 | cell (400, 600) | §3 r3 |
| tile (100, 50), type 17 | cell (400, 624) | §3 r3 |
| tile (50, 100), type 0 | cell (−400, 600) | §3 r3 |
| unit (px, py) = (−25, 47) | cell (−1, 1) (C division: −2 + 1, 4 − 3) | §4 r4 |
| moved (dx, dy) = (60, 40) | (120 + 40)/2 = 80 ≥ 0x50 → reveal | §5 r1 |
| moved (50, 59) | (118 + 50)/2 = 84 → reveal; (50, 55): 80 → reveal; (40, 59): 79 → none | §5 r1 |
| existing (y 10, x 5, cel 0) + new (10, 5, cel 2) | refused (group 0 = 0) | §1 r4 |
| existing (10, 5, cel 0) + new (10, 5, cel 5) | refused (g(5) = −1) | §1 r4 |
| existing (10, 5, cel 5) + new (10, 5, cel 6) | inserted after it (g = 1 ≠ −1, 6 > 5) | §1 r4 |
| fade v = 1, full, open mode 0, W = 800, Hp = 560, sx = 400, sy = 270 | e = 0 → m = 0 | §10 r4 |
| same, sx = 470, sy = 270 | e = 70 → m = 1 | §10 r4 |
| same, sx = 530, sy = 270 | e = 130 → m = 2; sx = 541 → box fails → m = 5 | §10 r4 |
| Lut Gholein f = 1 | 15 cells, n = 1–9, 11–14, 17, 18; first at (X − 453 + 160, Y − 119) for n = 1 | §6 |
| Lut Gholein f = 2 | 14 cells, n = 21–24, 26–29, 31–34, 37, 38 | §6 |

1.14d data (`#[ignore]`, `D2_GAME_DIR`):

| Input | Expected | Source |
|---|---|---|
| fresh seed {0, 666}; LevelType 1, `fl`, main 0, sub 5 | record 0 (range (0, 83), Style 0, sequences 1–46); `roll(4)` = 666 & 3 = 2 → Cel3 = 2; seed {666, 0} | `data/runtime-maps.md` §10, `sim/rng.md` sim-0001 |

Captures and recordings (none recorded yet; `render/capture.md` tools):

| Id | Case | Look for |
|---|---|---|
| automap-0001 | New character, Blood Moor, walk 30 s, Tab (full), screenshots with `--draws-every 1`; fade options 0–3 via F10 | cel draws (`0x004F6510`) per frame: order, (X, Y), draw mode vs §10; marker lines vs §11 |
| automap-0002 | Same, options → mini, `Left` both sides, a panel open on each side | clip rect and origin vs §9, §10 r4 |
| automap-0003 | Save and exit after visiting Blood Moor twice (leave via Rogue Encampment between); hex dump `<name>.map`, `.ma0` | §7 layout |
| automap-0004 | Two-player game in one party, partner in another level of the act | 0x90 traffic and the roster cross (§12) |

## Provenance

Read on the 1.14d `Game.exe`: cell store `0x00457B00`, `0x00457C30`,
`0x00457900`, `0x004579E0`, `0x00457E40`; picker `0x0061FFF0` (argument
registers from the call at `0x00457D49`); tile add `0x00457CF0`; units
`0x00458DC0`, `0x00457E80`; reveal `0x00459020`, `0x00458F40`,
`0x00459150`, `0x00459140` (caller `0x00465692`); town `0x004591A0`;
layers `0x00458CF0`, `0x00458D40`, `0x00458680`; files `0x00457F40`,
`0x004584C0`, `0x00458200`, `0x00458440`, `0x00458470`, `0x00458750`,
`0x004586E0`; options `0x004576C0`–`0x004577F0`, `0x0045A4C0`,
`0x0045A720`, `0x00457640`, `0x00457520`; files `0x0045A2B0`; draw
`0x0045AD60`, `0x00459700` (blocks from the disassembly), `0x00459440`;
markers `0x0045AC90`, `0x0045A860`, `0x00459BC0`, `0x0045A7F0`,
`0x0045A760`, `0x0045AB60`, `0x0045E9C0`; colours `0x0045A620`; text
`0x00459E90`, `0x0045A050`, `0x0045A0A0`, `0x0045A190`, `0x0045A120`,
`0x00459FF0`; teardown `0x0045A5C0`. Tables read from the file:
`0x00711258` (49 pairs), `0x006D6608`, `0x006D6638`, `0x00711254` =
10, `0x007113E0` = (1, 1, 1). `leveldefs` +0x08 / +0x34, `monstats`
bit 9, `monstats2` +0x118, `objects` +0x1BC matched to
`data/fields.tsv`. No D2MOO code used.

## Open questions

1. *Answered* (2026-10-08, static): `0x00492C10` in §9 r1,
   `0x00464820` in §11 r1.
2. *Answered* (2026-10-08, static): §11 r3 (relation codes; the 0x200
   test has no effect).
3. *Answered* (2026-10-08): the header draws the table text of each id
   (`ui/text.md` §2), so no label is needed by the rule; game types 6 /
   8 and `0x0040DF60` are out of scope (Phases 0–6), §13 r5.
4. *Answered* (2026-10-08, static): every placed unit, §5 r4.
5. PROVISIONAL: the `.ma` record header field order and chain link
   (§7 r2–r3) are read from the save/load call sequence; settled by
   automap-0003.
6. PROVISIONAL: the byte at player +0x18 tested by fade 3 (§10 r4) is
   read as-is with no meaning assigned; settled by automap-0001 (fade 3
   frames in town vs field).
7. PROVISIONAL: text colour of the names (§11 r7) is the AL value at the
   call (marker colour byte); settled by automap-0004 (name pixels).
