# Spec: UI — Options menus (Esc game menu tree, settings, Configure Controls)

- **Status:** draft (2026-10-08, RE on the 1.14d `Game.exe` and the 1.14d MPQ art; no capture yet).
  Points that need a capture are PROVISIONAL with a REC id (METHODS M22).
- **Target version:** 1.14d, English install, expansion installed (classic tables named only)
- **Crate/module:** `d2-client::ui::front_end::options`
- **Related specs:** `ui/frontend-menus.md` (main menu, §F1.4), `ui/controls.md` (§1 binding table, §2 key
  files, §3 commands, §3.3 key-config menu tables, §4.1 key dispatch, §5 assignment), `ui/key-commands.tsv`
  (the command list), `ui/control-panel.md` §9 r5 (mini-panel "Game Menu" button), `ui/panels.md` §2
  (`SetUIState`), `ui/ui-states.tsv` (ui 9 `UI_ESCMENU`, ui 11 `UI_CONFIG`), `ui/text.md` (Unicode text
  draw), `ui/messages.md` (`menu\boxpieces` border `0x00452E50`, "Text Display Beta"),
  `render/sprite-placement.md` §2 (DC6 placement), `render/blend-modes.md` §2 (rectangle draw modes),
  `render/lighting.md` §5 (light quality), `ui/automap.md` §8 (automap options),
  `audio/sound-table.md` §9 and `audio/sound-table-2.md` §15 (audio settings and slider math, sound
  deferred), `audio/triggers.md` (NPC speech), `client/ui.md` §A5 (one logical size), §A6 (controls file),
  `client/msg-ui.md` (exit code 23 row: `0x0044B880`, `0x0044C860`).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 42–53 |
| Inputs | 54–66 |
| Outputs / state changes | 67–76 |
| Rules | 77–78 |
|   O1. Where options live; opening and closing the game menu | 79–111 |
|   O2. Menu records and the tree | 112–168 |
|   O3. Save and Exit Game (`0x0047F2D0`) | 169–174 |
|   O4. Draw (`0x0047E3D0`, while ui 9 is open, from the UI draw `0x00456F46`) | 175–222 |
|   O5. Input (handler table `0x006D6030`, 7 entries, registered while ui 9 is open) | 223–262 |
|   O6. Row effects (apply = +0x114, init = +0x118; registry writes are REG_DWORD) | 263–307 |
|   O7. Settings storage and the d2rs config mapping | 308–339 |
|   O8. d2rs stubs (rows drawn and navigated like the original, value kept in `settings.toml`, no effect) | 340–353 |
|   O9. Configure Controls (ui 11, `UI_CONFIG`) | 354–439 |
| Constants & data dependencies | 440–459 |
| Randomness | 460–463 |
| Edge cases & original bugs | 464–488 |
| Test vectors | 489–514 |
| Provenance | 515–539 |
| Open questions | 540–546 |
<!-- /index -->

## Summary

1.14d has no Options entry on the front-end main menu; every option is reached in game through the Esc
game menu (ui 9): Options → Sound Options / Video Options / Automap Options / Configure Controls /
Previous. The menu rows are pre-rendered DC6 label images (no string-table text), drawn centred or
label-left / value-right, with a spinning pentagram on each side of the selected row and skull sliders.
Each setting row has an apply callback that stores the value (registry `HKCU\Software\Blizzard
Entertainment\Diablo II`) and an init callback that re-reads and applies it at every game join.
Configure Controls opens a separate screen (ui 11) that edits the 114-entry binding table of
`ui/controls.md` with two keys per command. This spec owns the menu engine, the tree, layout, input,
the settings mapping and the d2rs stubs; the effects of each setting are owned by the specs named.

## Inputs

| Input | Source |
|---|---|
| Menu record tables (`0x00713044` … `0x0071F728`) | the 1.14d image (§O2) |
| Label / value art `data\local\ui\<lang>\*.dc6`, widgets `data\global\ui\WIDGETS\OptBar`, `OptBarC`, `OptSkull`, `data\global\ui\CURSOR\pentspin`, `data\global\ui\MENU\textslid`, `menu\boxpieces` | the user's MPQs (`client/assets.md`) |
| Stored settings | registry `HKCU\Software\Blizzard Entertainment\Diablo II` (HKLM fallback on read), §O7; d2rs: `settings.toml` |
| Binding table, `.key` files | `ui/controls.md` §1–§2 |
| Pointer, keys | handler tables §O5 / §O9 (Win32 messages) |
| Expansion installed | `0x00408F20` (selects the exp / classic tables) |
| Sound device up, renderer capabilities | `0x004DF880`, `0x004DF940`, `0x004DF960`, `0x004F5170`, `0x004F5260` (enable tests, §O6) |
| Wall-clock time | pentagram spin, key-config scroll repeat and message timeout (UI only, not game logic) |

## Outputs / state changes

| Output | Where |
|---|---|
| ui 9 / ui 11 open and close, saved UI states restored | `ui/panels.md` §2, §O1 |
| Setting values written (registry; d2rs `settings.toml`) and applied | §O6, §O7 |
| Binding table edits, `.key` + `default.key` written on Accept | §O9, `ui/controls.md` §2.4 |
| Game exit started (Save and Exit Game) | §O3, `client/msg-ui.md` |
| UI sound requests `cursor_pass` (1), `cursor_select` (2) (sound deferred: named only) | §O5, §O9 |

## Rules

### O1. Where options live; opening and closing the game menu

1. **No Options on the main menu.** The 1.14d front end has no Options, Video or Sound entry:
   the main-menu descriptors are exactly those of `ui/frontend-menus.md` §F1.4 r3, no front-end
   descriptor or callback reaches the options code (the only callers of the open function
   `0x0047E090` are the Esc command `0x004690ED`, the mini-panel button `0x0047ED5B` and the
   key-config screen `0x004A505B` / `0x004A5CBB`), and `string.tbl` holds no Options label for it
   (5117 "TCP/IP Options" is the multiplayer TCP/IP screen title). Options exist only in game.
