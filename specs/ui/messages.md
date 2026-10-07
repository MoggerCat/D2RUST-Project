# Spec: UI — Message consumers (chat lines, overhead text, NPC dialog, hire popup, item-socket dialog, NPC intros)

- **Status:** draft (2026-10-07, RE on the 1.14d `Game.exe`; no capture
  yet). The UI half of the S→C messages whose model half is
  `client/msg-ui.md` §4–§10: that spec owns the handlers, the message
  layouts and the dispatch; this one owns what each UI consumer shows,
  the UI state it keeps and the input it takes.
- **Target version:** 1.14d, English install
- **Crate/module:** `d2-client::ui::original` (`messages`, `overhead`,
  `dialog`, `socket_ui`, `npc_intro`)
- **Related specs:** `client/msg-ui.md` §4–§10 (handlers, outputs);
  `ui/text.md` §5 (color codes), §7 (`DrawText`), §9 (vertical window),
  §10 (wrap); `ui/text-fonts.tsv` (font ids); `ui/panels.md` §2
  (`SetUIState`), §5 (UI pass order), §7 (shared parts);
  `ui/panels-2.md` §14 (NPC menu); `ui/menus.md` §2 (menu box), §3 (hire
  list); `audio/triggers.md` §10 (NPC speech, greetings);
  `render/unit-composite.md` (overlays); `world/quests.md` §6.7 (intro
  record); `world/npc.md` §2, §4 (interaction, 0x38);
  `sim/client-messages.tsv` (0x30, 0x31, 0x38, 0x3E, 0x44, 0x4D).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 50–65 |
| Inputs | 66–75 |
| Outputs / state changes | 76–85 |
| Rules | 86–87 |
|   1. Entry points | 88–101 |
|   2. Screen message list (`0x0049E3A0(text, color)`) | 102–136 |
|   3. Chat line formats (0x26, `client/msg-ui.md` §4 r3) | 137–173 |
|   4. Recipe scroll text (0x26 type 7) | 174–189 |
|   5. Overhead text | 190–249 |
|   6. NPC text list `[0x007BF250]` (0x27 type 1) | 250–308 |
|   7. Dialog panel (`0x004A1320`, `0x004A10E0`) | 309–423 |
|   8. Timed text box (`0x004A1510(id)`, 0x27 type 2 kind 3) | 424–436 |
|   9. Hire offers and the hire popup (0x4E, 0x4F, 0x50 code 2) | 437–453 |
|   10. Other 0x50 codes (UI effects) | 454–488 |
|   11. Item-socket dialog (UI state 0x0E, 0x58 codes) | 489–558 |
|   12. NPC alert (0x8A, `client/msg-ui.md` §9 r3) | 559–565 |
|   13. NPC intro table `0x00726850` (0x91) | 566–594 |
|   14. Interact NPC `[0x007C0D25]` / `[0x007C0D29]` (answers `client/msg-ui.md` OQ7, writer part) | 595–611 |
| Constants & data dependencies | 612–629 |
| Randomness | 630–634 |
| Edge cases & original bugs | 635–655 |
| Test vectors | 656–678 |
| Provenance | 679–705 |
| Open questions | 706–731 |
<!-- /index -->

## Summary

The consumers below run in the UI layer when `client/bridge.md` §10
delivers a `ChatLine`, `NpcText`, `HireOffer`, `QuestSpecial`, `OpenUi`,
`NpcInteract` or `NpcIntro` output. Chat and system lines go into a
screen message list drawn at the top left for 10 s; overhead text is a
per-unit record drawn as a framed bubble above the unit, moved aside
when it overlaps another bubble; NPC dialog text scrolls up in a 325 ×
112 panel while the speech plays; a short quest text shows in a timed
box; a hire offer turns the hireling icon on (and pops the hireling
panel once per install); 0x58 opens, fills and closes the item-socket
dialog (UI state 0x0E, used by the Horadric orifice and the imbue /
socket / personalize services); 0x91 marks NPCs whose next greeting is
the "return" line. The interact-NPC globals are written only by UI code
(§14).

## Inputs

| Name | Type | Source |
|---|---|---|
| UI outputs | `ChatLine`, `NpcText`, `HireOffer`, `HireListReset`, `QuestSpecial`, `OpenUi`, `NpcInteract`, `NpcIntro` | `client/msg-ui.md` §4–§10 |
| `W`, `H`, `sx`, `sy`, open mode | frame size, panel shift, `[0x007A5210]` (`0x0045AE90`) | `ui/panels.md` §1, §4 |
| UI state flags | `0x007A27C0 + 4i` (`0x004538D0(i)`) | `ui/panels.md` §2 |
| wall clock | `GetTickCount`, `timeGetTime` | host (see §Edge cases) |
| registry | `HKCU\…\Diablo II` values `Text Display Beta`, `PopupHireling` | `0x00414F10` read, `0x004150E0` write |

## Outputs / state changes

Screen message list `[0x007BF1E4]` and message log `[0x007BF1F0]`;
overhead records (unit +0xA4); NPC text list `[0x007BF250]`; dialog
panel `[0x007BF0A4]`; timed box `[0x007BF1A8]`; hire popup flags
`[0x007BEEE4]`; item-socket dialog state `[0x007C5470]`–`[0x007C5498]`;
intro table `0x00726850`; `SetUIState` calls (0x0E, 0x23, 0x24, 0x25,
8); C→S 0x30, 0x31, 0x38, 0x44, 0x4D; UI sounds (`0x004B9A00(id, unit,
0, 0, 0)`).

## Rules

### 1. Entry points

| Message | Model half | UI entry | Here |
|---|---|---|---|
| 0x26 | `client/msg-ui.md` §4 | `0x0049F490` | §2, §3, §4, §5 |
| 0x27 | `client/msg-ui.md` §5 | `0x004A1600` | §5, §6, §7, §8 |
| 0x4E / 0x4F | `client/msg-ui.md` §6 | `0x004B3240` / `0x004B3290` | §9 |
| 0x50 | `client/msg-ui.md` §7 | `0x004B9210` | §9, §10 |
| 0x58 | `client/msg-ui.md` §8 | `0x004C0550` | §11 |
| 0x8A | `client/msg-ui.md` §9 | `0x004B3380` | §12 |
| 0x91 | `client/msg-ui.md` §10 | `0x004B3510` | §13 |

0x89 is `render/lighting.md` §10 r4; 0x78 (trade) is out of scope.

### 2. Screen message list (`0x0049E3A0(text, color)`)

