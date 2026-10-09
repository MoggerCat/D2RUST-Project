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
| Summary | 43–52 |
| Inputs | 53–63 |
| Outputs / state changes | 64–70 |
| Rules | 71–72 |
|   1. Grid geometry | 73–89 |
|   2. Tint colours | 90–117 |
|   3. Grid items (`0x00483FF0`) | 118–143 |
|   4. Placement tint (cursor item over a grid) | 144–160 |
|   5. Hover state (`0x00487000`) | 161–214 |
|   6. Equipment boxes (`0x004845A0`) | 215–244 |
|   7. Not drawn here | 245–254 |
|   8. Item graphic (`0x0046EE80(item, x, top)`; answers OQ 2) | 255–311 |
|   9. Item checks used by the tints (answers OQ 6) | 312–328 |
|   10. Grid click → C→S message (`0x0048FFE0`) | 329–398 |
|   11. Gold amount dialog (`0x00454150`) | 399–430 |
|   B5. `CellGrid` answers (`client/ui.md` §B5) | 431–438 |
| Constants & data dependencies | 439–448 |
| Randomness | 449–452 |
| Edge cases & original bugs | 453–460 |
| Test vectors | 461–487 |
| Provenance | 488–510 |
| Open questions | 511–559 |
<!-- /index -->

## Summary

Inside the inventory panel family (`ui/panels.md` §9) the client draws,
for one page at a time, every item lying in that page's grid over a
coloured cell tint, then the equipped items centred in their body-location
boxes with their own tint, and, while an item is on the cursor, a
placement tint showing where it would go (fits, swap, or refused). The
hovered item gets a different tint and shows its sockets; its footprint
is the anchor of the hover box. A left click on a grid becomes one C→S item message chosen from the cursor state, the cursor item and the item under it (§10); the gold button opens an amount dialog whose OK sends the gold message (§11).

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
   and the blend of mode 0: rule 3 (was §Open questions 1).
3. **Answers OQ 1 (rules).** The palette at `0x0081E668` holds the
   first 1,024 bytes of the act's `pal.pl2` as `PALETTEENTRY` (byte 0
   red, 1 green, 2 blue; `render/composition.md` §4), so the triples of
   rule 1 are (red, green, blue). Draw mode 0 of a rectangle has blend
   kind `k` = 2: each pixel becomes `A2[256·d + c]` (the 25 % source
   alpha table, `d` = destination index, `c` = the tint index;
   `render/blend-modes.md` §8 r2), over columns `x … x + w − 1`, rows
   `y … y + h − 1`. The tints are translucent; the pixels stay a capture
   case (§Test vectors).

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
   (`ui/text.md` §8; queue `0x00502280`; contents `ui/item-tips.md`).
2. With a cursor item: the cursor cell is computed from the cursor
   graphic's size and the item's w × h, centred and clamped to the grid
   (`0x00487000` second branch); hover-in-grid := 1, hovered item cleared.
   Exact formula: rule 3 (was §Open questions 3).
3. **Cursor cell** (answers OQ 3; `0x00487000`, record `R` of §1 r1,
   mouse (mx, my), cursor item of w × h cells, `gw` × `gh` = its
   inventory graphic's frame size, `0x004DBEA0`): start with c = (mx −
   R.left) / cellW and r = (my − R.top) / cellH (u32 division of the
   wrapped difference, as in §1 r4). If w is even: c = ((gw >> 2) −
   R.left + mx) / cellW (u32); if h is even: r = ((gh >> 2) − R.top +
   my) / cellH. If w = gridX: c = gridX >> 1; if h = gridY: r = gridY >>
   1. If w > 1: c −= w >> 1, c := 0 when negative (signed test), and if
   w + c > gridX the handler **returns** without any change (the cursor
   cell and hover flags keep their previous values); the same for h, r,
   gridY. Then cursor cell := (c, r), hover-in-grid `[0x007BCBE4]` := 1,
   hovered item and last hovered := 0.