2. **Open** `0x0047E090(save, menu)` (Esc, command 56: `ui/controls.md` §3 row 56; mini-panel
   function 7: `ui/control-panel.md` §9 r5; both pass save = 1, menu = 0):
   - save ≠ 0: for every ui i = 0 … 37 whose record in `0x00713058` (12 bytes: close, keep,
     unused) has close = 1: remember "was open" in `0x00713060 + 12·i` when keep = 1, and close
     it (`SetUIState(i, off, 0)`). Close = 1 for all ui except 0 and 9; keep = 1 for ui 6, 7,
     10, 17, 21, 35.
   - `SetUIState(9, on, 0)`; current menu := `menu`; selected row := row count − 1 (for the game
     menu: Return to Game).
3. **Close and restore** `0x0047E200(restore)`: ui 9 open → `SetUIState(9, off, 0)`; restore ≠ 0
   → `SetUIState(i, on, 0)` for every i remembered in r2. Used by Return to Game, Esc, Save and
   Exit Game and Accept (§O9).
4. **Esc anywhere in the tree closes the whole menu.** The menu's key table (§O5 r1) has no Esc
   entry; Esc reaches command 56, which with ui 9 open calls `0x0047E200(1)`. There is no "Esc =
   previous menu". PROVISIONAL: Esc in a sub-menu closes the whole menu and restores the saved
   UI states (because the menu table lacks VK 0x1B and command 56 tests only ui 9); settled by
   REC-212.
5. **Load / free.** At every game join the in-game UI set-up (`0x00456970`) runs `0x0047DD70`:
   for each table in order game, Options, Sound, Video (exp or classic), Automap (exp or
   classic), for each row (exp-only rows only when expansion is installed): load the label cel
   `DATA\LOCAL\UI\<lang>\<name>` (`<lang>` = 3-letter code of `0x00525260`, ENG), for a choice
   row also its value cels, then call the row's init callback (+0x118). Then the widgets
   `OptBar`, `OptBarC`, `OptSkull` and the widest `pentspin` frame width (52). Freed at game end
   (`0x0047DFD0`). So every stored setting is applied once per game join, before the first
   world draw.

### O2. Menu records and the tree

1. **Header** (5 × u32 at the header address): row count n, row pitch, label baseline offset,
   pentagram offset, slider offset. Game menu and Options: (n, 50, 39, 51, 0); sub-menus: (n, 45,
   34, 49, 36).
2. **Row** (0x550 bytes): +0x00 kind (−1 title, 0 action, 1 choice, 2 slider); +0x04 exp-only;
   +0x08 y_top (written by the draw, 0 = not drawn); +0x0C image name; +0x110 enabled test (null
   = always); +0x114 apply; +0x118 init; +0x11C update (called before each draw); +0x120 count;
   +0x124 value (choice index or slider position); +0x128 slider style (0 plain, 1 centre-marked);
   +0x12C four value-image names (260 bytes each); +0x53C label cel; +0x540 four value cels.
3. **Tree** (expansion tables; classic differences after the table). Images:
   `data\local\ui\eng\<image>.dc6`; "d" = d2data, "x" = d2exp; w×h = total width (frames tiled at
   256 px) × height.

| Menu (header, rows) | Row | Kind | Image (w×h, mpq) | Values (images) | Default | Enabled when | Apply → effect |
|---|---|---|---|---|---|---|---|
| Game `0x00713044`, `0x00713220` | 0 | action | `Options` (160×36, d) | — | — | always | `0x0047F2A0`: Options menu |
| | 1 | action | `Exit` (434×36, d) | — | — | always | `0x0047F2D0`: Save and Exit Game (§O3) |
| | 2 | action | `ReturnToGame` (356×36, d) | — | — | always | `0x0047F300`: `0x0047E200(1)` |
| Options `0x00714210`, `0x00714238` | 0 | action | `SoundOptions` (309×36, d) | — | — | sound device up (`0x0047CD80`) | `0x0047F310`: Sound menu |
| | 1 | action | `VideoOptions` (295×36, d) | — | — | always | `0x0047F340`: Video menu |
| | 2 | action | `AutoMapOptions` (378×36, d) | — | — | always | `0x0047F3A0`: Automap menu |
| | 3 | action | `CfgOptions` (423×36, d) | — | — | always | `0x0047F400`: Configure Controls (§O9) |
| | 4 | action | `Previous` (313×36, d) | — | — | always | `0x0047F430`: Game menu |
| Sound `0x00714224`, `0x00715CC8` | 0 | title | `SoundOptions` | — | — | never | — |
| | 1 | slider 21, style 0 | `Sound` (95×26, d) | 0–20 | 20 (vol 100) | sound device up | Master Volume = 5p |
| | 2 | slider 21, style 0 | `Music` (84×26, d) | 0–20 | 10 (vol 50) | sound device up | Music Volume = 5p |
| | 3 | choice 2 | `3DSound` (144×26, d) | `SmallOff` 51, `SmallOn` 40 | Off | device up and 3D available (`0x004DF940`) | Sound Mixer (`0x004E00D0`) |
| | 4 | choice 2 | `EAX` (362×26, d) | `SmallOff`, `SmallOn` | Off | device up and EAX available (`0x004DF960`) | Sound Mixer (`0x004E0140`) |
| | 5 | slider 21, style 1 | `3DBias` (108×26, d) | 0–20 | 10 (bias 50) | mixer mode 1 or 2 (`0x004DF980`) | Positional Bias = 5p |
| | 6 | choice 3 | `NPCSpeech` (157×26, d) | `AudioOnly` 184, `TextOnly` 169, `AudioText` 260 | AudioText | sound device up | §O6 r6 |
| | 7 | action | `SPrevious` (231×26, d) | — | — | always | `0x0047F460`: Options menu |
| Video `0x0071875C`, `0x0071ACA0` | 0 | title | `VideoOptions` | — | — | never | — |
| | 1 | choice 2, exp-only | `Resolution` (173×26, x) | `640x480` 123, `800x600` 135 | 800x600 | always | §O6 r7 |
| | 2 | choice 3 | `LightQuality` (265×26, d) | `Low` 66, `Medium` 108, `High` 57 | High | always | §O6 r8 |
| | 3 | choice 2 | `BlendShadow` (266×26, d) | `SmallOff`, `SmallOn` | On | always | §O6 r9 |
| | 4 | choice 2 | `Perspective` (173×26, d) | `SmallOff`, `SmallOn` | On (Off when not capable) | renderer has perspective (`0x004F5170`) | §O6 r10 |
| | 5 | slider 21, style 1 | `Gamma` (105×26, d) | 0–20 | 10 (gamma 155) | gamma ramp available (`0x004F5260`) | §O6 r11 |
| | 6 | slider 100, style 1 | `Contrast` (151×26, x) | 0–99 | 99 (contrast 100) | always | §O6 r12 |
| | 7 | action | `SPrevious` | — | — | always | Options menu |
| Automap `0x0071D734`, `0x0071F728` | 0 | title | `AutoMapOptions` | — | — | never | — |
| | 1 | choice 2, exp-only | `AutoMapMode` (213×26, x) | `Full` 180, `Mini` 131 | Full | always | §O6 r13 |
| | 2 | choice 4 | `AutoMapFade` (70×26, d) | `SmallNo` 40, `Center` 104, `Everything` 173, `Auto` 86 (x) | No (0) | always | `ui/automap.md` §8 (`0x004576F0`) |
| | 3 | choice 2 | `AutoMapCenter` (336×26, d) | `SmallNo`, `SmallYes` 51 | Yes | always | `0x00457750` |
| | 4 | choice 2 | `AutoMapParty` (194×26, d) | `SmallNo`, `SmallYes` | Yes | always | `0x004577C0` |
| | 5 | choice 2 | `AutoMapPartyNames` (190×26, d) | `SmallNo`, `SmallYes` | Yes | party shown (`0x004577B0`) | `0x004577F0` |
| | 6 | action | `SPrevious` | — | — | always | Options menu |

   Classic install (`0x00408F20` = 0): Video `0x00718748` / `0x00718770` (7 rows: no Resolution),
   Automap `0x0071D720` / `0x0071D748` (6 rows: no AutoMapMode). Unused label art in the MPQs
   (`SaveAndExitGame`, `KeysAndButtons`, `MouseSensitivity`, `2dsound`, `dolby`, …) is not referenced.
