# Spec: UI — Control panel overlays (globes, bars, belt, skill buttons, run / menu buttons, mini panel, new-stats / new-skills buttons, help button)

- **Status:** draft (2026-10-07, RE on the 1.14d `Game.exe`; 2026-10-08
  REC-240 / REC-238: §5 r4, r5, r8, r13, r14, §3 r6, §6 r1; REC-252:
  §3 r6 font and 640 × 480, §4 r2 stamina tip as pop-up text and the
  `stambarblue` flag; 2026-10-09: §8 r6, §9 r9, §11 help button; no capture yet). Answers UP-28 (`ui/panels.md` §6 r3, §Open questions 1, control
  panel part). `ui/panels.md` §6 owns the border and the base art; this
  spec owns everything drawn on top of it and the control panel input.
- **Target version:** 1.14d, English install
- **Crate/module:** `d2-client::ui::control_panel`
- **Related specs:** `ui/panels.md` §1 (W, H, sx, sy), §2 (`SetUIState`),
  §4 (open mode), §5 (UI pass order), §6 (border and base art),
  §7 (shared parts); `ui/text.md` (fonts, width A, `DrawText`, vertical
  window §9); `render/blend-modes.md` §1 (draw modes), §8 (line,
  rectangle); `ui/messages.md` §5 r4 (font left by a refused bubble);
  `data/fields.tsv` (`belts`, `states` flag bits); `sim/stat-lists.md`
  §9.3 (state queries).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 45–58 |
| Inputs | 59–72 |
| Outputs / state changes | 73–80 |
| Rules | 81–82 |
|   1. Draw order (`0x00499450`) | 83–105 |
|   2. Art files | 106–119 |
|   3. Life and mana globes | 120–188 |
|   4. Experience and stamina bars | 189–238 |
|   5. Belt | 239–462 |
|   6. Run / walk and menu buttons | 463–483 |
|   7. Skill buttons | 484–514 |
|   8. New-stats and new-skills buttons | 515–588 |
|   9. Mini panel (state 0x15) | 589–702 |
|   10. Control panel mouse input | 703–743 |
|   11. Help button (state 0x22, `UI_HELPBUTTON`) | 744–828 |
| Constants & data dependencies | 829–844 |
| Randomness | 845–848 |
| Edge cases & original bugs | 849–861 |
| Test vectors | 862–905 |
| Provenance | 906–943 |
| Open questions | 944–1001 |
<!-- /index -->

## Summary

Step 7 of the UI pass (`0x00499450`) draws the 800 × 600 border, the
base art and then, in a fixed order, the life and mana globes, the
experience bar, the run / walk button, the stamina bar, the mini-panel
menu button, the belt (with its pop-up rows), the two skill buttons, the
belt item hover text and the mini-panel button tool tip. Step 8 adds the
life / mana numbers, the new-stats / new-skills buttons and the mini
panel. Globe and stamina values are smoothed over a few client updates.
All positions are relative to the frame size W × H, so the 640 × 480
and 800 × 600 layouts differ only where a rule says so (resolution mode
2 = 800 × 600). Mouse down / up / move handlers on the game window give
the press, release and hover behavior.

## Inputs

| Name | Type | Source |
|---|---|---|
| `W`, `H` | frame size `[0x0071146C]`, `[0x00711470]` | `ui/panels.md` §1 |
| resolution mode | `0x004F5160` (2 = 800 × 600) | `ui/panels.md` §Inputs |
| video mode | `0x004F5140` (6 changes two x positions) | `ui/panels.md` §Inputs |
| open mode | `0x0045AE90` | `ui/panels.md` §4 |
| local player P | stats 6–11 (life, mana, stamina ×256), 12, 13, 26, 74; states 2, 100, 106, 136, group 24 | model |
| client update counter C | `[0x007A0498]` | `audio/triggers.md` §1 r5 |
| mouse | `0x00468730()` x, `0x00468740()` y; message x u16 +0x0C, y u16 +0x0E | input |
| game type | `[0x007A0610]` (`0x0044DB30`; 0 = single player) | `client/msg-ui.md` §7 |
| belt record | `belts.bin` record (resolution `[0x007A5218]` · 7 + belt type) | §5 |

## Outputs / state changes

Draws; hover tool tips (`0x00502280(text, x, y, color, centre)`, drawn
by `0x00503000` in UI pass step 10, §5 r14; centre = 1 → x is the text centre);
`SetUIState` calls (3, 6, 7, 0x15, 0x21, 0x22, mini-panel targets); run / walk toggle
(`0x0044BE80`); registry `Show HP Text`, `Show MP Text`, `PopupHireling`
(§9), `Mini Panel` (§9 r9), `Help Menu` (§11); UI sound 4 on presses.

## Rules

### 1. Draw order (`0x00499450`)

1. Resolution mode 2 only: left border when the open mode is 2 or 3,
   right border when it is 1 or 3 (`ui/panels.md` §6 r1).
2. Base art `0x004983D0` (`ui/panels.md` §6 r2).
3. Life globe `0x00496F80` (§3), mana globe `0x00497110` (§3),
   experience bar `0x00498EA0` (§4), run / walk button `0x00497480`
   (§6), stamina bar `0x004975B0` (§4), menu button `0x004977C0` (§6),
   belt `0x00499040` (§5), skill buttons `0x00496CF0` (§7), belt item
   hover text `0x00497A40` (§5 r8).
4. Level-change timer: when P exists and its level id
   (`0x0061A1B0(P)`) differs from `[0x007BEFF0]`: `[0x007BEFF0]` := it,
   `[0x007BEFEC]` := 60 (§Open questions 4).
5. Mini-panel menu button tool tip `0x00498340` (§6 r4).

Step 8 of the UI pass (`ui/panels.md` §5) then runs, in its order: the
level-up button sync `0x004A64C0` (§8 r6, draws nothing), the help
button (`[0x22]` → `0x00495180`, §11), the
new-stats button (`[6]` → `0x004A6B30`, else `0x004A6A70`), the
new-skills button (`[7]` → `0x004A6E60`, else `0x004A6DA0`) (§8), the
life / mana numbers `0x00498120` (§3 r6) and, with state 0x15 open, the
mini panel `0x0047F710` (§9).

### 2. Art files

Loaded by `0x004967F0` (`0x004520C0`, `"%s\UI\"` + name): `Panel\Hlthmana`
`[0x007BEF68]`, `Panel\overlap` `[0x007BEF80]`, `Panel\runbutton`
`[0x007BEF0C]`, `Panel\menubutton` `[0x007BEF70]`,
`Panel\ctrlpnl_popbelt` `[0x007BEF10]`, `Panel\Level` `[0x007C02DC]` and
`Panel\Levelsocket` `[0x007C02E0]` (`0x004A6460`), `Panel\minipanel` /
`Panel\minipanel_s` `[0x007BC960]` and `Panel\minipanelbtn`
`[0x007BC964]` (`0x0047F0C0`, §9 r1); also loaded there and drawn
elsewhere: `Panel\btn`, `InvChar6` / `InvChar`, `Arrow`, `Invwarn`,
`DuroIcon`, `GemSocket`, `menupanel`, `skillpoints`, `hostilepic`.
Every cel draw below is `CelDraw` (`0x004F6480`) with light 0xFF and
draw mode 5 unless a rule says otherwise.

### 3. Life and mana globes

1. **Shown value** (`0x00496DD0(stat)`, stat 6 life, 8 mana, 10
   stamina; record k = 0, 1, 2 at `0x007BEF30 + 16k`: last value +0,
   last change C +4, start value +8, start C +0x0C). v = P's stat, m =
   its max (`0x00625D10` / `0x00625D60` / `0x00625DB0`); m = 0 → 0.
   1. C ≤ 1: start := last := v.
   2. Life only: v < last → start := last := v and both times := C
      (drops show at once).
   3. v ≠ last: if |last − v| ≥ m / 8 (C division): start := v and
      start C := C; else start C := the last change C and start := v
      when v = m, else the old last. Then last := v, last change C := C.
   4. e = C − last change C; d = last change C − start C clamped to 7…15.
      If v > 0, e < d and last ≠ start: shown = start + `_ftol`(((e + 1)
      / d) · (last − start)) (x87: `fild`, `fidiv`, `fimul`, truncation
      toward 0, binary64 at PC = 53, not the integer product; §Open questions 1); else shown = v. Then clamp to 0…m.
2. **Life** (`0x00496F80`): m = max life; m = 0 → nothing (no globe, no
   overlay). f = (80 · shown) / m (C division); f = 1 or 2 with P a
   living player (type 0, mode ≠ 0x11) → 2.
   - Healing potion (state 100 `healthpot`): s = min(stat 74 · 80 / 100,
     80) (C division); if s > f: `Hlthmana` frame 0 with the vertical
     window `0x004F64E0(x, H − 13, skip f, lines s − f, mode 0)`, x = 28
     in video mode 6, else 29.
   - Fill: frame 2 when poisoned (state 2), else 0; f ≠ 0 →
     `0x004F64E0(29, H − 13, skip 0, lines f, mode 5)`.
   - Cover: `overlap` frame 0 at (28, H − 5).