1. **Add** (wide text ECX, color DL). A 0x26-byte record: line pointers
   +0x00…+0x14 (6), line count u16 +0x18, color u32 +0x1A, expiry u32
   +0x1E = `GetTickCount() + 10000`, next +0x22. The text is wrapped
   (`ui/text.md` §10) to `W − 70` pixels in font 13 (`FontInGameChat`,
   set with `0x00502EF0` and restored after); the first 6 lines are
   kept, the rest are freed. The record is appended at the **tail** of
   `[0x007BF1E4]` (the head is the oldest); `[0x007BF1E8]` counts the
   records (a count that does not match the walked list is fatal
   0xD6C).
2. A copy goes to the message log (`0x0049DBC0`: `[0x007BF1F0]`, newest
   first, at most 128 records; at 128 the oldest is freed first). When
   the color is 4 and the record has exactly one line, the line is
   converted to 8-bit (`0x005263E0`, 256 bytes) and passed to the text
   filter object's method +0x18 (`0x00611560`; `client/msg-ui.md` OQ9).
3. If the list now holds more than 18 lines in total, the head record
   (oldest) is removed (`0x0049D490`), once per add. UI sound 6
   (`cursor_switch`) is requested with no unit. If the message log
   (state 0x18) is open and its object `[0x007BF1B4]` exists, its method
   +0x3C runs and `0x0049E340` refreshes it.
4. **Draw** (`0x0049DC40`, called by the text pass `0x004A0E70` only while
   state 0x18 is closed; §5 r1). Font 13. Start y = 20, or 95 while
   state 0x13 is open. x and re-wrap by open mode: mode 1 → x = 15,
   re-wrap; mode 2 → x = `W / 2 + 15` (C division), re-wrap; modes 0 and
   3 → x = 15, no re-wrap. Records head to tail, their lines in order;
   a line counter k (from 0, over all records) gives y = start + 15 k.
   - no re-wrap: line width w = width C (`0x00501730`) of the line; w = 0
     → skipped (k unchanged); else the framed box `0x0046EFD0(x − 4, y −
     14, w + 8, 16, 0, 1)`, then `DrawText(line, x, y, color, 0)`, k + 1;
   - re-wrap: each line is wrapped again to 300 pixels and each piece is
     drawn the same way with the box `(x − 5, y − 14, w + 10, 16, 0, 1)`.
5. **Expiry** (end of the same draw): every record whose expiry is
   below `GetTickCount()` is unlinked and freed (count − 1).

### 3. Chat line formats (0x26, `client/msg-ui.md` §4 r3)

`0x0049F490` reaches its type switch only after the squelch, filter and
conversion steps of `client/msg-ui.md` §4 r3.1–r3.3; `T` is the converted
wide text (≤ 256 units), `N` the 16-unit wide copy of the message name
(`0x00526F20`), `n` = byte length of the name, `t` = byte length of the
text, `c8` = u8@8. Strings: 3654 `chatmsg1` " whispers: ", 3658
`strwhisperworked` "ÿc0You whispered to ÿc1%sÿc0: %s", 3994 `colorcode`
"ÿc", 4048 `SysmsgPlayer1` ": ". `Prefix(s, k)` (`0x004521C0`) = when s
is not empty, `"ÿc"` + the digit `'0' + k` + s. `Strip(s)`
(`0x00452300`; s ≥ 500 units is fatal 0x37F) removes every `ÿc` + the
unit after it, scanning from the start again after each removal, and
stops at a `ÿc` that ends the string.

| Type | Condition | Line added (`0x0049E3A0`) | Color |
|---|---|---|---|
| 4 | – | T | c8 |
| 1 | u8@3 ∈ {0, 1} | T | c8 |
| 1 | else, 1 ≤ n ≤ 15 | `Prefix(N, 4)` + `Prefix(": " + T, 0)` | c8 |
| 1 | else, t ≤ 305 | T | c8 |
| 1 | else | nothing | – |
| 2 | 1 ≤ n ≤ 15 | N + " whispers: " + T | 2 |
| 2 | else, t ≤ 305 | T | 2 |
| 2 | else | nothing | – |
| 6 | – | `Strip(swprintf(3658, N, wide(text)))` | 2 |
| 5 | unit present | overhead text (§5 r2) | – |
| 7 | – | recipe scroll (§4) | – |
| other | – | nothing | – |

1. "n = 0" (an empty name) takes the "else" rows, like n ≥ 16 (the
   test is `n − 1 > 14` unsigned).
2. Type 6 formats into a 0x264-unit buffer with `0x005269D0`
   (`ui/text.md`); `Strip` removes the colors of the format **and** any
   `ÿc` the player typed, so the echo is one color (2).
3. Types 1 and 2 do not strip: a player's `ÿc` codes in T recolor the
   rest of the line (`ui/text.md` §5).

### 4. Recipe scroll text (0x26 type 7)

1. `0x0048BBE0(text, lang, c8)`: the text, converted with
   `MultiByteToWideChar(CP_ACP, MB_PRECOMPOSED, …, 256)` (no language
   check: the language loop at `0x0049F87F` computes a length it never
   uses), is copied to `[0x007BCEA8]`, `[0x007BCEA4]` := c8, then
   `SetUIState(0x25, on, 0)` (state 0x25 changes only in an expansion
   game, `ui/panels.md` §2 r3).
2. Draw `0x0048BC10` (UI pass `[0x25]`, `ui/panels.md` §5 step 4), only
   while the game is an expansion game (`[0x007A04F4]`, `0x0044DCC0`)
   and state 0x25 is open: `menu\recipescroll` (`0x004520C0`, cached in
   `[0x007BCE8C]`) frames 0, 1, 2, 3 at (sx, H + sy − 224), (sx + 256,
   H + sy − 224), (sx, H + sy − 48), (sx + 256, H + sy − 48), light
   0xFF, draw mode 5; then the text in font 4 (`FontFormal10`) with
   `DrawText(text, sx + 80, 130 − sy, c8, 0)`.

### 5. Overhead text

1. **Text pass** `0x004A0E70` (UI pass step 10, `ui/panels.md` §5), in
   order: the overhead counter `[0x007BF20E]` += 1; the view rectangle
   is read (`0x00476070`, `0x004760C0`) into `[0x007BF214]` (left, top);
   placed-bubble count `[0x007BF224]` := 1 with slot 0 = the dialog
   panel rectangle (`[0x007BF278]`, `[0x007225FC]`, +325, +112) while
   the dialog panel is up (`[0x007BF1C0]` = 1, §7), else 0; overhead
   bubbles of players (`0x00464950(0x004A0B30)`), then monsters
   (`0x00464990(0x004A0D20)`), then objects (`0x004649D0(0x004A0D20)`);
   the dialog panel step (§7 r6); the timed box (§8 r2); the screen
   messages (§2 r4) while state 0x18 is closed; the menu box
   `[0x007BF1B0]` (`0x004B8100`, `ui/menus.md` §2) when it exists.
