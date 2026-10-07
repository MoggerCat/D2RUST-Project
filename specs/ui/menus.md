# Spec: UI — Waypoint input, NPC menu box, hire list, shop transactions

- **Status:** draft (2026-10-07, RE on the 1.14d `Game.exe`; no capture
  yet). Split out of `ui/panels.md` (size): that spec owns the panel art,
  the UI states and the open / close rules, this one the input handling,
  the menu box and the message fields named below.
- **Target version:** 1.14d, English install
- **Crate/module:** `d2-client::ui::panels` (`waypoint`, `npc`, `shop`)
- **Related specs:** `ui/panels.md` §2–§4 (`SetUIState`, open mode, cursor
  jump), §13 (waypoint art and rows), §14 (NPC option table, shop art),
  `ui/text.md` (fonts, width A, `DrawText`), `world/waypoints.md` (0x49),
  `world/npc.md` (0x2F / 0x30 / 0x36 / 0x38), `world/vendors.md` (0x32 /
  0x33 / 0x35, cost §9), `world/hirelings.md`, `render/camera.md` §3
  (tile origin), `render/sprite-placement.md` §2,
  `sim/client-messages.tsv` (layouts).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 36–47 |
| Inputs | 48–57 |
| Outputs / state changes | 58–62 |
| Rules | 63–64 |
|   1. Waypoint menu input (ui 0x14) | 65–121 |
|   2. NPC menu box | 122–197 |
|   3. Hire list (`0x004B5C60`, NPC option "hire") | 198–226 |
|   4. Shop transactions | 227–273 |
| Constants & data dependencies | 274–281 |
| Randomness | 282–285 |
| Edge cases & original bugs | 286–297 |
| Test vectors | 298–309 |
| Provenance | 310–326 |
| Open questions | 327–341 |
<!-- /index -->

## Summary

The waypoint menu takes mouse and key input through two mouse handlers
and one key handler that work in 640 × 480 panel coordinates; all close
paths share one latch, so each open sends C→S 0x49 at most once. The NPC
menu, the hire list and the "waiting for confirmation" note are one kind
of menu box: an object with a position, a size and up to ten text items,
placed around the NPC's screen point and drawn as a framed box with the
selected item in color 3. A shop click fills a pending transaction
(kind, item, price) and sends C→S 0x32, 0x33 or 0x35 at once or after a
confirm dialog.

## Inputs

| Name | Type | Source |
|---|---|---|
| mouse event | x u16 +0x0C, y u16 +0x0E | window message record (`ui/panels.md` §Inputs) |
| `W`, `H`, `sx`, `sy` | frame size, panel shift | `ui/panels.md` §1.1 |
| NPC interaction | NPC GUID `[0x007C0D25]`, active `[0x007C0D29]` | `world/npc.md` §2 |
| tile origin | view `+0x24`, `+0x28` | `render/camera.md` §3 |
| quest flags | `0x004B32D0()` / `[0x007C0D43]` | the client copy of the player's quest records |

## Outputs / state changes

C→S 0x49, 0x36, 0x38 (action 3), 0x32, 0x33, 0x35; `SetUIState` calls;
menu box draws; click sounds (`0x004B9A00(id, 0, 0, 0)`).

## Rules

### 1. Waypoint menu input (ui 0x14)

1. **Panel-local mouse.** The handlers use `x' = x − sx`, `y' = y + sy`
   (the 640 × 480 coordinates). Both ignore the event while
   `0x0047F210()` is true: ui 0x14 and 0x15 (mini panel) both open and
   the mouse strictly inside x (`W / 2 + a`, `W / 2 + a + 20 n`), y
   (`H − 76`, `H − 50`), with `a`, `n` = 56, 7 when `[0x007BC978]` ≠ 0,
   else 35, 8.