3. **Mana** (`0x00497110`): shown := min(shown, m); f = 80 · shown / m
   (no minimum). Mana potion (state 106 `manapot`): s = min(stat 26 ·
   80 / 100, 80), s > f → `Hlthmana` frame 1, window (x, H − 13, skip f,
   lines s − f, mode 0), x = W − 112 in video mode 6, else W − 111. Fill:
   frame 1 at (W − 111, H − 13), skip 0, lines f, mode 5. Cover:
   `overlap` frame 1 at (W − 110, H − 9).
4. The window rows are counted from the cel's bottom row (`ui/text.md`
   §9 r2), so the fill grows upward; 80 rows = full.
5. **Text toggles** (mouse down, §10 r1): x 30…110, y H − 75…H − 15
   toggles `Show HP Text` `[0x007BEFDC]`; x W − 111…W − 31, same y,
   toggles `Show MP Text` `[0x007BEFE0]` (both inclusive; each stored at
   once with `0x004150E0`; read at start, `0x004967F0`).
6. **Numbers** (`0x00498120`, current font, i.e. font 1 after the belt
   draw of step 7): life when `Show HP Text` is on or the mouse is in
   the life toggle rectangle: `panelhealth` (4165, "Life: %d / %d")
   with shown >> 8 (raised to 1 when ≤ 1 and P is a living player) and
   m >> 8 (arithmetic shifts), `DrawText` at (65 − w / 2, H − 95), color
   0; mana likewise with `panelmana` (4166) at (W − 80 − w / 2, H − 95)
   (w = width A, C division).
   Checked for REC-238 (`0x00498185`–`0x004982FB`): `0x00498120` sets
   no font (Font16 in the plain game view, left by §5 r4); the text is
   formatted into a 100-unit buffer (`0x005269D0(100, buffer, fmt,
   …)`); w = width A (`0x00501820`) of the formatted text, halved by
   C division (`cdq; sub; sar`); `DrawText(text, 65 − w / 2, H − 95,
   color 0, not centred)` (`0x00502320`) for life, `(W − 80 − w / 2,
   H − 95)` for mana. No backing box: these are plain text draws, not
   pop-up text (§5 r14).
   Settled for REC-252 (2026-10-08): the font is font 1 (Font16) in
   every frame. The belt draw `0x00499040` sets it unconditionally as
   its first act (`0x00499053`) and is called unconditionally by step 7
   (`0x004994BF`); none of the later step-7 draws (`0x00496CF0`,
   `0x00497A40`, `0x00498340`) nor the step-8 buttons (`0x004A6A70`,
   `0x004A6B30`, `0x004A6DA0`, `0x004A6E60`) calls the font setter
   `0x00502EF0`. The divisions by 256 are `sar 8` (arithmetic) here;
   the "≤ 1" raise is an unsigned compare (`cmp esi, 1; ja`), so shown
   0 and 1 become 1 (shown is never negative after r1's clamp). The
   formatted text is copied into a second 100-unit buffer
   (`0x00526700`, a wide-string append onto the zeroed buffer) before
   the width and the draw; that copy changes nothing visible. Neither
   function has a resolution branch: the 640 × 480 positions are the
   same formulas with W = 640, H = 480 (life (65 − w / 2, 385), mana
   (560 − w / 2, 385)).

### 4. Experience and stamina bars

1. **Experience** (`0x00498EA0`): L = stat 12, X = stat 13; next =
   `experience` value of P's class at the base level (`0x006253B0(P, 12)`,
   `0x00611800`), prev = value at L − 1; span = next − prev; cur = X −
   prev when X > prev, else 0.
   - Hover x W/2 − 146…W/2 − 23, y H − 43…H − 34 (inclusive): tool tip
     `panelexp` (4163, "Experience: %u / %u") with X, next at (W/2 −
     146, H − 51), color 0, centred.
   - Bar: span ≠ 0; when cur ≥ 36,092,162 both cur and span >> 7; px =
     119 · cur / span (unsigned), px > 119 → 0. Only while L < the class
     max level (`0x00611830`) and px > 0: two lines color 0xFF
     (`render/blend-modes.md` §8 r1) from (W/2 − 144, y) to (W/2 − 144 +
     px, y) for y = H − 38 and H − 37.
2. **Stamina** (`0x004975B0`): colors (`0x004FB180`, nearest palette
   index, computed once): red (255, 0, 0), gold (244, 192, 76), blue (0,
   0, 255). v = shown stamina, m = max. Color gold, scale m; if m + 5 <
   v or P has a state of group 24 (`stambarblue`, `0x0063A7B0`): scale :=
   v, color blue. "Group 24" is the `states` flag bit 24 (`stambarblue`,
   `data/fields.tsv`), not the `group` column: `0x0063A7B0(unit, k)`
   (k ≤ 0x27) ANDs the mask of every state with flag k (`[0x00744304]`
   + 0xCC + 4k, built at load) with the unit's state bits (`0x0063A130`,
   `0x00625BB0`); any common bit → true. w = 0 when scale ≤ 0, else 102 · v / scale; w < 25 →
   red. Rectangle `0x0046EFD0(W/2 − 127, H − 27, w, 18, color, mode 2)`
   (= `DrawRectangle(x, y, x + w, y + 18, color, 2)`).
   - Hover x W/2 − 127…W/2 − 25, y H − 27…H − 9: tool tip `panelstamina`
     (4164, "Stamina: %d / %d") with v >> 8 and m >> 8 at (W/2 − 76, H −
     52), centred, color 0 — or color 3 with the value shown as the max
     when v >> 8 > m >> 8 or P has state 136 (`shrine_stamina`).
   - Settled for REC-252 (2026-10-08, `0x00497684`–`0x00497766`): the
     tip is **pop-up text** (§5 r14), not a `DrawText`:
     `0x00502280(text, W/2 − 76, H − 52, colour, centre 1)` (W/2 by C
     division, `cdq; sub; sar 1`). v and m are read again (stat 10 shown,
     `0x00496DD0`, and the max `0x00625DB0`) and divided by 256
     rounding toward zero (`cdq; and edx, 0xFF; add; sar 8`; equal to
     `>> 8` for the non-negative values that occur). Colour 0
     (no remap, `ui/text.md` §5) normally; colour 3 (blue, (105, 105,
     255) in the act 1 palette, `ui/text.md` §5 table) and the
     first number := m / 256 when v / 256 > m / 256 (signed) or P has
     state 136 (`0x00639DF0(P, 0x88)`). Text: string 4164 formatted
     with (first number, m / 256) into a 100-unit buffer (`0x005269D0`).
     The mouse test (`0x00468730` / `0x00468740`, both ends inclusive)
     runs after the bar is drawn, every frame, with no other condition.
     Drawn by `0x00503000` at UI pass step 10 in the current font, i.e.
     Font16 (font 1, set by the belt draw after this function, §5 r4)
     over the colour-0 mode-2 box of §5 r14; its box is centred on W/2 −
     76 and its bottom is H − 50 (y0 = y + 2). One slot: a later
     `0x00502280` caller in the same frame replaces it (§5 r14). No
     resolution branch (640 × 480: x 244, y 428).

### 5. Belt

1. **Belt type** `[0x00722354]` (each draw, `0x00499040`): the item in
   P's body location 8 (`0x0063BDE0(inventory, 8)`) of item type 19
   gives `0x00621ED0(item)` (< 0 fatal 0xB07); no belt item → 2
   (`default`); an item of another type leaves it unchanged. The record
   is `belts.bin` record `[0x007A5218] · 7 + type` (`0x00660CB0`): box
   count u8 +4, boxes from +8 (16 bytes: left, right, top, bottom).
   `belts.txt` order: 0 belt, 1 sash, 2 default, 3 girdle, 4 light belt,
   5 heavy belt, 6 uber belt (the `Expansion` row is not a record);
   records 7–13 are the 800 × 600 copies (boxes at x 423–545, y 530–591
   vs 343–465, 410–471).
2. **Pop-up flag** `[0x007BEF98]`: cleared at the belt draw when state
   0x1F (belt rows) is closed and `[0x007BEF9C]` = 0, and whenever state
   9 (game menu) is open; set by the mouse-move handler (r9) and by the
   state 0x1F draw (`0x00498E90`).
3. **Pop-up rows** (popped only): by type: 0 → rows 0, 1; 1 → row 0; 2
   → none, and `[0x007BEFA0]` := 0 (only 4 boxes); 3 → rows 0, 1, 2; 4
   → row 0; 5 → rows 0, 1; other → rows 0, 1, 2; `[0x007BEFA0]` := 1 for
   every type but 2. Row k: `ctrlpnl_popbelt` frame 0 at (W/2 + 21, H −
   41 − 32 k).