2. **Record** (`client/msg-ui.md` §4 r4; unit +0xA4, UI state): text,
   language, end = counter at creation + d. The record of 0x27 (type 1,
   kind 3, count 1) holds the decimal string of a string id; the record
   of 0x26 type 5 holds the player's text.
3. **Per unit** (`0x004A0B30` players, `0x004A0D20` monsters and
   objects; units without a record are skipped). (px, py) = the unit's
   client pixel point (`0x00620900`) − (`[0x007BF214]`, `[0x007BF218]`);
   py −= 30 (players) or 10 (others); open mode 1 → px −= W / 4, mode 2
   → px += W / 4 (W / 4 rounded toward 0), mode 3 → nothing drawn.
   Drawn only when 0 < py < H and −100 < px < W + 100. Text:
   - monsters, objects: v = `atol(record text)`; 1 ≤ v ≤ 0xFFFE → the
     string v (`0x00524A30`), at most 199 units; else nothing;
   - players: if the text starts with bytes 0xFF 0xFF, v = `atol` of
     the rest; v in 1069–1295, 21894–22037 or v = 10916 → string v (the
     `Playersubtitles` and act-intro voice lines; at most 199 units);
     otherwise (or the string is empty) the record text converted with
     the record's language (`0x0049E280`; failure → nothing).
4. **Bubble** (`0x004A0A00(record, unit, &(px, py), text)`): if the
   counter is above the record's end, the record is freed and unit
   +0xA4 := 0, and this frame still draws it. Font 13; the text is
   wrapped to 180 pixels into a box: w = widest line (width A) + 20, h =
   15 · lines + 6 (`0x0049D3C0`). Position: x = px − w / 2 (w >> 1), y =
   py − 50 − 15 · lines − 3; rectangle (x, y, x + w, y + h). Placement
   (r5) runs while fewer than 16 bubbles are placed; it can move the
   rectangle or refuse (no draw, and the font is **not** restored:
   reproduce). With 16 placed, the bubble is drawn unplaced. Placed or
   unplaced, a drawn bubble counts: `[0x007BF224]` += 1.
5. **Placement** (`0x0049E070(rect, n)`, n = placed count, slots
   `0x007BF0A8` + 16 i): n = 0 → slot 0 := rect, accept. Else, no
   `IntersectRect` with slots 0…n − 1 → slot n := rect, accept. Else
   for dx = 50, 100, … 350, for dy = 35, 70, … 245, for i = 0…7 with
   m = table `0x00722654`[i] = (1, 1, −1, 1, 1, −1, −1, −1): the
   candidate is (x + m·dx, y + m·dy) with size **400 × 280** (not the
   bubble's); it must satisfy x' ≥ 0, x' + 400 ≤ W, y' ≥ 0, y' + 280 ≤
   H and intersect none of slots 0…n − 1; the first such candidate is
   stored in slot n and becomes the bubble's rectangle (accept). None →
   refuse. (The table looks like four (x, y) pairs but each entry is
   used for both axes, so only (+, +) and (−, −) moves are tried, each
   twice: reproduce.)
6. **Bubble draw** (`0x0049D9A0` → `0x0049D8E0(box, x, y)`): open mode
   1 and x > W / 2 − 10, mode 2 and x < W / 2 + 10, or mode 3 → not
   drawn. Box field +0x16 = 3 → not drawn; 0 → set to 1 (§Open
   questions 2). Drawn only with y ≥ 0 and y + h < H: the framed box
   `0x0046EFD0(x, y + 4, w, h − 5, 0, 1)`, then in font 13 the first
   min(lines, 10) lines, line i at `DrawText(line, x + 10, y + 18 + 15 ·
   (i + lines − min(lines, 10)), 0, 0)`.

### 6. NPC text list `[0x007BF250]` (0x27 type 1)

1. Structure (`.\Text\Text.cpp`): header {u32 +0 (0), count u16 +4,
   head +8}; entries {string id u16 +0, kind u32 +4, next +8}. 0x27
   type 1 (not the overhead case, `client/msg-ui.md` §5 r2.1) frees the
   old list, builds a new one from the message bytes 6–39 (count u8@6 ≥
   8 is fatal 0x112) by pushing each entry at the **head**, then sorts
   it (`0x006616E0`, count > 1): an insertion sort by string id,
   ascending; an entry moves only past strictly greater ids, so equal
   ids keep the reversed order of the message.
2. Freed when the interaction ends (`0x004A1730`, from `0x004B3C20`) and
   on game exit (`0x004A0680`).
3. **Use: the talk topic box** (`0x004B5890(3381 "talk", k)`, built by
   the NPC menu's talk option `0x004B5BC0`, k = the NPC's intro-table
   index `0x004B1D70()` (§13), entry pointer `[0x007C0C7D]`): a menu box
   (`ui/menus.md` §2; a second one while `[0x007C0D6F]` is set is fatal
   0x76F) with the caption 3381, then:
   - "introduction" (3399, handler `0x004B41E0`) when the entry's flag
     +0x14 = 0;
   - "gossip" (3395, handler `0x004B41C0`);
   - one item per list entry of **kind 2**, in list order (`0x00661390`
     counts them, `0x006613C0(list, i)` picks the i-th; handler
     `0x004B1A80` replays the text, `ui/panels-2.md` §14.9), captioned
     by `0x0049F910(id)`: the caption id of the first pair (text id u16,
     caption id u16) of the 527-entry table `0x00722678` whose text id
     equals id, searching entries 0–99 when id < entry 100's text id
     (164), else entries 100–526; not found → 3724 "Invalid Quest
     Value";
   - NPC 201 (Jerhyn): "about the merchants" (3392, `0x004B1B80`);
   - NPC 244, 245, 246, 265, 520 (Cain) with the Horadric Cube (`box `)
     in the local player's inventory: 2231 "Horadric Cube"
     (`0x004B1C00`);
   - "cancel" (3400, `0x004B5810`).
   With no list (`0x0049F900()` = 0) only caption and "cancel".
   Caption item: height 21, color 4, font 1, not selectable; the others
   height 15, color 0, font 1, selectable; anchor `0x004B1C80`
   (`ui/menus.md` §2.6), p1 `0x004B5810`, p2 `0x004B4010`, p5 = 1, p9
   = 1, style 1.
4. "gossip" (`0x004B41C0` → `0x004B40D0(index = entry +0x0D, 1)`): the
   NPC must still be present, else menu state 0, `0x00487990`, the
   interaction ends (`0x004B3C20`) and state 8 closes. Else
   `[0x007C0C69]` := 1, end callback `0x004B18C0` (`0x0049E7E0`), the
   topic box is freed, the entry's +0x11 := 1; the first time in a game
   (`[0x007C0C6A]` = 0) every entry's index is re-rolled (`0x004B17A0`,
   r5; sets `[0x007C0C6A]` := 1) and the new +0x0D is used; the text
   played (`0x004A10E0`, §7 r1) is u16 +0 of text record (index modulo
   count +9).
