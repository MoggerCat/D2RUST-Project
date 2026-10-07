# Spec: UI — Inventory screen (grid and item drawing, tints, equipment slots, hover)

- **Status:** draft (2026-10-07, RE on 1.14d `Game.exe`; no capture
  yet). Pixel results are unverified until the §Test vectors captures run.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::ui` (`CellGrid`, `ui/panels/inventory.rs`,
  `stash_cube.rs`, `shop.rs`)
- **Related specs:** `client/ui.md` §B5 (row owned by this spec; code
  TODOs `ui/inventory.md §B5` mean §B5 below), `ui/panels.md` §9 (panel
  family, layout records, empty-slot pictures, weapon-swap tabs, close
  button: not restated here), §11 stash, §12 cube, §14 shop,
  `items/inventory.md` (grid model, pages, records, belt slots),
  `items/inventory-moves.md` §7 (click validation), `ui/controls.md`
  (show-belt and belt keys), `ui/text.md` §8 (hover boxes)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 41–50 |
| Inputs | 51–61 |
| Outputs / state changes | 62–68 |
| Rules | 69–70 |
|   1. Grid geometry | 71–87 |
|   2. Tint colours | 88–106 |
|   3. Grid items (`0x00483FF0`) | 107–132 |
|   4. Placement tint (cursor item over a grid) | 133–149 |
|   5. Hover state (`0x00487000`) | 150–163 |
|   6. Equipment boxes (`0x004845A0`) | 164–193 |
|   7. Not drawn here | 194–200 |
|   8. Grid click → C→S message (`0x0048FFE0`) | 201–263 |
|   9. Gold amount dialog (`0x00454150`) | 264–292 |
|   B5. `CellGrid` answers (`client/ui.md` §B5) | 293–299 |
| Constants & data dependencies | 300–305 |
| Randomness | 306–309 |
| Edge cases & original bugs | 310–317 |
| Test vectors | 318–338 |
| Provenance | 339–349 |
| Open questions | 350–382 |
<!-- /index -->

## Summary

Inside the inventory panel family (`ui/panels.md` §9) the client draws,
for one page at a time, every item lying in that page's grid over a
coloured cell tint, then the equipped items centred in their body-location
boxes with their own tint, and, while an item is on the cursor, a
placement tint showing where it would go (fits, swap, or refused). The
hovered item gets a different tint and shows its sockets; its footprint
is the anchor of the hover box. A left click on a grid becomes one C→S item message chosen from the cursor state, the cursor item and the item under it (§8); the gold button opens an amount dialog whose OK sends the gold message (§9).

## Inputs

| Name | Type | Source |
|---|---|---|
| layout records | grid and equipment rectangles | `inventory.bin` via `0x004835B0` (`ui/panels.md` §9.2) |
| current page | byte `0x007BCC04` (0 inventory, 1–2 trade, 3 cube, 4 stash) | panel state |
| inventory mode | `0x007BCBF0` (`ui/panels.md` §9.1) | panel state |
| mouse position | `0x00468730` (x), `0x00468740` (y) | input |
| cursor item | `0x0063C1E0(inventory)` | item model |
| screen size | width `0x0071146C`, height `0x00711470`; clip `0x007A5220` × `0x007A521C` | display |

## Outputs / state changes

Draw calls only, plus the hover state of §5 (`0x007BCBF4` hovered item,
`0x007BCBE4` / `0x007BCBE8` hover-in-grid / hover-on-equipment flags,
`0x00721E4C` / `0x00721E50` cursor cell, `0x00721E3C`–`0x00721E48` hover
anchor, `0x007BCC20` hovered body location).

## Rules

### 1. Grid geometry

1. A grid layout record (24 bytes) holds u8 gridX (+0), u8 gridY (+1),
   i32 left (+4), right (+8), top (+0xC), bottom (+0x10), u8 cell width
   (+0x14), u8 cell height (+0x15). The rectangles of the inventory panel
   are `ui/panels.md` §9.2; measured values are its §Test vectors.
2. Record used by `0x00483FF0` for a player owner: page 0 → `0x007BCB88`,
   1 → `0x007BCB18`, 2 → `0x007BCA30`, 3 → `0x007BCB70`, 4 →
   `0x007BCB40` (classic game) or `0x007BCA78` (expansion); for a
   monster owner (hireling, unit type 1) → `0x007BCB58`; other unit
   types draw nothing.
3. Cell (c, r) has its top-left corner at (left + cellW · c, top +
   cellH · r) and size cellW × cellH. A cell is tinted only when its
   top-left corner is inside [0, clipW) × [0, clipH).
4. Mouse → cell (`0x00487000`, no cursor item): c = (mouseX − left) /
   cellW, r = (mouseY − top) / cellH, unsigned division.

### 2. Tint colours

1. `0x00483960` fills five palette indices, each the nearest entry
   (smallest squared distance, first one on a tie, `0x00605210`) of the
   256-entry palette `0x0081E668` to a triple compared with entry bytes
   0, 1, 2:

   | Index | Byte | Triple | Used for |
   |---|---|---|---|
   | 0 | `0x007BCAD8` | (0x80, 0, 0) | refused / unusable |
   | 1 | `0x007BCAD9` | (0, 0x80, 0) | hovered, fits |
   | 2 | `0x007BCADA` | (0, 0, 0x80) | usable item in a grid |
   | 3 | `0x007BCADB` | (0x80, 0x80, 0) | swap target |
   | 4 | `0x007BCADC` | (0x80, 0x40, 0x40) | unidentified |

2. A tint is a filled rectangle (`0x0046EFD0` → `0x004F6300` → renderer
   slot +0xB8) with draw mode argument 0. Byte order of the palette entry
   and the blend of mode 0: §Open questions 1.

### 3. Grid items (`0x00483FF0`)

1. Reload the layout records (`0x004835B0`), pick the record (§1 r2).
   For every item of the owner's inventory list that lies in a page grid
   (`0x0063E020` = 1) and whose page (`0x00628250`) is the current page:
   position (c, r) from its path (static path +0xC / +0x10 for unit types
   2, 4, 5; else `0x006488C0` / `0x00648900`), size w × h from
   `0x006286C0`.
2. Hovered item (item = `0x007BCBF4` and `0x007BCBE4` ≠ 0): tint 1, or
   tint 0 when the cursor state `0x00468830` = 8 and `0x0062A0A0(item)`
   = 0. Every cell of the w × h footprint is tinted, then the item is drawn
   (`0x0046EE80`, §Open questions 2). If the item has sockets
   (`0x00629900`, `0x0062BC20` ≠ 0) and flag 0x800 (socketed), the socket
   pictures are drawn over it (`0x00483E40`).
3. Other items, tint:
   - 0 when the requirement check `0x0062EAF0(item, player, 0, 0, 0, 0)`
     fails (`items/inventory.md` §4.2), or item flag 0x4 is set, or
     `0x004C2240` ≠ 0, or `0x0062A4E0(item)` = 0;
   - else 4 when flag 0x10 (identified) is clear;
   - else 2, except 0 for an item whose class has the quest flag
     (+0x12A) and is code `ass`, `xyz` or `tr2` while its quest test
     `0x00483F80` holds.
   Then the footprint is tinted and the item drawn; no sockets.
4. For a hireling owner nothing more is drawn. For the player, if an
   item is on the cursor, the placement tint of §4 follows.

### 4. Placement tint (cursor item over a grid)

1. Grid under the mouse by inventory mode (`0x00483FF0` tail): mode 0x0B
   (player trade): page 1 none, page 2 → `0x00483B10`, others
   `0x00483AB0`; modes 0x0C / 0x0D (stash): page 4 → `0x00483B40`, else
   `0x00483AB0`; mode 0x0E (cube): page 3 → `0x00483BF0`, else
   `0x00483AB0`; any other mode: no hit test. `0x00483AB0` = mouse inside
   the inventory grid rectangle (left ≤ x < right, top ≤ y ≤ bottom).
2. `0x00483C50` draws only when the cursor cell (`0x00721E4C`,
   `0x00721E50`) is ≥ 0 and hover-in-grid is set; not in mode 1 or 0x13
   with mouse x < width / 2; not with mouse y ≥ height − 0x27.
3. Item fits at the cursor cell (`0x0063B9D0`): footprint tint 1.
   Else, with the item under it found by `0x0063BB20`: tint 0 when there
   is none, or when the swap test fails and the item under is not a cube
   (code `box`); otherwise tint 3 over the item under (position from
   `0x0045AE20` / `0x0045ADF0`).

### 5. Hover state (`0x00487000`)

1. No cursor item: the item at the mouse cell (`0x0063BD10`) becomes the
   hovered item (`0x007BCBE4` := 1) or none (both cleared). When it
   changes, the hover anchor is set from its grid position (c, r) and
   size w × h: x = left + cellW · c + (w · cellW) / 2, top = top + cellH ·
   r, bottom = top + cellH · (r + h) (`0x00721E3C` / `0x00721E44`,
   `0x00721E40`, `0x00721E48`); the hover box is drawn from these
   (`ui/text.md` §8; queue `0x00502280`).
2. With a cursor item: the cursor cell is computed from the cursor
   graphic's size and the item's w × h, centred and clamped to the grid
   (`0x00487000` second branch); hover-in-grid := 1, hovered item cleared.
   Exact formula: §Open questions 3.

### 6. Equipment boxes (`0x004845A0`)

1. Slot rectangles `0x007BCC58` + 0x14 · L (left +0, top +8, width byte
   +0x10, height byte +0x11) for body location L = 0–10
   (`ui/panels.md` §9.2); cell size from the page-0 record
   (`0x007BCB9C` / `0x007BCB9D`).
2. Item of size w × h in L is drawn at x = left + (slotW − cellW · w) / 2,
   y = top + (slotH − cellH · h) / 2 (arithmetic shift), then: L 3 x + 3;
   L 8 x + 2, y − 2; L 2, 6, 7, 10 y − 2; L 9 x + 2, y − 4; L 4 / 5 the
   weapon-swap offset (4, −4) only when `0x007BCC4C` ≠ 0, which never
   happens in 1.14d (`ui/panels.md` §9.5).
3. Two-handed ghost: for L 4 (or 5) with an item, when `0x0063D340` = 2
   and neither hand item passes `0x0062E740`, the other hand's box gets
   tint 0 and the item is drawn there too, centred vertically by its
   height.
4. Tint of the item's own box: hovered (item = `0x007BCBF4`, `0x007BCBE8`
   ≠ 0) → tint 1 (tint 0 when cursor state = 8), drawn, sockets as §3 r2;
   else tint 0 when (no ghost and (`0x0062E6F0` or `0x0062E740`)) or the
   checks of §3 r3 first bullet hold; else tint 4 when unidentified; else
   no tint (equipped items never get tint 2). Off-screen boxes are not
   tinted.
5. Then `0x004844A0`: with a cursor item and hover-on-equipment set (not
   in mode 1 with mouse x < width / 2, not with mouse y ≥ height − 0x27;
   mouse x < width / 2 clears `0x007BCC20` instead), the hovered box
   `0x007BCC20` gets tint 1 when the body location accepts the item
   (`0x0062ED50`) and its requirements pass, or when the cursor item can
   be socketed into the item there (`0x004843E0`: filler type
   `0x0062BEB0`, target socketed + identified, flag 0x100 clear, filled <
   sockets, mode not 0x0A / 0x0B); else tint 0.

### 7. Not drawn here

Empty-slot pictures, weapon-swap tabs (never drawn in 1.14d) and the
panel art: `ui/panels.md` §9.3–§9.5. The belt panel (ui 0x1F, toggled by
`ui/controls.md` command 22) and the cursor item graphic: §Open
questions 4 and 5.

### 8. Grid click → C→S message (`0x0048FFE0`)

Left mouse down on a page grid (callers: the inventory, stash, cube and
trade-page down handlers; `ui/panels.md` §15). Arguments: owner unit,
inventory, page (low byte of EAX; 0xFF → nothing), mouse (x, y), the
message's key state (wParam; bit 2 = `MK_SHIFT`), grid record (§1). The
mouse cell is ((x − left) / cellW, (y − top) / cellH), unsigned. "Ready"
below = the busy test `0x004C2240` = 0 and the send throttle `0x00486D10`
≠ 0; a send is followed by `0x004C21F0` and, for the place / swap /
stack / socket / scroll / cube sends, `0x004C1D60(0, 0, 0)` and the click
sound `0x004B9A00(0, 0, 0)`. Server rules: `items/inventory-moves.md` §7.
Messages are sent through `0x004786A0` (two u32), `0x00478680` (one
u32) and `0x00478700` (u32 + three u32); field order is the layout in
`sim/client-messages.tsv`.

1. **Cursor state 6** (`0x00468830` = 6, an item is being used on
   another, e.g. a scroll of identify): the item under the cell
   (`0x0063BD10`) and the used item (`0x004680A0`) both exist, ready, the
   owner is the local player and the page is not 1 or 2 → **0x27**
   (target, used). Consumed whenever an item is under the cell.
2. **Cursor state 8**: item under the cell, ready, own player, page not 1
   or 2 → **0x4C** [item] (`world/cube.md` §10).
3. **No cursor item**, item under the cell:
   1. Not the player's own inventory context (`0x00486B30` = 0 or its
      ECX result ≠ 1): when the inventory mode ≤ 0x12, the NPC store
      click `0x004B3870(…, x, y, 0, 0, 0)` (`ui/menus.md` §4.5); a
      handled click sets the inventory mode to 5. No message here.
   2. Not ready → nothing.
   3. **Ctrl** down (`GetAsyncKeyState(VK_CONTROL)` < 0): with a store
      open (`0x004B3230`) and its NPC (`0x00463990`): sellable item
      (`0x0062A130`) → **0x33** sell (`world/vendors.md`, price
      `0x0062FDC0`); else the "cannot" note `0x004CB9C0`. Without a store
      or NPC: nothing is sent, the click is consumed (Ctrl-click never lifts).
   4. **Shift** (wParam bit 2) and the item fits the belt (`0x0062BAD0`)
      and lies on page 0 → **0x63** [item] (to belt), click sound.
   5. Else **0x19** [item] (lift to the cursor); `[0x007BCBEC]` := 1.
4. **Cursor item present**: placement test at the cursor cell
   (`0x0063B9D0` with `[0x00721E4C]`, `[0x00721E50]`) gives the item
   under it `u` and the overlap count `n`.
   1. `n` ≥ 2: if `0x0063BB20` finds a cube (code `box`) under the
      footprint → rule 4.4 with `u` = that cube; else as `n` ≤ 1.
   2. `n` = 0, or `u` none: on page 3 a cursor item that is itself a cube
      is refused (consumed, no message). Else the drop cell from the
      mouse (`0x00486BD0`), placement test there, ready → **0x18** [item,
      x, y, page].
   3. `n` = 1 with `u`:
      - stackable onto `u` (`0x0062C850` ≠ 0): ready, inventory mode ≠
        0x0B, page ≠ 2 → **0x21** (cursor, `u`);
      - else, if the cursor item can fill a socket of `u` (`0x004843E0`,
        §6 r5) and ready → **0x28** (cursor, `u`);
      - a scroll (item type 22 `scro`) on a book (type 18 `book`) of the
        same kind (`0x00627F80` equal), ready, mode ≠ 0x0B →
        `0x004A9DF0`, **0x29** (cursor, `u`);
      - `u` is a cube → rule 4.4;
      - else, unless 0x28 / 0x29 was just sent: swap — `0x0063BB20`
        (single overlap) ready, page ≠ 2 → **0x1F** (cursor, `u`, cell x,
        cell y).
   4. **Into the cube**: ready; free space found on page 3 of the cube's
      grid (`0x0063B850`, record of page 3) → **0x2A** (cursor, cube);
      none → the "cannot" note `0x004CB9C0`.
5. Return value 1 = consumed. Equipment clicks (`0x00490780`,
   `0x00490BA0`, `0x00490FC0`): `ui/panels.md` §15 (not read here).

### 9. Gold amount dialog (`0x00454150`)

1. **Openers** (kind = ECX): inventory gold button release
   (`0x00486EF0`, `ui/panels-2.md` §18 r1) → 1; the same with the stash
   open (`0x00489AC0`) → 3, the stash gold button (`0x00489AC0`) → 4;
   with the cube open (`0x0048A190`) → 1; player trade (`0x00489580`) →
   2; kind 0 from `0x004A7A90`. Only when the player exists, has no
   cursor item and no other dialog is open (`[0x007A27A0]` = 0).
2. Opening: hot-key mode 0 with key-up kept and the latch `[0x007A27B4]`
   := 1 (`ui/controls.md` §4.1); `[0x007A279C]` := kind; amount
   `[0x007A2A68]` := 0; the box `0x004B7CD0`; the limit `m` = stat 14
   (gold) of the player, stat 15 (stash gold) for kind 4. Controls
   (positions as passed, 640 × 480 frame: §Open questions 7): spinner
   at (0xDF, 0xDB) (`0x004BC480`), number field at (0x102, 0xE4), width
   100, max 10 digits, limit `m` (`0x004BBD80`), OK button at (0xFA,
   0x11F) and Cancel at (0x163, 0x11F) (`0x004BB0F0`). Kinds 3, 6, 7
   pre-fill the field with `m`; kind 2 also calls `0x004B90B0`.
3. Spinner (`0x00453FE0`): step `s` from the spinner; up: amount := min(
   amount + s, `m`); down: amount := amount − s, or 0 when s > amount.
4. **OK** (`0x00454080`): close (`0x00453EE0`: latch cleared, hot-key
   mode 1, controls freed). Amount 0: kind 2 → `0x004B9110`; others
   nothing. Else by kind: 0, 1 → C→S **0x50** [player GUID, amount]
   (`items/inventory-moves.md` §7.22); 2 → `0x004B9110` (trade gold,
   `ui/panels.md` §15); 3 → **0x4F** button 0x14, p1 = amount >> 16, p2
   = amount & 0xFFFF; 4 → **0x4F** button 0x13, same split; kinds 2–4
   then play sound 0xDD (`0x004B9A00`). Server meaning of 0x4F 0x13 /
   0x14 (withdraw / deposit): `ui/panels.md` §Open questions 6.
5. Cancel (`0x00454140`) and the box close (`0x00453FC0`): close only.

### B5. `CellGrid` answers (`client/ui.md` §B5)

Cell size and origin: §1 (no gaps: cells are adjacent, pitch = cell
size). Highlight: the five tints of §2 applied by §3, §4, §6. Item
graphic placement: §3 r1 and §6 r2 (graphic draw §Open questions 2).
Cursor item drawing and belt: open (§Open questions 4, 5).

## Constants & data dependencies

Tint triples of §2; bottom margin 0x27; layout records from
`inventory.bin`; item codes `ass`, `xyz`, `tr2`, `box`; item flags 0x4,
0x10, 0x100, 0x800.

## Randomness

None.

## Edge cases & original bugs

1. Equipped items are never tinted blue; grid items always are unless
   red / unidentified / hovered.
2. Sockets show only on the hovered item.
3. A tint is skipped per cell when the cell's top-left corner is off the
   clip, so a partly visible footprint is partly tinted.

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| 800 × 600, page 0 record 16, item 1 × 1 at (0, 0) | tint rect at (left, top) size cellW × cellH, index 2 | §1, §3 r3 |
| unidentified ring in grid | tint 4 | §3 r3 |
| hovered item | tint 1 + sockets if socketed | §3 r2 |
| 2 × 3 armor on cursor over free cells | tint 1 over 2 × 3 | §4 r3 |
| cursor item over one item it can swap with | tint 3 over that item | §4 r3 |
| two-handed weapon in right hand | left box tint 0 + ghost item | §6 r3 |
| capture: inventory with the cases above, 800 × 600 | identical pixels | (to record) |
| no cursor item, left click on a potion, Shift held, page 0 | C→S `63` [item GUID] | §8 r3.4 |
| no cursor item, left click on an item | C→S `19` [item GUID] | §8 r3.5 |
| cursor 1 × 1 item over an empty cell (3, 1), page 0 | C→S `18` [item, 3, 1, 0] | §8 r4.2 |
| cursor item over one other item, not stackable | C→S `1F` [cursor, target, cell x, cell y] | §8 r4.3 |
| cursor gem over a socketed item with a free socket | C→S `28` [gem, item] | §8 r4.3 |
| cursor item over the cube with room | C→S `2A` [item, cube] | §8 r4.4 |
| gold dialog kind 1, typed 123, OK | C→S `50` [player GUID, 123] | §9 r4 |
| gold dialog kind 4, amount 70000, OK | C→S `4F 13 00 01 00 70 11` (p1 = 1, p2 = 0x1170) | §9 r4 |
| gold dialog, spinner down with step 1000 at amount 300 | amount 0 | §9 r3 |

## Provenance

1.14d `Game.exe` `.\UI\inv.cpp` functions `0x00483960`–`0x004845A0`,
`0x00487000`, palette match `0x00605210`, fill `0x004F6300`; register
arguments checked with `tools/ghidra/disasm.py`. §8: `0x0048FFE0`
(message ids from the `mov cl` before each send in `all.asm`); §9:
`0x00454150`, `0x00453EE0`, `0x00453FC0`–`0x00454140` (jump table
`0x00454124` read from the binary), opener call sites `0x00486FAD`,
`0x004896CA`, `0x00489BE9`, `0x00489C0A`, `0x0048A2F1` (2026-10-07,
`claude/pc2-ui`). No D2MOO code used.

## Open questions

1. **Answered** (2026-10-07): byte 0 of a `0x0081E668` entry is red
   (`render/composition.md` §4: the act's `pal.pl2`, R, G, B), so the
   triples of §2 are (R, G, B); the tints are recomputed whenever an
   act palette loads (`0x004547B0` → `0x00483960`). Fill mode 0 is the
   rectangle's `k` = 2 blend, `d' = T[256·d + color]`
   (`render/blend-modes.md` §8 r2). Pixel proof: capture `inv-0001`.