4. **When it runs; the use press** (r4, 2026-10-09, static asm;
   REC-707 settled). `0x00487000` runs only on **WM_MOUSEMOVE**: its
   callers are the inventory move handler `0x004873A0` (handler table
   `0x006D5F80`: msg 0x200; its five call sites cover the grids of the
   open inventory mode) and the shop move handlers `0x004889D0`
   (table `0x0070F8A0`) and `0x00488F10` (shop menu `0x004B2900`,
   `0x004B2940`). `0x004873A0` is also the move handler body of
   `0x004894D0` (table `0x0070F964`), the stash `0x00489A30`
   (`0x0070F9F8`), the cube `0x0048A0F0` (`0x0070FA58`), and of
   `0x00489060` (from the quest item-place screen's move `0x004BFA30`,
   `ui/messages.md`). No press or release handler calls it (the same
   tables give WM_LBUTTONDOWN `0x004912A0`, WM_LBUTTONUP `0x00486EF0`,
   WM_RBUTTONDOWN `0x004917C0`), so a press acts on the hover state left
   by the last move.
   The right press `0x004917C0` (inside the grid rectangle) calls the
   use request `0x00487740` (`items/use.md`); when it sends C→S 0x20
   (`0x004786D0`) it then plays the use sound and sets **hover-in-grid
   `[0x007BCBE4]` := 0** (`0x00487918`). The hovered item `[0x007BCBF4]`
   and last hovered `[0x007BCBF8]` keep their values: §3 r2 needs both,
   so the item draws its §3 r3 tint, and the tip needs a hover flag
   (`ui/item-tips.md` §1 rule 2), so none is drawn until the next move
   sets `[0x007BCBE4]` again (same item: no anchor change, since it
   equals last hovered). This matches `a1-panel-cube` (right click on
   the cube, no move: tint 2, no tip) against `a1-npc-shop-tooltip`
   (move: tint 1). Other writers that clear all four hover globals:
   `0x00487990` (UI changes, `ui/panels.md`), `0x004879D0` (item
   freed, `client/model.md`), the equipment press (`ui/panels-3.md` §29
   r1 step 4).

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
panel art: `ui/panels.md` §9.3–§9.5. The cells have no art of their own:
the grid lines and cell frames are part of the panel background
(`InvChar6` frames 4–7, `bank` / `TradeStash`, `supertransmogrifier`).
The belt (boxes, tints, items, key labels) and the belt rows (ui 0x1F):
`ui/control-panel.md` §5. The cursor item: `ui/panels-3.md` §23 r9. Gold
line and gold buttons: `ui/panels-2.md` §21.

### 8. Item graphic (`0x0046EE80(item, x, top)`; answers OQ 2)

1. Only for an item unit (type 4); else nothing (returns 0). Arguments:
   item ECX, left x EDX, top y on the stack (grid §3 r1: the footprint's
   top-left cell corner; equipment §6 r2; cursor `panels-3.md` §23 r9;
   belt `ui/control-panel.md` §5 r4).
2. **File** (`0x004DBB50` → `0x004DB7B0` case 4 → `0x004DABC0`, the
   item's `items` record by code `0x006335F0`):
   1. identified (item flag 0x10, `0x006280A0`) set item (quality 5,
      `0x00627E70`): the `setitems` row of the file index (`0x00483440`)
      `invfile` (+0x62) when not empty, else the base record's
      `setinvfile` (+0x60) when not empty;
   2. else identified unique (quality 7) with file index > 0
      (`0x00629DA0`): the `uniqueitems` row's `invfile` (+0x5A) when not
      empty, else the base record's `uniqueinvfile` (+0x40) when not
      empty (a unique with file index 0 never reaches its row; it uses
      `uniqueinvfile` only when that is not empty, else falls through);
   3. else the item type's `VarInvGfx` (itemtypes +0x23, primary type
      `0x0062B400`) = 0: the base record's `invfile` (+0x20);
   4. else the type's `InvGfx` `n + 1` (itemtypes +0x24 + 0x20·n) with
      `n` = the item's gfx variant (item data +0x49, `items/bitstream.md`)
      clamped to [0, `VarInvGfx` − 1].
   The file is `<data>\items\<name>.dc6` (`0x005FE610`, format
   `0x006E35D4` `%s\items\%s.dc6`, data root `DATA\GLOBAL`), frame 0,
   direction 0.
   Gold (primary type 4): an amount class 0 (< 100), 1 (100–499), 2
   (500–4,999), 3 (≥ 5,000) of stat 14 is passed as the cel context's
   +0x40 value: how it selects the frame of the gold picture: §Open
   questions 7 (answered: r6).