5. Gossip index (`0x004B1680(entry)`): up to 10 draws on the local
   player's unit seed (+0x20 / +0x24, multiplier 0x6AC690C5, the same
   step as `sim/rng.md`; mask when the count is a power of two, else
   modulo; count 0 → index 0); each draw is stored in +0x0D. A draw i
   is kept when i ≥ 2, the text record (15 bytes at +5 + 15 i) has the
   player's class at +0x0B (or 7 = any), and either its byte +2 is 0 or
   `0x0065C310([0x007C0D43], u32 +7, 0)` equals u32 +3; a kept draw on
   the list `0x00725CB0` with text id 0xFF is replaced by 2 when game
   quest 12 bit 13 is set (`0x0065C310([0x007C0D47], 12, 13)` = 1).
   After 10 rejected draws, +0x0D := 2. Other text record fields: §Open
   questions 4.

### 7. Dialog panel (`0x004A1320`, `0x004A10E0`)

1. **Open.** `0x004A1320(id CX, place EDX, flag)` (0x27 type 2, quest
   log `0x004A27D0`) and `0x004A10E0(unit ECX, id DX)` (NPC talk and
   gossip):
   - place 0 (and every `0x004A10E0`): top centre, x `[0x007BF278]` =
     `(W − 325) / 2` (C division), y `[0x007225FC]` = 12,
     `[0x007BF236]` := 0; place ≠ 0: x = sx, y = 261 − sy,
     `[0x007BF236]` := 1;
   - the previous dialog speech is faded out (`0x004B9610` →
     `0x004B9EF0(…, 0, 4)`), `[0x007BF24B]` := 0; `0x0044DA40`;
   - mouse window `0x00467430(x − 10, y, x + 335, min(y + 182, H − 53))`
     and `0x00466FE0`; `[0x007BF212]` := id;
   - **only if no panel is up** (`[0x007BF0A4]` = 0): `[0x007BF1BC]` :=
     0; the panel object is built from string id in font 8
     (`FontFormal11`, r2; failure fatal 0x1063 / 0x1024); +0x16 := 0;
     `[0x007BF1C0]` := 1; `0x004A1320` also sets `[0x007BF1FA]` := 1;
     when the skip handlers are not registered (`[0x007BF27C]` = 0):
     register the 7 handlers of `0x00722600` (r7) on the game window,
     `[0x007BF22C]` := `GetTickCount()`, `[0x007BF27C]` := 1; the
     dialog speech for id (`0x004E0650`, `audio/triggers.md` §10 r2, r6)
     is requested on the local player with d = 5 (`0x004A10E0` first
     runs `0x004BAA50` and detaches the NPC's skill voices,
     `0x004CB190`), handle `[0x007BF24B]`, sound `[0x007BF260]`.
     A second open while a panel is up changes only the id, the
     position and the stopped speech: the old text keeps scrolling.
   - `0x004A1320` with flag ≠ 0 and id ∈ {127, 396, 20002, 20131,
     20169} (`A1Q5InitQuestTome`, `A2Q4SuccessfulNarrator`,
     `AncientsAct5IntroGossip1`, `A5Q3FoundAnyaAnya`,
     `A5Q6InitAncients`): `[0x007BF230]` := 1 (r8).
   - `0x004A10E0` with a unit: a different unit already in a dialog
     (`[0x007BF20A]`, `[0x007BF202]`) gets C→S 0x30 [1][its GUID] and
     `0x004B3830`; then `[0x007BF20A]` := 1, `[0x007BF202]` := the
     unit's GUID. Without a unit: the timed box is closed and both are
     cleared.
2. **Text** (`0x004A0320(text, &count, &speed)`): lines split at LF,
   at most 100 units each (a longer line is cut and the rest starts the
   next line). The first line is the scroll speed: when all its units
   are < 0x80, speed := `atol` of it, else speed := 8; it is never
   shown. An empty text gives no lines. Line width ≥ 308 only logs.
3. **Scroll** (`0x0049D5A0`, per draw, font 8). Panel fields: lines +0,
   count +4, position p +8 (1/1024 pixel), speed +0x0C, t_last +0x1A,
   t_start +0x1E, step +0x22, acc +0x26, same u16 +0x2A. With t =
   `timeGetTime()`:
   - t_start = 0: t_last := t_start := t (p stays 0);
   - else e = (t − t_start) >> 2, a = 0; if t = t_last: step = 0 →
     same += 1, else acc += step and a = acc; if t ≠ t_last: acc := 0,
     step = 0 and same ≠ 0 → step := (t − t_last) / same (once), then
     t_last := t; p := (a + e) · speed.
4. **Panel draw** (x = `[0x007BF278]`, y = `[0x007225FC]`), only while
   `[0x007BF1F6]` (the "Text Display Beta" registry value, default 1,
   set by the NPC Speech option: 0 for "speech", 1 for "text" and
   "both", `0x0047CEF0`) or `[0x007BF1FA]` is set:
   - place 0 only: framed box `0x0046EFD0(x, y − 5, 325, 122, 0, 1)`,
     border `0x00452E50` (`menu\boxpieces`) around (x − 1, y − 6, x +
     326, y + 117);
   - line i (from the first) has q_i = p − 18432 i; with 0 ≤ q_i < 96256
     (94 px) it is drawn whole: `DrawText(line, x + 16, y + 112 −
     (q_i >> 10), 0, 0)`. The walk stops at the first line with q_i <
     18432;
   - the line above the first whole one (q = q_first + 18432) is drawn
     with the vertical window (`ui/text.md` §9) at (x + 16, y + v), v =
     94 − (q_first >> 10), skip 0, lines min(v, 18);
   - the line below the last whole one is drawn at (x + 16, y + 130 −
     (q_last >> 10)) with skip s = 17 − (q_last >> 10) and lines h − s
     (h = font height, `0x00501A40`), when s < h;
   - no whole line: the **last** line, q = p − 18432 (count − 1), when q
     < 112 · 1024: v = 112 − ((q + 1024) >> 10), drawn at (x + 16, y +
     v), skip 0, lines min(v, 18) (nothing when v = 0).
   - Finished when (p >> 10) > 18 · max(count − 1, 1) + 112
     (`[0x007BF264]` := 0 if it was 1).
5. **Wait for speech** (`0x004A0770`): while the speech handle
   `[0x007BF24B]` is set, its request is not yet playing (`0x004B95C0`)
   and the master volume is not 0 (`0x00514CC0`), the panel is neither
   scrolled nor drawn (it stays up). Otherwise the handle is dropped and
   r3–r4 run.