4. **Slots** (font 1 for the rest of the frame, never restored): box i
   = 0 … count − 1, boxes > 3 only while `[0x007BEFA0]` = 1; the item
   in belt slot i (`0x0063C7F0`); colors (`0x004972B0`): red (128, 0, 0)
   `[0x007BEF6C]`, green (0, 128, 0) `[0x007BEF6D]`, blue (0, 0, 128)
   `[0x007BEF6E]`, (128, 128, 0) `[0x007BEF6F]`. Box rectangle = record
   box i (left, right, top, bottom).
   - a usable item (`0x006280A0(item, 4)` = 0, `0x00628C20(item)` ≠ 0
     and `0x004C2240(item)` = 0): when it is the hovered item
     `[0x007BEFA8]` and the belt is hovered `[0x007BEF94]`, a green
     rectangle 29 × 29 at (left, top) (`0x0046EFD0(…, 29, 29, green,
     mode 0)`); else no rectangle;
   - any other item: a red rectangle 29 × 29 at (left, top), mode 0;
   - rectangle primitive (`0x004992FD`): `0x0046EFD0(x, y, w, h,
     color, mode)` = `DrawRectangle(x, y, x + w, y + h, color, mode)`
     (`0x004F6300`, `render/blend-modes.md` §8 r2): pixels x … x + 28,
     y … y + 28, the color a palette index (nearest match of the RGB
     triple, `0x004FB180`, recomputed by `0x004972B0` at every slot),
     draw mode 0 = blend kind 2, `T[256·d + color]` (the same tint
     primitive and mode as the grid tints, `ui/inventory.md` §2 r2–r3). It is drawn **before** the item,
     so the item covers it;
   - the item drawn with `0x0046EE80(item, left, top)`;
   - boxes 0–3 of a usable item (`0x00628C20` ≠ 0) get a key label
     (`0x00499327`–`0x004993EF`): the command of slot i is
     `0x00722404`[i] = 23, 24, 25, 26 (`CfgBelt1`–`4`, read from the
     binary). No label when both its slot-1 key (`0x00469AA0(cmd, 1)`)
     and its slot-0 key (`0x00469AA0(cmd, 0)`) are 0xFFFF. Else the
     text is **always** the short key name of the slot-1 entry
     (`0x0046A530(cmd, 1)`, r13), even when only slot 0 is bound (then
     slot 1's key is 0xFFFF and the label is 3762 "None": reproduce).
     The live binding table `0x007A6F90` is read at every draw, so a
     rebound key shows its new name at once. The name is copied into a
     100-unit buffer, then units are cut from the end (one per step)
     until width A (`0x00501820`) ≤ 28; `DrawText` (`0x00502320`) at
     (left + 2, bottom − 2), color 4, font 1 (set at the start of the
     belt draw, `0x00499053`). Default keys `1`–`4` (VK 0x31–0x34,
     slot 1; slot 0 unbound, `ui/controls.md` §3) → labels `1`–`4`.
5. **Cursor-item highlight** (`0x00497920`, when type ≠ 2, state 0x1F is
   open or the belt is popped): with an item on the cursor and the belt
   hovered, the hovered box `[0x0072235C]` (0 ≤ it < count): empty and
   the item fits a belt (`0x0062BAD0`) → green; occupied and a swap is
   possible (`0x0063C830`) → (128, 128, 0); else red; rectangle 29 × 29
   at the hovered box's (left, top) (`0x004978D0`, the same
   `0x0046EFD0(…, 29, 29, color, 0)` primitive as r4), drawn after
   every slot, so over the items.
6. **Hit area** (`0x00498DC0`): popped: x from box (count − 4).left to
   box 3.right, y from box (count − 4).top to box 3.bottom; not popped:
   y > H − 48, x W/2 + 23…W/2 + 145, y H − 39…H − 10 (inclusive).