3. **Visibility**: the cel's extent at (x, top + h) must touch [0, W] ×
   [0, H] (`0x004DAB40`), else nothing is drawn (returns 0).
4. **Draw**: `0x004F6480` at (x, top + h) (`h` = the frame height,
   `0x006018F0`; so the frame's top-left sits at (x, top) for offsets 0),
   light 0xFF, draw mode 1 when the item flag 0x400000 (ethereal) is set,
   else 5; remap = the item's inventory color map `0x0062C100(0, item,
   &byte, 1)` (no unit, so no state color; `render/shading.md` §6 r4 with
   `inv` ≠ 0: `InvTrans`, set / unique `invtransform`, affix
   `transformcolor`, gem `transform`).
5. Then the unit overlay draw `0x0046E300(item, 0xFF, 0, x + w / 2, top +
   h / 2, 0)` (w, h = frame size, halves rounded down): the item's own
   overlays centred on the picture (owner: the render overlay spec,
   `render/draw-order.md`); none for a plain item.
6. **Gold picture frame** (answers §Open questions 7). The amount class
   of r2 is stored in the cel context's direction field (+0x40; the
   other fields: +0x34 cel file := none, +0 frame := 0, `0x004DBB50`).
   The cel pick maps it through the file-direction table
   (`0x00601840`: frame = frame count × `0x006E45A0`[`ffs(D)`][dir] +
   frame, `render/unit-composite.md` §6 r3; the cache path `0x006001F0`
   rescales it only for files with more than one direction,
   `0x00600CB0`). The gold `invfile` is `invgld` (`misc.txt`; `gold`
   `VarInvGfx` 0, so r2 step 3), a DC6 of **1 direction × 1 frame**
   (28 × 28, offsets 0; d2data, measured), and row `ffs(1)` = 1 of the
   table is 0 for directions 0–3 (read from the image). So the class has
   no visible effect in 1.14d: every gold pile in a grid draws frame 0
   of `invgld`. (`invgldm` / `invgldh`, also 1 × 1 frames of 28 × 28, are
   named by no 1.14d table or string and are not drawn by this path.)

### 9. Item checks used by the tints (answers OQ 6)

1. `0x0062A4E0(item)` "usable state": 1 unless the item data flags
   (+0x18) have 0x100 or 0x4000.
2. `0x004C2240(item)`: 1 when the item has state 2 (`0x00625760(item,
   2)`) or the local player has state 54 `uninterruptable`.
3. `0x0062A0A0(item)`: the `items` record's `Transmogrify` (+0x139) ≠ 0;
   read only while the cursor state is 8 (a shop cursor, `ui/panels-3.md`
   §23 r7), giving tint 0 to a hovered item without it.
4. `0x0062E6F0` / `0x0062E740`: the item type's `Shoots` (itemtypes
   +0xC) / `Quiver` (+0xE) link, non-zero for launchers and their ammo.
5. Quest test `0x00483F80` (item code at item unit +0x80) on the client
   quest record `0x004B32D0()` (`0x0065C310(record, quest, bit)`): `ass`
   (Book of Skill) → quest 9 bit 5 clear; `xyz` (Potion of Life) → quest
   20 bit 5 clear; `tr2` (Scroll of Resistance) → quest 37 bit 8 clear
   or bit 7 set; any other code → 0. A true test gives tint 0 (§3 r3).

### 10. Grid click → C→S message (`0x0048FFE0`)

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
      x, y, page]. Drop cell (read 2026-10-09; ECX cursor item, EAX the
      grid record, stack mouse x, y and the two out pointers, `ret 0x10`;
      no item → fatal 0x14E8): the §5 r3 cursor-cell formula with the
      same steps (c, r from the mouse; even w / h use the graphic size
      `0x004DBEA0` >> 2; w = gridX → gridX >> 1, h = gridY → gridY >> 1;
      w > 1 → c −= w >> 1, negative → 0; the same for r), but with no
      "w + c > gridX → return" test: the cell is always written, and an
      out-of-grid footprint fails the placement test that follows.
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
   `0x00490BA0`, `0x00490FC0`): `ui/panels-3.md` §29.

### 11. Gold amount dialog (`0x00454150`)

The same dialog is also written up in `ui/panels-2.md` §21 r6–r9 (a
parallel PC 2 session, staging-6 merge; the two agree).

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
   (positions as passed, 640 × 480 frame: §Open questions 8): spinner
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
size). Cell art: none (§7). Highlight: the five tints of §2 (translucent,
§2 r3) applied by §3, §4, §6. Item graphic: file and draw §8, placement
§3 r1 and §6 r2. Cursor cell §5 r3; cursor item drawing `ui/panels-3.md`
§23 r9; belt `ui/control-panel.md` §5.

## Constants & data dependencies

Item graphic fields: `items` `invfile` +0x20, `uniqueinvfile` +0x40,
`setinvfile` +0x60, `InvTrans` +0x142, `Transmogrify` +0x139;
`uniqueitems` `invfile` +0x5A; `setitems` `invfile` +0x62; `itemtypes`
`VarInvGfx` +0x23, `InvGfx1`–`6` +0x24…; item flag 0x400000 (ethereal).
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
| tint index 2 over destination index `d` | pixel `A2[256·d + 2]` | §2 r3 |
| ring with gfx variant 7 (`VarInvGfx` 5) | file `invrin5` | §8 r2 |
| identified unique, `uniqueitems` `invfile` empty, base `uniqueinvfile` `invxyz` | `invxyz` | §8 r2 |
| ethereal sword at (419, 315), frame 28 × 84 | mode 1 draw at (419, 399) | §8 r4 |
| 2 × 3 item on the cursor, graphic 56 × 84, record 16 (left 419, top 315, 29 × 29 cells, 10 × 4), mouse (500, 340) | c = (14 − 419 + 500) / 29 = 3 → 3 − 1 = 2; r = (340 − 315) / 29 = 0 → 0 − 1 → 0; cursor cell (2, 0) | §5 r3 |
| same, mouse (700, 340) | c = (14 − 419 + 700) / 29 = 10 → 9; 2 + 9 > 10 → no change | §5 r3 |
| capture: inventory with the cases above, 800 × 600 | identical pixels | (to record) |
| no cursor item, left click on a potion, Shift held, page 0 | C→S `63` [item GUID] | §10 r3.4 |
| no cursor item, left click on an item | C→S `19` [item GUID] | §10 r3.5 |
| cursor 1 × 1 item over an empty cell (3, 1), page 0 | C→S `18` [item, 3, 1, 0] | §10 r4.2 |
| cursor item over one other item, not stackable | C→S `1F` [cursor, target, cell x, cell y] | §10 r4.3 |
| cursor gem over a socketed item with a free socket | C→S `28` [gem, item] | §10 r4.3 |
| cursor item over the cube with room | C→S `2A` [item, cube] | §10 r4.4 |
| gold dialog kind 1, typed 123, OK | C→S `50` [player GUID, 123] | §11 r4 |
| gold dialog kind 4, amount 70000, OK | C→S `4F 13 00 01 00 70 11` (p1 = 1, p2 = 0x1170) | §11 r4 |
| gold dialog, spinner down with step 1000 at amount 300 | amount 0 | §11 r3 |

## Provenance

1.14d `Game.exe` `.\UI\inv.cpp` functions `0x00483960`–`0x004845A0`,
item graphic `0x0046EE80`, `0x004DBB50`, `0x004DB7B0`, `0x004DAA70`,
`0x004DABC0`, `0x0062E8D0`, `0x0062E920`, `0x006283F0`, `0x004DAB40`,
`0x005FE610`; checks `0x0062A4E0`, `0x004C2240`, `0x0062A0A0`,
`0x0062A060`, `0x0062E6F0`, `0x0062E740`, `0x00483F80`;
`0x00487000`, palette match `0x00605210`, fill `0x004F6300`; register
arguments checked with `tools/ghidra/disasm.py`. §10: `0x0048FFE0`
(message ids from the `mov cl` before each send in `all.asm`); §11:
`0x00454150`, `0x00453EE0`, `0x00453FC0`–`0x00454140` (jump table
`0x00454124` read from the binary), opener call sites `0x00486FAD`,
`0x004896CA`, `0x00489BE9`, `0x00489C0A`, `0x0048A2F1` (2026-10-07,
`claude/pc2-ui`). No D2MOO code used.

§5 r4 (2026-10-09, pc1-day4, static asm): every `call 0x487000` and
`call 0x4873A0` site in `all.asm`; the handler tables `0x006D5F48`–
`0x006D5F88`, `0x0070F8A0`, `0x0070F930`–`0x0070F970`,
`0x0070F9D0`–`0x0070FA10`, `0x0070FA30`–`0x0070FA60` read from the
image (`re/scripts/rd.py`); `0x004917C0`, `0x00487740`
(`0x00487918`), `0x00487990`; every write of `0x007BCBE4` /
`0x007BCBF4`.

## Open questions

1. **Answered** (2026-10-07, §2 r3: byte 0 is red; mode 0 is the
   translucent `A2` blend; the pixels stay a capture case, §Test
   vectors): byte 0 of a `0x0081E668` entry is red
   (`render/composition.md` §4: the act's `pal.pl2`, R, G, B), so the
   triples of §2 are (R, G, B); the tints are recomputed whenever an
   act palette loads (`0x004547B0` → `0x00483960`). Fill mode 0 is the
   rectangle's `k` = 2 blend, `d' = T[256·d + color]`
   (`render/blend-modes.md` §8 r2). Pixel proof: capture `inv-0001`. Was: Palette entry byte order at `0x0081E668` (is byte 0
   red?) and the blend of fill mode 0 (opaque or translucent). Needs
   recording: a capture of an inventory with a blue-tinted item, read
   the tint pixel.
