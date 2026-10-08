# Spec: UI — Panels, part 2 (NPC menu and shop, character panel details, skill tree input, stash and cube buttons)

- **Status:** draft (2026-10-07, RE on the 1.14d `Game.exe`; no capture
  yet). Continues `ui/panels.md` (owner of the UI states, the open mode,
  the panel art and every § not listed here); section numbers continue
  its numbering: §14 moved here unchanged from `panels.md` (size), §17–§20
  are new detail sections for `panels.md` §8–§12, §21 the gold lines,
  buttons and dialog, §22 the d2rs widget answers. Continued in
  `ui/panels-3.md` (§23–§27).
- **Target version:** 1.14d, English install
- **Crate/module:** `d2-client::ui` (`PanelRules`, panel draw lists, hit
  tests), `d2-client::ui::panels` (`character`, `skill_tree`, `stash`,
  `cube`, `npc`, `shop`)
- **Related specs:** `ui/panels.md` (§1 coordinates `W`, `H`, `sx`, `sy`,
  §2 `SetUIState`, §7 close button, §8–§13, §16 tables), `ui/menus.md`
  (menu box, hire list, shop transactions), `ui/text.md` (§6 width A, §7
  `DrawText`, §8 `DrawFramedText`), `render/blend-modes.md` (rectangle
  modes 2 and 6), `monsters/init.md` §8.1 (`0x006538A0`),
  `world/npc.md`, `world/vendors.md`, `skills/levels.md` §6.4 (0x3B),
  `sim/client-messages.tsv` (0x2F, 0x30, 0x31, 0x3C).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 44–53 |
| Inputs | 54–63 |
| Outputs / state changes | 64–68 |
| Rules | 69–70 |
|   14. NPC menu (ui 8) and NPC shop (ui 0x0C) | 71–245 |
|   17. Character panel details (`panels.md` §8; answers UP-6) | 246–360 |
|   18. Inventory close button and click area (`panels.md` §9; answers UP-7, UP-8) | 361–385 |
|   19. Skill tree input and draw order (`panels.md` §10; answers UP-21, UP-22, UP-23) | 386–456 |
|   20. Stash and cube buttons (`panels.md` §11, §12; answers UP-10, `panels.md` OQ 13) | 457–530 |
|   21. Gold amounts, gold buttons and the gold dialog (`panels.md` §9 r6, §11 r6; answers `panels.md` OQ 5) | 531–625 |
|   22. d2rs widget answers (`client/ui.md` §B1, §B2; code `TODO(spec: ui/panels.md …)`) | 626–683 |
| Constants & data dependencies | 684–698 |
| Randomness | 699–703 |
| Edge cases & original bugs | 704–724 |
| Test vectors | 725–750 |
| Provenance | 751–779 |
| Open questions | 780–806 |
<!-- /index -->

## Summary

Detail rules of the panels named in `ui/panels.md` that did not fit that
file: the NPC option table, the shop buttons and tabs (§14), the
character panel's damage block, chance popups and name lines (§17), the
inventory close button and click area (§18), the skill tree's mouse
handling and draw order (§19) and the stash and cube button rectangles
(§20). Coordinates use the `panels.md` §1 expressions; "strict" means
both bounds excluded, "inclusive" both included.

## Inputs

| Name | Type | Source |
|---|---|---|
| frame size, panel shift | `W`, `H`, `sx`, `sy` | `ui/panels.md` §1.1 |
| mouse | event x u16 +0x0C, y u16 +0x0E, wParam +0x08; current position `0x00468730` / `0x00468740` | window message record |
| player | unit, stats (base `0x006253B0`, full `0x00625480`), skills, inventory | bridge snapshot |
| last hovered monster classes | `[0x00711F90]`, `[0x00711F94]` (initial 19, `0x00467A00`) | §17.4 |
| NPC interaction | NPC GUID `[0x007C0D25]`, active `[0x007C0D29]`, class `[0x007C0D2D]` | `world/npc.md` §2 |

## Outputs / state changes

Draw items, queued hover texts, C→S 0x2F, 0x30, 0x31, 0x3C, the button
and tab state globals named per rule, `SetUIState` calls.

## Rules

### 14. NPC menu (ui 8) and NPC shop (ui 0x0C)

1. **Option table.** `0x00726C48`, 48 records of 39 bytes: npc class u32
   @0, count u32 @4 (options + 1 for the trailing cancel), string ids
   u16 × 5 @8, handler pointers u32 × 5 @0x12, flag u8 @0x26.
   `npc-menus.tsv` holds the static (file) contents. Handlers:

   | Handler | Option | Sends |
   |---|---|---|
   | `0x004B6C70` | talk (3381) | dialog: C→S 0x2F / 0x30 (`0x004B6A30`) |
   | `0x004B42B0` | trade (3396), trade/repair (3334) | C→S 0x38 action 1, NPC GUID, 0 |
   | `0x004B3D40` | gamble (3398) | C→S 0x38 action 2, NPC GUID, 0 |
   | `0x004B5C60` | hire (3397) | opens the hire list (`ui/menus.md` §3); choosing a row sends C→S 0x36 (`0x004B1E9E`). The hire-list request C→S 0x38 action 3 is sent when the menu opens (`ui/menus.md` §2.2) |
   | `0x004B52C0` | go west (3383) | C→S 0x38 action 0, NPC GUID, 1 |
   | `0x004B5260` | sail west (3385) | C→S 0x38 action 0, NPC GUID, 0x28 |
   | `0x004B2020` | Identify Items (4020) | C→S 0x34 NPC GUID |
   | `0x004B1DD0` | Resurrect (22695) | C→S 0x62 NPC GUID |

2. **Per-interaction edits.** At every interaction start
   (`0x004B6DD0` → `0x004B2250`) the table is reset: npc 150, 155, 210
   → count 2, slot 2 cleared; 154 (Charsi) → count 3, slot 3 cleared; 515
   → count 2 (3 if its slot 1 is Resurrect), slot 2 cleared; 511 → count
   3, slot 3 cleared; 512 → count 4, slot 4 cleared; 367 → count 2,
   slots 1–2 cleared; Cain 244, 265, 245, 246, 520 → slot 1 = Identify
   Items (`0x004B2020`), count 3 (`0x004B5640` later sets their count
   again). Then the menu builder `0x004B66B0` adds: Kashya (150) with
   character level (stat 12) > 7 → slot 1 = hire (3397, `0x004B5C60`),
   count 3 (`0x004B6410`); for 150, 198, 252, 367, 515 in an expansion
   game `0x004B6440` inserts or removes Resurrect depending on whether
   the mercenary is dead (`[0x00725494]` ≠ −1). Other additions (imbue,
   sockets, personalize, act travel east): §Open questions 8.
   `0x004B6440(record, insert)` does nothing unless the expansion is
   installed and the game is an expansion game. Insert: if a slot already
   has handler `0x004B1DD0` (Resurrect), nothing; else if a slot `j` has
   the hire handler `0x004B5C60`, slots `j … count − 1` move up one and
   Resurrect goes to `j` (before hire); else Resurrect goes to slot
   `count − 1`, after the last option (the builder shows slots 0 …
   `count − 2`, then its own cancel); count += 1. The inserted string id is 0x1507 (5383), a
   placeholder replaced at build time (`ui/menus.md` §2.3). Remove: the Resurrect
   slot is taken out, later slots move down one, count −= 1.
3. **Menu box.** `0x004B4830` builds a menu object (`0x004B7EB0`) with one
   item per option (`0x004B85F0`); special captions: `NPCHeal` (3337)
   gets the heal cost, identify (4020) a cost, `0x1507` a quest text.
   Draw (`0x004B8100`): background either a framed box of the object's
   size (`0x0046EFD0(w, h, 0, 1)`) or a cel at (x, y + h); items top to
   bottom, each at its own x and height; the selected item in color 3,
   or flanked by two `pentspin` frames (`frame = counter % 7`) at
   (x − 24, y + 4) and (x + w + 2, y + 4). Box position and sizes:
   §Open questions 8 (answered: `ui/menus.md` §2).