2. **Mouse down** (`0x0049D160`). If x > W / 2 or y > H − 49: send the
   latched close (rule 5) and `SetUIState(0x14, off, jump 1)`. Else:
   - tab = tab hit (rule 3); if not −1: if it differs from the current
     tab, click sound `0x004B9A00(6, 0, 0, 0)`; then set the tab
     (rule 4) and rebuild the rows (`0x0049C7F0`);
   - pressed row `[0x007BF06D]` := −1; if `273 ≤ x' ≤ 308` and
     `387 ≤ y' ≤ 420`: close pressed `[0x007BF06C]` := 1 and sound
     `0x004B9A00(4, 0, 0, 0)`; else pressed row := row hit (rule 3) and,
     if not −1, the same sound.
   The event is consumed when x ≤ W and y ≤ H − 49.
3. **Hit tests.** Tab (`0x0049C490`): none unless `y' ≤ 30` and `x' ≤
   320`; expansion installed and expansion game: tab 0, 1, 2, 3 for
   `x'` < 64, 128, 192, 256, else 4; otherwise 0, 1, 2 for `x'` < 80,
   160, 240, else 3 (no left or top bound). Row (`0x0049C510`): the
   first used row `r` (0–8, byte `[0x007BF040 + 5r]` ≠ 0) with
   `17 < x' < 297` and `hy < y' < hy + 30` (strict), `hy` = 60, 96,
   132, 168, 205, 241, 277, 313, 349 (fields +0x10 / +0x14 of table
   `0x007224E8`: x 17, `hy`); else −1.
4. **Tab set** (`0x0049C760(t)`): `t` ≥ `[0x007224E4]` (= 5) → 0; then,
   walking down from `t`: tab 4 needs quest record 28 bit 0
   (`0x0065C310` on `0x004B32D0()`), tab 3 record 23, tab 2 record 15,
   tab 1 record 7; a failed check sets `t` := that tab − 1 and the walk
   continues. Note: the tab **draw** (`panels.md` §13.3) tests record 26 for tab 4,
   the setter record 28 (reproduce). On open (`0x0049CF90`) the tab is
   set from the act of the player's level (`0x006427F0`), or 0 without
   a room / level.
5. **Latched close send.** Every C→S 0x49 level-0 send (`0x0049C6C0`,
   `0x0049C700`, `0x0049CEC0`, `0x0049CF50`, `0x0049C9C0`, `0x0049D160`)
   requires the latch `[0x007BF085]` = 0, a player and its room
   (`0x00620BB0`), and sets the latch. The row choice (rule 6) sets it
   too. Only the open (`0x0049CF90`, after the rows), `0x0049C6A0` and
   `0x0049C7F0` (tab > 5) clear it. So each open sends at most one 0x49:
   the row's, or one level 0.
6. **Mouse up** (`0x0049D010`). If x > W or y > H − 49: clear close
   pressed and pressed row, not consumed. Else, if close pressed: if
   `273 ≤ x' ≤ 308` and `387 ≤ y' ≤ 420`, `SetUIState(0x14, off, 0)`
   and `0x0049C6C0`; clear both, consumed. Else if pressed row ≠ −1 and
   the row hit now equals it: send 0x49 [wp GUID][level of the row:
   u32 at `0x007BF03C + 5r`], latch := 1, `0x0049C6C0` (sends nothing),
   clear both, `SetUIState(0x14, off, 0)`, sound `0x004B9A00(0x8B7, 0,
   0, 0)`; consumed. Otherwise clear both, consumed.
7. **Self-close** (`0x0049C9C0`, start of the draw): when there is no
   player or its mode is 0x11 (`0x00463DF0`): `SetUIState(0x14, off,
   0)` (its close hook sends the latched level 0), then the latched
   send (nothing left); the draw then **continues** for that frame.
8. **Key close** (`0x0049CEC0`): any key message except `WM_CHAR` with
   Tab (0x102 / 9) and `WM_SYSKEYDOWN` with F4 (0x104 / 0x73): latched
   send, `SetUIState(0x14, off, jump 1)`, consumed.

### 2. NPC menu box