2. **Answered** (2026-10-07, §8; the gold amount frame: OQ 7). Was: Item
   graphic draw `0x0046EE80`: which `invfile` / unique / set graphic and
   frame, the draw position (arguments in registers), and the palette
   shift / colour map for coloured items. Disassembly of `0x0046EE80`.
3. **Answered** (2026-10-07, §5 r3). Was: Cursor-cell formula with a
   cursor item (`0x00487000` second branch, `0x004DBEA0` graphic size,
   the even / odd size handling).
4. **Answered** (2026-10-07, `ui/control-panel.md` §5). Was: Belt panel
   drawing (ui 0x1F, belt rows) and its slot tint.
5. **Answered** (2026-10-07, `ui/panels-3.md` §23 r9, `ui/panels-2.md`
   §21). Was: Cursor item drawing (position, hotspot) and the gold /
   other buttons (`ui/panels.md` §Open questions 5). Pixel check:
   capture `inv-0001` (`docs/handoff/pc2-rec-pc2-ui.md`). The gold
   dialog is §11; the inventory gold button's art and the gold amount
   text: capture `inv-0003`.
6. **Answered** (2026-10-07, §9). Was: The checks `0x004C2240`,
   `0x0062A4E0`, `0x0062A0A0`, `0x0062E6F0`, `0x0062E740`, cursor state
   8, and the quest test `0x00483F80`.