4. **Shop panel** (ui 0x0C after trade / gamble, `0x00488400`): art
   `%s\ui\panel\buysell` (`0x004B23E0`) frames 0–3 as left quads; NPC
   store grid (`0x00483FF0`); action buttons: `[0x00722160]` buttons,
   frame = base u16 (+0x0A) + state (+0) of table `0x007BC9E4` (stride
   20), at x = `sx − 1 + X[mode][i]` (table `0x00722168`, 4 × 4: mode 0:
   273; 1: 169, 273; 2: 169, 221, 273; 3: 116, 169, 221, 273), y =
   `H + sy − 63`; tabs: `[0x00722158]` tab records `0x00722110` (stride
   18: x i32, y i32, string u16, active u32 @+0x0A, visible u32 @+0x0E;
   defaults Armor 4036, Weapons 4037, Weapons 4037, Misc 4039 at x 42,
   121, 201, 281, y 20); a visible tab draws `Panel\buyselltabs` frame
   `i` (active) or `i + 4` at (`sx + 80 i`, `H + sy − 449`) and its
   caption in Font16 at x = `sx + x − width / 2`, y = `H + sy − 480 + y`,
   color 4 (active) or 0 (not drawn while `0x004B3500()` ≠ 0).
5. Shop messages: buy C→S 0x32, sell 0x33, repair 0x35 (`0x004B2650`),
   gamble identify 0x37 (`0x004C0F20`, `0x004C0FF0`), hire 0x36
   (`0x004B1E80`), closing the shop ends the interaction with C→S 0x30
   (`0x004B3C20`). Item rules: `world/vendors.md`.
6. Menu box geometry and items, the hire list and the client fields
   of 0x32 / 0x33 / 0x35 / 0x36: `ui/menus.md` §2–§4.
7. **Record lookup** (`0x004B2E30`, unit in EAX): no unit → −1; else the
   **first** record of the 48 (`[0x00725A74]`) whose npc class equals the
   unit's class (unit +4); no match → **record 0** (Akara's record), not
   −1. So NPC 257 always uses record 22 (`3334:trade`); record 29 is
   never reached. (Answers UP-16, lookup.)
8. **Flag byte** (@0x26, `npc-menus.tsv` `flag`): read only by the talk
   end `0x004B6A30` (`0x004B6BCF`). When a talk sequence ends with no
   next message, the interaction's NPC (looked up by rule 7) is still
   there, the NPC class is not one of the classes of `0x004B1A10` (146,
   251, 266, 331, 377, 378, 406, 408, 521, 527, 537, 538, 539) and the record's
   flag ≠ 0, the menu state `[0x007C0C6B]` := 1 and the NPC menu is built
   again (`0x004B66B0`); otherwise `[0x007C0C6B]` := 0, `0x00487990`,
   the interaction ends (`0x004B3C20`, C→S 0x30) and
   `SetUIState(8, off, 0)`. Flag 0 records (146, 251, 527, 537, 538,
   539) therefore close the menu after talking. (Answers UP-16, flag.)
9. **Talk messages** (answers UP-15, talk bytes). The talk option
   `0x004B6C70` sends nothing: it closes the menu box (`0x004B8020`),
   sets `[0x007C0C6B]` := 1 and opens the topic box `0x004B5890(3381,
   k)` (`k` = `0x004B1D70()`); choosing a topic replays its text
   (`0x004A10E0`) and sends nothing (`0x004B1A80`). The messages are:
   - interaction start `0x004B6DD0(type CL, GUID EDX, a3, a4)`, NPC found:
     C→S **0x2F** [unit type u32 @1][GUID u32 @5] (`0x004B6E87`), after the
     option table reset (§14.2);
   - same call, NPC not found: the player's four fields +0x150 … +0x15C of
     its data block (unit +0x14) := 0, then C→S **0x30** [a4 low byte,
     zero-extended u32 @1][GUID u32 @5] (`0x004B6E45`);
   - interaction end `0x004B3C20(GUID ECX)`: C→S **0x30** [1 u32 @1][GUID
     u32 @5] (`0x004B3CBD`), then `SetUIState(8, off, 0)`;
   - a quest message shown at the start (`0x004B6FB6`) or the next one of
     a sequence (`0x004B6B49`): C→S **0x31** [NPC GUID u32 @1][message
     id u16, zero-extended, u32 @5].
   All four go through the 9-byte helper `0x004786A0`.