6. **End.** The text pass (§5 r1) keeps a result R := 0. While
   `[0x007BF1C0]` = 1 and the panel exists with +0x16 = 0 or 1, R :=
   `0x004A0770([0x007BF1BC])` (1 = still running):
   - a skip request (`[0x007BF1BC]` ≠ 0): +0x16 := 2, R = 0;
   - r5 waiting: R = 1;
   - else r3–r4; not finished: R = 1; finished: `[0x007BF1BC]` := 1;
     the panel is freed (`0x0049D410`) unless a unit dialog runs
     (`[0x007BF20A]`); `0x0049F960` (r8); then, if an end callback is
     set (`[0x007BF258]`, argument `[0x007BF25C]`, EDX 0; set by
     `0x0049E7E0`, e.g. the quest log `0x004A27B0`, NPC talk
     `0x004B18C0`), R = its result; otherwise the timed box flag
     `[0x007BF1C4]` := 0 if it was 1, else a unit dialog runs
     `0x004B3D10(GUID)`; `0x00453AE0`; `[0x007BF20A]` := 0; mouse
     window off; handlers unregistered; R = 0.
   - In the same pass: a dead or absent local player (`0x00463DF0`), or
     a unit dialog whose unit is present in mode 12, sends C→S 0x30
     [1][GUID] (unit dialog only, then `0x004B3830`) and runs
     `0x004A0880`.
   Then a drawn timed box adds 1 to R (§8 r2). If R = 0 and
   `[0x007BF1C0]` = 1: `[0x007BF1FA]` := `[0x007BF20A]` := 0,
   `[0x007BF1BC]` := 1, `[0x007BF1C0]` := 0, handlers unregistered if
   registered. At the end of every pass `[0x007BF1BC]` := 0.
7. **Skip input** (table `0x00722600`, 7 entries {kind, message,
   handler}): right button up `0x0049DA10` (consumed only); left and
   right button down `0x004A17D0`; keys Esc and Space (kind 3)
   `0x004A1770`; WM_CHAR `0x004A1870` (a character equal to the key of
   binding 7, `0x00469AA0(7, 1)` ≤ 0xDF, does not skip); WM_SYSKEYDOWN
   `0x004A18C0` (F4 passes). A skip (`0x004A1770`): `[0x007BF1FA]` := 0
   (the text hides at once unless the Text Display option is on);
   within 100 ms of the open (`[0x007BF22C]`) the event is only
   consumed; else `0x004A08C0`: `[0x007BF1BC]` := 1; with neither a
   unit dialog nor the panel up, a shown timed box is closed; with
   one, the end callback (EDX 1) or `0x004B3D10` / `0x0049F960`, then
   `0x00453AE0` and the handlers unregistered.
8. **Close** (`0x0049F960`): mouse window off, panel freed, speech faded
   (4), handlers unregistered; if `[0x007BF230]` was set: C→S **0x31**
   [0xFFFFFFFF u32 @1][the id `[0x007BF212]`, zero-extended u32 @5],
   `[0x007BF230]` := 0.

### 8. Timed text box (`0x004A1510(id)`, 0x27 type 2 kind 3)

1. The previous box `[0x007BF1A8]` is freed. String id, wrapped to 180
   pixels in font 13 into a box (w = widest + 20, h = 15 · lines + 6);
   +0x16 := 1; expiry `[0x007BF1AC]` := `GetTickCount() + (L + 25) ·
   200` ms, L = the text length in units; `[0x007BF1C4]` :=
   `[0x007BF1C8]` := 1; x `[0x007BF1D0]` := 320 − w / 2, y
   `[0x007BF1D4]` := 100 − h / 4 (fixed, not scaled by W or H).
2. Text pass (§5 r1): while `[0x007BF1C4]` is set (`[0x007BF1C8]` = 0 is
   fatal 0xD02): before the expiry the box is drawn as a bubble (§5 r6)
   at (x, y); after it the box is freed (`0x0049E300`) and
   `[0x007BF1C4]` := `[0x007BF20A]` := 0.

### 9. Hire offers and the hire popup (0x4E, 0x4F, 0x50 code 2)

1. The 10-entry hire table `0x007C0C85` (`client/msg-ui.md` §6) is the
   source of the hire list rows (`ui/menus.md` §3.3, §3.5).
2. **Hire popup** (0x50 code 2 → `0x004B3340` → `0x004939B0`; also the
   transaction result 5, `ui/panels-2.md` §14.10). The hire-table entry
   `0x004B3340` finds is left in EAX and never read. Steps:
   `[0x007BEECC]` := 0; `SetUIState(0x23, on, 0)` (the hireling icon,
   expansion only); registry `PopupHireling` read (missing = 0); 0 →
   `[0x007BEEE4]` := 1.
3. The portrait pass `0x00494020` (UI pass `[0x13]`, not in open modes
   2 / 3, not with state 9 or 0x0B open, not with `[0x007BEECC]` = 2):
   with `[0x007BEEE4]` set and state 0x24 closed, `SetUIState(0x24, on,
   0)` (the hireling inventory); if that succeeds, `PopupHireling` := 1
   and `[0x007BEEE4]` := 0. So the hireling panel opens by itself once
   per install, after the first hire.

### 10. Other 0x50 codes (UI effects)

| Code | UI effect |
|---|---|
| 1 | quest-log values `[0x007BF2A4]`, `[0x007BF2A8]`, `[0x007BF2AC]` (quest-log draw; `client/msg-ui.md` OQ3) |
| 3 | `[0x007C0D39]`, `[0x007C0D3D]`: no instruction in `Game.exe` reads either (all.asm scan: the two writes in `0x004B25C0`): nothing shown |
| 4 | Inifuss scroll stones (r1) |
| 23 | none in the UI (client loop, `client/msg-ui.md` OQ8) |
| 36 | `[0x007C025F]` read by `0x004BDE40` (an entry of the pointer table `0x00727834`, client unit spawns of class 0x95); `[0x007C0265]` never read: not a UI draw (§Open questions 5) |

1. **Code 4** writes the 12 bytes `[0x007BF098]` and clears
   `[0x007BF254]`. The Inifuss scroll (state 0x10, draw `0x0049FF10`,
   `[0x007BF228]` = the scroll item): code `bks ` → `0x0049FBA0(0)`
   (no stones); `bkd ` → `0x0049FBA0(1)` only while `[0x007BF254]` = 0,
   else the plain scroll `0x0049FA10`; `tr1 ` → `0x0049FA10`. Opening
   `bkd ` (`0x0049FF90`) sets `[0x007BF254]` := 1 and sends C→S
   **0x3E** with the item GUID (`0x00478680`); so the stones appear
   when code 4 answers.