7. **Answered** (2026-10-07, §8 r6: frame 0 always). Was: Gold picture frame (§8 r2): how the cel context +0x40 value (gold
   amount class 0–3) picks the frame inside the cel loader `0x006001F0`
   (`0x00600CB0` scaling by the frame count), and the frame count of the
   `gld` inventory file. Disassembly read of `0x00600CB0`; DC6 header of
   the gold `invfile`.
8. ~~Gold dialog control positions (§11 r2) at 800 × 600: the values are
   passed unshifted; does the box code (`0x004B7CD0`, `0x004BBD80`) add
   the panel shift?~~ Capture `inv-0003`.
9. Answered (2026-10-08, by §8): item graphic draw `0x0046EE80` (partial answer to OQ 2, 2026-10-07):
   the cel is drawn with its top-left at the cell's top-left (draw y =
   top + cel height, `0x004F6480`), draw mode 5, or 1 when item flag
   0x400000 (ethereal) is set, palette from `0x0062C100`; a gold pile
   (`0x0062B400` = 4) uses graphic variant 0 below 100 gold, 1 below 500,
   2 below 5000, else 3 (`0x004DBB50` second argument); then
   `0x0046E300(item, 0xFF, 0, x + celW / 2, y + celH / 2)`. The rest
   (which file `0x004DBB50` picks, what `0x0046E300` draws) is answered
   by §8 (the other PC 2 session's full read; staging-6 merge).