2. Item graphic draw `0x0046EE80`: which `invfile` / unique / set graphic
   and frame, the draw position (arguments in registers), and the palette
   shift / colour map for coloured items. Disassembly of `0x0046EE80`.
3. Cursor-cell formula with a cursor item (`0x00487000` second branch,
   `0x004DBEA0` graphic size, the even / odd size handling).
4. Belt panel drawing (ui 0x1F, belt rows) and its slot tint.
5. ~~Cursor item drawing (position, hotspot)~~: capture `inv-0001`
   (`docs/handoff/pc2-rec-pc2-ui.md`). The gold dialog is §9; the
   inventory gold button's art and the gold amount text: capture
   `inv-0003`.
6. The checks `0x004C2240`, `0x0062A4E0`, `0x0062A0A0`, `0x0062E6F0`,
   `0x0062E740`, cursor state 8, and the quest test `0x00483F80`.
7. ~~Gold dialog control positions (§9 r2) at 800 × 600: the values are
   passed unshifted; does the box code (`0x004B7CD0`, `0x004BBD80`) add
   the panel shift?~~ Capture `inv-0003`.
8. Item graphic draw `0x0046EE80` (partial answer to OQ 2, 2026-10-07):
   the cel is drawn with its top-left at the cell's top-left (draw y =
   top + cel height, `0x004F6480`), draw mode 5, or 1 when item flag
   0x400000 (ethereal) is set, palette from `0x0062C100`; a gold pile
   (`0x0062B400` = 4) uses graphic variant 0 below 100 gold, 1 below 500,
   2 below 5000, else 3 (`0x004DBB50` second argument); then
   `0x0046E300(item, 0xFF, 0, x + celW / 2, y + celH / 2)`. Still open:
   which file `0x004DBB50` picks (invfile / unique / set) and what
   `0x0046E300` draws.