2. `0x0049FBA0(a)` first draws the plain scroll (`0x0049FA10`) and
   loads `UI\menu\scroin2` / `UI\menu\scroin3` once (`[0x007BF270]`,
   `[0x007BF26C]`); with a = 0 nothing more. Stones (a = 1, font 5
   during the call): an animation counter `[0x007BF247]` steps once per
   > 50 ms of `GetTickCount` (`[0x007BF243]`; reset to 0 when that time
   is 0); stone i (0–4) with symbol s = u16 `[0x007BF098 + 2i]` (s ≥ 6 →
   skipped, s = 5 fatal 0x1673) starts when the counter passes
   `0x00722F08`[i] = (0, 12, 24, 36, 48): f = counter − start; draw mode
   m = 0 for f < 5, 1 for f < 10, 2 for f < 15, else 5 (f = 1 also
   requests UI sound 2671 `shrine_portal`; blend modes:
   `render/blend-modes.md` §1):
   `scroin2` frame **i** at (`0x00722EB8`[2s] + sx, `0x00722EB8`[2s +
   1] − sy) — for s = 0…4: (242, 104), (255, 222), (148, 310), (47,
   222), (75, 104) — light 0xFF, mode m; then `scroin3` frame (f if f <
   21, else 0) at (`0x00722EE0`[2s] + sx, `0x00722EE0`[2s + 1] − sy) —
   (303, 161), (322, 242), (254, 290), (190, 238), (211, 162) — mode 3.

### 11. Item-socket dialog (UI state 0x0E, 0x58 codes)

1. **State**: mode `[0x007C5470]` (0 = object, from 0x58 code 0; 1 =
   NPC service: Charsi 154 imbue, Larzuk 511 sockets, Anya 512
   personalize); step `[0x007C5474]` (0 closed, 1 open and empty, 2 item
   placed, 3 waiting); object GUID `[0x007C547C]`; placed item GUID
   `[0x007C5480]`; note box `[0x007C5484]`; last click
   `[0x007C5488]`; backgrounds `menu\upgrade` (mode 1, `[0x007C5494]`)
   and `menu\horadricback` (mode 0, `[0x007C5498]`), loaded at game
   start (`0x004BF9D0`) and freed at exit (`0x004C0100`). Two buttons
   (`0x007278FC` = 2; 40-byte records `0x00727900`: caption u16 +0, x
   +2, y +6, hit left +0x12, right +0x16, top +0x1A, bottom +0x1E (all
   inclusive), base frame +0x22, pressed +0x26, armed +0x27): button 0
   caption 4017 "imbue", at (122, 256), hit 122–154 × 224–256, frame 16;
   button 1 caption 4143 "close", at (177, 256), hit 177–209 × 224–256,
   frame 10; frame += 1 while pressed; cel `panel\buysellbtn`
   (`0x00454600`).
2. **Open** (code 0 → `0x004C0380`): `0x00456300(0, 0)`; step := 1;
   last click := now; the 8 handlers of `0x00727950` registered (r6);
   `0x00487B80`; `SetUIState(0x0E, on, 0)`. Refused: mode 0 → C→S
   **0x44** [local player GUID or −1][object GUID][0][2] (17 bytes,
   `0x00478700`) and close; mode 1 → end the NPC interaction
   (`0x004B3FE0`) and close. Accepted: `0x0044DA40`, mouse window
   (107, 91, 222, 278).
3. **Close** (`0x004C0090`, codes 1, 5, 6, 7): step := 0;
   `0x004BFA70(cursor item)`; handlers unregistered; note box freed;
   `SetUIState(0x0E, off, 0)`; mouse window off. Codes 6 and 7 then end
   the NPC interaction (`0x004B3FB0`: menu state 0, `0x00487990`,
   `0x004B3C20` → C→S 0x30, `SetUIState(8, off, 0)`). Code 4 (server
   refused, item kept): step := 2 and the note box freed. Code 5 (item
   consumed): the model part of `client/msg-ui.md` §8, UI sound 5 (arg ≠
   0) or 3, close.
4. **Draw** (`0x004C01E0`, UI pass `[0x0E]`; step 0 is fatal 0x31D): a
   dead or absent local player closes the dialog. Else font 1: the
   background of the mode, frame 0, at (105, 270), light 0xFF, mode 5;
   both buttons; hover (mouse x < 320 and y < H − 48, inside a hit
   rectangle): caption = 3401 "ok" for button 0 in mode 0, 22748 "add
   sockets" / 22749 "personalize" for button 0 at NPC 511 / 512, else
   the record caption; box `0x0046EFD0(X − w/2 + 10, Y − h − 42, w + 12,
   h + 6, 0, 2)`, text `DrawText(caption, X − w/2 + 16, Y − 38, 0, 0)`
   (X, Y = +2, +6; w = width A, h = font height). Mode 1: the
   instruction 10076 (NPC 154), 22750 (511) or 22747 (512) wrapped to
   300, line i centred at x = 160 − w/2, y = 30 + 20 i, color 4. The
   note box, if any. Step 2: the placed item (unit type 4) at x = 167 −
   w/2, y = 106 + (h < 112 ? (114 − h) / 2 : 0), (w, h) = its cel size
   (`0x004DBEA0`), drawn with `0x0046EE80`.
5. **Place / take**: left button down (`0x004BFC50`) is ignored within
   400 ms of the last accepted click; then (step 1 or 2) the inventory
   handler gets it first; the button press `0x004BFBC0` (sound 1); with
   a cursor item inside x 123–211, y 106–220 of an item type that
   passes `0x006280A0(item, 0x10)`: mode 1 → NPC 154 `0x0062C590`, 511
   `0x0062C770`, 512 `0x0062C6A0` must accept it; mode 0 → its code must
   be `hst ` (the Horadric staff); accepted → `0x004BFA70`: step 1 → 2,
   cursor emptied, placed := its GUID, its drop sound; step 2 → 1,
   placed := 0, the item back on the cursor, sound 1. Refused → sound 3.
