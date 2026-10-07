# Spec: UI — Control panel overlays (globes, bars, belt, skill buttons, run / menu buttons, mini panel, new-stats / new-skills buttons)

- **Status:** draft (2026-10-07, RE on the 1.14d `Game.exe`; no capture
  yet). Answers UP-28 (`ui/panels.md` §6 r3, §Open questions 1, control
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
| Summary | 42–55 |
| Inputs | 56–69 |
| Outputs / state changes | 70–77 |
| Rules | 78–79 |
|   1. Draw order (`0x00499450`) | 80–100 |
|   2. Art files | 101–114 |
|   3. Life and mana globes | 115–160 |
|   4. Experience and stamina bars | 161–186 |
|   5. Belt | 187–253 |
|   6. Run / walk and menu buttons | 254–272 |
|   7. Skill buttons | 273–295 |
|   8. New-stats and new-skills buttons | 296–323 |
|   9. Mini panel (state 0x15) | 324–384 |
|   10. Control panel mouse input | 385–414 |
| Constants & data dependencies | 415–423 |
| Randomness | 424–427 |
| Edge cases & original bugs | 428–440 |
| Test vectors | 441–457 |
| Provenance | 458–474 |
| Open questions | 475–495 |
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
by `0x00503000` in UI pass step 10; centre = 1 → x is the text centre);
`SetUIState` calls (3, 0x15, mini-panel targets); run / walk toggle
(`0x0044BE80`); registry `Show HP Text`, `Show MP Text`, `PopupHireling`
(§9); UI sound 4 on presses.

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

Step 8 of the UI pass (`ui/panels.md` §5) then draws, in its order: the
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
      toward 0; §Open questions 1); else shown = v. Then clamp to 0…m.
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
   v, color blue. w = 0 when scale ≤ 0, else 102 · v / scale; w < 25 →
   red. Rectangle `0x0046EFD0(W/2 − 127, H − 27, w, 18, color, mode 2)`
   (= `DrawRectangle(x, y, x + w, y + 18, color, 2)`).
   - Hover x W/2 − 127…W/2 − 25, y H − 27…H − 9: tool tip `panelstamina`
     (4164, "Stamina: %d / %d") with v >> 8 and m >> 8 at (W/2 − 76, H −
     52), centred, color 0 — or color 3 with the value shown as the max
     when v >> 8 > m >> 8 or P has state 136 (`shrine_stamina`).

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
   - the item drawn with `0x0046EE80(item, left, top)`;
   - boxes 0–3 of a usable item (`0x00628C20` ≠ 0): the name of the
     key bound to belt slot i (binding `0x00722404`[i], primary
     `0x00469AA0(b, 1)`, else secondary `(b, 0)`; none → no label),
     `0x0046A530` text cut from the end until width A ≤ 28, `DrawText`
     at (left + 2, bottom − 2), color 4.
5. **Cursor-item highlight** (`0x00497920`, when type ≠ 2, state 0x1F is
   open or the belt is popped): with an item on the cursor and the belt
   hovered, the hovered box `[0x0072235C]` (0 ≤ it < count): empty and
   the item fits a belt (`0x0062BAD0`) → green; occupied and a swap is
   possible (`0x0063C830`) → (128, 128, 0); else red; rectangle 29 × 29
   (`0x004978D0`, mode 0).
6. **Hit area** (`0x00498DC0`): popped: x from box (count − 4).left to
   box 3.right, y from box (count − 4).top to box 3.bottom; not popped:
   y > H − 48, x W/2 + 23…W/2 + 145, y H − 39…H − 10 (inclusive).
7. Row count `0x004979F0` (types 0–5: 3, 2, 1, 4, 2, 3; else 4).
8. **Item hover text** (`0x00497A40`): belt hovered, a hovered item, no
   cursor item, and (box ≤ 3 or popped or `[0x007BEF9C]`): the item's
   name (`0x0048C060`, 128 units) and, unless its quality is 3
   (`0x00627E70`), its stat lines (`0x004E6410`), colored with
   `Prefix(…, 3)` / `Prefix(…, 0)` (`ui/messages.md` §3); when the shop
   sell price applies (`0x00489840`, `0x004B2AD0`) a space and the price
   text are appended; queued at (`[0x00722360]`, `[0x00722364]`),
   color 0, centred (positions must be ≥ 0).
