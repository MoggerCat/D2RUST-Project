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

## Summary

Inside the inventory panel family (`ui/panels.md` §9) the client draws,
for one page at a time, every item lying in that page's grid over a
coloured cell tint, then the equipped items centred in their body-location
boxes with their own tint, and, while an item is on the cursor, a
placement tint showing where it would go (fits, swap, or refused). The
hovered item gets a different tint and shows its sockets; its footprint
is the anchor of the hover box.

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

## Provenance

1.14d `Game.exe` `.\UI\inv.cpp` functions `0x00483960`–`0x004845A0`,
`0x00487000`, palette match `0x00605210`, fill `0x004F6300`; register
arguments checked with `tools/ghidra/disasm.py`. No D2MOO code used.

## Open questions

1. Palette entry byte order at `0x0081E668` (is byte 0 red?) and the
   blend of fill mode 0 (opaque or translucent). Needs recording: a
   capture of an inventory with a blue-tinted item, read the tint pixel.
2. Item graphic draw `0x0046EE80`: which `invfile` / unique / set graphic
   and frame, the draw position (arguments in registers), and the palette
   shift / colour map for coloured items. Disassembly of `0x0046EE80`.
3. Cursor-cell formula with a cursor item (`0x00487000` second branch,
   `0x004DBEA0` graphic size, the even / odd size handling).
4. Belt panel drawing (ui 0x1F, belt rows) and its slot tint.
5. Cursor item drawing (position, hotspot) and the gold / other buttons
   (`ui/panels.md` §Open questions 5).
6. The checks `0x004C2240`, `0x0062A4E0`, `0x0062A0A0`, `0x0062E6F0`,
   `0x0062E740`, cursor state 8, and the quest test `0x00483F80`.