6. **Buttons** (left button up `0x004C04E0` → `0x004C0450`; Esc / Space
   `0x004C0150`; mouse move `0x004BFA30`; WM_CHAR consumed):
   - a release inside an armed button's hit rectangle plays sound 1
     first; every button is then disarmed and released;
   - button 0 (`0x004BFAF0`, step 2 only): sound 2; a note box (menu box
     `ui/menus.md` §2.1 at (150, 130), p1 `0x004BF9B0`, p5 = 1, p9 = 1,
     style 0, one item 3353 "Waiting for confirmation of
     transaction...", height 15, font 1, color 0); step := 3;
     mode 0 → C→S 0x44 [player][object][placed item][3]; mode 1 → C→S
     **0x38** [0 u32 @1][NPC GUID @5][placed item GUID @9]
     (`0x004B2370`);
   - button 1 and Esc / Space (`0x004C02F0`, not in step 3): step 2 →
     the item back on the cursor; sound 1; mode 0 → 0x44 [player][object]
     [0][2] and close; mode 1 → end the interaction and close.

### 12. NPC alert (0x8A, `client/msg-ui.md` §9 r3)

Overlay 72 is `overlay.txt` row 72: `npcalert` (`patch_d2`; `npc hail`
in the base files), file `NPCSpeechBalloon`, 16 frames, drawn by the
overlay rules of `render/unit-composite.md`. The sounds are
`wussie_cheer_1` (4603), `wussie_help_me` (4607), `guard_halt` (3983).

### 13. NPC intro table `0x00726850` (0x91)

1. 46 entries (`[0x0072554C]`) of 0x16 bytes: NPC class u32 +0x00, act
   u8 +0x04, text records pointer +0x05 (15-byte records, §6 r5), count
   u32 +0x09, gossip index u32 +0x0D, gossip heard u8 +0x11, **return
   greeting due u8 +0x12**, u8 +0x13, no-introduction u8 +0x14, greeting
   due u8 +0x15. Static values: +0x15 = 1 in all; +0x14 = 1 for 146,
   175, 176, 210, 244, 265; +0x13 = 1 for 155, 210, 367, 521.
2. Writers of +0x12: 0x91 (`0x004B3510`) := 1; game start / exit reset
   (`0x004B32F0`, from `0x00453DE0`) clears +0x11 and +0x12 of all; the
   NPC menu open (r3) clears it.
3. **Menu open** (`0x004B66B0(arg 0)`, first open of a talk, after the
   menu is built): the first entry whose class = the NPC's class (none →
   no greeting): if +0x12 = 1: +0x12 := 0 and the greeting mode := 2
   ("return"); then only if +0x15 = 1: +0x15 := 0, the greeting
   (`0x004E0590(NPC, mode)`, `audio/triggers.md` §10 r1) is requested
   (skill voices detached first), handle `[0x007C0DB8]`; with mode 2,
   C→S **0x4D** [class u16] (`0x004785B0`, `sim/client-messages.tsv`:
   clears the player's intro bit) and `[0x007C0DB4]` := 1. With +0x12
   set but +0x15 clear, the flag is cleared and nothing plays or is
   sent.
4. +0x15 is re-armed for all entries when the local player leaves town
   (`0x004B3E10`, from the room-change handler `0x00460E70` when the town
   flag `[0x007A5260]` goes from 1 to 0); entries with +0x11 = 1 then
   get +0x11 := 0 and a new gossip index; an active interaction with a
   present NPC ends (menu state 0, `0x00487990`, `0x004B3C20`,
   `SetUIState(8, off, 0)`, active := 0). The re-roll with no local
   player is fatal 0xFAB.

### 14. Interact NPC `[0x007C0D25]` / `[0x007C0D29]` (answers `client/msg-ui.md` OQ7, writer part)

All writers are UI code (datarefs scan, 12 writes):

| Writer | Writes | When |
|---|---|---|
| `0x004B1640(unit EAX)` | active := 1, GUID := unit +0x0C, class `[0x007C0D2D]` := unit +4; no unit → active := 0 | from `0x004B4FD0` and the 0x28 handler `0x004B6DD0` (×2) |
| `0x004B66B0` (NPC menu open) | active := 1, GUID, class (`0x004B6778`–`0x004B678E`); active := 0 when state 8 cannot open (`0x004B68A4`, after C→S 0x30) | menu open |
| `0x004B6DD0` (0x28) | active := 1, GUID, class (`0x004B6F3A`, `0x004B7066`) | 0x28 NPC path |
| `0x004B3C20` (interaction end) | active := 0 (`0x004B3CD0`) | every end path |
| `0x004B3E10` | active := 0 (`0x004B3E9E`) | leaving town (§13 r4) |

So the interact NPC is UI state; the model half of `client/msg-ui.md`
§7 code 3 and §9 must look it up at delivery (cross-file request).
`0x004B3C20` also writes the model: the NPC's unit flag +0xC4 |= 2 and
the local player's data fields +0x150…+0x15C := 0 (cross-file request).

## Constants & data dependencies

- Strings (English, `string.tbl` < 10000, `patchstring.tbl` 10000–19999,
  `expansionstring.tbl` ≥ 20000): 127, 396, 2231, 3381, 3392, 3395,
  3399, 3400, 3401, 3654, 3658, 3724, 3994, 4017, 4048, 4143, 10076,
  20002, 20131, 20169, 22747–22750; ranges 1069–1295, 10916,
  21894–22037.
- Fonts: 1, 4, 5, 8, 13 (`ui/text-fonts.tsv`).
- Tables in `Game.exe`: `0x00722654` (8 i32), `0x00722678` (527 × 4
  bytes), `0x00722EB8` / `0x00722EE0` (5 × 2 i32), `0x00722F08` (5
  u32), `0x00722600` (7 × 12), `0x00726850` (46 × 22), `0x00727900`
  (2 × 40), `0x00727950` (8 × 12).
- Files: `menu\recipescroll`, `menu\boxpieces`, `menu\upgrade`,
  `menu\horadricback`, `menu\scroin2`, `menu\scroin3`,
  `panel\buysellbtn`, `NPCSpeechBalloon`.
- Registry values `Text Display Beta`, `PopupHireling` (key `Diablo
  II`).

## Randomness

The gossip index (§6 r5) draws on the **local player's unit seed** in
the client (not a game seed); no other rule draws.

## Edge cases & original bugs

Reproduced by default.

- Screen messages, the timed box, the dialog scroll and the Inifuss
  animation run on the wall clock (`GetTickCount` / `timeGetTime`), not
  on game ticks; d2rs feeds them from its frame clock and compares them
  in milliseconds (CLAUDE.md rule 10 timing: a recording records both).
- A refused bubble placement leaves font 13 current (§5 r4).
- Placement candidates are 400 × 280 and only diagonal (§5 r5).
- A bubble of more than 10 lines draws its first 10 lines at the
  positions of the last 10 (§5 r6).
- The timed box is centred on x = 320 at every resolution (§8 r1).
- A second dialog open keeps the old text (§7 r1).
- 0x27 with a type other than 1 or 2 frees `[0x007BF250]` without a null
  test (`0x004A170D`): with no list this reads address 8 (a crash in
  1.14d); d2rs treats it as "no list".
- 0x50 code 2 passes a hire-table entry the popup never reads (§9 r2).
- The per-frame bubble box field +0x16 is read before it is written
  (§Open questions 2).

## Test vectors

Synthetic (rules as cited).

| Input | Expected | Source |
|---|---|---|
| 0x26 type 2, name "Bob", text "hi" | screen line "Bob whispers: hi", color 2 | §3 |
| 0x26 type 6, name "Bob", text "ÿc1hi" | "You whispered to Bob: hi", color 2 | §3 r2 |
| 0x26 type 1, u8@3 = 2, name "Bob", text "hi", u8@8 = 0 | "ÿc4Bobÿc0: hi", color 0 | §3 |
| 0x26 type 1, u8@3 = 1, text "hi", u8@8 = 4 | "hi", color 4 | §3 |
| 0x26 type 2, empty name, text of 306 bytes | nothing | §3 r1 |
| 13 one-line messages, then a 14th that wraps to 13 lines (6 kept) | 19 lines > 18 → the oldest record removed: 13 records, 18 lines | §2 r1, r3 |
| 800 × 600, open mode 2, one message | x = 415, re-wrapped to 300 | §2 r4 |
| monster bubble, one 80-px line, (px, py) = (400, 310) before the −10, mode 0, none placed | box w 100, h 21 at (350, 232); frame (350, 236, 100, 16); text at (360, 250) | §5 r3–r6 |
| two bubbles with equal rectangles (40, 40, 140, 61) at 800 × 600 | second moved to (90, 75) (dx 50, dy 35, m = 1; 490 ≤ 800, 355 ≤ 600) | §5 r5 |
| overhead record from 0x27 str 3983, counter 1000 | end 1000 + 8 · 4 + 125 = 1157 (n = 4 chars "3983") | `client/msg-ui.md` §4 r4 |
| dialog text "67\nA\nB\nC\nD\nE" | 5 lines, speed 67; finished when (p >> 10) > 184 | §7 r2, r4 |
| timed box text of 20 units | shown 9000 ms | §8 r1 |
| 0x91 slot 148 (Akara), then the NPC menu opens, +0x15 = 1 | +0x12 cleared, greeting mode 2 (`akara_greeting_return`), C→S `4d 94 00` | §13 r3 |
| same, +0x15 = 0 | +0x12 cleared, no sound, no 0x4D | §13 r3 |
| 0x58 code 0 for object GUID 7, state 0x0E refused, player GUID 1 | C→S 0x44 `44 01 00 00 00 07 00 00 00 00 00 00 00 02 00 00 00` | §11 r2 |
| item dialog step 2, mode 1, NPC GUID 5, item GUID 9, button 0 released | C→S `38 00 00 00 00 05 00 00 00 09 00 00 00`, step 3 | §11 r6 |

## Provenance

1.14d `Game.exe` (exports, `tools/ghidra/disasm.py`): chat `0x0049F490`,
`0x004521C0`, `0x00452300`, `0x0049E3A0`, `0x0049DBC0`, `0x0049DC40`,
`0x0049E280`; recipe scroll `0x0048BBE0`, `0x0048BC10`; text pass
`0x004A0E70`, bubbles `0x004A0B30`, `0x004A0D20`, `0x004A0A00`,
`0x0049E070`, `0x0049D3C0`, `0x0049D9A0`, `0x0049D8E0`; NPC text list
`0x004A1600`, `0x00661240`, `0x00661510`, `0x006616E0`, `0x006615F0`
(comparator `0x006615D0`), `0x006612A0`, `0x00661390`, `0x006613C0`,
`0x0049F910`, `0x004B5890`, `0x004B5BC0`, `0x004B40D0`, `0x004B1680`,
`0x004B17A0`, `0x004B1D70`; dialog `0x004A1320`, `0x004A10E0`,
`0x004A05E0`, `0x004A0320`, `0x0049D5A0`, `0x004A0770`, `0x004A08C0`,
`0x004A1770`, `0x004A17D0`, `0x004A1870`, `0x004A18C0`, `0x0049F960`,
`0x0049D440`, `0x0049D570`, `0x0047CEF0`; timed box `0x004A1510`; hire
`0x004B3340`, `0x004939B0`, `0x00494020`; 0x50 `0x004B9210`,
`0x004B25C0`, `0x0049FF60`, `0x0049FF10`, `0x0049FF90`, `0x0049FBA0`;
item dialog `0x004C0550`, `0x004C0380`, `0x004C0090`, `0x004C01E0`,
`0x004BFDD0`, `0x004BFC50`, `0x004BFBC0`, `0x004BFA70`, `0x004BFAF0`,
`0x004C0450`, `0x004C02F0`, `0x004C0150`, `0x004BF9D0`, `0x004C0100`,
`0x004B3FB0`, `0x004B3FE0`, `0x004B2370`; intros `0x004B3510`,
`0x004B32F0`, `0x004B66B0`, `0x004B3E10`, `0x00460E70`; interact NPC
`0x004B1640`, `0x004B6DD0`, `0x004B3C20`. Imports `GetTickCount`
`0x006CC260`, `timeGetTime` `0x006CC544`, `SetRect` `0x006CC498`,
`IntersectRect` `0x006CC4C4`. Tables and strings read from the image,
`.tbl` strings and `overlay.txt` / `sounds.txt` rows with Python scripts
outside the repo. No capture yet.

## Open questions

1. **Needs recording.** Pixel and timing proof of §2–§8: chat lines at
   800 × 600 (whisper, echo, broadcast), two monsters speaking at once
   (bubble move), an NPC dialog panel scrolling (frame times), the timed
   box (recording list).
2. The per-frame bubble box (`0x004A0A00`, 0x2C bytes from
   `0x0040B380`) is not cleared, and `0x0049D9A0` tests its +0x16 for 3;
   whether the pool can hand back a block holding 3 there (bubble
   skipped) needs the allocator (`0x0040A080`) read. d2rs uses 0 (drawn).
3. **Answered** (2026-10-07): binding 7 is the automap key (the mini
   panel's Automap button shows the keys of binding 7,
   `0x0047F4E0`; `ui/control-panel.md` §9 r5), so the automap key
   typed as a character does not skip a dialog (§7 r7). Was: Binding 7 of `0x00469AA0` (the key that does not skip a dialog):
   which command it is (a controls spec).
4. The 15-byte gossip text records (+5 of an intro-table entry): field
   meanings beyond text id +0, flag +2, quest +3 / +7, class +0x0B
   (`0x004B1680`, `0x004B41C0`, `0x004B41E0`): Ghidra read of the two
   handlers.
5. 0x50 code 36 (`0x004BDE40`, pointer table `0x00727834`, client units
   of class 0x95): owner a client effects spec.
6. The inventory handlers that see a socket-dialog click first
   (`0x004922A0`, `0x00489190`, `0x004890B0`, `0x00489060`) and the item
   checks `0x006280A0`, `0x0062C590`, `0x0062C6A0`, `0x0062C770`:
   the inventory and items specs.