1. **Menu object** (`0x004B7EB0`, x in ECX, y in EDX, then p1 … p9;
   0xB10 bytes, linked at `[0x007C0E54]`, registers window handlers 0x0E
   and 1): anchor x (+0x24, +0x2C), y (+0x28, +0x30); p1 callback (+0x60,
   required, else no box), p2 (+0x64), p3 (+0x18), p4 (+0x14, §2.4), p5
   auto layout (+0x58), p6 width (+0x34), p7 height (+0x38), p8 (+0x1C),
   p9 (+0x20); selected item (+0x44) := −1; style (+0x5C) := 2, changed
   by `0x004B8370` (style 1: the NPC menu and the hire list; style 0: the
   waiting note). Items (`0x004B85F0(box, text; h, a2, color, font,
   handler, selectable)`): at most 10 (fatal beyond), stride 0x110 from
   +0x68: text (cut to 119 units), height +0x158, a2 +0x164, font +0x168,
   color +0x16C (color 3 with style 1 is fatal), handler +0x170,
   selectable +0x174 (counted in +0x4C). The first selectable item added
   while none is selected becomes selected (`0x004B82F0`). With p5 = 0
   only the item's x offset is computed (§2.4, last sentence).
2. **NPC menu** (`0x004B4830`, built at interaction start and when a
   sub-menu goes back): without an active interaction or its NPC
   (`0x00463990(GUID, 1)`): end it (`0x004B3C20`), `SetUIState(8, off,
   0)`, done. For NPC classes 252, 198, 515 and 150 it first sends C→S
   0x38 [action 3 u32 @1][NPC GUID u32 @5][player GUID u32 @9, −1
   without a player] (13 bytes, `0x004786D0` at `0x004B48E8`; the
   hire-list request, `world/npc.md` §4). Box: anchor §2.6, p1
   `0x004B45D0`, p2 `0x004B4620`, p5 = 1, p9 = 1, the rest 0; style 1.
   Items: the NPC's name (`0x00464A60`), height 21, font 1 (Font16),
   color 4, not selectable; then, for the first record of `0x00726C48`
   with the NPC's class (`npc-menus.tsv`), option slots 0 … count − 2
   (captions §2.3), height 15, font 1, color 0, the slot's handler,
   selectable; then `lowercasecancel` (4142), height 15, font 1, color
   0, handler `0x004B45D0`, selectable.
3. **Captions** (`0x004B4A2F`–`0x004B4C74`), by the slot's string id:
   - 0x1507 (Resurrect, inserted by `ui/panels.md` §14.2):
     `hireresurrect2` (22696, "Resurrect %s: %d") with the mercenary's
     name (string id `[0x00725494]`, or string 11021 when that id is
     0x421) and the cost `[0x007C0DD0]`;
   - 3337 `NPCHeal`: the slot is shown only while life (stat 6) is below
     max life (`0x00625D10`): the string, then the heal cost
     (`0x00622DE0`; 0 is shown as 1) as `%d`;
   - 4020 `NPCIdentify1`: n = `0x0062A530(player)`; n = 0: the record's
     count := 2 and the slot is skipped; else count := 3 and, while quest
     record 4 bits 0 and 1 are both clear, `NPCIdentify2` (4021,
     "Identify Items: ") followed by `100 × n` as `%d`, else 4020 as is;
   - 11168: shown only when quest record 41 bit 0 is clear and (bit 1 is
     set, or the difficulty is 2);
   - any other id: the string as is.
4. **Auto layout** (`0x004B8410`, p5 ≠ 0; measured in font 1): `w` = max
   width A of the item texts + 20; `h` = sum of item heights + 15; x :=
   anchor x − w / 2; y := anchor y − (p4 ≠ 0 ? 21 × (number of not
   selectable items) : h / 3) (C division); fatal when w > W − 80 or
   h > H − 98; if x + w > W − 10: x := W − w; if y + h > H − 58: y :=
   H − h − 48; then x := 10 if x ≤ 10 and y := 10 if y ≤ 10; size :=
   (w, h). Each item (`0x004B83A0`): its width A in its own font; x
   offset = (w − width + 1) / 2 + 1 when width < w, else 0.