7. Row count `0x004979F0` (types 0–5: 3, 2, 1, 4, 2, 3; else 4).
8. **Item hover text** (`0x00497A40`): belt hovered `[0x007BEF94]`, a
   hovered item `[0x007BEFA8]`, no cursor item (`0x0063C1E0`), and
   (hovered box `[0x0072235C]` < 4 (signed) or popped `[0x007BEF98]` or
   `[0x007BEF9C]`). The item must resolve by its GUID (`0x006335F0`,
   none → fatal 0x61D). This is **not** the inventory item tool tip
   (`0x0048DD90`, `ui/inventory.md`): it is a short text of two parts.
   1. Name N := `0x0048C060(item, buffer, 128)` (the client item name,
      the same call as the shop name of `ui/menus.md` §4, 128 units).
   2. Stats S := empty; unless the item's quality (`0x00627E70`) is 3
      (superior, `items/quality.md`), S := `0x004E6410(buffer, item,
      0x100, 1, 0)` (the item's property lines, 256 units).
   3. Text T (384 units) := `Prefix(S, 3)` then `Prefix(N, 0)` appended
      (`0x004521C0`, `0x005267E0` copy, `0x00526700` append;
      `ui/messages.md` §3: an empty part gets no colour code). The
      text is drawn from its first line at the bottom upward (LF
      moves the pen up, `ui/text.md` §7), so the stat lines (colour 3,
      blue) are the lower lines and the name (colour 0, white) is drawn
      above them, as the stat-line builder leaves them (its line breaks
      are `0x004E6410`'s, specified in `ui/item-tips.md` §6).
   4. Only when the text position (`[0x00722360]`, `[0x00722364]`) is ≥
      0 in both: when an NPC trade inventory mode is open
      (`0x00489840`: `[0x007BCBF0]` ∈ {1, 2, 3}) and
      `0x004B2AD0(item, sell 1, …, price buffer, 64)` returns non-zero
      with a non-empty price text, T += 3998 `newline` ("\n") + the
      price text (so the price is the top line, above the name). Then
      `0x00502280(T, x, y, color 0, centre 1)`: the pop-up text of r14
      (font, frame, placement).
9. **Mouse move** (`0x00499BB0`, only in game `[0x007A061C]`, input not
   blocked `0x0044DA30` = 0, P alive, `0x0044BFE0` = 0): popped and over
   the belt → hover tracking `0x00498930` (consumed while state 0x1F is
   open); popped and off the belt → `[0x007BEF9C]` := `[0x007BEF94]` := 0
   and, with state 0x1F closed, popped := 0 (the rows fold down); not
   popped and inside the strip x W/2 + 23…W/2 + 145, y H − 39…H − 10 →
   unless `0x00453A90(9)`, popped := `[0x007BEF9C]` := 1 and hover
   tracking; consumed; elsewhere `[0x007BEF94]` := 0. Hover tracking
   details: r12.

10. **Box hit** `0x004987E0(x EDI, y)`: the first box `i` (0 … count −
    1) of the current belt record (r1) with left ≤ x ≤ right and top ≤ y
    ≤ bottom (inclusive); none → −1.
11. **Belt click** `0x00498870(inventory, x, y)` (from §10 r2): box `b`
    (r10), none → nothing; cursor item `c` (`0x0063C1E0`), belt item `e`
    = slot `b` (`0x0063C7F0`). Cursor state 6 → nothing. When `c` exists
    and fits a belt (`0x0062BAD0`): unless `c` is blocked
    (`ui/inventory.md` §9 r2), C→S **0x25** [`c` GUID][`e` GUID] when
    `e` exists, else C→S **0x23** [`c` GUID][`b` u32], then
    `0x004C21F0(c)`; in every case of this branch the item's put sound
    `0x004B9A00(0x004C1D60(c, 0, 0, 0), 0, 0, 0)`. Otherwise (no `c`, or
    `c` does not fit): only with `e`, no `c` and `e` not blocked: C→S
    **0x24** [`e` GUID] (`0x00478680`), `0x004C21F0(e)`.
12. **Hover tracking** `0x00498930(inventory, x, y)` (§5 r9): box `b`
    (r10), none → nothing; cursor state 6 or 8 → nothing. With a cursor
    item: hovered box `[0x0072235C]` := `b`, belt hovered `[0x007BEF94]`
    := 1, hovered item `[0x007BEFA8]` := last `[0x007BEFAC]` := 0. Else
    `e` = slot `b`: hovered box := `b`; no `e` → belt hovered := 0,
    hovered := last := 0; `e` → belt hovered := 1 and, when `e` ≠ last:
    hovered := last := `e`, text position (`[0x00722360]`,
    `[0x00722364]`) := (box.left + 14, box.top) of `e`'s own box
    (`0x00660D10` with `e`'s x position `0x0045ADF0`). Reset
    `0x00498D60`: hovered box and text position := −1, belt hovered,
    hovered, last := 0; `0x00498E80`: hovered, last := 0.
13. **Key names** (answers REC-240 part 1). Both take (command ECX,
    slot EDX), find the **first** live entry (`0x007A6F90`, 10-byte
    entries, `ui/controls.md` §1 r1) with that command and slot, and
    name its key k (no entry → k = 0xFFFF).
    - **Long name** `0x00469DE0` (key-config screen, mini-panel tips
      §9 r6): k = 0xFFFF → 3762 `KeyNone` "None"; k in the "long" column
      of the table below → that string; any other k → a one-unit string
      holding the low byte of k (static buffer `0x007A741C`, so VK 0x31
      → "1", VK 0x41 → "A"; also every k ≥ 0xE0 other than 0x100–0x104).
    - **Short name** `0x0046A530` (belt labels r4, run tip §6 r1): k in
      the "short" column → that string; any other k (including 0xFFFF)
      → the long name `0x00469DE0` of the same (command, slot).

    Strings (ENG `string.tbl`; jump tables `0x0046A448` / `0x0046A2CC` /
    `0x0046A2B8` long, `0x0046A73C` / `0x0046A6BC` short):

    | VK | Long id, text | Short id, text |
    |---|---|---|
    | 0x01, 0x02, 0x03, 0x04 | 3763 "Mouse 1", 3764 "Mouse 2", 3765 "Cancel", 3766 "Mouse 3" | — |
    | 0x08 | 3790 "Backspace" | 3884 "bks" |
    | 0x09, 0x0C, 0x0D | 3791 "Tab", 3792 "Clear", 3793 "Enter" | — |
    | 0x10, 0x11 | 3794 "Shift", 3795 "Ctrl" | 3887 "sft", 3888 "ctl" |
    | 0x12, 0x13, 0x14 | 3796 "Alt", 3797 "Pause", 3798 "Caps Lock" | — |
    | 0x15, 0x17, 0x18, 0x19 | 3771 "Kana", 3772 "Junja", 3773 "Final", 3774 "Kanji" | — |
    | 0x1B | 3775 "Escape" | 3870 "esc" |
    | 0x1C–0x1F | 3776 "Convert", 3777 "Non-Convert", 3778 "Accept", 3779 "Mode Change" | — |
    | 0x20 | 3799 "Space" | — |
    | 0x21, 0x22 | 3800 "Page Up", 3801 "Page Down" | 3892 "pup", 3893 "pdn" |
    | 0x23, 0x24 | 3802 "End", 3803 "Home" | — |
    | 0x25–0x28 | 3780 "Left", 3781 "Up", 3782 "Right", 3783 "Down" | — |
    | 0x29, 0x2A, 0x2B | 3784 "Select", 3804 "P - Tell Ken", 3785 "Execute" | — |
    | 0x2C, 0x2D, 0x2E | 3805 "Print Screen", 3806 "Insert", 3807 "Delete" | 3895 "psn", 3896 "ins", 3897 "del" |
    | 0x2F | 3808 "Help" | — |
    | 0x5B, 0x5C, 0x5D | 3786 "Left Windows", 3787 "Right Windows", 3788 "Apps Menu" | — |
    | 0x60–0x69 | 3809–3818 "Num Pad 0"–"Num Pad 9" | 3899–3908 "np0"–"np9" |
    | 0x6A, 0x6B | 3819 "Num Pad *", 3820 "Num Pad +" | 3909 "np*", 3910 "np+" |
    | 0x6C | 3821 "Separator" | 3912 "np." |
    | 0x6D | 3822 "Num Pad -" | 3911 "np-" |
    | 0x6E | 3823 "Num Pad ." | — |
    | 0x6F | 3824 "Num Pad /" | 3913 "np/" |
    | 0x70–0x87 | 3825–3848 "F1"–"F24" | — |
    | 0x90 | 3789 "Num Lock" | 3883 "nml" |
    | 0x91 | 3849 "Scroll Lock" | 3914 "slk" |
    | 0xBA–0xC0 | 3850–3856 ";", "=", ",", "-", ".", "/", "~" | — |
    | 0xDB–0xDE | 3857–3860 "[", "\\", "]", "'" | — |
    | 0x100 | 3766 "Mouse 3" | 3861 "m3" |
    | 0x101, 0x102 | 3767 "Mouse 4", 3768 "Mouse 5" | 3862 "m4", 3863 "m5" |
    | 0x103, 0x104 | 3769 "Mouse Wheel Up", 3770 "Mouse Wheel Down" | 3864 "mwu", 3865 "mwd" |

    Quirk (reproduce): the short table gives VK 0x6C (Separator) "np."
    and VK 0x6D (Subtract) "np-", while VK 0x6E (Decimal) has no short
    name and shows "Num Pad ." — not a swap of 0x6D / 0x6E but the
    table as compiled. Digits and letters (VK 0x30–0x39, 0x41–0x5A)
    and every VK not listed are the one-unit low-byte string.
14. **Pop-up text** (the hover tool tip of every `0x00502280` caller in
    this spec; answers REC-238). `0x00502280(text ECX, x EDX, y, color,
    centre)` copies the text (only when non-null and shorter than 1,024
    units; else the buffer is cleared) to `0x00841EC8` and stores x
    `[0x008426C8]`, y `[0x008426CC]`, color `[0x008426D0]`, centre
    `[0x008426D4]` and bar `[0x008426D8]` := 0. **One slot**: a later
    call in the same frame replaces an earlier one; the UI pass clears
    it at step 1 (`ui/panels.md` §5). It is drawn once, by `0x00503000`
    (UI pass step 10, after the control panel, step 8 and the NPC menu):
    1. Nothing when the text is empty. **Font: the current font**
       (`[0x0072DFFC]`; this call sets none at first). In a frame the
       belt draw (§5 r4) sets font 1 (Font16) and no later draw of
       steps 8–10 before `0x00503000` sets another without restoring it
       in the plain game view, so the control-panel tips are Font16.
    2. W = max width (`0x00501840`) + 8; Ht = text height
       (`0x005019C0`); x0 = x − (W >> 1) (arithmetic) when centre = 1,
       else x; y0 = y + 2; (Sw, Sh) = the screen size (`0x004F59B0`).
    3. Too tall: when Ht > Sh − 10: font 1 → font 0 (Font8) and back
       to 2; else when the current font is 0 → font 6 (Font6) and back
       to 2; else (any other font) draw as is.
    4. x' = 0 when x0 ≤ 0, else x0; x' = min(x', Sw − W).
       b = max(y0, Ht − 5) when that is < Sh − 5, else b = Sh − 5.
    5. Backing: bar = 0 (every `0x00502280` caller) →
       `DrawRectangle(x', b − Ht, x' + W, b, color 0, mode 2)`
       (`render/blend-modes.md` §8 r2: mode 2 is blend kind 2, each
       pixel `T[256·d + 0]` with the mode-2 table `T`). (bar ≠ 0, only through `0x005022F0`: the first
       bar · W >> 7 pixels are palette colour (80, 0, 0) in mode 1, the
       rest colour 0 mode 2; not used by this spec.)
    6. Text: `0x00501A80(text, x', b − (line-height byte +0x0A of the
       font × [0x0072E008]) / 10, block width W, color, centred 1)`
       (`ui/text.md` §7 with W as the block); `[0x0072E008]` = 3 for
       English (`0x00502C60`), so b − 3 for Font16 (line-height byte
       10). Lines are centred in the block and go up from the bottom.
    7. The font of step 1 is restored if step 3 changed it.

    So a centred tip's centre is x, its box is W wide (text width + 8)
    and its bottom is y + 2 (when above Ht − 5, on screen); the colour
    argument is the text colour of the first line (colour codes inside
    the text override it).

### 6. Run / walk and menu buttons

1. **Run / walk** (`0x00497480`): frame = 2 while running
   (`0x0044BE90` ≠ 0), else 0; + 1 while pressed (`[0x007BEFD8]`) with
   the mouse inside x W/2 − 145…W/2 − 128, y H − 28…H − 8 (inclusive,
   `0x00497440`); `runbutton` at (W/2 − 145, H − 10). Hover in that
   rectangle → tool tip (`0x00497300`): `RunOn` (4179, "Run"), then for
   slot 1 and then slot 0 of command 0x23, each only when that slot's
   key (`0x00469AA0`) ≠ 0xFFFF, ` (%s)` (4178) formatted (30 units,
   `0x005269D0`) with the **short** key name `0x0046A530(0x23, slot)`
   (§5 r13), at (W/2 − 145, H − 23), color 0, centred; drawn as §5 r14.
2. **Menu button** (`0x004977C0`): frame = 2 while state 0x15 (mini
   panel) is open, else 0; + 1 while pressed (`[0x007BEFD0]`) inside x
   W/2 − 8…W/2 + 5, y H − 39…H − 13 (`0x00497780`); `menubutton` at
   (W/2 − 8, H − 16).
3. Release actions: §10 r2.
4. **Tool tip** (`0x00498340`): mouse in the menu-button rectangle →
   `panelcmini` (4168, "Close Mini Panel") while state 0x15 is open,
   else `panelmini` (4167, "Open Mini Panel"), at (W/2 − 1, H − 39),
   color 0, centred.

### 7. Skill buttons

1. `0x00496CF0`: left skill (`0x00620190(P)`) and right skill
   (`0x006201D0(P)`). A skill whose level for P is ≤ 0
   (`0x006442A0(P, skill, 1)`) is replaced in the model before the draw
   (`0x00643CE0`, `0x006470F0`, then `0x00643BC0(P, 0, −1)` left /
   `0x00643C50(P, 0, −1)` right, then re-read): a model write in a draw
   (cross-file request).
2. Icons (`0x00496BE0(P, skill, x, y, left)`): left at (117, H), flag
   1; right at (W − 165, H), flag 0. The icon file is the class file of
   the skill's `skilldesc` (`0x004A8C80`, `ui/panels.md` §10.3) and the
   frame its `IconCel` (byte +7 of the record, `0x004A9690`); state from
   `0x004A8D30(P, skill)` (2026-10-09, one read): u := the skill-use
   check `0x004D9FC0(P, skill)` (`client/stat-lists.md` §3 r6.8:
   `0x00647960` codes, plus 8 while the local cast lock runs); u = 0 → 0
   (usable), u = 6 (`aura`) → 4, any other u (1, 2, 3, 4, 5, 7, 8) → 1.
   Then (`0x00496C24`…`0x00496C42`) the state is forced to 1 when the
   skills.txt record (`0x00644140(skill)`) lacks `InTown` (byte +5 &
   byte `[0x006CE268]` = mask 1, i.e. flags bit 8) and P's room
   (`0x00620BB0`) is in town (`0x0061AB00`):
   `k = (u == 0 ? 0 : u == 6 ? 4 : 1); if (!rec.InTown && room(P) && is_town(room(P))) k = 1`.
   While the mouse is
   in x…x + 48, y − 48…y the state 4 stays 4, 0 stays 0, other → 1;
   drawn with `CelDrawColor` (`0x004F64B0`, light 0xFF, mode 5, the
   state as color argument); charges / quantity overlays
   (`0x004AA1F0`, `0x004A8D50`, `0x004A9300`, `0x004A8ED0`,
   `0x004A9260`): §Open questions 3.
3. Press (§10 r1): x 117…165 or W − 165…W − 117, y > H − 48 and ≤ H.
   Release (§10 r2): toggles state 3 (skill select) and calls
   `0x004A8CE0(1)` (left) or `0x004A8CE0(0)` (right).

### 8. New-stats and new-skills buttons

1. **800 × 600** (resolution mode 2): the buttons sit on the base art.
   New stats: `Panel\Level` at (W/2 − 194, H − 8): frame 2 while state 6
   is closed (`0x004A6A70`); while it is open (`0x004A6B30`) frame 1
   when pressed (`[0x007C02E4]`) with the mouse inside, else 0. New
   skills: the same at (W/2 + 163, H − 8) with state 7 and
   `[0x007C02E8]` (`0x004A6DA0`, `0x004A6E60`). Hover (strict): new
   stats W/2 − 194 < x < W/2 − 160, new skills W/2 + 163 < x < W/2 +
   197, both H − 42 < y < H − 8 (`0x004A65E0`, `0x004A6690`); hover
   with state 9 closed → tool tip `strlvlup` (3986, "New Stats") at
   (W/2 − 179, H − 50) / `strnewskl` (3987, "New Skill") at (W/2 + 178,
   H − 50), color 0, centred.
2. **640 × 480**, only while state 6 / 7 is open: hidden (and the pressed
   flag cleared) when the open mode is 3, or for new stats while state 2,
   0x0C, (0x16 and 1) or (0x16 and 4) is open (`0x004A6A00`), for new
   skills while state 4, 0x0C or (0x16 and 1) is open (`0x004A6D50`).
   x0 = 40 (new stats; W/2 + 40 in open mode 2) or W − 73 (new skills;
   W − W/2 − 73 in open mode 1). Caption 3986 / 3987 in the current font
   with `DrawText` at (x0 + 1 + cw/2 − tw/2, H − 142), color 0 (cw =
   `Levelsocket` frame width, `0x00601840` / `0x006018C0`; tw = width A);
   `Levelsocket` frame 0 at (x0, H − 105); `Level` frame (1 pressed and
   inside, else 0) at (x0 + 3, H − 109). Inside (strict): new stats x0
   < x < x0 + 34 in open mode 2, else 41 ≤ x ≤ 73; new skills x0' − 73 <
   x < x0' − 40 with x0' = W (W − W/2 in open mode 1); y H − 139 < y <
   H − 102 / H − 138 < y < H − 102 (`0x004A6580`, `0x004A6630`).
3. Press and release handling of these buttons: rules 4–5 (was §Open
   questions 5).
4. **Press** (WM_LBUTTONDOWN entries of the handler tables `0x006D5FE0`
   / `0x006D6004`: new stats `0x004A66E0`, new skills `0x004A6790`):
   nothing at 800 × 600 while state 9 is open. Hit: 800 × 600 the hover
   rectangles of rule 1 (current mouse, `0x004A65E0` / `0x004A6690`);
   640 × 480 the "inside" rectangles of rule 2 with the event's x, y and
   the open mode (`0x004A6580` / `0x004A6630`). A hit sets the pressed
   flag (`[0x007C02E4]` / `[0x007C02E8]`), plays sound 0
   (`0x004B9A00(0, 0, 0)`), runs the cursor press (`ui/panels-3.md` §23
   r5) and consumes the event, except when the open mode is 2 (new
   stats) / 1 (new skills) and `0x004B3470()` ≠ 0 (then not consumed).
   A miss is not consumed.
5. **Release** (WM_LBUTTONUP entries: `0x004A6840` / `0x004A6920`):
   nothing at 800 × 600 while state 9 is open; else the cursor release
   (`ui/panels-3.md` §23 r6). Pressed and a hit (800 × 600: current mouse; 640 × 480: the
   event) → pressed := 0 and, 800 × 600: `SetUIState(2, on, 0)` (new
   stats) / `SetUIState(4, on, 0)` (new skills); 640 × 480:
   `SetUIState(6, off, 0)` then `SetUIState(2, on, 0)` / `SetUIState(7,
   off, 0)` then `SetUIState(4, on, 0)`; consumed. Otherwise pressed :=
   0, not consumed. No press check beyond the flag (a press elsewhere
   never set it).
6. **Level-up button sync** (`0x004A64C0`, every UI pass, step 8,
   before the help button; 2026-10-09 read). It opens and closes the
   states 6 / 7 that select the button variants of r1–r2. It draws
   nothing. (q-scenes-compare took it for the help button; that is §11.)
   - State 0x0B (key configuration) open → nothing.
   - P := the local player (`0x00463DD0`); `statpts` = stat 4,
     `newskills` = stat 5 (`0x006253B0(P, s, 0)`, signed).
   - State 6 open and `statpts` = 0 → `SetUIState(6, off, 0)`. State 6
     closed, `statpts` ≠ 0 and state 2 (character) closed →
     `SetUIState(6, on, 0)`.
   - Then state 7 open and `newskills` ≤ 0 → `SetUIState(7, off, 0)`.
     State 7 closed, `newskills` > 0 and state 4 (skill tree) closed →
     `SetUIState(7, on, 0)`.
   So the glowing button appears in the pass after the points arrive.
   It does not appear while the matching panel is open. Once open, it
   stays open (also over the panel) until the points are spent.
   ```
   level_up_sync():
     if ui_open(0x0B): return
     p = local_player()
     if ui_open(6): if stat(p, 4) == 0: set_ui(6, OFF)
     elif stat(p, 4) != 0 and not ui_open(2): set_ui(6, ON)
     if ui_open(7): if stat(p, 5) <= 0: set_ui(7, OFF)
     elif stat(p, 5) > 0 and not ui_open(4): set_ui(7, ON)
   ```

### 9. Mini panel (state 0x15)

Measured (revision 2026-10-09, q-scenes-compare): state 0x15 is open
from the game's start: every recorded in-game scene (PC 1's Windows
recordings and the Wine ones, `facts/render/scenes`) draws
`minipanel_s` and the menu button at frame 2 with no input that opened
it. The opener and its saved-setting gate are r9 (REC-519 settled):
open at entry unless `Diablo II\Mini Panel` exists and is ≠ 0.

1. **Variant** (`0x0047F0C0`, game start): multiplayer (game type ≠ 0)
   → `Panel\minipanel`, 8 buttons, `[0x007BC978]` := 0; single player →
   `Panel\minipanel_s`, 7 buttons (no party button), `[0x007BC978]` :=
   1. Pressed flags `[0x007BC8F8]` + 4 i cleared.
2. **Sides** (`0x0047EA60` left, `0x0047EB50` right): left blocked when
   any of the states 0x17, 0x19, 0x1A, 0x0C, 0x1C, 0x1D, 0x18, 3, 0x10,
   0x16, 0x24, 2, 0x0F, 0x14, 0x1B is open (`[0x007BC96C]` := result);
   right blocked when any of 0x17, 0x19, 0x1C, 0x1A, 0x0C, 0x18, 0x1B,
   3 is open, or (state 0x1F open or the belt has extra rows) with a
   row count > 1, or state 1 or 4 is open. `[0x007BC968]` ends as 1 only
   when state 1 or 4 is open (the other cases set it and then clear it:
   reproduce; the draw uses the return value, the input uses the flag).
3. **Draw** (`0x0047F710`) by (left blocked, right blocked):
   - (no, no): layout 2, art frame 0 at (W/2 − 74 − 3 (single) or W/2 −
     84 − 3 (multi), H − 47);
   - (yes, no): layout 3, art at (W/2 + 56 − 2 (single) or W/2 + 35 − 2,
     H − 47);
   - (no, yes): layout 1, art at (W/2 − 205, H − 47);
   - (yes, yes): nothing; `[0x007BC970]` := 1 (else 0).
4. **Buttons** (`0x0047E8B0(layout)`): x0 = W/2 − 84 (multi) / W/2 − 74
   (single); layout 1 → W/2 − 202; layout 3 → W/2 + 35 (multi) / W/2 +
   57 (single). Button i (0 … n − 1) at (x0 + 21 i, H − 50); function
   f_i and base frame b_i: multi f = i, b = 2i; single f = i (+1 from i
   = 3), b = 2i (+2 from i = 3). Draw (`0x0047E9A0`): `minipanelbtn`
   frame b_i + 1 while pressed, else b_i.
5. **Functions** (`0x0047EC50`; tool tip string, key binding):

   | f | Tool tip | Binding | Action on release |
   |---|---|---|---|
   | 0 | 4169 "Character" | 0 | state 2: open → off, else toggle |
   | 1 | 4170 "Inventory" | 1 | state 1 likewise |
   | 2 | 4171 "Skill Tree" | 12 | state 4 likewise |
   | 3 | 4172 "Party Screen" | 2 | multiplayer only: state 0x16 likewise |
   | 4 | 4173 "Automap" | 7 | state 0x0A likewise |
   | 5 | 4174 "Message Log" | 3 | state 0x1F off if open; state 0x18 likewise |
   | 6 | 4175 "Quest Log" | 4 | `0x004A3FE0(0)` (quest log) |
   | 7 | 4176 "Game Menu (Esc)" | – | `0x00456300(0, 0)`, `0x00453AC0`, `0x0047E090(1, 0)`, `0x00453AD0`, `0x00453AE0` |

6. **Tool tips** (`0x0047F650(layout)`): region (strict) W/2 + o − 84 <
   x < W/2 + o + 89, H − 69 < y < H − 47 with o = 0 (layout 2), −118
   (1), +119 (3); the button with x_i < x < x_i + 20 gets
   (`0x0047F490`): the string, then for the primary and secondary key
   of its binding ` (%s)` (4178) with the key name (`0x00469DE0`), at
   (x_i − 3, H − 76), color 0, centred.
7. **Press** (`0x0047EF30`, no cursor item, `[0x007BC970]` = 0): region
   as r6 with o = (−118 when the belt has extra rows and a row count >
   1) + (−118 when `[0x007BC968]`) + (119 when `[0x007BC96C]`); button i
   (strict x test against the button x table `[0x007BC898 + 4i]`, the
   positions of the layout last drawn — only the draw `0x0047F710`
   calls the builder `0x0047E8B0`, so press, release (r8) and tool tips
   (r6) all test the last drawn layout, whatever o is; before the
   first draw the table is zero; no button under x → nothing, not
   consumed): f = 7, or P a living player and (state 9 closed, or
   i ≥ 4 single / i ≥ 5 multi) → pressed i := 1; in every case UI sound
   4, the press latch `[0x007BC97C]` := 1, consumed.
8. **Release** (`0x0047ED90`, only with the latch; latch := 0; no cursor
   item; a cursor in mode ≠ 7 is reset `0x00468070(0)`; `[0x007BC970]`
   = 0): all pressed flags := 0; same region and offsets; the button
   under the mouse runs its function when f = 7, or when P is not
   blocked (`0x0044BE50` = 0) and not dead (`0x00451F70` ≠ 0x11); then
   `0x0044DA40`, `0x0044DA70`, consumed. Outside the region:
   `[0x007BC974]` := 0, not consumed. (The function runs whatever button
   was pressed: a release over another button runs that one.)
9. **Open at game entry** (2026-10-09 read; REC-519). The in-game UI
   set-up `0x00456970` (game join: S→C 0x01, `client/model.md` §7 r2) zeroes all 38
   UI flags, opens state 0 and then calls `0x004567F0`. This is the
   only opener at entry, and a saved setting decides it:
   - It reads `Diablo II\Mini Panel` (`0x00414F10("Diablo II", "Mini
     Panel", 1, &v)`, `ui/frontend-options.md` §O7 r1). If the value
     exists and v ≠ 0, nothing more happens: the mini panel starts
     closed. If the value is absent or 0, the panel opens.
   - Open is an inline copy of `SetUIState(0x15, toggle)` that does not
     call `0x00455F20`. Nothing happens while a modal text screen is up
     (`0x004A0000`). With the flag clear, the gate column 0x15 of every
     open state applies (`ui/panels.md` §3 r3): 1 closes, 4 ends an NPC
     interaction, 2 or 3 abandon the open. Then the flag flips when
     there is no P or P is alive (`ui/panels.md` §2 r5). If it went
     0 → 1, the handler table `0x006D6204` (2 entries: press
     `0x0047EF30`, release `0x0047ED90`, r7–r8) is registered.
     Right after the reset only state 0 is open, and its gate row is
     all 0, so the panel opens.
   - The setting is written at game exit (`0x00456D80` →
     `0x0047F130`): `Mini Panel` := 1 when the "was open" record of
     ui 0x15 in the Esc-menu save table is 0, else 0.
     The record is `[0x0071315C]` = `0x00713060 + 12·0x15`
     (`ui/frontend-options.md` §O1 r2; 0 in the image). Only an Esc-menu
     open with save ≠ 0 (Esc, mini-panel function 7) rewrites it, and
     it is not reset at game entry. So the stored value is "mini
     panel open when the game menu was last opened in this process",
     not the live flag at exit: Save and Exit through the menu keeps
     the panel's state for the next game. A game left without ever
     opening the menu writes 1 (closed next time).
   - The other opener is the help screen. Its draw `0x004966C0`
     (state 0x21) opens 0x15 when it is closed and sets `[0x007BEF00]`.
     `0x00494FC0` closes it again, and clears that flag, when the help
     screen closes (close hook `0x00455E10`, UI pass `0x0045723C`,
     help click `0x00496766`).
   ```
   mini_panel_at_entry():
     if registry_read("Mini Panel") is Some(v) and v != 0: return
     open_inline(0x15)              # gate column 0x15, alive test, handlers
   at_game_exit():
     registry_write("Mini Panel", 1 if esc_saved_open[0x15] == 0 else 0)
   ```

### 10. Control panel mouse input

1. **Down** (`0x00499500`; ignored while `0x0044DA30` ≠ 0 or P is dead /
   absent): over the belt (§5 r6) → `[0x007BEFA4]` := 1, consumed. Else
   y ≤ H − 48 → not consumed. Else, in order: left skill (x 117…165, y ≤
   H) → `[0x007BEFA4]` := `[0x007BEF84]` := 1; right skill (W − 165…W −
   117) → `[0x007BEF88]`; life / mana text toggles (§3 r5); menu button
   rectangle → `[0x007BEFD0]` := 1; run button rectangle →
   `[0x007BEFD8]` := 1; the last two set `[0x007BEFA4]` := 1 and play
   UI sound 4. Every case below y = H − 48 sets `[0x00722358]` := 1 and
   is consumed.
2. **Up** (`0x004996A0`, same guards): `[0x007BEF84]` := `[0x007BEF88]`
   := 0.
   - No press recorded (`[0x007BEFA4]` = 0): over a skill button with
     state 3 open → `SetUIState(3, off, 0)`; not consumed.
   - Popped belt: over the belt → cursor mode 6 → `0x00453EC0`, mode 8
     → C→S **0x4C** (`0x00478680`, −1); then the belt click
     `0x00498870(x, y, inventory)`; consumed. Menu button → state 0x15
     off if open, else toggle (not consumed). Skill buttons →
     `SetUIState(3, toggle, 0)`, `0x004A8CE0(left)`. Run button → run
     := !run (`0x0044BE80`).
   - Belt not popped: y ≤ H − 48 → not consumed. Belt strip (§5 r6) →
     as above. Skill buttons: cursor modes 6 / 8 as above, then state 3
     off if open else on, `0x004A8CE0(left)`. Run button → toggle. Menu
     button → state 0x15 off if open, else toggle; if it was pressed,
     `[0x007BEFD4]` := !`[0x007BEFD4]`. Consumed; `[0x00722358]` := 0.
   - Always: `[0x007BEFA4]` := `[0x007BEFD0]` := `[0x007BEFD8]` := 0.
   Release tests do not check that the press was on the same button
   (reproduce, as `ui/panels.md` §7 r3).
3. **The two leftovers of §8 r4 and r2** (answer §Open questions 5).
   `0x004B3470()` = "an NPC menu is up": an NPC interaction is active
   (`[0x007C0D29]` ≠ 0), its NPC exists (`0x00463990([0x007C0D25], 1)`)
   and the NPC menu state `[0x007C0C6B]` = 1 (the menu box of
   `ui/menus.md` §2.2; `ui/panels-2.md` §14). So with the NPC menu up a
   press on the new-stats button in open mode 2 (or new-skills in
   mode 1) still arms the button and plays the sound but leaves the
   event to the next handler. `[0x007BEFD4]` has no reader: its only
   accesses are the read-and-toggle at `0x00499AC9` / `0x00499AD2` in
   r2 (scan of every reference); it changes nothing observable and d2rs
   may omit it.

### 11. Help button (state 0x22, `UI_HELPBUTTON`)

2026-10-09 read. The "Help (H)" caption with a socketed level button
above the right globe. It shows in every game until the player opens
help once. No character level, act, difficulty or first-game test
applies. The only gate is the registry value `Help Menu`.

1. **Open.** Only at game entry. The set-up `0x00456970` (§9 r9) opens
   0x22 inline right after the mini panel: nothing while a modal text
   screen is up, then the gate column 0x22 of the open states (as §9
   r9), then flag := 1 when there is no P or P is alive (an
   assignment, not a toggle). If it went 0 → 1, the handler table
   `0x006D6300` is registered (2 entries: WM_LBUTTONDOWN `0x004952B0`,
   WM_LBUTTONUP `0x00495320`). No other `SetUIState` site opens 0x22
   (scan of all 136 calls; the two at `0x00495295` / `0x00495377`
   close it). Esc does not close it (Esc flag 0, `ui/panels.md` §2
   r9). The gate closes it when ui 0x0E, 0x14, 0x17, 0x19–0x1E, 0x20
   or 0x21 opens (`ui-states.tsv` row 0x22, action 1).
2. **Setting** (`0x004942D0`): a process-wide cache `[0x00722310]`,
   −1 in the image. On the first call it is set to 0 and then filled
   by the registry read `Diablo II\Help Menu` (`0x00414F10`), so an
   absent value stays 0. Later calls return the cache.
3. **Draw** (`0x00495180`, step 8 with `[0x22]`; one layout for both
   resolutions):
   - If any of states 4, 3, 1, 0x0C, 0x17, 0x19 is open: hidden flag
     `[0x007BEF08]` := 1, nothing drawn.
   - Else hidden := 0. If the setting (r2) is ≠ 0:
     `SetUIState(0x22, off, 0)`, nothing drawn (the button closes in
     the first pass of a game once help has been used).
   - Else the caption (`0x00495030`). Start with string 4177 ("Help",
     the recorded glyphs). Command 6 is `CfgHelp`
     (`ui/controls.md` §3). For its slot 1, then its slot 0, when the
     slot is bound (`0x00469AA0` ≠ 0xFFFF), append ` (%s)` (4178)
     formatted with that key's name (`0x0046A530`: short name, long
     fallback, §5 r13; formatted with a 30-character limit). The
     text is drawn with `DrawText` (`0x00502320`) at
     (W − 58 − w / 2, H − 197), color 0, not centred, in the current
     font (font 1 after the belt, §Edge cases). Here w is width A
     (`0x00501820`) and the halving truncates.
   - `Panel\Levelsocket` frame 0 at (W − 75, H − 160). Then
     `Panel\Level` at (W − 72, H − 164), frame 1 while the pressed
     flag `[0x007BEF04]` ≠ 0, else 0. Both are plain cel draws (mode
     5, light 0xFFFFFFFF, palette 0) of the new-stats cels
     `[0x007C02E0]` / `[0x007C02DC]` (§8).
   At 800 × 600, with H bound only: caption y 403, socket (725, 440),
   button (728, 436). At 640 × 480: y 283, (565, 320), (568, 316).
4. **Press** (`0x004952B0`): if hidden is set at the last draw,
   nothing. Otherwise hit when the *current* mouse (`0x00468730` /
   `0x00468740`, not the event) is at W − 75 ≤ x ≤ W − 40 and
   H − 196 ≤ y ≤ H − 160 (inclusive). A hit sets pressed := 1 and
   consumes the event (message +0x18 := 1, +0x1C := 0, `0x00420290`).
   It plays no sound and runs no cursor press. A miss is not
   consumed.
5. **Release** (`0x00495320`): same guard and same hit test. The
   pressed flag is **not** checked. A hit runs, in order:
   `SetUIState(0x22, off, 0)`; `SetUIState(0x21, toggle, 0)` (the help
   screen); `Help Menu` := 1 (`0x004150E0`, REG_DWORD); cache := 1;
   pressed := 0. The event is consumed. A miss does nothing, so
   pressed stays set and the button keeps frame 1 until a release over
   it (reproduce).
6. **Help key** (command 6, `0x004689D0`, `ui/controls.md` §3) also
   writes `Help Menu` := 1 when the value is absent or 0. It does not
   touch the cache. Its toggle of 0x21 closes 0x22 through the gate
   (r1) for the rest of that game. But a game started later in the
   same process shows the button again, because the cache read
   earlier is still 0. The button stays away only from the next
   process start.
   ```
   help_button_draw():
     if any_open(4, 3, 1, 0x0C, 0x17, 0x19): hidden = 1; return
     hidden = 0
     if help_menu_setting() != 0: set_ui(0x22, OFF); return
     text = str(4177)
     for slot in (1, 0):
       if key(6, slot) != NONE: text += fmt(str(4178), key_name(6, slot))
     draw_text(text, W - 58 - width_a(text) / 2, H - 197, 0, centred = 0)
     cel(LEVELSOCKET, 0, W - 75, H - 160)
     cel(LEVEL, 1 if pressed else 0, W - 72, H - 164)
   help_button_release():
     if hidden or not inside(mouse): return NOT_CONSUMED
     set_ui(0x22, OFF); set_ui(0x21, TOGGLE)
     registry_write("Help Menu", 1); cache = 1; pressed = 0
     return CONSUMED
   ```

## Constants & data dependencies

- Strings: 3986, 3987, 3998, 4163–4179 (4177 the help caption, §11); key names 3762–3860
  (long) and 3861–3914 (short), §5 r13.
- Tables: `belts.bin` (14 records of 0x108 bytes); binding table of the
  belt keys `0x00722404` (4 dwords); switch tables `0x00499434`
  (pop-up rows), `0x00497A1C` (row count), `0x0047ED6C` (mini panel),
  `0x0047F630` (tool tips); key-name jump tables `0x0046A448`,
  `0x0046A2CC`, `0x0046A2B8` (long), `0x0046A73C`, `0x0046A6BC` (short).
- Registry values under `Diablo II`: `Show HP Text`, `Show MP Text`
  (§3 r5), `Mini Panel` (§9 r9), `Help Menu` (§11).
- Handler tables: `0x006D6204` (mini panel, 2 entries), `0x006D6300`
  (help button, 2 entries).
- States: 2, 100, 106, 136; `states` flag bit 24 (`stambarblue`, §4
  r2); stats 6–13, 26, 74.

## Randomness

None.

## Edge cases & original bugs

Reproduced by default.

- The belt draw sets font 1 and never restores it, so later text in the
  frame without its own font (the life / mana numbers, the stash
  `GoldMax`, `ui/panels-2.md` §20.6) uses font 1.
- `[0x007BC968]` (input) and the right-blocked return value (draw)
  disagree except when state 1 or 4 is open (§9 r2): the press and
  release regions can sit 118 pixels from the drawn panel.
- The skill buttons can change P's skills from inside the draw (§7 r1).
- Globe smoothing uses x87 floating point (§3 r1.4).

## Test vectors

Synthetic (rules as cited).

| Input | Expected | Source |
|---|---|---|
| life 50·256, max 100·256, no change for 20 updates | f = 40 rows | §3 r1, r2 |
| life 1 (×256 = 256) of max 25,600, alive | f = 0 → 0 (80 · 256 / 25,600 = 0) | §3 r2 |
| life 512 of max 25,600, alive | f = 1 → 2 | §3 r2 |
| healthpot, stat 74 = 30, f = 10 | potion window skip 10, lines 14 (s = 24) | §3 r2 |
| 800 × 600, X = 1,000, prev 500, next 1,500 | px = 59; lines (256, 562)–(315, 562) and y 563 | §4 r1 |
| stamina v = m = 25,600 | w = 102, gold | §4 r2 |
| stamina v = 5,000, m = 25,600 | w = 19 → red | §4 r2 |
| 800 × 600, mouse (300, 580), stamina 12,800 of 25,600, no state 136 | pop-up `Stamina: 50 / 100` at (324, 548), centre 1, colour 0, Font16, box bottom 550 | §4 r2, §5 r14 |
| same, P has state 136 | `Stamina: 100 / 100`, colour 3 | §4 r2 |
| 640 × 480, Show HP Text on, shown life 25,600 of 25,600 | `Life: 100 / 100` `DrawText` at (65 − w / 2, 385), Font16, colour 0, no box | §3 r6 |
| single player, 800 × 600, layout 2 | art at (323, 553); buttons at x 326, 347, 368, 389 (f 4), 410, 431, 452 | §9 r3, r4 |
| multiplayer, 640 × 480, layout 1 | buttons x 118 + 21 i, y 430 | §9 r4 |
| 640 × 480 belt type 0 popped | rows at y 439, 407 | §5 r3 |
| belt click on box 2 (empty), 1 × 1 potion on the cursor | C→S 0x23 `[potion GUID][2]` and the put sound | §5 r11 |
| belt click on box 0 holding a potion, no cursor item | C→S 0x24 `[potion GUID]` | §5 r11 |
| belt slot 0, `CfgBelt1` slot 1 = VK 0x31, slot 0 unbound | label `1` | §5 r4, r13 |
| `CfgBelt1` rebound: slot 1 = VK 0x70 (F1) | label `F1` | §5 r4, r13 |
| `CfgBelt2` slot 1 = VK 0x6C | label `np.` | §5 r13 |
| `CfgBelt3` slot 1 = 0xFFFF, slot 0 = VK 0x33 | label `None` (cut to width A ≤ 28) | §5 r4 |
| `CfgBelt4` both slots 0xFFFF | no label | §5 r4 |
| `CfgBelt1` slot 1 = 0x103 (wheel up) | label `mwu` | §5 r13 |
| run tip, command 0x23 slot 1 = VK 0x52, slot 0 unbound | `Run (R)` | §6 r1, §5 r13 |
| hovered belt potion (quality 2), no shop | `ÿc3` + stat lines + `ÿc0` + name, colour 0, centre at the text position | §5 r8 |
| same, NPC trade mode 1, price text `P` | as above + `
` + `P` | §5 r8 |
| 800 × 600, state 0x22 open, `Help Menu` absent, `CfgHelp` slot 1 = VK 0x48, slot 0 unbound, caption width A 56, no blocking state | `Help (H)` `DrawText` at (714, 403) colour 0; `Levelsocket` f0 at (725, 440); `Level` f0 at (728, 436) (scene `a4-town-pandemonium-fortress` rows 258–267) | §11 r3 |
| same, state 1 (inventory) open | nothing drawn; press and release ignored | §11 r3–r5 |
| same, `Help Menu` = 1 at process start | first pass: `SetUIState(0x22, off, 0)`, nothing drawn | §11 r2–r3 |
| 800 × 600, button shown, mouse (725, 404) down then up | press consumed, frame 1; release: 0x22 off, 0x21 toggled, `Help Menu` := 1 | §11 r4–r5 |
| press at (761, 440), release at (761, 441) (outside: x > W − 40) | not consumed, nothing | §11 r4–r5 |
| game entry, `Mini Panel` absent or 0 | state 0x15 open before the first pass | §9 r9 |
| game entry, `Mini Panel` = 1 | state 0x15 closed | §9 r9 |
| game left by Save and Exit with the mini panel open when Esc opened the menu | `Mini Panel` := 0 | §9 r9 |
| statpts 5, state 6 closed, state 2 closed, state 0x0B closed | `SetUIState(6, on, 0)` in that pass | §8 r6 |
| statpts 5, state 2 open | state 6 stays closed | §8 r6 |
| state 7 open, newskills 0 | `SetUIState(7, off, 0)` | §8 r6 |
| pop-up `Run` at (x 255, y 577), centre 1, 800 × 600, Font16, max width 26 (synthetic), one line | W = 34; box (238, 563)–(272, 579) when Ht = 16 (`DrawRectangle`, colour 0, mode 2); text block x 238, y 576 | §5 r14 |

## Provenance

1.14d `Game.exe` (exports, `tools/ghidra/disasm.py`): `0x00499450`,
`0x00496F80`, `0x00497110`, `0x00496DD0`, `0x00498EA0`, `0x00497480`,
`0x00497300`, `0x004975B0`, `0x004977C0`, `0x00497780`, `0x00497440`,
`0x00499040`, `0x004972B0`, `0x00497920`, `0x004978D0`, `0x00498DC0`,
`0x004979E0`, `0x004979F0`, `0x00497A40`, `0x00496CF0`, `0x00496BE0`,
`0x004A9690`, `0x004A8C80`, `0x004A8D30`, `0x00498340`, `0x00498120`,
`0x004967F0`, `0x00496990`, `0x004A6460`, `0x004A6A70`, `0x004A6B30`,
`0x004A6DA0`, `0x004A6E60`, `0x004A6A00`, `0x004A6D50`, `0x004A6580`,
`0x004A6630`, `0x004A65E0`, `0x004A6690`, `0x0047F0C0`, `0x0047F710`,
`0x0047EA60`, `0x0047EB50`, `0x0047E8B0`, `0x0047E9A0`, `0x0047F650`,
`0x0047F490`, `0x0047EC50`, `0x0047EF30`, `0x0047ED90`, `0x00499500`,
`0x004996A0`, `0x00499BB0`, `0x00498E90`, `0x00660CB0`, `0x00660D10`,
`0x00469AA0`, `0x00469DE0`, `0x0046A530`, `0x004978D0`, `0x0046EFD0`,
`0x00497A40`, `0x00489840`, `0x00502280`, `0x005022F0`, `0x00503000`,
`0x00502C60` (2026-10-08: REC-240, REC-238; jump tables read from the
image with `pefile`); REC-252 (2026-10-08, disassembled): `0x004975B0`
(`0x00497684`–`0x00497766`), `0x00498120` (whole), `0x00499040`
(`0x00499053`), `0x00499450` call list, `0x0063A7B0`, `0x0063A130`,
font-setter `0x00502EF0` call sites.
Strings, `belts.txt`, `states.txt` and `itemstatcost.txt` rows read
with Python scripts outside the repo. No capture yet.
2026-10-09 (pc1-day3-c items 42–43, read in all.asm / disasm.py;
q-scenes-compare, REC-519): `0x004A64C0` (whole; caller `0x004570BD`),
help button `0x00495180`, `0x00495030`, `0x004952B0`, `0x00495320`,
`0x004942D0` (cache `[0x00722310]`), help key `0x004689D0`, UI pass
`0x004570BD`–`0x004570CA`; game-entry set-up `0x00456970`
(`0x00456BAD` mini panel, `0x00456BB2`–`0x00456CA6` help button, gate
jump tables `0x00456958`, `0x00456D6C` read from the image),
`0x004567F0`, exit `0x00456D80` → `0x0047F130`, Esc save table
`0x0047E090` (`0x00713058`, 12-byte records), help-screen mini panel
`0x004966C0` / `0x00494FC0`; handler tables `0x006D6204`,
`0x006D6300` and the strings `Diablo II`, `Mini Panel`, `Help Menu`
read from the image with `pefile`; `SetUIState` call-site scan (136
sites) for ui 0x15 / 0x22. Measured: scene
`a4-town-pandemonium-fortress` draw rows 258–267 (`facts/render/scenes`).

## Open questions

1. **Answered** (2026-10-09, pc1-data Step 4 item 4; REC-21's static
   half). Globe smoothing: the precision is PC = 53 (CRT start-up,
   `items/treasure.md` OQ5), so `0x00496F3F`–`0x00496F48` (`fild`
   e + 1, `fidiv` d, `fimul` Δ, `0x00682FD0`: exact `fstp` to a double,
   `cvttsd2si`) equals binary64 `trunc(fl(fl((e + 1) / d) · Δ))`. It is
   **not** the integer `(e + 1) · Δ / d`: the smallest case is d = 11,
   e + 1 = 3, Δ = ±55 (float 14.999… → 14, integer 15); over d 7…15 and
   |Δ| ≤ 200000 they differ in 48,562 cases (exhaustive, Python
   binary64). PROVISIONAL (REC-610) only for a video runtime DLL
   changing PC on the game thread.
   *Recorded, REC-610 DirectDraw half* (2026-10-09, PC 1, Windows 10,
   Intel HD 630, windowed `-w`, modules DDRAW.dll + dxgi.dll): the x87
   control word of the game thread at every server-tick return
   (`0x0052FD1E`, 63 and 103 ticks in two runs) is **0x027F** (PC = 53,
   RC = nearest, all masked), MXCSR 0x1FA0; `-w -d3d` still loads only
   DirectDraw (windowed 1.14d runs DirectDraw), so Direct3D (full
   screen) and Glide (no glide DLL in this install) stay open.
   ```
   shown = start + trunc_f64((e1 as f64 / d as f64) * delta as f64)
   ```
2. **Answered** (2026-10-07, §5 r10–r12; belt use by key
   `0x00498A90`: `ui/controls.md` §7 r3). Was: Belt hover tracking
   `0x00498930` / `0x00498A90` / `0x00498D60` / `0x00498E80` (hovered
   box `[0x0072235C]`, item `[0x007BEFA8]`, text position
   `[0x00722360]`, `[0x00722364]`) and the belt click `0x00498870`:
   Ghidra read.
3. The skill icon overlays (`0x004AA1F0`, `0x004A8D50`, `0x004A9300`,
   `0x004A8ED0`, `0x004A9260`, used also by the skill select panel
   `0x004AA7E0`): Ghidra read; owner this spec or a skill-select spec.
4. `[0x007BEFEC]` (set to 60 on a level change): its reader (level name
   display) and units.
5. **Answered** (2026-10-07, §10 r3: `0x004B3470` = NPC menu up; `[0x007BEFD4]` is never read). Earlier *partly answered* (2026-10-07, §8 r4–r5: the press and release
   handlers; still open: `[0x007BEFD4]`, toggled by the menu-button
   release, and `0x004B3470`). Was: New-stats / new-skills press and
   release handlers (writers of `[0x007C02E4]` `0x004A66E0`,
   `[0x007C02E8]` `0x004A6790`) and `[0x007BEFD4]` (toggled by the
   menu-button release).
6. **Needs recording.** Pixel proof of §3–§9 at 640 × 480 and 800 × 600
   (globes at several fills, belt popped, mini panel in layouts 1–3).
7. **Answered** (2026-10-08, REC-240; §5 r4, r5, r8, r13): belt key
   labels are the short name of the slot-1 key of commands 23–26 read
   live from the binding table (`0x0046A530`, falling back to the long
   name `0x00469DE0`); the hover text is the reduced name + stat-line
   text of `0x00497A40` (not the inventory tip) with a newline + price
   in NPC trade, drawn as pop-up text; the highlight rectangles are
   `DrawRectangle` mode 0 with a nearest-palette colour. Was: key-label
   source of a rebound key, the belt tip strings, the rectangle draw.
8. **Answered** (2026-10-08, REC-238; §5 r14, §3 r6): the pop-up text
   of `0x00502280` is drawn by `0x00503000` in the current font (Font16
   in the plain game view) over a colour-0 mode-2 box W = text width + 8
   wide, clamped to the screen; the globe numbers are plain `DrawText`
   in the current font. Was: the tip font and the globe-number draw.
9. **Needs recording.** Pixel check of §5 r14 (a hovered run button,
   the experience bar tip) and of §5 r4 with a rebound belt key; owner
   this spec.