9. **Mouse move** (`0x00499BB0`, only in game `[0x007A061C]`, input not
   blocked `0x0044DA30` = 0, P alive, `0x0044BFE0` = 0): popped and over
   the belt → hover tracking `0x00498930` (consumed while state 0x1F is
   open); popped and off the belt → `[0x007BEF9C]` := `[0x007BEF94]` := 0
   and, with state 0x1F closed, popped := 0 (the rows fold down); not
   popped and inside the strip x W/2 + 23…W/2 + 145, y H − 39…H − 10 →
   unless `0x00453A90(9)`, popped := `[0x007BEF9C]` := 1 and hover
   tracking; consumed; elsewhere `[0x007BEF94]` := 0. Hover tracking
   details: §Open questions 2.

### 6. Run / walk and menu buttons

1. **Run / walk** (`0x00497480`): frame = 2 while running
   (`0x0044BE90` ≠ 0), else 0; + 1 while pressed (`[0x007BEFD8]`) with
   the mouse inside x W/2 − 145…W/2 − 128, y H − 28…H − 8 (inclusive,
   `0x00497440`); `runbutton` at (W/2 − 145, H − 10). Hover in that
   rectangle → tool tip (`0x00497300`): `RunOn` (4179, "Run") + for the
   primary and secondary key of binding 0x23 ` (%s)` (4178) with the key
   name, at (W/2 − 145, H − 23), color 0, centred.
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
   `0x004A8D30` (0, 1, 4), 1 also when the skill's flag
   `[0x006CE268]` bit is clear and P stands in town; while the mouse is
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
3. Press and release handling of these buttons: §Open questions 5.

### 9. Mini panel (state 0x15)

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
   (strict x test): f = 7, or P a living player and (state 9 closed, or
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

## Constants & data dependencies

- Strings: 3986, 3987, 4163–4176, 4178, 4179.
- Tables: `belts.bin` (14 records of 0x108 bytes); binding table of the
  belt keys `0x00722404` (4 dwords); switch tables `0x00499434`
  (pop-up rows), `0x00497A1C` (row count), `0x0047ED6C` (mini panel),
  `0x0047F630` (tool tips).
- States: 2, 24 (group), 100, 106, 136; stats 6–13, 26, 74.

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
| single player, 800 × 600, layout 2 | art at (323, 553); buttons at x 326, 347, 368, 389 (f 4), 410, 431, 452 | §9 r3, r4 |
| multiplayer, 640 × 480, layout 1 | buttons x 118 + 21 i, y 430 | §9 r4 |
| 640 × 480 belt type 0 popped | rows at y 439, 407 | §5 r3 |

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
`0x004996A0`, `0x00499BB0`, `0x00498E90`, `0x00660CB0`, `0x00660D10`.
Strings, `belts.txt`, `states.txt` and `itemstatcost.txt` rows read
with Python scripts outside the repo. No capture yet.

## Open questions

1. **Needs recording.** Globe smoothing: whether `(e + 1) / d · Δ` under
   the process's x87 precision control truncates exactly like the
   integer `(e + 1) · Δ / d` (cases like d = 3, Δ = 3); a recording of a
   globe refilling (frame by frame) settles it.
2. Belt hover tracking `0x00498930` / `0x00498A90` / `0x00498D60` /
   `0x00498E80` (hovered box `[0x0072235C]`, item `[0x007BEFA8]`, text
   position `[0x00722360]`, `[0x00722364]`) and the belt click
   `0x00498870`: Ghidra read.
3. The skill icon overlays (`0x004AA1F0`, `0x004A8D50`, `0x004A9300`,
   `0x004A8ED0`, `0x004A9260`, used also by the skill select panel
   `0x004AA7E0`): Ghidra read; owner this spec or a skill-select spec.
4. `[0x007BEFEC]` (set to 60 on a level change): its reader (level name
   display) and units.
5. New-stats / new-skills press and release handlers (writers of
   `[0x007C02E4]` `0x004A66E0`, `[0x007C02E8]` `0x004A6790`) and
   `[0x007BEFD4]` (toggled by the menu-button release).
6. **Needs recording.** Pixel proof of §3–§9 at 640 × 480 and 800 × 600
   (globes at several fills, belt popped, mini panel in layouts 1–3).