4. **No string ids.** No row has a `string.tbl` label; the text is in the images (per language
   folder). d2rs draws the images, never a font.
5. **Entering a menu** (every navigation callback, `0x0047F2A0` … `0x0047F460`): `SetUIState(9,
   on, 0)` (already on: no change), current menu := target, selected := target row count − 1 (the
   Previous / SPrevious row; for the Game menu, Return to Game). The drag flags stay as they are.

### O3. Save and Exit Game (`0x0047F2D0`)

1. `[0x0070EE8C]` := 0 (`0x0044B880`), post `WM_CLOSE` (0x10) to the game window, `[0x007A0674]`
   := 1 (`0x0044C860`), then `0x0047E200(1)`. The rest of the exit path is `client/msg-ui.md` (code
   23 row) and `ui/frontend-menus.md` §F1.3 (in game → exit row). Sound: `cursor_select` (§O5 r6).

### O4. Draw (`0x0047E3D0`, while ui 9 is open, from the UI draw `0x00456F46`)

W = `[0x0071146C]`, H = `[0x00711470]`, h = W / 2. Divisions truncate toward zero. Draw mode 5 =
normal, 1 = the disabled look (`render/blend-modes.md`). Cel y is the value passed to the cel draw
(`render/sprite-placement.md` §2); multi-frame images are drawn frame by frame at +256 px.

1. **Rows.** y0 = (H − 80) / 2 − (pitch × n) / 2. For each row in order (exp-only rows skipped on
   classic, they take no slot): +0x08 := 0, then for drawn rows: mode := 5 if enabled else 1;
   call the update callback if any; y_top := y0 + pitch × k (k = drawn-row index), stored at +0x08;
   yb := y_top + baseline offset.
   - kind −1 / 0: label centred: x = h − 1 − (w >> 1), y = yb.
   - kind 1: label left at x = h − 230; value cel (+0x540 + 4 × value) right-aligned: x = h + 230 −
     w, both at y = yb.
   - kind 2: label left at x = h − 230, y = yb, then the slider (r2) with base Y = y_top + slider
     offset.
2. **Slider** (`0x0047E260`; every slider row has a label, so its "no label" layout, x origin h −
   145 / art centred at h, is never used): X0 = h − 60; t = trunc(265 × p / (n − 1)) (x87,
   truncating; computed as −265 / (n − 1) × p); draw in order:
   - rectangle x [h − 59, h − 59 + t + 12) × y [Y − 30, Y), color 0, mode 1 (style 1) or 2 (style 0);
   - rectangle x [h − 59 + t + 12, h + 230) × y [Y − 30, Y), color 0, mode 1 (style 1) or 0 (style 0)
     (`render/blend-modes.md` §2: modes 0–2 with color 0 darken the background);
   - bar `OptBarC` (style 1; 2 frames 255 + 35 = 290 × 37) or `OptBar` (style 0; 290 × 33), right
     edge at h + 230 (left X0 = h − 60), y = Y, row mode;
   - skull `OptSkull` (28 × 28) at x = X0 + t, y = Y − 1 − (style = 0 ? 1 : 0), row mode.
3. **Pentagrams** (after all rows): cel `data\global\ui\CURSOR\pentspin` (8 frames, widths 51,
   43, 27, 9, 23, 40, 50, 52; height 51–53; offsets per frame), frame counter f ∈ 0…7
   (`[0x007BC944]`, 0 at load, never reset): left at x = h − 52 − 249, frame (f = 0 ? 0 : 8 − f);
   right at x = h + 249, frame f; both y = y_top(selected) + pentagram offset, mode 5. Then, when
   more than 50 ms passed since the last advance (`0x00454850`: GetTickCount − `[0x007A313C]` >
   50; its only caller), f := (f + 1) mod 8 and the stamp := now. The left one spins the other way.
4. **Positions** (y_top / label baseline / pentagram y; x: labels h − 230, values right edge h +
   230, bar h − 60 … h + 230, pentagrams h − 301 and h + 249; h = 400 at 800 × 600, 320 at 640 ×
   480):