10. **Cain's count reset** `0x004B5640` (answers UP-14 rest). It runs from
    the transaction-result handler `0x004B6390` while the menu state is 10
    (set by the Resurrect `0x004B1DFF`, hire `0x004B1ED7` and Identify
    `0x004B20DD` sends). It first closes the waiting note
    (`[0x007C0D67]`). Result byte `[0x007C0D89]` = 5 → `0x004939B0` only;
    result ≠ 3, 6 → menu state 12 and a one-item note box (handler
    `0x004B4CF0`; its caption: §Open questions 3); result 3 or 6 → if the
    interaction NPC is 244, 245, 246, 265 or 520, the counts of records
    16, 17, 18, 19 and 38 (Cain's) := 2; then the NPC menu is rebuilt
    (`0x004B4830`), whose identify caption sets the count again (`ui/menus.md`
    §2.3).
11. **Shop buttons** (`0x00487ED0`, run by the shop setup `0x00491940`;
    answers UP-17). Button records `0x007BC9E0` + 20 i (`i` 0–3): +0
    enabled u32, +4 state u32 (the "state +0" of rule 4), +8 string id
    u16, +0x0A u16, +0x0E base frame u16 (the "base u16 +0x0A" of rule 4,
    counted from `0x007BC9E4`), +0x10 u32. `[0x007C0DB0]` (`0x004B3500`)
    is written only with 0 (`0x004B6FD7` …) and cleared by the
    interaction memset, so the "≠ 0" branches of this rule and of rule 4
    are dead: the tab captions are always drawn.
    - NPC 147, 148, 177, 199, 202, 252, 254, 255, 405, 512, 513: count
      `[0x00722160]` = 4, mode `[0x0072215C]` = 3, `[0x007BCC05]` = 3;
      record 0 (1, 0, 3335 "Buy", 2, frame 2, 3), record 1 (1, 0, 3336
      "Sell", 3, frame 4, 4), record 2 (0, 0, 0, 1, frame 0, 1), record 3
      (1, 0, 4144 "Close", 9, frame 10, +0x10 unchanged);
    - NPC 154, 178, 253, 257, 511: records 0 and 1 as above, record 2 (1,
      0, 3338 "Repair", 4, frame 6, 1), record 3 (1, 0, 10095 "Repair all
      equipment: %d", 1, frame 18, 8); count 4, mode 3;
    - any other NPC: count 0 (no buttons).
    The draw (rule 4) draws every record `0 … count − 1`, enabled or
    not, with frame = base + state of `panel\buysellbtn`.
12. **Shop tabs** (`0x00487A10(page)`, answers UP-17). Pages > 4 are read
    as 0. Only while `[0x007BCC08]` = 0 and the NPC has an inventory: the
    store items are counted per page (`0x00628250` of the item's record;
    pages ≥ 5 not counted); every tab record gets active := 0, visible :=
    (its page has items); the current page `[0x007BCC04]` := page; if
    that page is empty, the page steps +1 (to 0 when above `[0x007BCC05]`
    or at 0xFF) at most 4 times until a page has items; that tab's active
    := 1. Start page (`0x00491940`): 147, 512 → 0; 154, 511 → 1; 148, 177,
    178, 202, 252, 253, 255, 257, 405, 513 → 3, and then `[0x007BCC08]` :=
    now + 500 ms; every other class → 0 without the delay. The inventory
    draw clears `[0x007BCC08]` once `GetTickCount` passes it
    (`0x0048EE24`–`0x0048EE36`).
13. **Shop mouse** (handler table `0x0070F870`: WM_RBUTTONDOWN
    `0x00491AD0`, WM_LBUTTONDOWN `0x00491D20`, WM_LBUTTONUP `0x00488B00`,
    WM_MOUSEMOVE `0x004889D0`). Left button down, inventory modes 0–4
    only (else consumed, nothing):
    - inventory close rectangle (`panels.md` §9.3), not over the belt
      (`0x00498DC0`) and `0x00483E10()` ≠ 0: close pressed `[0x007BCE90]`
      := 1, click sound, consumed;
    - with a cursor item (`0x004680A0` ≠ 0), not over the belt: if x ≤
      `[0x007BCAA0]` (the left edge of the player's inventory area) and
      y ≤ `[0x007BCAAC]`: consumed; inside the store grid rectangle
      (`[0x007BCB5C]`–`[0x007BCB68]`, inclusive; the grid of the NPC's own
      `inventory.bin` record, `0x00621050` on the NPC, loaded by
      `0x004835B0` with its `inv` rectangle into `[0x007BCBB0]`), with
      the cursor mode `0x00468830` ≠ 6 and a price (`0x004B2AD0`): the
      sell click `0x004B3870(1, x, y, wParam, quick 1, 0)` (`ui/menus.md`
      §4.5), elsewhere on that side nothing more; x > `[0x007BCAA0]` or
      y > `[0x007BCAAC]`: the grid handler `0x004912A0`;
    - tab row: x in [`sx`, `W / 2 − 1`], y in [`H + sy − 479`, `H + sy −
      449`] (inclusive): tab 0 for x ≤ `sx + 80`, 1 for ≤ `sx + 160`, 2
      for ≤ `sx + 240`, 3 for `sx + 240` < x < `sx + 320`; a visible tab
      that is not the current page: click sound, `[0x007BCC08]` := 0, rule
      12 with that page; consumed;
    - button bar: x in the store record's `inv` x range
      (`[0x007BCBB0]`, `[0x007BCBB4]`) and y in [`H + sy − 109`,
      `[0x007BCBBC]`]: the button `i` with `sx + X[3][i]` < mx < `sx +
      X[3][i] + 45` and `H + sy − 109` < my < `H + sy − 65` (strict; the
      latched mouse `[0x007BCA4C]`, `[0x007BCBD8]`; `X` = rule 4 table
      without the − 1 of the draw). Enabled: click sound; if another
      button is pressed (state 1) it is released and this one pressed; if
      this one was pressed it is released, the inventory mode := 1 and,
      without a cursor item, the cursor is reset (`0x00468070`); if none
      was pressed it is pressed. Not enabled: `0x00487BE0`. Consumed.

### 17. Character panel details (`panels.md` §8; answers UP-6)

1. **No stat points.** With the base `statpts` (stat 4, `0x006253B0`) = 0
   the points box, its two labels, the number and the four add buttons
   are not drawn (`0x004A7D00`, two separate tests). Mouse down
   (`0x004A7720`) inside the panel area (`panels.md` §4.4) outside the
   close rectangle is consumed without a press or sound. Mouse up
   (`0x004A78C0`) clears no add-button flag, sends nothing, calls
   `0x0044DA70` and is consumed.
2. **Mouse up with points** (corrects `panels.md` §8.5 "clears every
   pressed field first"): the four buttons are walked in table order;
   each button's pressed field is cleared as it is reached; the first
   button whose rectangle holds the release spends (count 1, or the full
   stat 4 value `0x00625480` with Shift) and the walk **stops**, so the
   pressed fields of the buttons after it stay set (a press on Vitality
   released on Strength spends Strength and leaves Vitality drawn
   pressed until the next release).
3. **Class line** (`0x004A8303`–`0x004A834B`, only while the player is a
   player unit): Font16 (set at `0x004A81ED`), color 0, the u16 string at
   the `charstats` record +0 of the player's class (D2MOO
   `wszClassName`; class outside the table → no record), centered in
   [`sx + 193`, `sx + 310`] at y `H + sy − 455`.
4. **Name line** (`0x004A8350`–`0x004A83E6`): the name (`0x0047A210`);
   `n` = its count of UTF-8 code points (`0x005262A0`, invalid bytes not
   counted); font: Font16 for `n` ≤ 10, Font8 (0) for `n` = 11 or 12,
   Font6 for `n` ≥ 13; converted to u16 (128 units), centered in [`sx +
   13`, `sx + 160`] at y `H + sy − 455`, color 0; then Font16 again.
5. **Damage block** (`0x004EDA20`): for the left skill (skill list +8,
   `0x00620190`) with slot `p` = 0, then the right skill (+0x0C,
   `0x006201D0`) with `p` = 6, `0x004ED570(player, skill, p)`. Entry `e`
   = (x1, y, x2) of table `0x0072D840` (stride 12): e0 (162, 93, 258), e1
   (162, 101, 258), e2 (263, 98, 307), e3 (162, 160, 270), e4 (162, 163,
   270), e5 (270, 160, 310), e6 (162, 117, 258), e7 (162, 125, 258), e8
   (263, 122, 307), e9 (162, 184, 270), e10 (162, 187, 270), e11 (270,
   184, 310); screen x = `sx` + x, y = `H + sy − 480` + y; "centered in
   e" = centered in [x1, x2] (`0x004A7080`). Once per run (flag
   `[0x007C8CA4]`), languages 6–9 move e0 and e6 up by 1. Nothing is
   drawn without a skill level record (`0x00644140`) or skilldesc row
   (`0x004A89D0`). In order, Font6:
   - the skill name: skilldesc `str name` (+0x0E; 5382 = 0x1506 without
     a row) copied and upper-cased for units < 0x100 by table `0x00730578`
     (`0x00452180`), color 0, centered in e`p`;
   - `d` = the skill state `0x004D9FC0` is 0, 1, 6 or 8 and skilldesc
     `descdam` (+0x12) < 0x90 with a non-null entry of `0x0072D768`;
     `a` = the same state test and `descatt` (+0x14) < 0x48 with a
     non-null entry of `0x0072D7F8`;
   - if `d`: `strchrskm` (4061 "Damage") color 0 centered in e`p+1`, then
     the `descdam` function with the entry e`p+2` (it draws the value;
     owner §Open questions 1);
   - if `a`: the `descatt` function gives (v1, c1, v2, c2); if v1 or v2 ≠
     0: the label = `strchratr` (4063 "%s\nAttack Rating"; 4065 when the
     skills record `0x00643CE0` is null) formatted with the name; if it
     has a LF before its end the two parts at y − 4 and y + 4, else the
     name alone at y, each centered in e`p+3`, color 0; then the value in
     e`p+5`: v2 ≠ 0 → Font6, `%d` v1 at y − 6 (`[0x0072D8D4]`; −7 in
     languages 6–9) color c1 and `%d` v2 at y + 2 (`[0x0072D8D0]`) color
     c2; else v1 ≠ 0 → `%d` v1 at y, Font16 when v1 < 1,000 else Font8,
     color c1. Entry e`p+4` is never used.
6. **Chance to hit** (`0x004A7340(AR)`, AR = v1 of the `descatt`
   function of that hand, `0x004EDA80`; 0 → no popup): `M` = the
   `monstats` row `[0x00711F90]` (fatal if invalid), `Lm` = its `Level`
   for the difficulty (+0xAA + 2d), `D` = its AC from `0x006538A0(class,
   …, d, Lm, 2, buf)` (`monsters/init.md` §8.1); classic game, difficulty
   ≠ 0 and `Align` (+0x4C) ≠ 1: `D` := D × 10 / 12. `A` = AR + the
   class's `charstats` ToHitFactor (+0x3C, `0x004A7300`). If D < 0: A −=
   D, D := 0; if A < 0: D −= A, A := 0. pct = 100 when A = D = 0, else
   100 A / (A + D). `Lp` = the player's level (stat 12, full value);
   chance = 2 × pct × Lp / (Lm + Lp), clamped to [5, 95] (C division).
7. **Chance to be hit** (`0x004A74A0`): `M` = row `[0x00711F94]` (no
   row → 0); `D` = player defense (`0x006223F0`) + stat 33; `A` = the
   first non-zero to-hit of `0x006538A0` flags 8, 0x10, 0x20 (buffer
   +0x0C); classic, difficulty ≠ 0, `Align` ≠ 1: A := A × 10 / 15. The
   same negative-value moves with A and D swapped roles (D < 0: A −= D,
   D := 0; A < 0: D −= A, A := 0); pct = 100 when both 0, else 100 A / (A
   + D); value = 2 × pct × Lm / (Lp + Lm), clamped to [5, 95] by the
   popup.
8. **Hovered monster classes** (`0x00466DE0`, run when the hovered unit
   changes): a monster (type 1) whose `monstats` row has `npc` clear and
   `Align` ≠ 1 and whose `monstats2` row has `isAtt` set:
   `[0x00711F90]` := its class; if also `inert` is clear,
   `[0x00711F94]` := its class. Both start as 19 at game start
   (`0x00467A00`).
9. **Popups** (drawn at once, in Font8: the font set at `0x004A819E`;
   mouse = the current position):
   - to-hit (`0x004A7AE0`): x in [`sx + 162`, `sx + 320`] and y in
     [`H + sy − 334`, `H + sy − 315`] → left hand (index 0), else y in
     [`H + sy − 308`, `H + sy − 289`] → right hand (index 1); no popup
     when that hand's chance is 0. Table `0x006DA430` (stride 24): index
     0 box (154, 115), line 1 (159, 128), line 2 (159, 140); index 1 box
     (154, 130), line 1 (159, 143), line 2 (159, 155), all + (`sx`,
     `H + sy − 480`). Rectangle `0x004F6300(x, y, x + 155, y + 30, color
     [0x007C02F8] = 0 (never written), mode 2)`; line 1 `charavghit`
     (4159), color 0; line 2 = 10103 (`charmonsterX` "%s: %d%%") with the
     monster's `NameStr` (`monstats` +6) and the chance, color 0; both
     not centered.
   - to-be-hit (`0x004A7180`): x in [`sx + 173`, `sx + 311`], y in
     [`H + sy − 286`, `H + sy − 267`]; block `b` = `0x00622720(player,
     expansion game)`; if `b` ≤ 0: `b` = `0x004A70C0`: walk the player's
     stat 348 (`passive_weaponblock`) entries (`0x006261D0`, ≤ 32): an
     entry with layer 0 sets `b` to its value; an entry whose layer
     matches the item in body location 5 or 4 (`0x00629BB0`) raises `b`
     to its value if larger; that `b` is kept only when `0x0064F380`
     reports weapon class 13 (else 0). Text: `b` ≠ 0 → 10105
     (`charmontohit2X`) with (b, chance, name, chance); else 10104
     (`charmontohit1X`) with (name, chance); `DrawFramedText(text, sx +
     139, H + sy − 315, 0, mode 2, k 0)` (`ui/text.md` §8).
   The formatter `0x005269D0` takes one argument per conversion,
   **`%%` included**; hence the dummy `chance` before the name in 10105.
10. **Draw order** (`0x004A7D00`, answers `panels.md` OQ 12): quads →
    close hover (queued) → points box, labels, number → add buttons → 15
    labels → damage block → to-hit popup → to-be-hit popup → close button
    → class line → name line → the 18 values. Each draw covers what was
    drawn before it; a value wider than its span is drawn over the labels
    and buttons.

### 18. Inventory close button and click area (`panels.md` §9; answers UP-7, UP-8)

1. Mouse up `0x00486EF0` first clears the close pressed flag
   `[0x007BCE90]` and `[0x007BCE94]` (`0x00486F03`). Then, unless the
   mouse is over the belt (`0x00498DC0`): a release in the close
   rectangle → `SetUIState(1, off, 0)` and return (no pressed check and
   no cursor-item check); elsewhere the cursor item is only read
   (`0x0063C1E0`, result unused). Then, if the gold-button flag
   `[0x007BCE30]` is set and the inventory mode is 0: cleared, and if
   `0x00486DA0()` and `[0x007BCE2C]` = 0, `0x00454150(1)`. Then
   `0x0044DA70`; consumed when the release is inside the right panel's
   area (rule 2) and not over the belt.
2. **Area test** (`0x00483AB0` and the same inline test in `0x00486EF0`,
   `0x00489060`, `0x004890B0`, `0x004894D0`, `0x00489A30`, `0x0048A0F0`,
   `0x004917C0`, `0x004922A0`, `0x00492310`): left ≤ x < right
   (`[0x007BCAA4]` exclusive) and top ≤ y ≤ bottom (`[0x007BCAAC]`
   **inclusive**). Record 0 at 640 × 480: x 320–639, y 0–441.
3. Belt test `0x00498DC0`: with the belt popup open (`[0x007BEF98]`)
   the popup rectangle (`0x00660CB0`); else y > `H − 48`, x in
   [`W / 2 + 23`, `W / 2 + 145`] and y in [`H − 39`, `H − 10`].
4. The `panel\inv_*` backgrounds (`panels.md` §9.4) have no
   `panel-layout.tsv` rows on purpose: their positions are slot
   rectangles of `inventory.bin`, which the §16.2 grammar cannot name;
   `panels.md` §9.4 is their source.

### 19. Skill tree input and draw order (`panels.md` §10; answers UP-21, UP-22, UP-23)

Icon hit of skill `i` (class skill index): its skilldesc page = tab `t`
and `X < mx < X + 48`, `Y − 48 < my < Y` (`panels.md` §10.5); pressed
flag `[0x007C0A38 + 4i]` (−1 = not learnable, 0, 1 pressed), remap
`[0x007C0838 + 4i]`. Close rectangle: variant 1–3 (`[0x00724CE0]`), `X`
= `W − sx + o`, `Y` = `H + sy − 63`, inclusive [`X`, `X + 34`] × [`Y −
34`, `Y`], tested on the **current** mouse position (`0x004AB5F0`).

1. **Mouse down** `0x004AB7E0`, in this order (not consumed when over the
   belt, without a player, or y > `H − 48`):
   1. `[0x007C0C4C]` = 0 (no draw yet; only the draw sets it, to 1):
      `0x0044DA70`, consumed, done.
   2. Base skill points (stat 5) < 1 and `W − sx − 320` < x < `W − sx −
      88`: for each class skill on tab `t` whose icon is hit, not pressed
      and not `passive` (`skills.txt` flags byte +4 & `[0x006CE278]` =
      0x10 clear): C→S **0x3C** [skill id u32 @1 (bit 31 clear: right
      hand)][0xFFFFFFFF @5] (`0x004786A0` at `0x004AB929`), then
      `SetUIState(4, off, 0)`; the walk goes on; consumed. The skill need
      not be learned.
   3. Tabs: x in [`W − sx − 88`, `W − sx`]: `panels.md` §10.2; consumed;
      the handler goes on.
   4. Close: variant 1–3 and the close rectangle: click sound, pressed
      `[0x007C0C38]` := 1; the handler goes on.
   5. `W − sx − 320` < x < `W − sx − 88`: points < 1 → consumed, done;
      else the first hit, not pressed icon: pressed := 1, remap := 3,
      mouse capture (`0x00467F20`), click sound, consumed, done; none:
      consumed, done.
   6. Otherwise not consumed.
   So one press can set both the close flag and the column-3 / row-6
   icon where they overlap (x [`W − sx − 149`, `W − sx − 120`], y
   [`H + sy − 97`, `H + sy − 78`] for `o` = −149).
2. **Mouse up** `0x004ABC30`: no player → nothing; capture release
   (`0x00467FA0`); over the belt → nothing more; `[0x007C0C4C]` = 0 →
   `0x0044DA70`, consumed. Close pressed → pressed := 0, release in the
   close rectangle → `SetUIState(4, toggle, 0)`; consumed, done (icon
   flags untouched). x ≤ `W / 2` → not consumed. Points are **re-checked**:
   base stat 5 ≤ 0 → no flag cleared, `0x0044DA70`, consumed. Else the
   walk: the hit icon with pressed = 1 → the checks of `panels.md` §10.5,
   C→S 0x3B, `[0x007C0C3C]` := 0, its flags := 0, consumed, done; every
   other visited icon whose pressed ≠ −1 gets pressed := 0, remap := 0.
3. **Free points number** (`0x004AC200`, only when base stat 5 ≥ 1):
   `%i` of base stat 5 at (`W − sx − 52`, `H + sy − 400`), current font
   (Font16), color 1 when the player has state 54 (`uninterruptable`,
   `0x00639DF0`) or `[0x007C0C3C]` ≠ 0 (only ever written 0), else 0.
   There is no drawn box: the frame is part of the background art.
4. **Tab tool tips** (`0x004AB310`, `t` = current tab), current mouse:
   only for x in [`W − sx − 89`, `W − sx + 1`]; region top `T` and
   string: tab 1 current: y in [`H + sy − 264`, `H + sy − 156`] → `T` =
   `H + sy − 264`, 4225 (`StrSklTreeb`); y in [`H + sy − 372`, `H + sy −
   264`) → `T` = `H + sy − 372`, 4224 (`StrSklTreea`); tab 2 current:
   [`H + sy − 372`, `H + sy − 264`] → 4224; [`H + sy − 156`, `H + sy −
   48`] → `T` = `H + sy − 156`, 4226 (`StrSklTreec`); tab 3 current:
   [`H + sy − 264`, `H + sy − 156`] → 4225; (`H + sy − 156`, `H + sy −
   48`] → 4226. Font16; rectangle `0x004F6300(W − sx − 89, T + 5, W − sx
   + 1, T + 25, color 0x004FB180(0, 0, 0), mode 6)`; text at (`W − sx −
   89`, `T + 20`), color 0: the centering span ends at the constant 90
   (`0x004A7080` with x2 = 0x5A), so the text is **left-aligned** at x1.
5. **Close tool tip** (`0x004AB630`): in the close rectangle, `strClose`
   queued at (`X + 15`, `H + sy − 98`), centered.
6. **Draw order** (`0x004AC690`): Font16, `[0x007C0C4C]` := 1; background
   frames 0–3 then `4t` … `4t + 3`; tab captions (`0x004AACE0`, per
   class, centered in [`W − sx − 90`, `W − sx`], §Open questions 2);
   points ≥ 1: the free-points number then every class skill of tab `t`
   in class order (`0x004AC200` → `0x004ABF60`), else the icons only
   (`0x004AC4D0`); per skill: the icon (`0x004F64B0`), its hover
   description if hovered (`0x004EF410` not learnable / `0x004EEF00`,
   drawn at once, so later icons cover it), the level number (its font
   switch is not undone); then the close button (`0x004AB530`), the
   close tool tip (queued), the tab tool tip (at once).

### 20. Stash and cube buttons (`panels.md` §11, §12; answers UP-10, `panels.md` OQ 13)

1. **Stash mouse down** `0x00492510` (inventory modes 0x0C / 0x0D only;
   mouse = the event's, latched into `[0x007BCA4C]` / `[0x007BCBD8]`):
   inventory close rectangle (unless the belt popup is open and the mouse
   is over it) → `[0x007BCE90]` := 1; else the stash gold button
   (`0x00489920`, inclusive: x in [`sx + 73`, `sx + 150`], y in [`H + sy −
   455`, `H + sy − 438`] expansion, [`H + sy − 259`, `H + sy − 242`]
   classic) → `[0x007BCE34]` := 1; else the stash close button
   (`0x00489980`, **strict**: `X` < x < `X + 40`, `Y − 35` < y < `Y + 5`,
   `X` = `sx + 272` / `sx + 275`, `Y` = `H + sy − 64` / `H + sy − 65`
   expansion / classic) → `[0x007BCE3C]` := 1, pressed `[0x007BCE38]` :=
   1, `[0x007BCE9C]` := 1. Each of the three: click sound 4
   (`0x004B9A00(4, 0, 0, 0)`), consumed. The release uses the same
   strict close rectangle (`panels.md` §11.7); the hover in the draw uses
   the inclusive [`X`, `X + 40`] × [`Y − 35`, `Y + 5`] (`0x0048F37D`–
   `0x0048F3A3`) with `strClose` queued at (`X + 12 − w / 2`, `Y − 35`).
2. **Cube mouse down** `0x004927C0` (mode 0x0E): inventory close
   rectangle, not over the belt → `[0x007BCE90]` := 1, sound 4; else cube
   close (`0x00489FB0`, strict: `sx + 275` < x < `sx + 315`, `H + sy −
   100` < y < `H + sy − 60`) → `[0x007BCE44]` := 1, pressed `[0x007BCE40]`
   := 1, transmute flags `[0x007BCE4C]` / `[0x007BCE48]` := 0,
   `[0x007BCE9C]` := 1, sound 4; else transmute (`0x0048A000`, strict:
   `sx + 144` < x < `sx + 184`, `H + sy − 223` < y < `H + sy − 183`) →
   close flags := 0 and, when the cursor holds nothing (`[0x007A6ABC]` =
   0, `0x004680A0`), `[0x007BCE4C]` := 1, pressed `[0x007BCE48]` := 1,
   `[0x007BCE9C]` := 1, sound 4; consumed either way.
3. **Cube mouse up** `0x0048A190` (mode 0x0E, else `[0x007BCE9C]` := 0
   only): `[0x007BCE90]` := 0; inventory close rectangle with the belt
   popup closed and no cursor item → `SetUIState(0x1A, off, 0)` (one 0x4F
   0x17 through the close hook); cube close rectangle (strict, as above)
   with pressed → pressed := 0, `0x0048A050` (`panels.md` §12.7);
   transmute rectangle with pressed → pressed := 0, C→S 0x4F 0x18. The
   draw's hover tool tips use the same strict rectangles (`0x0048F0D7`,
   `0x0048F11F`): `strClose` at (`sx + 289 − w / 2`, `H + sy − 100`),
   `strUiMenu2` at (`sx + 158 − w / 2`, `H + sy − 223`). Both tips are
   pop-up text (`0x00502280` `D2Win_SetPopUpUnicodeText`, colour 0, centre
   0) in font 1 (set at `0x0048F0CA`), drawn later by `0x00503000` with the
   box of `ui/control-panel.md` §5 r14; string ids 4144 (`strClose`,
   `0x0048F0E0`) and 3341 (`strUiMenu2`, `0x0048F12C`).
4. **Exit / dead close** (`0x0048EEB0`–`0x0048F183`): when `0x0044DA30()`
   (exit flag) or `0x00463DF0()` (no local player, or its mode is 0x11
   dead) is set in inventory mode 0x0E (the only tests at `0x0048EEB0`–
   `0x0048EEC4`; a missing cube does not close, `world/cube.md` §11 r3), the draw calls
   `SetUIState(0x1A, off, 0)` with the mode still 0x0E and the latch
   `[0x007BCC54]` untouched. If ui 0x1A was open, its close hook (case
   0x1A of `0x00455AE0`, jump table `0x00455E80`: `0x00455DDD` →
   `0x0048A500`) sets mode 0 and calls `0x0048A050`, which sends the
   latched 0x4F 0x17; then `0x0048F183` sends 0x4F 0x17 again
   unconditionally: **two** messages (one if ui 0x1A was already
   closed). (`index/switches.tsv` lists this table one case too low:
   case 25 there is ui 0x1A.)
5. **GoldMax font** (UP-9, partly): the stash path (`0x0048F18F`–
   `0x0048F2E4`) sets no font; the text uses the font left by the last
   `0x00502EF0` call before it (this frame or an earlier one): §Open
   questions 4. Text: 4051 formatted with `0x005269D0` (100 units) and
   the cap `0x00623460(player)`, `DrawText` color 0, not centered.
6. **GoldMax font** (answers UP-9, `panels.md` §11.3): font 1
   (`Font16`). Every frame, step 7 of the UI pass runs the belt draw
   `0x00499040`, which sets font 1 (`0x00499053`) and never restores it
   (`ui/control-panel.md` §5 r4). In a frame with the stash open, every
   other `0x00502EF0` caller reached between that step and step 5 of
   the next frame saves and restores the font (static read of all 176
   call sites: steps 8–10 `0x0047B720`, `0x004B8100`, `0x00503000`,
   `0x004549F0`, `0x0046C060`, the text pass `0x0049DC40`,
   `0x0049DEE0`, `0x0049D5A0`, `0x0049D8E0`; steps 1–4 `0x00454AD0`,
   `0x004ADEA0`, `0x00492FA0`, `0x00493100`; the death text `0x00453100`
   runs only for a dead player, who cannot have the stash open), so
   `GoldMax` is drawn in font 1. Exceptions, reproduced: a refused
   overhead bubble in the previous frame's text pass leaves font 13
   (`ui/messages.md` §5 r4), and a frame in which a UI state changes
   runs the open / close hooks (`0x00455720`, `0x00455AE0`) whose menu
   builders are not covered here.

### 21. Gold amounts, gold buttons and the gold dialog (`panels.md` §9 r6, §11 r6; answers `panels.md` OQ 5)

1. **Gold line** `0x00488100(k)` (k in ECX). The inventory family
   (`0x0048EDF0`) calls it with k = 2 in the NPC trade modes 1–9 (after
   `0x00488400`, at `0x0048EE72`), k = 0 in the stash modes 0x0C / 0x0D
   (at `0x0048F350`, after the stash grid) and k = 1 in every mode (at
   `0x0048FF31`, after the equipped items `0x004845A0`, `ui/inventory.md`
   §6, and the shop extras `0x004886C0`). It sets font 1 (Font16) and
   restores the previous font on return. The value is the full stat
   (`0x00625480(P, stat, 0)`), formatted `%d` (`_snprintf`, 20 bytes,
   widened to u16), drawn with `DrawText` color 0, not centered:

   | k | Stat | Value pen | Button (`Panel\goldcoinbtn`, frame `p`) |
   |---|---|---|---|
   | 0 stash | 15 `goldbank` | (`sx + 165`, `H + sy − 440`) expansion game, (`sx + 165`, `H + sy − 244`) classic | `p` = (`[0x007BCE34]` ≠ 0), at (`sx + 75`, `H + sy − 440 + p`) / (`sx + 75`, `H + sy − 244 + p`) |
   | 1 inventory | 14 `gold` | (`W − sx − 212`, `H + sy − 72`) | `p` = (`[0x007BCE30]` ≠ 0), at (`W − sx − 236`, `H + sy − 71 + p`) |
   | 2 shop | 15 `goldbank` | right-aligned: x = `sx + 198 −` width A of the value (`text.md` §6), y = `H + sy − 106` | none; then the label 3315 `stash` "Stash" at (`sx + 21`, `H + sy − 106`) |

   The button is a cel draw (draw mode 5, light 0xFF, no remap) of the
   cel `[0x007BCC34]` (`Panel\goldcoinbtn`, loaded by `0x004868B0`): 2
   frames of 20 × 18, offsets 0 (d2data, measured). Pressed = frame 1,
   drawn one row lower.
2. **Stash hover** (k = 0, current mouse, `0x00486820(x0, y0, 20, 18)`,
   inclusive): x in [`sx + 73`, `sx + 93`], y in [`Y − 18`, `Y`], `Y` =
   `H + sy − 438` expansion / `H + sy − 242` classic → tool tip 4124
   `strGoldWithdraw` "Withdraw" queued at (`sx + 53`, `H + sy − 458` /
   `H + sy − 262`), color 0, not centered (queue `0x00502280`,
   `ui/control-panel.md` §Outputs).
3. **Hit rectangles.** Inventory gold button (`0x00486DA0`, x in EDI, y
   in ESI): no cursor item and x in [`W − sx − 237`, `W − sx − 217`], y
   in [`H + sy − 87`, `H + sy − 69`] (inclusive). Stash gold button:
   §20.1 (`0x00489920`).
4. **Press.** Inventory mode 0 (`0x00492310`): a press in the inventory
   gold rectangle sets `[0x007BCE30]` := 1 and plays sound 4
   (`0x004B9A00(4, 0, 0, 0)`); consumed. Stash modes: §20.1
   (`[0x007BCE34]`).
5. **Release.** Inventory mode 0: §18.1 (flag set → cleared; release in
   the rectangle and `[0x007BCE2C]` = 0 → gold dialog kind 1). Stash
   modes (`0x00489AC0`, `0x00489BD2`–`0x00489C0A`; no pressed-flag
   check): release in the inventory gold rectangle and `[0x007BCE2C]` =
   0 → gold dialog kind 3; then release in the stash gold rectangle and
   `[0x007BCE2C]` = 0 → kind 4; then, unless the mouse is over the belt,
   `[0x007BCE34]` := 0 and `[0x007BCE30]` := 0.
6. **Gold dialog** `0x00454150(k)` (k in ECX): nothing without a player,
   with a cursor item or while a dialog is open (`[0x007A27A0]` ≠ 0).
   Else: if the latch `[0x007A27B4]` = 0, key mode 0 with key-up kept
   (`0x0046AA20(0, 1)`, `ui/controls.md` §4.1 r5) and latch := 1; value
   `[0x007A2A68]` := 0; then by k (jump table `0x00454558`):

   | k | Set-up | Max (full stat) | Prompt (string id) |
   |---|---|---|---|
   | 0 | `[0x007C02EC]` := 1 (`0x004A7A90`) | 14 | 4033 `strDropGoldHowMuch` |
   | 1 drop | `0x004898A0` | 14 | 4033 |
   | 2 trade offer | `0x004898A0` | 14 | 4046 `strTradeGoldHowMuch` |
   | 3 deposit | `0x004898A0` | 14 | 4049 `strBankGoldDeposit` |
   | 4 withdraw | `0x004898A0` | 15 | 4050 `strBankGoldWithdraw` |
   | ≥ 5 | close (rule 7) only | — | — |

   `0x004898A0` sets `[0x007BCE2C]` := 1, inventory mode 0x0D → 0x0C,
   and closes the chat box (`SetUIState(5, off, 0)`). k is kept in
   `[0x007A279C]`. The dialog is a menu box (`0x004B7CD0(215, 140,
   callback 0x00453FC0, 0, 0, 1)`, the `ui/menus.md` §2 family) holding
   the prompt (font 1, word-wrapped to 200 pixels by `0x00502970`, one
   box line per wrapped line, `0x004B85F0(24, 0, 4, 1, 0, 0)`) and four
   controls, coordinates as passed: `0x004BC480(223, 219, 0, callback
   0x00453FE0)`, a numeric edit box `0x004BBD80(258, 228, 100, 1, max,
   10, callback 0x00453FD0, 1)`, OK `0x004BB0F0(250, 287, 0, callback
   0x00454080)`, Cancel `0x004BB0F0(355, 287, 1, callback 0x00454140)`.
   Kind 3 pre-fills the edit box with the max (the test also names kinds
   6 and 7, which never get this far); kind 2 then calls `0x004B90B0`. The controls' art, caret and key input: §Open questions
   6 (answered: r9).
7. **Close** `0x00453EE0` (Cancel, and the first step of OK): latch set
   → key mode 1 (`0x0046AA20`), latch := 0; value `[0x007A2A68]` := the
   edit box's value (its +0x2C method); the box and the four controls
   are freed; kinds 1–4 → `0x00489880` (`[0x007BCE2C]` := 0, mode 0x0D →
   0x0C); kind 0 → `0x004A7A80`.
8. **OK** `0x00454080`: close (rule 7), then with `v` = the value:
   - `v` = 0: kind 2 → `0x004B9110`; other kinds nothing;
   - kinds 0, 1: C→S **0x50** [P's GUID u32 (−1 without P)][`v` u32]
     (`0x004786A0`; `sim/client-messages.tsv` `DropGold`); no sound;
   - kind 2: `0x004B9110` (player trade), sound 0xDD;
   - kind 3: C→S **0x4F** [button 0x14 u16][p1 = `v >> 16` u16][p2 =
     `v & 0xFFFF` u16] (`0x00478600`), sound 0xDD;
   - kind 4: C→S 0x4F [button 0x13][`v >> 16`][`v & 0xFFFF`], sound 0xDD.

   Sound 0xDD = `0x004B9A00(0xDD, 0, 0, 0)` (`audio/triggers.md` §1 r1).
   Server meaning of 0x4F 0x13 / 0x14 and of 0x50: `panels.md` OQ 6.
   Server side of 0x4F 0x13 / 0x14 (withdraw / deposit checks; the
   stash maximum is a flat 2,500,000; carried gold over level × 10000 →
   S→C 0x2C event 19 only): `world/vendors-2.md` §10.2.

9. **Gold dialog box and controls** (the box, OK / Cancel buttons, the
   numeric edit box, the spinner; their art, input and value rules):
   `panels-3.md` §28.

### 22. d2rs widget answers (`client/ui.md` §B1, §B2; code `TODO(spec: ui/panels.md …)`)

1. **Button frames** (`Button`, §B1). No panel button of `panels.md`
   §7–§13 or of §14, §17–§21 here changes its image on hover: hover only
   queues a tool tip, fills a rectangle (`panels.md` §13 r4) or changes
   the remap of a skill icon (`panels.md` §10.3, `panels-3.md` §25). The
   image changes only with the button's pressed flag (or a state), as
   each rule states: close buttons frame 10 released / 11 pressed
   (`panels.md` §7.2); add buttons `Panel\Level` frame 0 / 1 (§8.4);
   cube transmute `Panel\miniconvert` frame 0 / 1 (§12.3); gold buttons
   frame 0 / 1 one row lower (§21.1); shop buttons and tabs §14.11,
   §14.12; skill icons `IconCel` / `IconCel + 1` (§10.3); waypoint tabs
   `2t` / `2t + 1` (§13.3, a state, not a press).
2. **Label pen** (`Label`, §B1). A label's pen is never the widget rect
   origin: each text rule gives the pen (x, y), y being the bottom row of
   the glyph cell (`ui/text.md` §4 r2) and x given or centered over a
   span (`panels.md` §1.6). A d2rs `Label` built from a
   `panel-layout.tsv` `text` row takes pen x from `x` (centered over `w`
   when `cond` has `centered`; `x − width / 2` for `half_centered`) and
   pen y from `y`; its rect is only the hit / clip area.
3. **Wheel scrolling** (`ScrollList`, §B2). No panel of `panels.md`
   §8–§14 or of this file scrolls with the mouse wheel. The handler
   tables of 1.14d hold `WM_MOUSEWHEEL` (0x20A) entries only for the
   game window (`0x0070F2C8` → `0x0044C400`: the commands bound to keys
   0x103 / 0x104, `ui/controls.md` §4.2 r3), the key-config screen
   (`0x006D6028` → `0x004A59D0`: one list step per event with |delta| ≥
   120 and click sound 0; `0x00724790` → `0x004A63B0`: assigns the wheel
   as a key, `ui/controls.md` §5), the menu box (`0x007273E8` →
   `0x004B7610`: the event is offered to each control of the open box in
   order) and the out-of-game controls (`0x0072DDCC` → `0x004FA340`)
   (scan of every 0x20A dword outside `.text`). A d2rs list in an
   original-UI panel uses step 0.
4. **UI image request** (`UiRules::ui_image`, §B1, §B6). An image request
   names a DC6 file (under `data\global\ui\`, or the explicit path of its
   rule), a frame and direction 0; the archive is `client/assets.md`'s.
   The request's point is the rule's **cel draw point** (X, Y): the frame
   covers columns `X + xoff … X + xoff + w − 1`, rows `Y + yoff − h + 1 …
   Y + yoff` (`render/sprite-placement.md` §2). Shading
   (`render/composition.md` §5): the plain cel draw `0x004F6480` with
   light 0xFF has no light map and no remap; the colored cel draw
   `0x004F64B0` with remap `k` uses text-color map `k` (none for `k` =
   0), exactly as glyphs (`ui/text.md` §4 r3–r4). Blend: draw mode 5
   (every UI cel unless a rule names another) → `Opaque` (DC6
   transparency only); mode 3 (Horadric animation `panels.md` §12.4,
   scroll symbols `panels-3.md` §27) → `IndexTable(ADD)`; mode 1
   (ethereal item graphic, `ui/inventory.md` §8) → `IndexTable(A1)`
   (`render/blend-modes.md` §1, §7). Filled rectangles (`0x004F6300`:
   tool-tip boxes, tints) follow `render/blend-modes.md` §8 r2 (mode 0 →
   `d' = A2[256·d + color]`, mode 2 → `A0`, mode 5 → `color`, mode 6 →
   `MAX[256·d]`).
5. **Open mode for the camera** (`ViewFeed::open_mode`, `render/camera.md`
   §1). The screen open mode is UI state only: the value `panels.md` §4.2
   gives after the last `SetUIState` (`SetScreenOpenMode` `0x0045AEA0`,
   read by `0x0045AE90`); no server message and no world-model field
   carries it. A feed without the original UI answers 0 (no panel open);
   with it, the feed answers what the UI set (`set_ui_open_mode`).
6. **Cursor** (`client/ui.md` §B6): `panels-3.md` §23.

## Constants & data dependencies

- Tables read from the image: `0x0072D840` (12 × 3 i32), `0x006DA430`
  (2 × 6 i32), `0x0072D8D0` = 2, `0x0072D8D4` = −6, `0x006CE268` … =
  1, 2, 4, 8, 0x10, 0x20; handler table `0x0070F870`; button X table
  `0x00722168` (`panels.md` §14.4).
- Strings (English; ids ≥ 10,000 from the `Patch_D2` `patchstring.tbl`,
  1,179 entries): 3335, 3336, 3338, 4061, 4063, 4065, 4144, 4159, 4224–
  4226, 5382, 10095, 10103–10105.
- Gold (§21): `Panel\goldcoinbtn` (2 frames, 20 × 18); strings 3315,
  4033, 4046, 4049, 4050, 4124; sound ids 4, 0xDD; stats 14, 15.
- `charstats` +0 (class name), +0x3C (ToHitFactor); `monstats` +6, +0x4C,
  +0xAA; `monstats2` `isAtt`, `inert`; `skills.txt` flags +4;
  `skilldesc` +0x0E, +0x12, +0x14; `itemstatcost` 348.

## Randomness

None. The shop tab delay (§14.12) and the cube animation are wall-clock
timed; the popups depend on the mouse position.

## Edge cases & original bugs

Reproduced by default.

- An unknown NPC class gets record 0 (Akara's options) (§14.7).
- The second NPC 257 record is dead (§14.7).
- A stat-button release stops at the first hit button and leaves later
  pressed flags set (§17.2).
- Skill-tree tab tool tips are left-aligned (centering span bug, §19.4).
- A no-points click on any non-passive skill icon, learned or not,
  sends 0x3C and closes the tree (§19.1).
- A skill-tree release with no points left keeps the icon pressed
  (§19.2).
- A cube that disappears while open sends 0x4F 0x17 twice (§20.4).
- The trade-NPC shop draws button record 2 although it is disabled
  (§14.11).
- Gold-button releases in the stash do not check the press flags: a
  release on the button opens the dialog even when the press began
  elsewhere (§21.5).
- A gold dialog OK with 0 sends nothing (§21.8).

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| unit class 999 at interaction | record 0 (148's talk, trade) | §14.7 |
| NPC 257 | record 22: trade, cancel | §14.7 |
| name `Abcdefghij` (10) / `Abcdefghijk` (11) / 13 code points | Font16 / Font8 / Font6 | §17.4 |
| AR 100 (after ToHitFactor), monster AC 300, Lp 5, Lm 10 | pct 25, chance 2·25·5/15 = 16 | §17.6 |
| AR 500, AC 100, Lp 10, Lm 5 | pct 83, 110 → 95 | §17.6 |
| monster TH 200, player defense 300 + 0, Lm 10, Lp 20 | pct 40, 2·40·10/30 = 26 | §17.7 |
| 640 × 480, mouse (200, 150), left chance 40 | rectangle (154, 115)–(309, 145) mode 2; 4159 at (159, 128); "<name>: 40%" at (159, 140) | §17.9 |
| `0x005269D0("%d%% %s", 5, 0, L"x")` | `5% x` | §17.9 |
| release at (639, 441) / (640, 441) / (639, 442), record 0, 640 × 480 | inside / outside / outside | §18.2 |
| 800 × 600, 3 free skill points | `3` at (668, 140) | §19.3 |
| 640 × 480, tab 1 current, mouse (600, 250) | rectangle (551, 221)–(641, 241) mode 6; 4225 at (551, 236) | §19.4 |
| 640 × 480 shop, mouse down (100, 10) | tab 1 (if visible and not current) | §14.13 |
| 640 × 480 shop (mode 3), mouse down (140, 400) | button 0 (116 < 140 < 161, 371 < 400 < 415) | §14.13 |
| stash close (expansion, 640 × 480) press at (272, 416) / (273, 416) | none / pressed | §20.1 |
| cube vanishes while ui 0x1A open | 0x4F 0x17 twice | §20.4 |
| 800 × 600, inventory open, 1,234 gold | `1234` at (508, 468) Font16; button frame 0 at (484, 469) | §21.1 |
| 640 × 480 expansion stash, 50,000 in stash | `50000` at (165, 40); button at (75, 40) | §21.1 |
| deposit dialog OK with 70,000 | `4F 14 00 01 00 70 11` | §21.8 |
| withdraw dialog OK with 5 | `4F 13 00 00 00 05 00` | §21.8 |
| drop dialog OK with 100, player GUID 1 | `50 01 00 00 00 64 00 00 00` | §21.8 |
| any panel, wheel event | no list scroll (step 0) | §22.3 |

## Provenance

1.14d `Game.exe` (exports `re/exports/funcs`, `tools/ghidra/disasm.py`,
`index/*.tsv`): NPC record lookup `0x004B2E30`, talk end `0x004B6A30`,
`0x004B1A10`, talk option `0x004B6C70`, topics `0x004B5890`,
`0x004B1A80`, interaction start `0x004B6DD0`, end `0x004B3C20`, result
handler `0x004B6390`, Cain reset `0x004B5640`; shop `0x00487ED0`,
`0x00487E20`, `0x00487E60`, `0x00487EA0`, `0x00487A10`, `0x00491940`,
`0x00491D20`, `0x00488B00`, `0x004B3500`; character panel `0x004A7D00`,
`0x004A7720`, `0x004A78C0`, `0x004EDA20`, `0x004EDA80`, `0x004ED570`,
`0x004E9870`, `0x004E9940`, `0x004A7080`, `0x004A7340`, `0x004A74A0`,
`0x004A7AE0`, `0x004A7180`, `0x004A70C0`, `0x00466DE0`, `0x005262A0`,
`0x005269D0`; inventory `0x00486EF0`, `0x00483AB0`, `0x00498DC0`;
skill tree `0x004AB7E0`, `0x004ABC30`, `0x004AB5F0`, `0x004AB630`,
`0x004AB310`, `0x004AC690`, `0x004AC200`, `0x004ABF60`; stash / cube
`0x00492510`, `0x00489920`, `0x00489980`, `0x004927C0`, `0x00489FB0`,
`0x0048A000`, `0x0048A190`, `0x0048EDF0`, close hook `0x00455AE0`;
gold `0x00488100` (call sites `0x0048EE72`, `0x0048F350`, `0x0048FF31`
with ECX 2, 0, 1), `0x00486820`, `0x00486DA0`, `0x00492310`,
`0x00489AC0`, dialog `0x00454150` (table `0x00454558`), `0x00453EE0`,
`0x00454080` (table `0x00454124`), `0x00454140`, `0x004898A0`,
`0x00489880`, `0x00478600`; wheel handler entries found by a scan of
the non-code sections for 0x20A (script outside the repo); DC6 header
of `goldcoinbtn` read with Python.
Table values read from the image and the English string tables
(`d2data` `string.tbl`, `Patch_D2` `patchstring.tbl`) with Python
scripts outside the repo. D2MOO (1.10f) gave the `charstats` field
names (+0, +0x3C) only. No capture yet.

## Open questions

1. The `descdam` (`0x0072D768`, 0x90 entries) and `descatt`
   (`0x0072D7F8`, 0x48 entries) functions of the damage block (§17.5):
   one skill-description function per id, shared with the skill hover
   boxes (`0x004EEF00`); owner: a skills description spec (PC 1). Read
   of each table entry.
2. Skill-tree tab captions `0x004AACE0` per class: string ids and rows
   (register arguments of the 524A30 / `0x004AAAA0` calls). Disassembly
   read.
3. The caption of the transaction-result note box of `0x004B5640` (switch
   on `[0x007C0D89]` = 7, 8, 10, 11, 12, 15, 16 at `0x004B5660`…).
   Disassembly read.
4. **Answered** (2026-10-07, §20.6: font 1, left by the belt draw). Was: The font in effect when the stash draws `GoldMax` (§20.5): a static
   read of every `0x00502EF0` caller reachable in the UI pass before
   step 5, or a capture of the stash at 800 × 600 (`GoldMax` glyph
   heights).
5. Meaning of the shop button fields +0x0A and +0x10 (§14.11), read by
   `0x00488B00` / the hover code. Ghidra read.
6. **Answered** (2026-10-07, `panels-3.md` §28; the server side:
   `world/vendors-2.md` §10.2). Was: Gold dialog controls (§21.6): the art, layout, caret, digit entry,
   max clamp and Enter / Esc handling of `0x004BBD80` (edit box),
   `0x004BB0F0` (buttons) and `0x004BC480`, and the box `0x004B7CD0`
   draw; the server effect of 0x4F 0x13 / 0x14. Disassembly read of
   those constructors and their vtables; a capture of the drop-gold
   dialog.