5. **Draw** (`0x004B8100`, the ui 8 step of `ui/panels.md` §5):
   background: with a cel (+0x0C ≠ 0) frame 0 of cel +0x08 at (x, y +
   h), mode 5; else the framed box `0x0046EFD0(x, y, w, h, 0, 1)`. Then
   the child widgets (list +0xB08), then each item `i` in its font: pen
   y = y + the heights of items 0 … i, pen x = x + its x offset, color =
   its color; the selected item: style 1 → color 3; style 2 →
   `cursor\Pentspin` (`[0x007A2880]`; 8 frames, offset x −1 … 21,
   offset y 0, measured on d2data) frame `c % 7` at (pen x − 24, pen y +
   4) and (pen x + width + 2, pen y + 4), mode 5, then `c` =
   `[0x007C0E50]` += 1 (frame 7 is never drawn); then `DrawText(text,
   pen x, pen y, color, 0)`.
6. **Anchor** (`0x004B1C80`): no box without the interaction's NPC.
   (px, py) = the NPC's client pixel point (`0x00620900`: static path
   +4 / +8 for unit types 2, 4, 5, else the dynamic path `0x006489C0` /
   `0x006489D0`); anchor x = px − tile origin x; anchor y = py − tile
   origin y − 150, raised to 20 when below 20.