| Menu (n) | 800 × 600: y_top of rows | 640 × 480: y_top of rows |
|---|---|---|
| Game (3) | 185, 235, 285 | 125, 175, 225 |
| Options (5) | 135, 185, 235, 285, 335 | 75, 125, 175, 225, 275 |
| Sound, Video exp (8) | 80, 125, 170, 215, 260, 305, 350, 395 | 20, 65, 110, 155, 200, 245, 290, 335 |
| Automap exp, Video classic (7) | 103, 148, 193, 238, 283, 328, 373 | 43, 88, 133, 178, 223, 268, 313 |
| Automap classic (6) | 125, 170, 215, 260, 305, 350 | 65, 110, 155, 200, 245, 290 |

   Baseline = y_top + 39 (Game, Options) or + 34 (sub-menus); pentagram y = y_top + 51 or + 49;
   slider Y = y_top + 36. Example: Game menu at 800 × 600, Return to Game selected: label at
   (400 − 1 − 178, 324) = (221, 324), pentagrams at (99, 336) and (649, 336).
   PROVISIONAL: the menu draws over the world with no backdrop of its own (because `0x0047E3D0`
   draws no rectangle or panel); settled by REC-212.

### O5. Input (handler table `0x006D6030`, 7 entries, registered while ui 9 is open)

1. Entries (kind, message, handler): left button down `0x0047D7F0`, left button up `0x0047D840`,
   VK Down (0x28) `0x0047D8A0`, Up (0x26) `0x0047D920`, Left (0x25) `0x0047D9A0`, Right (0x27)
   `0x0047DA90`, Enter (0x0D) `0x0047DB80` (registered by the ui-9 open hook in `0x00455720`,
   then `0x00467A70(0)`; removed by the close hook in `0x00455AE0`, then `0x00466FE0`). Every
   handler consumes its message. Other keys go to the
   binding dispatch (`ui/controls.md` §4.1; Esc: §O1 r4). Right button and wheel: nothing here.
2. **Row under the pointer** (`0x0047D520`, pointer y only; x is not tested): none unless (H − 80)
   / 2 − (pitch × n) / 2 < y < (H − 80) / 2 + (pitch × n) / 2; else the last row with y_top ≠ 0 and
   **y ≥ y_top + n** (n = the row count, not the pitch: original quirk); the result is dropped
   (none) when that row is a title or its enabled test fails.
3. **Hover**: each draw compares the pointer with the last one (`[0x007BC980]`, `[0x007BC984]`);
   when it moved: dragging (`[0x007BC948]`) → slider drag (r5) with the new x; else the row under
   the pointer, if any, becomes the selected row (no sound).
4. **Keys**: Down / Up: selected := next / previous row, wrapping, repeated while the row is a
   title or disabled; then `cursor_pass`. Left / Right on an enabled row: choice: value − 1 / + 1,
   wrapping (unsigned test against count − 1); slider: p − 1 / + 1, clamped to 0 … n − 1; action:
   nothing. When the value changed: apply callback, `cursor_pass`; unchanged: nothing.
   Enter: activate (r6).
5. **Mouse**: left down: the row under the pointer (if any) becomes selected, then the slider drag
   test (`0x0047D670`) with the pointer (x, y), dragging := 1. Drag (`0x0047D670`): runs when the
   drag latch `[0x007BC94C]` is set or the row under the pointer is the selected one; the selected
   row must be enabled and a slider; when the latch is clear, also h − 59 < x < h + 230 (strict).
   With x0 = h − 48: x < x0 → p := 0; x > x0 + 265 → p := n − 1; else p := trunc(trunc((x − x0) /
   f + 1.0) / 2), f = (f32)(265 / (n − 1) × 0.5) (6.625 for n = 21, ≈1.33838 for n = 100); latch :=
   1. A changed p runs the apply callback and `cursor_pass`. Left up: when dragging and the row
   under the pointer is the selected one → activate (r6); then `0x0044DA40`, dragging := 0, latch
   := 0.
6. **Activate** (`0x0047D5C0`, Enter or click release; disabled rows ignored): action → its apply
   callback, then `cursor_select`; choice → value + 1 wrapping to 0, apply, `cursor_pass`; slider →
   nothing.
7. **Value ↔ position** (sliders; x87 truncating control word 0xC00): position from value
   `0x0047CC90`: p = trunc((v − min + 1) × (n − 1) / (max − min)); value from position
   `0x0047CD00`: v = trunc(min + (max − min) / (n − 1) × p). Ranges: Sound, Music, 3D Bias 0–100, n
   21 (v = 5p; `audio/sound-table-2.md` §15); Gamma 55–255, n 21 (v = 55 + 10p); Contrast 0–100, n
   100. PROVISIONAL: the divisions and products run at 53-bit (double) precision, so Contrast p 99
   → 100 and v 100 → p 99 (because MSVC's default x87 precision is 53-bit unless a renderer
   changed it); settled by REC-213.

### O6. Row effects (apply = +0x114, init = +0x118; registry writes are REG_DWORD)

1. **Sound / Music / 3D Bias** (`0x0047CDA0`/`0x0047CDC0`, `0x0047CDF0`/`0x0047CE10`,
   `0x0047CE70`/`0x0047CE90`): apply: v := value of p, setter `0x00514CD0` / `0x00514D00` /
   `0x00514D30` stores it and writes `Master Volume` / `Music Volume` / `Positional Bias`. Init: p
   from the live value (read at start-up by `0x00514B60`: defaults 100 / 50 / 50, values > 100
   ignored). Owner of the effect: `audio/sound-table.md` §8–§9.
2. **3D Sound / EAX** (`0x004E00D0`, `0x004E0140`; update `0x0047CE50` / `0x0047CED0`): value = mixer
   mode ∈ {1, 2} (3D Sound) or = 2 (EAX), re-read every draw; apply re-sets the mixer (`Sound
   Mixer`, `audio/sound-table.md` §9).
3. **Action rows**: §O2 r5, §O3, §O9.
4. **Choice init generally**: the registry value is read over the static default (+0x124 of the
   record), so a missing value keeps the default.
5. **Automap rows** (`ui/automap.md` §8): Fade apply `0x0047D3F0` (value > 4 → 0, `0x004576F0`,
   registry `AutoMapFade`), init `0x0047D430` (read; > 4 → set 0; value := live); Center
   `0x00457750` / `0x00457740`; Party `0x004577C0` / `0x004577B0`; Names `0x004577F0`, init value =
   party ∧ names; the update callbacks re-read the live values every draw, so F10–F12 changes show
   at once.
6. **NPC Speech** (`0x0047CEF0`, init `0x0047CF50`): stores `NPC Speech` (`0x00514D60`), then:
   AudioOnly (0) → speech on (`0x004E0690(1)`), text off (`0x0049D570(0)`); TextOnly (1) → speech
   off, text on; AudioText (2) → both on. Init: value := stored (default 2) and, only when the sound
   device is up, the same apply. Text flag: `ui/messages.md` ("Text Display Beta"); speech:
   `audio/triggers.md`.
7. **Resolution** (exp, `0x0047D0A0`, init `0x0047D0E0`): registry `Resolution` (0 = 640 × 480, 1
   = 800 × 600); apply and init call `0x0044BA20(value ? 2 : 0)` (resolution mode). Missing value
   → 1.
8. **Light Quality** (`0x0047CF80`, init `0x0047CFE0`): registry `Light Quality` (default 2); Low →
   low-quality 1, missile lights 0; Medium → 0, 0; High → 0, 1 (`0x004F5220`, `0x004CCF20`;
   `render/lighting.md` §5; this answers its open question 10: read at every game join, §O1 r5).
9. **Blended Shadows** (`0x0047D040` / `0x0047D070`): registry `Blended Shadows` (default 1) →
   `0x004F5200` (`render/blend-modes.md`).
10. **Perspective** (`0x0047D130`, init `0x0047D180`): registry `Perspective` (static default 1) →
    `0x004F51E0`, then the view is recomputed (`0x00476000`, `render/camera.md`). Init: read; when
    the renderer has no perspective (`[0x0072DA4C]` = 0: video mode < 4) value := 0 and nothing is
    applied or written.
11. **Gamma** (`0x0047D200`, init `0x0047D230`): registry `Gamma` (default 155), v = 55 + 10p,
    applied with `0x004776C0(v)`. Init reads, sets p, and when gamma is available re-applies and
    writes back the value of p (a stored 160 gives p 10 and is written back as 155: the value is
    snapped to the 10-step grid).
12. **Contrast** (`0x0047D2B0`, init `0x0047D2F0`): registry `Contrast`, default 100 (0 when video
    mode `0x004F5140` = 4); apply writes v but passes **p** to the renderer option 11
    (`0x004F52B0(11, p)`), then `0x004F5230(0)`. Init reads, sets p, writes v back and applies.
13. **Automap Mode** (exp, `0x0047D370`, init `0x0047D3B0`): registry `AutoMapMode` (0 full, 1 mini;
    value > 2 → 0), `0x0045A720` (`ui/automap.md` §8 r3); init value := live mode.

### O7. Settings storage and the d2rs config mapping

1. 1.14d: `HKCU\Software\Blizzard Entertainment\Diablo II\<value>` (`0x00414F10` read: HKCU then
   HKLM, REG_DWORD or REG_SZ parsed with `strtoul`; absent → caller's default kept;
   `0x004150E0` write: REG_DWORD to HKCU). Controls: `.key` files (`ui/controls.md` §2).
2. d2rs (ours, M20): `<config_dir>/d2rs/settings.toml`, `version = 1`, strict like
   `client/ui.md` §A6; unknown keys and out-of-range values are errors. Values keep the 1.14d
   integers so a registry import is a copy. Mapping (d2rs-own):

| Row | 1.14d registry value | Range, default | d2rs `[section] key` |
|---|---|---|---|
| Sound | `Master Volume` | 0–100, 100 | `[audio] master_volume` |
| Music | `Music Volume` | 0–100, 50 | `[audio] music_volume` |
| 3D Sound, EAX | `Sound Mixer` | 0–2, 0 | `[audio] mixer` |
| 3D Bias | `Positional Bias` | 0–100, 50 | `[audio] positional_bias` |
| NPC Speech | `NPC Speech` | 0–2, 2 | `[audio] npc_speech` |
| Resolution | `Resolution` | 0–1, 1 | `[video] resolution` |
| Light Quality | `Light Quality` | 0–2, 2 | `[video] light_quality` |
| Blended Shadows | `Blended Shadows` | 0–1, 1 | `[video] blended_shadows` |
| Perspective | `Perspective` | 0–1, 1 | `[video] perspective` |
| Gamma | `Gamma` | 55–255, 155 | `[video] gamma` |
| Contrast | `Contrast` | 0–100, 100 | `[video] contrast` |
| Automap Mode | `AutoMapMode` | 0–1, 0 | `[automap] mode` |
| Automap Fade | `AutoMapFade` | 0–3, 0 | `[automap] fade` |
| Automap Center | `AutoMap Centers` | 0–1, 1 | `[automap] centers` |
| Automap Party | `AutoMap Party` | 0–1, 1 | `[automap] party` |
| Automap Party Names | `AutoMap Party Names` | 0–1, 1 | `[automap] party_names` |
| Configure Controls | `<save>\<char>.key`, `<save>\default.key` | — | `controls.toml` (`client/ui.md` §A6) |

3. Writes happen at the same moments as the original's registry writes (each apply; Gamma /
   Contrast init write-back), so a crash right after a change keeps it.

### O8. d2rs stubs (rows drawn and navigated like the original, value kept in `settings.toml`, no effect)