7. **Waiting note** (`0x004B2828`–`0x004B28BB`; also after 0x36,
   `0x004B1F01`): a box at the §2.6 anchor, p1 `0x004B1DC0`, p5 = 1,
   p9 = 1, style 0, one item `TransactionResults1` (3353, "Waiting for
   confirmation of transaction..."), height 15, font 1, color 0, not
   selectable; deadline `[0x007C0D53]` = now + 60,000 ms. A second one
   while one exists is fatal.

### 3. Hire list (`0x004B5C60`, NPC option "hire")

1. Closes the NPC menu box. If a hire list is up (`[0x007C0D77]`): close
   it and its list widget `[0x007C0D83]`, rebuild the NPC menu
   (`0x004B4830`), done.
2. Box: x = (W − 490) / 2, y = (H − 40) / 2 − 195 (C division), p1
   `0x004B5C20` (back: close both, rebuild the NPC menu), fixed size
   490 × 350 (p5 = 0), p9 = 1; style 1. Items: `ItemDesc1s` (3364,
   "Your Gold: %d     Hire which Mercenary?") with gold = stat 14 +
   stat 15, height 21, font 1, color 4, not selectable; `Back` (3400,
   "cancel"), height 315, font 1, color 0, handler `0x004B5C20`,
   selectable.
3. List widget (`0x004BF8F0`) at ((W − 490) / 2, (H − 40) / 2 − 160),
   490 × 280, row callback `0x004B3660`; if it cannot be made: back. One
   row per used hireling record (10 records from `0x007C0C85`, stride
   16: name id u16 @0; used when the u32 @+8 ≠ 0); with none, the single
   row `ItemDesc1t` (3365, "There are no Mercenaries left to hire.").
   Row text: §Open questions 1.
4. **Choose** (`0x004B3660`): click sound 2; row = the widget's
   selection; `[0x007C0C60]` := row, `[0x007C0C64]` := NPC GUID; only
   for row < 10 whose item value is < 10. If the player has no hireling
   (`0x00478F20(P, 7)` = −1) and (classic game, or `0x00478EE0(P, 7)` = 0
   and `[0x00725494]` = 0xFFFF): `0x004B1E80`: C→S **0x36** [NPC GUID u32
   @1][the record's u16 @0, zero-extended, u32 @5] (9 bytes,
   `0x004786A0` at `0x004B1E9E`), close the list, `[0x007C0C6B]` := 10,
   waiting note (§2.7). Otherwise `0x004B3610`: close the list,
   transaction kind `[0x007C0D31]` := 5, confirm dialog `0x004B2F50`
   (§Open questions 2).

### 4. Shop transactions

1. **Click** (`0x004B3870`, from the shop-grid and body-location
   handlers of `ui/panels.md` §15; ECX = a unit passed by the caller,
   EDX = the item, then six arguments a1 … a6): needs the interaction's
   NPC (class `c`). Repair-all branch (a6 ≠ 0): only for `c` ∈ {154,
   178, 253, 257, 511} and only while the repair-all button is on
   (`0x00489870`); price := `0x0062FE60(0, ECX unit, c, difficulty,
   quest flags, 0)`, kind 3, item GUID 0, pending := 1, sent at once
   (§4.3). Else the item must not be at location 4 (`0x0062B400`), and
   with the quick flag (a5 ≠ 0) a click within 500 ms of the last send
   (`[0x007C0DF1]`) is refused.
2. **Kind and price** `[0x007C0D7F]` (cost `0x0062FDC0(player, item,
   difficulty, quest flags, c, t)`, `world/vendors.md` §9):
   - a1 ≠ 1 (store item): refused while the player has a cursor item;
     kind 1 (buy), t = 2 in a gamble shop (`[0x007C0DB0]` ≠ 0), else 0;
   - a1 = 1 (player item), by `c`: 147, 148, 177, 199, 202, 252, 254,
     255, 405, 512, 513, 514 → sell (t = 1); 154, 178, 253, 257, 511 →
     with the repair button on (`0x00489860`): repair (kind 3, t = 3) if
     the item is repairable (`0x004B1F80`), else refused; with it off:
     sell; 244, 245, 246, 265, 520 → kind 4 (no price) unless
     `0x006280A0(item, 0x10, …)` = 1 (refused); any other class:
     refused. A sell needs `0x0062A130(item)` ≠ 0 (else refused), then
     kind 2.
   - Then pending `[0x007C0DE0]` := 1, item GUID `[0x007C0DE5]`, item
     class `[0x007C0DE1]`. Kind 1 or 2 with the quick flag: sent at once
     (§4.3); every other case: click sound 1, `[0x007C0C6B]` := 4 and
     the confirm dialog `0x004B2F50` (§Open questions 2).
3. **Send** (`0x004B2650(1, flags)`; cancel `0x00487C20` without a
   pending transaction): kind 1 → 0x32, 2 → 0x33, 3 → 0x35 (click sound
   0x0F), other → cancel. Repair all: refused within 2,000 ms of the
   previous one (`[0x007C0E48]`). A missing item (`0x00463990(GUID, 4)`)
   aborts: pending := 0, no message. 17 bytes (`0x00478700`): [id][NPC
   GUID u32 @1][item GUID u32 @5][u32 @9][u32 @13]; `m` = the item's
   mode (unit +0x10, low 16 bits):

   | Msg | u32 @9 | u32 @13 |
   |---|---|---|
   | 0x32 buy | `m << 16`, bitwise OR 2 in a gamble shop, OR 0x80000000 when `flags` bit 2 is set and the item's `items.txt` record (`0x006335F0`) byte +0x1A5 ≠ 0 | price |
   | 0x33 sell | `m` | price |
   | 0x35 one item | `m` | the item's stat 72 (durability) value, not a price |
   | 0x35 repair all | 0 | 0x80000000 (item GUID 0) |

   Then `[0x007C0C6B]` := 5, sent time `[0x007C0DF1]` := now, and the
   waiting note (§2.7). For a sell, `[0x007C0D3F]` := whether the item
   is the player's cursor item.

## Constants & data dependencies

- Strings (English): 3337, 3353, 3364, 3365, 3400, 4020, 4021, 4142,
  22696; item heights 21 / 15; hire box 490 × 350, list 490 × 280.
- Waypoint row table `0x007224E8` (9 × 24 bytes), tab count
  `[0x007224E4]` = 5; quest records 7, 15, 23, 26, 28.
- `cursor\Pentspin.dc6` (d2data, 8 frames).

## Randomness

None.

## Edge cases & original bugs

Reproduced by default.

- Waypoint tab 4 is drawn on quest record 26 but selected on record 28
  (§1.4): with 26 set and 28 clear, a click on the drawn tab 4 opens
  tab 3 or lower.
- The waypoint self-close keeps drawing the menu in the frame it closes
  (§1.7).
- C→S 0x35 for one item carries the durability, not the price (§4.3).
- The pentspin counter uses `% 7` on an 8-frame file (§2.5).

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| 800 × 600, expansion game, waypoint open, mouse down (300, 80) | x' = 220, y' = 20 → tab 3 | §1.3 |
| 640 × 480, classic, mouse down (100, 10) | tab 1 | §1.3 |
| 800 × 600, rows 0–2 used, mouse (200, 200) | x' = 120, y' = 140 → row 2 (132 < 140 < 162) | §1.3 |
| waypoint: row chosen, then the close hook runs | one 0x49 (the row's), no level-0 message | §1.5, §1.6 |
| hire list at 640 × 480 | box (75, 25) 490 × 350; list (75, 60) 490 × 280 | §3.2, §3.3 |
| auto box, 3 items of widths 60, 90, 40, heights 21, 15, 15, p4 = 0, anchor (320, 200), 640 × 480 | w 110, h 66, x 265, y 178 | §2.4 |
| sell: NPC GUID 5, item GUID 9, mode 0, price 35 | `33 05 00 00 00 09 00 00 00 00 00 00 00 23 00 00 00` | §4.3 |

## Provenance

1.14d `Game.exe` (exports, `tools/ghidra/disasm.py`): waypoint
`0x0049D160`, `0x0049D010`, `0x0049C490`, `0x0049C510`, `0x0049C760`
(jump table `0x0049C7DC`), `0x0049C7F0`, `0x0049C6A0`, `0x0049C6C0`,
`0x0049C700`, `0x0049CEC0`, `0x0049CF50`, `0x0049CF90`, `0x0049C9C0`
(tab draw quest checks `0x0049CBB6`–`0x0049CBFE`), `0x0047F210`; menu
box `0x004B7EB0`, `0x004B85F0`, `0x004B83A0`, `0x004B8410`,
`0x004B8100`, `0x004B82F0`, `0x004B8370`, `0x004B1C80`; NPC menu
`0x004B4830`; hire `0x004B5C60`, `0x004B5C20`, `0x004B3660`,
`0x004B1E80`, `0x004B3610`; shop `0x004B3870` (class switches
`0x004B3B50` / `0x004B3B60`, `0x004B3BD8` / `0x004B3BE8`), `0x004B2650`;
message helpers `0x004786A0` (9 bytes), `0x004786D0` (13), `0x00478700`
(17). Table values (`0x007224E8`, switch tables) read from the image and
the `cursor\Pentspin` DC6 header measured with Python scripts outside
the repo. No capture yet.

## Open questions

1. Hire list row text and columns (`0x004B5E3B`–`0x004B6300`: strings
   3995–3997, 3366–3370, the `0x004B7C00(…, 0x23, 0x1E)` call, list
   vtable +0x30): read the row builder; check against a capture of
   Kashya's hire list.
2. The confirm dialog `0x004B2F50` (kinds 1–5): layout, buttons and which
   answer calls `0x004B2650`. Ghidra read.
3. Which callers of `0x004B3870` set the `flags` bit 2 of §4.3 (the 0x32
   bit 31) and the quick flag a5 (`0x00487BA0`, `0x00488B00`,
   `0x0048FFE0`, `0x00490780`, `0x00490BA0`, `0x00490FC0`,
   `0x00491AD0`, `0x00491D20`), and which unit the ECX argument is.
4. Pixel proof of §1–§3: a capture of the waypoint menu with a hovered
   row, Akara's menu and Kashya's hire list at 800 × 600.