| Row | Why | Stub behaviour |
|---|---|---|
| Sound, Music | sound deferred (`client/audio.md`) | enabled (the sound device counts as up); apply stores the value; nothing audible |
| Sound Options (Options row 0), NPC Speech | sound deferred | enabled; NPC Speech applies its **text** half (§O6 r6) for real, the speech half is stored only |
| 3D Sound, EAX, 3D Bias | DirectSound3D / EAX hardware paths; d2rs reproduces mixer mode 0 only (`audio/sound-table.md` §9) | disabled (mode 1, skipped by Up / Down, hover and clicks); values shown from `mixer` (Off) |
| Resolution | one logical size 800 × 600 (`client/ui.md` §A5) | enabled; value stored; the frame stays 800 × 600 |
| Perspective | 3D renderer path (video mode ≥ 4) | disabled, shown Off (the original's look on a 2D renderer); `perspective` untouched |
| Gamma, Contrast | hardware gamma ramp / Glide contrast | enabled; values stored (Gamma snaps to the 10-step grid on game join as in §O6 r11); no effect on pixels |

Implemented for real: Light Quality, Blended Shadows, all Automap rows, the navigation rows, Save
and Exit Game, Configure Controls.

### O9. Configure Controls (ui 11, `UI_CONFIG`)

1. **Open** (`0x0047F400`): ui 9 open → `SetUIState(9, off, 0)` (the remembered UI states of §O1
   r2 are kept); `SetUIState(11, on, 0)`. The ui-11 open hook (in `0x00455720`): register handler
   tables `0x006D6024` (1 entry) and `0x006D60C0` (13 entries), then `0x004A5200(1)`; the close
   hook (in `0x00455AE0`, `0x00455C73`) unregisters them and calls `0x004A5200(0)` (when not
   editing: key mode back, `0x0044DD20`, `0x00466FE0`). `0x004A5200(1)` does: table choice
   (`ui/controls.md` §3.3: exp 62 rows, classic 51), and when not editing: snapshot the binding
   table (`0x00469D20`), key mode 0 (`ui/controls.md` §4.1 r5), `0x0044DCE0`, `0x00467A70`; top
   row := 0, selected row := 0; the column (`[0x007246D4]`, initial 1) and the latch
   (`[0x00724728]`, initial 1) keep their values across opens.
2. **Layout** (font 13 FontInGameChat; M = (W − 620) / 2, T = (H − 40 − 420) / 2, C = (620 − 49) /
   3 = 190; at 800 × 600 M = 90, T = 70; at 640 × 480 M = 10, T = 10):
   - box: rectangle (M, T) … (W − M, T + 369), color 0, mode 1; `menu\boxpieces` border
     (`0x00452E50`) around (M, T + 3, W − M − 3, T + 368) and around (M, T + 323, W − M − 3, T +
     368) (the button strip);
   - headings at y = T + 30, color 4: 3921 `CfgFunction` "Function" x = M + 18; 3922
     `CfgPrimaryKey` "Key/Button One" x = M + 18 + C; 3923 `CfgSecondaryKey` "Key/Button Two" x = M
     + 18 + 2C;
   - 15 visible rows r = 0…14 (list row top + r): text y = T + 52 + 18r; separator rows (command
     57) are blank; label (`ui/controls.md` §3.3 string id) at x = M + 30, color 0 (selected and
     editing: 3); slot 1 key name at x = M + 30 + C, slot 0 key name at x = M + 30 + 2C (names
     `0x00469DE0`; unbound 3762 "None");
   - selected row: rectangle (M + 18, T + 35 + 18r) w 3C = 570, h 18, color 0, mode 1, under the
     text;
   - key color (`0x004A4FA0`): the cell being edited, or the selected row's cell in the current
     column: 3; else 1 when the command has neither key, 4 when it has only one, 5 when it has both;
   - while editing, the edited cell blinks: hidden when (draw counter `[0x007C02D0]` & 15) ≤ 4 (the
     counter +1 per draw);
   - scroll bar, `data\global\ui\MENU\textslid` (17 frames 12 × 13): track frame 13 × 21 at x = W −
     M − 31, y = T + 59 + 12k; up arrow frame 11 (9 while pressed on it) at (W − M − 31, T + 47);
     down arrow frame 10 (8 pressed) at (W − M − 31, T + 305); thumb frame 14 × s (s = 5 exp, 6
     classic) at x = W − M − 32, y = Ty + 12 + 12j with Ty = (246 − 12s) × top / maxTop + 47 + T,
     maxTop = rows − 15 (47 exp, 36 classic);
   - buttons at text y = T + 351, centre c_i = M + 206i + 103: 0 Cancel 3974 `CfgCancel`, 1 Default
     3972 `CfgDefault`, 2 Accept 3973 `CfgAccept` (table `0x007246D8`, 26-byte records); text x =
     c_i − w/2; hit box c_i ± (w/2 + 10) × [T + 329, T + 367]; color 4, or under the pointer 1
     (Cancel), 3 (Default), 2 (Accept);
   - message: for 2,000 ms after an assignment error, its string (3977–3979, `ui/controls.md` §5)
     centred at x = W/2 − w/2, y = T + 399, color 1.
3. **Keys (not editing)**, each ignored while the exit flag `0x0044DA30` is set: Down / Up
   (`0x004A5AB0` / `0x004A5A40`): selected ± 1, skipping separators, no wrap, scrolling to keep it
   among the 15; `cursor_pass` when it changed. Left (`0x004A5B30`): column 0 → 1; Right
   (`0x004A5B90`): 1 → 0; `cursor_pass` when it changed. Column 1 = slot 1 = "Key/Button One".
   Enter (`0x004A5F70`), Delete / Backspace (`0x004A5BF0`), Esc / Space (`0x004A5C70`): when the
   latch is 0 it becomes 1 and the key does nothing; else (key not auto-repeated): Enter on a
   non-separator row → start editing, `cursor_select`, latch := 0; Delete / Backspace → unbind
   (selected command, slot = column) (`0x00469D70`), `cursor_select`; Esc / Space → Cancel (r6),
   `cursor_pass`. Wheel (`0x004A59D0`): n = delta / 120; n ≠ 0 → scroll the view (top row only) by
   −2n rows, clamped, `cursor_pass`.
4. **Mouse (not editing)**: list area x ∈ [M + 18, M + 3(C + 6)], y ∈ [T + 40, T + 310]. A press
   in it selects the first visible row r with y ≤ T + 58 + 18r and the column (x < M + 18 + 2C → 1,
   else 0; a press on the Function text selects column 1); a separator row changes nothing; while
   held below T + 40 / above T + 310 the selection moves by one per mouse event. A release on the
   same row and column as the selection starts editing (`cursor_select`), so a single click on a
   key cell edits it. Arrows: held → top ∓ 1 every > 50 ms (GetTickCount); track above / below the
   thumb: ∓ 14 every > 50 ms; thumb: dragged. Buttons: press and release on the same button runs it,
   `cursor_select`.
5. **Editing** (`0x004A5D00`): editing := 1; the screen's tables are replaced by tables
   `0x00724738` (left / right down, key down / up incl. system keys), `0x0072472C` (middle down),
   `0x00724780` (X down), `0x0072478C` (wheel). Key down, not auto-repeated: Esc → stop editing, no
   change, `cursor_pass`, latch := 0; any other key K → assign K (`ui/controls.md` §5:
   `0x00469C20`, command < 56, allowed keys, Print Screen and wheel limits): success → stop
   editing, `cursor_select`, latch := 0 when K is Enter, Delete, Backspace or Space, else 1;
   failure → still editing, latch := 1, error message (r2). Key up: latch 0 → latch := 1 (this
   swallows the release of the Enter that started editing); else assign K as above (keys that send
   only a key-up, e.g. Print Screen). Middle → key 0x100, X1 → 0x101, X2 → 0x102, wheel up / down →
   0x103 / 0x104: assigned, no sound. Left / right press: stop editing without change, then
   handled as a normal press (r4); left and right can never be assigned.
6. **Two-key rules and conflicts** (`0x00469C20`): each command has slot 1 and slot 0; a key value
   is held by at most one entry of the whole table: assigning K first unbinds the entry that holds
   K (any command or slot, including the same command's other slot), then writes K to (command,
   slot); Esc (command 56) is not listed and cannot be assigned.
7. **Buttons**: Default (`0x004A5010` → `0x00469A60(0)`): the archive `DATA\LOCAL\CMD\<lang>\
   default.key` is tried (`0x004698D0(0)`; the 1.14d copies fail the header test,
   `ui/controls.md` §2.5), then live table := the in-memory defaults `0x00712220` (the compiled
   table, or a valid `<save>\default.key` read at game start), mouse slots rebuilt; nothing is
   written. Accept (`0x004A5020`): write `<save>\<char>.key` and `<save>\default.key`
   (`0x00469780`, `ui/controls.md` §2.4: answers its open question 6), rebuild, `SetUIState(11,
   off, 0)`, `0x0047E200(1)` (back to the game, saved UI states restored). Cancel (`0x004A5040`,
   also Esc / Space): restore the snapshot (`0x00469D50`), `SetUIState(11, off, 0)`,
   `0x0047E090(0, 1)`: the Options menu with Previous selected.
8. d2rs: Accept writes `controls.toml` (`client/ui.md` §A6: `[bindings]` per action in slot order
   1, 0; `[unbind]` for actions left without keys) instead of the `.key` files (one file for all
   characters); Default = `preset = "original"`.

## Constants & data dependencies

| Constant | Value | Source |
|---|---|---|
| Record size, header size | 0x550, 0x14 | §O2 |
| Headers (n, pitch, baseline, pentagram, slider) | (3/5, 50, 39, 51, 0), (n, 45, 34, 49, 36) | `0x00713044` … `0x0071D734` |
| UI-state save table | 38 × 12 bytes | `0x00713058` |
| Pentagram step | > 50 ms, 8 frames, max width 52 | `0x00454850`, `pentspin` |
| Slider geometry | X0 = W/2 − 60, track 265 px, knob 28 px, bar 290 px, drag x0 = W/2 − 48 | `0x0047E260`, `0x0047D670`; doubles `0x006D7448` (−265), `0x006D73F8` (265), `0x006CEF10` (0.5), `0x006CEEF0` (1.0) |
| Gamma | 55–255, default 155 | `0x0047D200`, `0x0047D230` |
| Key-config box | 620 × 369, 15 rows × 18 px, columns 190, buttons 206 apart | `0x004A5270`, `0x004A5060` |
| Key-config strings | 3921–3923, 3972–3974, 3977–3979, 3762 | `string.tbl` (ENG) |
| Menu handler tables | `0x006D6030` (7), `0x006D6024` + `0x006D60C0` (1 + 13), editing `0x00724738` (6), `0x0072472C`, `0x00724780`, `0x0072478C` | image |

Art (DC6 headers read from the 1.14d MPQs, 1 direction): labels and values as in §O2 r3
(`data\local\ui\eng\`, heights 36 for Game / Options rows and titles, 26 for sub-menu rows);
`data\global\ui\WIDGETS\optbar` (frames 255 × 33, 35 × 33), `optbarC` (255 × 37, 35 × 37), `optskull`
(28 × 28); `data\global\ui\CURSOR\pentspin` (8 frames); `data\global\ui\MENU\textslid` (17 × 12 × 13);
`data\global\ui\MENU\boxpieces` (22 × 14 × 15).

## Randomness

None.

## Edge cases & original bugs

1. Row hit test (§O5 r2) compares y with y_top + **row count**, not the pitch, and ignores x: the
   pointer anywhere across the screen selects rows; the boundaries are 3–8 px below the row tops.
2. Esc closes the whole tree from any sub-menu (§O1 r4); Cancel from Configure Controls returns to
   Options with Previous selected; entering any menu selects its last row.
3. `AutoMapFade` 4 from the registry passes the "> 4" test: the value cel index 4 reads past the
   4-entry array (+0x550 = the next record's kind) → garbage or a crash. Same for `AutoMapMode` 2
   (test "> 2", 2 values). d2rs: `settings.toml` ranges 0–3 / 0–1 (§O7) reject these values, a
   documented deviation; the menu itself never produces them.
4. Contrast apply writes v to the registry but gives the renderer the position p (§O6 r12).
5. A stored volume that is not a multiple of 5 shows at p = ⌊(v + 1) / 5⌋ and is kept until moved
   (`audio/sound-table-2.md` §15 r3); a stored Gamma is snapped to 55 + 10p at each game join when
   gamma is available.
6. NPC Speech is applied at game join only when the sound device is up (`audio/triggers.md`: no
   other start-up path).
7. Classic tables reference `Contrast` and `Auto` art that exists only in d2exp.
8. `audio/sound-table-2.md` §15 r4 gives the audio sliders x0 = h − 133 (no label, +0x53C read as 0
   from the static image); at run time the loader stores the label cel at +0x53C, so x0 = h − 48
   and the drag window is h − 59 … h + 230 (§O5 r5). That spec's r4 and its two drag test vectors
   (x = 267, 300) need this correction (owner: `audio/sound-table-2.md`).
9. Key config: a press on the Function column selects column One; the latch swallows one Enter /
   Delete / Backspace / Esc / Space press after binding one of Enter, Delete, Backspace, Space or
   after Esc cancelled an edit.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| Esc in game (nothing else open) | Game menu, selected row 2 (Return to Game); pentagrams at (99, 336), (649, 336) at 800 × 600 | §O1 r2, §O4 r3–r4 |
| Down on Return to Game | selected Options (wrap), `cursor_pass` | §O5 r4 |
| Enter on Options | Options menu, selected Previous (row 4), `cursor_select` | §O2 r5, §O5 r6 |
| Sound menu, selected SPrevious (7); Up, Up (d2rs: 3D Bias, EAX, 3D Sound disabled) | NPC Speech (6), then Music (2) | §O5 r4, §O8 |
| Music at p 10 (volume 50), Left | p 9, Music Volume 45, `cursor_pass` | §O5 r4, r7 |
| Sound at p 20, Right | p 20, no apply, no sound | §O5 r4 |
| Light Quality High (2), Right | Low (0), registry 0, low-quality 1, missile lights 0, `cursor_pass` | §O5 r4, §O6 r8 |
| Light Quality Low (0), Left | High (2) (wrap) | §O5 r4 |
| Automap Fade Auto (3), Enter | No (0), `cursor_pass` | §O5 r6 |
| Gamma with no registry value, then Right | p 10 → 11, Gamma 165 | §O5 r7, §O6 r11 |
| Contrast registry 100, Left | p 99 → 98, Contrast 98 | §O5 r7 |
| Drag on Music, W 800: x 352 / 400 / 617 / 618 | p 0 / 4 (vol 20) / 20 / 20 | §O5 r5 |
| Press on a slider row at x 341 (W 800) | no drag (strict h − 59 < x) | §O5 r5 |
| Sound menu 800 × 600 (row tops 80, 125, 170, …), pointer y 175 / 180 / 300 | Sound (row 1) / Music (row 2) / no change (row 4 EAX disabled) | §O5 r2 |
| Pointer y 80 or 440 in the Sound menu | no row (strict bounds) | §O5 r2 |
| Esc in Video Options | menu closed, remembered panels reopened | §O1 r4 (REC-212) |
| Key config 800 × 600 | headings at x 108, 298, 488, y 100; row 0 text y 122; buttons centred at 193, 399, 605 | §O9 r2 |
| Key config: Inventory row, column One, Enter, press C | Character slot 0 → None, Inventory slot 1 = C, `cursor_select`, editing ends | §O9 r5–r6 |
| Editing, press Esc, then Enter | no change; the Enter is swallowed (latch), next Enter edits | §O9 r3, r5 |
| Editing, wheel up | key 0x103 assigned if the command has no up handler, else message 3978 | §O9 r5, `ui/controls.md` §5 |
| Key config, Cancel | table restored; Options menu, Previous selected | §O9 r7 |

## Provenance

1.14d `Game.exe` (Ghidra exports + `tools/ghidra/disasm.py`, 2026-10-08): menu engine — load
`0x0047DD70`, `0x0047DBB0`; free `0x0047DFD0`, `0x0047DF00`; open `0x0047E090` (jump table
`0x0047E1E4`); close `0x0047E200`; draw `0x0047E3D0` (jump table `0x0047E708`), slider `0x0047E260`,
multi-frame cel `0x00502680`, rectangle `0x0046EFD0`; pentagram gate `0x00454850`, cel `0x00453AA0`
(`[0x007A2878]`, loaded in `0x00456970` from `%s\UI\CURSOR\Pentspin`); hit `0x0047D520`; activate
`0x0047D5C0`; drag `0x0047D670`; handlers `0x0047D7F0`, `0x0047D840`, `0x0047D8A0`, `0x0047D920`,
`0x0047D9A0`, `0x0047DA90`, `0x0047DB80`; conversions `0x0047CC90`, `0x0047CD00`; registration
`0x00455720` / `0x00455AE0` (tables `0x006D6030`, `0x006D6024`, `0x006D60C0`). Rows: records at
`0x00713220` … `0x00721708` read from the image (kinds, names, callbacks, counts, defaults); callbacks
`0x0047CD80` … `0x0047D4D0`, `0x0047F2A0` … `0x0047F460`; setters `0x00514CD0`, `0x00514D00`,
`0x00514D30`, `0x00514D60`, start-up `0x00514B60`; sound tests `0x004DF880`, `0x004DF940`,
`0x004DF960`, `0x004DF980`, `0x004DF9B0`; video `0x004F5140`–`0x004F52B0`, `0x004776C0`,
`0x0044BA20`, `0x00476000`; automap `0x0045A720`, `0x004576C0`–`0x004577F0`; registry `0x00414F10`,
`0x00414B00`, `0x004150E0`, `0x00414C70`; strings `0x006CC8B8` "Diablo II", `0x006D73A8` …
`0x006D73E8`, `0x006D66A0`, `0x006DEA8C` … `0x006DEAB8`. Key config: `0x004A43E0`, `0x004A44C0`,
`0x004A4500`, `0x004A45C0`, `0x004A4720`, `0x004A4780`, `0x004A47C0`, `0x004A47F0`–`0x004A4A10`,
`0x004A4AD0`, `0x004A4FA0`, `0x004A5060`, `0x004A5190`, `0x004A5200`, `0x004A5270` (draw),
`0x004A5980`–`0x004A63B0`; button records `0x007246D8`; binding helpers `0x00469A60`, `0x00469C20`,
`0x00469D40`, `0x00469D50`, `0x00469D70`, `0x00469D90`, `0x00469DE0`, `0x004698D0`. Art: DC6 headers of
the files in d2data / d2exp (`tools/mpq-tool` into a scratch dir). Strings: ENG `string.tbl` (d2data).
D2MOO (1.10f) `D2MenuItemStrc` / `D2MenuInfoStrc` used only as a hint for the record layout; every
field confirmed from the 1.14d records and their users.

## Open questions

- **REC-212** Esc menu tree capture. Settles §O1 r4 (Esc from a sub-menu closes the whole menu) and
  §O4 (no backdrop; positions and pentagram placement of §O4 r4 against pixels).
- **REC-213** Slider float precision. Settles §O5 r7 (53-bit vs 24/64-bit x87 precision: Contrast
  p 99 → 100 or 99).
