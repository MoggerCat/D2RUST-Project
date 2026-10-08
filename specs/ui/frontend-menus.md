# Spec: UI — Front-end menus (title, main menu, character select, character create, difficulty)

- **Status:** draft (2026-10-08, RE on the 1.14d `Game.exe` merged front end; no capture yet). Points
  that need a capture are PROVISIONAL with a REC id (METHODS M22).
- **Target version:** 1.14d, English install
- **Crate/module:** `d2-client::ui::front_end`
- **Related specs:** `ui/text.md` (fonts, text draw), `ui/panels.md` (UI art conventions, DC6 draw,
  states), `render/sprite-placement.md` (DC6 placement), `render/blend-modes.md` (draw modes),
  `formats/d2s.md`, `formats/d2s-load.md`, `formats/d2s-appearance.md` (save fields, paper doll),
  `items/generation.md` §10.3 (start items), `client/model.md` §7 r9 (the C→S 0x67 the game start sends),
  `sim/intents-events.md` §8 (the single-player session sequence), `client/msg-ui.md` OQ8 (the
  end-of-game target `[0x0070EE8C]`), `client/audio.md` (sounds, deferred).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 52–60 |
| Inputs | 61–71 |
| Outputs / state changes | 72–81 |
| Rules | 82–83 |
|   F1.1 Control descriptors (the data every front-end screen is built from) | 84–123 |
|   F1.2 Art preload | 124–151 |
|   F1.3 Screen flow (single player) | 152–183 |
|   F1.4 Main menu (`0x004336C0`) | 184–212 |
|   F1.5 Title animation (logo fire) | 213–240 |
|   F1.6 Palette and sounds | 241–263 |
|   F2.1 Save folder (`0x00407050`) | 264–282 |
|   F2.2 Scan and entry filter (`0x00438F70`, `0x0043C8A0`, `0x00438AD0`) | 283–309 |
|   F2.3 Sort order (`0x00438AD0`) | 310–315 |
|   F2.4 Layout (`0x0043AE30`, draw `0x004380F0`) | 316–339 |
|   F2.5 Selection, scrolling and keys | 340–384 |
|   F2.6 OK / Enter (`0x00439840`) | 385–397 |
|   F2.7 Other buttons | 398–422 |
|   F2.8 Difficulty box (`0x00439780`) | 423–442 |
|   F2.9 Control records and art | 443–479 |
|   F3.1 Character-create screen build (`0x00435580`) | 480–515 |
|   F3.2 Class line-up (positions, creation order) | 516–531 |
|   F3.3 Class animation state machine (D2Win anim control `0x00500850`) | 532–591 |
|   F3.4 Name entry (edit box, descriptor 204) | 592–606 |
|   F3.5 Check boxes (hardcore, expansion; ladder named only) | 607–630 |
|   F3.6 OK / Cancel behaviour and the new save | 631–656 |
|   F3.7 Sounds (deferred) | 657–661 |
|   F3.8 Art (`0x004326F0`, all `data\global\ui\FrontEnd\…`; frames from the 1.14d MPQs, 1 direction) | 662–690 |
| Constants & data dependencies | 691–727 |
| Randomness | 728–731 |
| Edge cases & original bugs | 732–763 |
| Test vectors | 764–797 |
| Provenance | 798–835 |
| Open questions | 836–880 |
<!-- /index -->

## Summary

The screens a single-player session passes through before the first game frame: the title / main
menu with its fire and logo animation, Single Player → character select (the saves found on disk),
then either character create (class line-up, name, hardcore / expansion) or the difficulty box, and
finally the game start (`client/model.md` §7 r9). Covers the flow and back paths, every button
(multiplayer ones named only: Phase 7+), the art files, positions, string ids and the input rules.
Menu sounds are named only (sound is deferred).

## Inputs

| Input | Source |
|---|---|
| Front-end art (DC6 / palette files under `data\global\ui\FrontEnd\`, `data\global\ui\CharSelect\`) | the user's MPQs (`client/assets.md`) |
| Strings (button labels, messages, class descriptions) | `.tbl` string tables (`formats/tbl.md`, `ui/text.md`) |
| Saves | the Save folder and its `.d2s` files (`formats/d2s.md`, `formats/d2s-appearance.md`) |
| Install facts | expansion installed (`client/model.md` Inputs `expansion_installed`) |
| Pointer and keys | the client input edge (`ui/controls.md`, `client/ui.md` §A4) |
| Wall-clock time | animation timing of the title fire / logo and the class animations (front end only; not game logic) |

## Outputs / state changes

| Output | Where |
|---|---|
| The screen drawn each frame (800 × 600 frame of `client/ui.md`) | the front-end draw pass |
| A new character's stub save written to the Save folder | `formats/d2s.md` (stub), §F3 of this spec |
| A deleted save (file removed) | §F2 of this spec |
| The chosen character, difficulty and hardcore / expansion flags handed to the game start | `client/model.md` §7 r9 (C→S 0x67), `sim/intents-events.md` §8 |
| Process exit (Exit Diablo II) | §F1 of this spec |

## Rules

### F1.1 Control descriptors (the data every front-end screen is built from)

1. Each front-end screen is built from a static table of 0x30-byte control
   descriptors at `0x00708D10` (descriptor *i* at `0x00708D10 + 0x30·i`).
   Two adders create a control from descriptor *i* through the D2Win factory
   `0x004F93C0` and push it on one of two lists: `0x0042F3A0` (list A,
   `[0x00779450]`, count `[0x00779948]`, max 64; popups) and `0x0042F430`
   (list B, `[0x00779350]`, count `[0x00779944]`, max 64; screens). A full
   list is a fatal error (string 0x610 / 0x627).
2. Descriptor fields (u32 each; the factory passes [1], [2] in registers):
   [0] control type, [1] x, [2] y, [3] w, [4] h, [5] hotkey (virtual key;
   27 Esc, 13 Enter, 0 none), [6] string id, [7] pointer to the cel-file
   global, [8] click / action callback, [9] extra (anim table, font, ...),
   [10] extra, [11] flags. Types used here: 2 image (`0x004FD6C0`), 3
   animated image (`0x00500850`), 4 text (`0x004FC7A0`), 6 button
   (`0x00501290`), 8 timer (`0x004FD9D0`), 1 edit box (`0x005001D0`).
3. **y is the bottom edge.** A button's hit box is `x ≤ mx < x+w`,
   `y−h ≤ my < y` (`0x00500DB0` tail). Full-screen images use y = 599 (800×600)
   or 479 (640×480).
4. Button art (`0x00501290`, draw `0x00500DB0`): a button cel is cut in
   256×256 tiles; `tiles = ceil(w/256)·ceil(h/256)` frames per state. Frames
   `0 … tiles−1` = up, `tiles … 2·tiles−1` = pressed. A disabled button draws
   from frame `tiles·2` if flag 0x20 is set (3-state art, e.g. `3WideButtonBlank`,
   6 frames), else the up frames with draw mode 1; enabled buttons draw mode 5
   (`render/blend-modes.md` §1). Tiles draw left→right at +256 px.
5. Button label (`0x00500C50`): string [6] via `D2Lang_GetStringByIndex`
   (`ui/text.md` §2), centered in w; font `[+0x60]` = **9 (FontExocet10)** if
   h ≥ 35, else **10 (FontRidiculous)** (`ui/text-fonts.tsv`); baseline =
   `y − (h − text_height)/2 + k` with k = 4 (h ≥ 35), 3 (h = 32), 2 (20 < h < 32),
   1 (h ≤ 20); +2 while pressed. Drawn with `0x00502360` (`ui/text.md` §9).
6. Text descriptors (type 4) point [10] at a two-dword record in the
   `0x007089AC` table whose first dword is the font id: `0x007089AC` → 3
   Font42, `0x007089B4` → 2 Font30, `0x007089BC` → 7 Font24, `0x007089C4` → 1
   Font16, `0x007089CC` → 0 Font8, `0x007089D4` → 4 FontFormal10,
   `0x007089DC` → 5 FontFormal12, `0x007089E4` → 4, `0x007089EC` → 8
   FontFormal11 (second dword: 0, 0, 0, 0, 4, 1, 0, 0, 0; meaning not traced).
7. Timer (type 8, `0x004FD9D0`, update `0x004FD860`): created with
   GetTickCount; once `now/1000 − start/1000 > [1]` (whole seconds) it calls
   [8] on every update until the screen is torn down.

### F1.2 Art preload

All front-end cels load once in `0x0042E6D0` (freed in `0x0042EF50`), each
into a fixed global; descriptors point at the globals. `0x00408F20` ≠ 0
(expansion installed) picks the `…EXP` art. Main-menu/title relevant ones:

| Global | File (`data\global\ui\…`) | Frames (DC6) | Archive |
|---|---|---|---|
| `0x007796F8` | `FrontEnd\blizno` | 6 (640×480 tiled) | d2data |
| `0x007796FC` | `FrontEnd\trademark` / `trademarkscreenEXP` | 12 | d2data / d2exp |
| `0x00779704` | `FrontEnd\Diablo2` | 2 | d2data |
| `0x00779708` | `FrontEnd\D2logoBlackLeft` | 30, 159×122, offset (−169, 47) | d2data |
| `0x0077970C` | `FrontEnd\D2logoFireLeft` | 30, ~170×155, offset (−174, 52) | d2data |
| `0x00779710` | `FrontEnd\D2logoBlackRight` | 30, 159×122, offset (9, 47) | d2data |
| `0x00779714` | `FrontEnd\D2logoFireRight` | 30, ~174×150, offset (0, 53) | d2data |
| `0x00779718` | `FrontEnd\TitleScreen` / `gameselectscreenEXP` | 12 (800×600 in 256² tiles) | d2data / d2exp |
| `0x0077971C` | `CURSOR\ohand` | — | d2data |
| `0x00779738` | `FrontEnd\WideButtonBlank` | 4 (272×35) | d2data |
| `0x00779754` | `FrontEnd\WideButtonBlank02` | 4 | d2exp |
| `0x00779758` | `FrontEnd\NarrowButtonBlank` | 4 (272×25) | d2exp |
| — | `CharSelect\ShortButtonBlank` | 2 (135×25) | d2data |
| — | `FrontEnd\MediumButtonBlank` | 2 (128×35) | d2data |

Other screens' art (character select `CharSelect\characterselectscreenEXP`,
create `FrontEnd\CharacterCreate` / `charactercreationscreenEXP` + `fire`
+ class folders, `CharSelect\DifficultyLevels`, popups) loads in the same
function or in `0x004326F0` (character-create art, per class).

### F1.3 Screen flow (single player)

| From | Trigger | 1.14d code | To |
|---|---|---|---|
| start | first screen | descriptors 0 (`blizno`, 640×480 at (0,479)) + 1 (timer 8 s) | Blizzard logo |
| Blizzard logo | click on the image or timer > 8 s | `0x00434D90`: `0x004F9050`≠0 → `0x00433640`; `0x004F9060(1)`; `0x0042FB20` | trademark |
| trademark | screen builder | `0x0042FB20`: descriptors 2 (`trademarkscreenEXP`), 3 (timer 9 s), 6, 7 (logo L/R), 4 (text, FontFormal12, (100,580) 600×80) | — |
| trademark | click on the image, or timer > 9 s | `0x00434DC0` → `0x004336C0` | main menu |
| main menu | Single Player (Enter has no binding) | `0x00435CD0`: `[0x007795EC]` := 0 (game kind single player); `0x00430BC0` looks for `<save dir>*.d2s` (`"%s*.d2s"` `0x006D41D4`, FindFirstFile) | found → `0x00515020`, `0x0042F480` (tear down), `0x0043B080` **character select**; none → `0x00435580` **character create** |
| main menu | Exit Diablo II or **Esc** (hotkey 27) | `0x00431C30`: `[0x007795E8]` := 0, `0x00515F50(0xB4)`, `0x004F9190` | leaves the front end (program exit) |
| main menu | Credits | `0x004312C0` | credits screen (`creditsbckgexpand`, descriptor 41) |
| main menu | Cinematics | `0x00431600` | cinematics list (descriptors 61–70, `CinematicsSelectionEXP`) |
| character select | Exit or **Esc** (descriptor 163, 5101 EXIT) | `0x00434DD0`: `0x0043B9A0`, `0x004336C0` | main menu |
| character select | Create New (descriptor 164, 10832 CREATE NEW) | `0x00435E00` | character create |
| character select | OK or **Enter** (descriptor 162, 5102 OK) | `0x00439840` | difficulty popup (`0x00439780`) when more than one difficulty is open, else game load (`0x00434A00`) |
| character create | Exit or **Esc** (descriptor 175, 5101) | `0x00430C30`: `0x00515020`, `0x0042F480`, `0x0043B080` | character select (also when no character exists) |
| character create | OK or **Enter** (descriptor 176, 5102) | `0x004369F0` | game load (new character) |
| difficulty popup | Normal / Nightmare / Hell | `0x00439B80` / `0x00439BA0` / `0x00439BC0` → `0x00439AF0` | game load |
| difficulty popup | **Esc** (descriptor 173: invisible 10×10 button at (900,900), hotkey 27) | `0x00432EE0` → `0x0042F3F0` (pops list A) | character select |
| in game | exit (0x50 code 23 path) | `0x0044B8A0` returns 4 for single player (`client/msg-ui.md` OQ8) | PROVISIONAL: character select (because 1.14d returns to it after Save and Exit); settled by REC-200 |

Rows 1–2 (the `blizno` picture and its 8 s timer): never shown in 1.14d. The start-up chain
`0x00435230` plays the Blizzard videos and goes straight to the trademark screen, and no caller
builds descriptors 0 / 1: none of the 347 call sites of the adders `0x0042F430` / `0x0042F3A0`
passes 0 or 1 in ECX (scan of the ECX write before each call, 2026-10-08; the smallest is 0x1F), and the timer
callback `0x00434D90` is referenced only by descriptors 0 and 1 (`0x00708D30`, `0x00708D60`).
REC-229 only verifies. The start-up chain is `ui/frontend-credits.md` C1.

Every screen builder starts with `0x0043C4F0` (descriptors 8 background,
6, 7 logo halves) except the character screens, which draw their own
backgrounds. Re-entering the main menu rebuilds all its controls.

### F1.4 Main menu (`0x004336C0`)

1. `0x0043B9A0`; session byte `[0x007795D4]+0x37` := 0; `0x0043C4F0`
   (background 8 = `gameselectscreenEXP` at (0,599) 800×600; logo halves 6, 7).
2. Expansion (`0x00408F20` ≠ 0) adds descriptors 0x12, 0x11, 0x13, 0x10;
   classic adds 0xA, 0xB, 9, 0xC (the same buttons 100 px higher). Then
   0xD, 0xE, 0xF, 0x115 (all installs). If `0x004FAC90` ≠ 0, Battle.net,
   Other Multiplayer and Open Battle.net are disabled (`0x004F96F0(ctl, 0)`).
3. Buttons (expansion layout; y = bottom edge):

| Desc | Button | String | x, y | w×h | Art (global) | Hotkey | Callback |
|---|---|---|---|---|---|---|---|
| 16 | Single Player | 5106 SINGLE PLAYER | 264, 324 | 272×35 | WideButtonBlank `0x00779738` | — | `0x00435CD0` |
| 17 | Battle.net | 5107 BATTLE.NET | 264, 366 | 272×35 | WideButtonBlank02 `0x00779754` | — | `0x00435D30` (out of scope) |
| 18 | Open Battle.net (gateway) | 0 (label set by `0x00431AF0`) | 264, 391 | 272×25 | NarrowButtonBlank `0x00779758` | — | `0x00434600` (out of scope) |
| 19 | Other Multiplayer | 5108 OTHER MULTIPLAYER | 264, 433 | 272×35 | WideButtonBlank | — | `0x00430C50` (out of scope: opens 0xFF–0x101, Open Battle.net 5115 / TCP/IP 5116) |
| 13 | Credits | 5110 CREDITS | 264, 528 | 135×25 | ShortButtonBlank | — | `0x004312C0` |
| 14 | Cinematics | 5111 CINEMATICS | 402, 528 | 135×25 | ShortButtonBlank | — | `0x00431600` |
| 15 | Exit Diablo II | 5109 EXIT DIABLO II | 264, 568 | 272×35 | WideButtonBlank | 27 (Esc) | `0x00431C30` |

   Classic layout (descriptors 9–12): same x, w, h, art and strings, y = 224,
   266, 291, 333. TCP/IP is not on the main menu (it is inside Other
   Multiplayer, descriptor 256, 5116).
4. Version text: descriptor 0x115 (277), type 4, (0, 599) 200×40, font
   Font16 (`0x007089C4`), flags 2. Text = `"v %d.%d%c"` (`0x006D48D4`) with
   1, 14, 'd' → **"v 1.14d"**; if session byte `+0x223` ≠ 0, `" %d"`
   (`0x006D48D0`) with 71 is appended. Set with `0x004FCFF0`.
5. Then `0x00514D80` ≠ 0 → `0x005148F0(1)` (not traced; not drawn here).

### F1.5 Title animation (logo fire)

1. Descriptors 6 and 7: type 3, both at (400, 120), w×h 181×170 / 188×177,
   anim tables `0x00708C30` (left) / `0x00708C80` (right). Each table has 5
   identical 16-byte entries `{base cel global, overlay cel global, overlay
   draw mode 3, speed 1}`: left `{0x00779708 BlackLeft, 0x0077970C FireLeft,
   3, 1}`, right `{0x00779710 BlackRight, 0x00779714 FireRight, 3, 1}`.
   The entry is chosen by the control's state `[+0x50]` (0 at creation;
   all 5 entries equal, so state does not change the picture).
2. Draw (`0x005005B0`), per frame, both at the control's (x, y) with the
   DC6 frame offsets (`render/sprite-placement.md`): first the black base
   with draw mode `[+0x54]` = 5 (default when the descriptor gives 0;
   opaque), then the fire overlay with draw mode 3 (additive,
   `render/blend-modes.md` §1). Both use the same frame index.
3. Frame timing: `ms_per_frame = speed·1000/25` = **40 ms (25 fps)**;
   frame = `((now − t_created)/40) mod (frames − 1)` with frames = the base
   cel's frame count (30) → **frames 0–28 loop; frame 29 is never shown**.
   (States 2 and 4 would play once and hold the last frame; unused here.)
   `t_created` = GetTickCount at control creation (`0x00500850`), so the
   animation restarts at frame 0 whenever a screen is (re)built; the left
   half is created just before the right, so they run in step.
4. Draw order on the main menu: background 8, logo left 6, logo right 7,
   buttons, version text, in creation order. PROVISIONAL: D2Win draws a
   list in creation order (because the builders add the background
   first); settled by REC-201.
5. `Diablo2.dc6` (descriptor 5, (240,120) 320×151) is preloaded but no
   builder found here adds it.

### F1.6 Palette and sounds

1. Palette: every screen built through `0x0043C4F0` loads
   `data\global\palette\sky\pal.dat` and `pal.pl2` (paths at
   `0x006D3A08` / `0x006D3A0C`, loader `0x0042F2E0`); trademark, credits and cinematics
   load the same files (`ui/frontend-credits.md` C7). Other
   palettes present (`Menu0`–`menu4`, `Trademark`, `loading`, `fechar`) are
   not loaded by the front-end screens traced here (`fechar` is only
   compared against by `0x0042F2E0`; `loading` is `ui/frontend-loading.md`).
2. Menu sound files (behaviour deferred to `client/audio.md`): button
   click `cursor\button.wav` (Sounds.txt row 4 `cursor_button_click`),
   `cursor\select.wav` (row 2), `cursor\windowopen.wav` (rows 3, 6), class
   select / deselect `cursor\intro\<class> select.wav` / `deselect.wav`
   (rows 17–30, Patch_D2 Sounds.txt), all under `data\global\sfx\`.

Part 2: character select and the difficulty box, single player, expansion (LoD) front end.
`0x00408F20()` ≠ 0 is "expansion front end"; the classic-only paths (`0x00438560` list draw,
`0x00439210` selection update, `0x0043A0D0` keys, `0x00439D60` / `0x00439DA0` arrow buttons, `0x0043B080`
classic rebuild) are named only. `[0x007795EC]` is the front-end connection mode; 0 is single player
(multiplayer modes 1–3: Phase 7+, named only). The screen's controls come from the front-end control table at
`0x00708D10` (0x30-byte records, created by `0x0042F430(id)` into the screen list, by `0x0042F3A0(id)` into
the pop-up layer; record layout in §F2.9).

### F2.1 Save folder (`0x00407050`)

1. The Save folder is read from the registry, never from the command line (no argument is consulted):
   key `Software\Blizzard Entertainment\Diablo II` (`0x00414930`), HKCU first, then HKLM (`0x00414990`,
   `0x00414B00`), value `NewSavePath` (string).
2. `NewSavePath` missing (`GetLastError() == 2`): read the legacy value `Save Path`, drop one trailing `\`;
   if it names an existing directory, the default path is computed (rule 3) and `0x00406DE0` decides between
   them: result 1 → keep the legacy path, else → the default path with a trailing `\`. The choice is written
   back as `NewSavePath` (`0x00415070`). PROVISIONAL: `0x00406DE0` returns 1 when the legacy folder holds
   entries other than `.` / `..` (because the function compares those two names); settled by REC-206.
3. The path read does not exist (`GetFileAttributesA` = −1) or is empty → default path
   (`0x00406D30`): `SHGetKnownFolderPath(FOLDERID_SavedGames {4C5C32FF-BB9D-43B0-B5B4-2D72E54EAAA4})` +
   `Diablo II` (`0x004067D0`); if that API is missing or fails, `SHGetFolderPathA(CSIDL_PERSONAL)` +
   `Diablo II\Save`. Written back as `NewSavePath`.
4. A trailing `\` is appended when missing, and the directory is created (`0x00406A10`). Every save path is
   `<folder><name>.d2s` (`"%s%s.d2s"`, `0x0043C8A0`, `0x0043C6B0`).
5. d2rs: the user's own Save folder is an input path; the registry is not read. The default rule 3 is the
   d2rs default.

### F2.2 Scan and entry filter (`0x00438F70`, `0x0043C8A0`, `0x00438AD0`)

The scan runs when the screen is built (`0x0043AE30` → `0x0043ADB0` → `0x00438F70`) and again after a delete.

1. `FindFirstFileA("<folder>*.d2s")`, in the order the file system returns. For each file:
2. Name = the file name up to its first `.` (so `a.b.d2s` → `a`, which then reads `a.d2s`).
   Skipped without a message when: length < 2 or > 15; the last character is `-`; the first is `_`.
3. Header read (`0x0043C8A0`): `fopen "rb"`, read ≤ 0x2000 bytes. Rejected (skipped) when fewer than 8
   bytes, magic ≠ 0xAA55AA55, or: version ≥ 0x5C with < 0x14F bytes or version > 0x60; version < 0x5C with
   < 0x82 bytes or version < 0x47. No checksum check (`formats/d2s.md` §2.7 rule 1). Fields kept: class
   +0x28, level +0x2B, status u16 +0x24, components +0x88 (11 bytes; all-zero first byte → the default set
   at `0x0070CCC8`), colours +0x98 (11). The legacy (< 0x5C) offsets are `formats/d2s-legacy.md`.
   The displayed name is the file name (rule 2), not the header's name field.
4. Entry build (`0x00438AD0`, 0x350 bytes): name +0x000, realm +0x100 (empty in single player), components
   +0x300 / colours +0x310 (padded to 16 with 0xFF), class +0x320, level +0x322, status +0x324,
   guild tag +0x32A (empty), paper doll +0x338, last-write time +0x340 (low) / +0x344 (high), next +0x34C.
   A realm name equal to one of 7 beta realm names (`Testv104`, `FatRealm`, `BetaWest`, `BetaEurope`,
   `BetaUSWest`, `BetaUSEast`, `BetaAsia`; `0x00437D20`) drops the entry: never true in single player.
5. Count: no maximum. `[0x00779DC4]` (entries) and `[0x00779DE0]` (accepted files) grow without a cap;
   only 8 are visible at once (§F2.4).
6. Paper doll: `0x005066C0(class', mode, components, colours)`: mode 5 (TN) for softcore; hardcore alive →
   mode 1 (NU) in the expansion front end; hardcore dead → class' = 8 for classes 0, 1, 6, else 9, mode 5.
   A failed build retries with class 7 / mode 5, all components 0 except +1 = 1, colours 0xFF; a second
   failure is fatal. Appearance bytes: `formats/d2s-appearance.md`. PROVISIONAL: class' 8 / 9 draws the dead
   (grey) figure of the female / male body (because 8 and 9 are outside the 7 class tokens and only used
   for dead hardcore); settled by REC-205.

### F2.3 Sort order (`0x00438AD0`)

Expansion front end: inserted by last-write time (`WIN32_FIND_DATA.ftLastWriteTime`), newest first; equal
times keep scan order (a new entry goes after existing ones with the same time). The first entry is the
most recently saved character. Classic front end: appended in scan order.

### F2.4 Layout (`0x0043AE30`, draw `0x004380F0`)

Positions are 800 × 600 frame pixels; control `y` in the table is the **bottom** edge (top = y − h + 1).

1. 8 slots (`[0x0070CC0C]` = 8), 2 columns × 4 rows; slot i (visible index) is column i & 1, row i / 2.
   Row bottom y = 178 + 93 × row (`[0x007799FC + 4i]`): 178, 271, 364, 457. Each slot is two click
   controls: text 0x84+i at x 37 (left) / 309 (right), 200 × 92; figure 0x8C+i at x 237 / 509, 72 × 93.
2. Entry k is drawn in slot k − first, `first` = `[0x00779DC8]`; entries outside [first, first + 8) get no
   paper doll draw.
3. Paper doll anchor: x = column x + 30 (67 / 339), y = row bottom − 13 (`render/sprite-placement.md`).
4. Text in the slot's text control, top to bottom (`0x004380F0`):
   - the title prefix (`world/quests-act1-rest.md` "Character title", `0x005068A0(class, p, hardcore,
     expansion)`, p = status bits 8–12) followed by the name (and ` {tag}` when a guild tag exists: never in
     single player); colour 1 (red) when hardcore, else 4 (gold) (`ui/text.md` colours);
   - level line: `" %d "` with the level and the class name (`0x00437F60`); PROVISIONAL text "Level N
     ClassName" (because the string id is passed in a register lost by the export); settled by REC-207;
   - when status & 0x20: string 22731 `EXPANSION CHARACTER` in colour 2 (green).
   Dead hardcore has no extra text: it shows only through the dead figure (§F2.2 rule 6) and draw flags
   from `0x006CE278` / `0x006CE27C` (hardcore, dead) versus `0x006CE2F8` / `0x006CE2FC`.
5. Selection box: control 0x96, `charselectbox` (256 × 93), moved to x 37 (even index) or 309 (odd), the
   selected slot's row y, shown only when the selection is visible (`0x004390A0`). 0x97 is the grey box
   (`charselectboxgrey`), created for multiplayer only.
6. Top label: control 0x9C (x 85, y 78, 466 × 42) shows the selected character's name (`0x004390A0`).

### F2.5 Selection, scrolling and keys

State: `sel` = `[0x0070CC00]` (index into the list), `first` = `[0x00779DC8]`, `n` = `[0x00779DC4]`.

1. On build: `sel` = 0, `first` = 0 (the newest save is selected).
2. Click (`0x0043A9D0`, on 0x84+i / 0x8C+i): ignored while a pop-up is up (`[0x00779C6C]`). The hit slot i
   gives k = first + i; k > n → ignored; else `sel` = k (k = n selects the empty slot: box shown, buttons
   off); `n` < 1 → `sel` = −1. Double click: a mouse-down (0x201) on the same `sel` within 500 ms of the
   previous click (`GetTickCount`, `[0x00779DEC]`) runs OK (§F2.6).
3. Keys (`0x00439E90`, the selection box's key handler; WM_CHAR 0x102 ignored):
   | Key | Effect |
   |---|---|
   | Home | `sel` = 0, `first` = 0 |
   | End | `sel` = n − 1; if n > 8, `first` = n − 8 (can be odd: rows then pair entries 1–2, 3–4 …) |
   | Left | `sel` odd → `sel` − 1 |
   | Right | `sel` even and < n − 1 → `sel` + 1 |
   | Up | `sel` ≥ 2 → `sel` − 2; above `first` → `first` = `sel` (even) or `sel` − 1, scroll bar −1 |
   | Down | `sel` + 2 ≤ n − 1 → `sel` + 2; below the last visible → `first` = `sel` − 7 (odd `sel`) or `sel` − 6, scroll bar +1 |
   Left / Right also scroll `first` to keep `sel` visible but do not move the scroll bar thumb, nor do
   Home / End (`0x00439E60` is called only by Up / Down).
4. Scroll bar: control 0xA7 (`joingamescrollbars`, x 564, y 457, 34 × 371), shown with range
   ⌈(n − 8) / 2⌉ when n > 8 (`0x004FC5E0` at `0x0043B04D`). Its callback (`0x00439DF0`): position change d
   → `first` += 2d, clamped to [0, n − 1]; `sel` is not changed. The arrows and thumb are the D2Win scroll
   bar widget's.
5. Mouse wheel (D2Win front-end handler `0x004FA340`, last entry of the 33-entry front-end message
   table `0x0072DC48`, registered by `0x004F8FE0`; disassembled 2026-10-08): k = (signed 16-bit wheel
   delta, high word of wParam) / 120, truncated toward 0. Ignored while the D2Win control held at
   `[0x0087E974]` (type 0x0B) is visible and enabled (`0x005085C0`). Among the controls of the list
   `[0x007D55D4]` (when set) or else the main D2Win list `[0x007D55BC]`, it takes the visible, enabled
   scroll bars (D2Win type 5) whose range is > 0 and keeps the one nearest the pointer (`0x004FA200`;
   no hit test: the wheel works with the pointer anywhere). On character select the only one is the
   list's scroll bar (control 0xA7, a text box created with flag 4, which gives it a scroll bar; range
   set by r4 only when n > 8; unlinked from the text box and given the callback `0x00439DF0` at build,
   `0x0043AF30` / `0x0043AF40`). The step (`0x00508030`, skipped while the bar's +0x58 is set): k > 0
   (wheel up) → position − 1; k < 0 → position + 1; k = 0 → unchanged (one step per message whatever
   |k|); clamped to [0, range]; then the callback: `first` += 2 × (position change), clamped to [0, n −
   1], `sel` unchanged, then `0x00438560` and `0x00439210` (as for the arrows, r4). The handler then
   offers the wheel to every visible, enabled button (D2Win type 6) with button flag 8 (k > 0) or 0x10
   (k < 0) (field +0x40; which front-end buttons set them is not traced: open question). With n ≤ 8
   saves (n as in r4) the wheel does nothing. This replaces the earlier "no wheel" reading (REC-204 now
   only verifies).
6. Buttons enabled (`0x004390A0`): no entry at `sel` → OK, Delete, Convert off; else OK and Delete on,
   Convert on unless status & 0x20; Create New on (`[0x00779DB8]` = 0; its only write is 0 at
   `0x0043AD21`).

### F2.6 OK / Enter (`0x00439840`)

Button 0xA2 (key Enter); also the double click. Ignored while a delete / convert is in progress
(`[0x00779DA0]`). With an entry at `sel`:

1. Status & 0x20 (expansion) and a classic front end → nothing.
2. Hardcore and dead (status & 0x0C = 0x0C) → message box string 5304 "You cannot create or join games
   with a dead hardcore character." (`0x00433450`); stays on the screen.
3. p = status bits 8–12. Expansion character p ≥ 5, or classic character p ≥ 4 → difficulty box (§F2.8).
4. Else the game starts at once at Normal: `0x00434A00(entry, class, realm, status)` copies name to the
   session +0xBD, realm +0xD5, status +0x1EF; difficulty session +0x210 stays 0 (set at OK). Next:
   `client/model.md` §7 r9 (C→S 0x67), `sim/intents-events.md` §8.

### F2.7 Other buttons

| Id | Label (string ids) | Key | Action |
|---|---|---|---|
| 0xA3 | `EXIT` (5101) | Esc | `0x00434DD0`: free the screen (`0x0043B9A0`), back to the main menu (`0x004336C0`; part 1) |
| 0xA4 | `CREATE NEW` (10832) / `CHARACTER` (21796) | — | `0x00435E00` → `0x00435580` character create (part 3) |
| 0xA5 | `CONVERT TO` (22732) / `EXPANSION` (22730) | — | `0x00439BE0`: convert pop-up (rule 2) |
| 0xA6 | `DELETE` (5272) / `CHARACTER` (21796) | — | `0x00439D50` → `0x00439CC0`: delete pop-up (rule 1) |

The second label line is set by `0x00500BF0(button, id)` at `0x0043AEF8`–`0x0043AF18`.

1. Delete. Pop-up (`0x0042F3F0` clears the layer, then 0xD6–0xD9): `PopUpOKCancel2` at (268, 350)
   264 × 176; text 0xD9 (268, 320, 264 × 120) string 5163 "Are you sure that you want to delete this
   character? Take note: this will delete all versions of this Character."; `NO` (5167) 0xD7 at (281, 337)
   96 × 32, key Esc, closes (`0x00439D40`); `YES` (5166) 0xD8 at (421, 337), `0x0043B6B0`: busy flag on;
   `0x0043C750(name)` deletes every non-directory file matching `<folder><name>.*` (all companion files:
   `.d2s`, `.key`, `.ma*`, `.map`, …), restarting the search after each delete and stopping at the first
   failed delete; then the screen is rebuilt (`0x0043B080` → `0x0043AE30`: rescan, `sel` = `first` = 0).
2. Convert (expansion front end only). Pop-up 0xDA–0xDD, same art and buttons (`NO` → `0x00439CA0`
   close; `YES` → `0x0043B520`); text 0xDD (268, 310, 264 × 120): 22734 "Warning:  Once you convert a
   character to expansion, you cannot play it in original Diablo II games.", 22735 (empty line), 22736 "Are
   you sure you wish to continue?". YES: entry already expansion → nothing; else `0x0043CAA0` rewrites the
   file (`formats/d2s.md` §2.7 rule 2); failure → message 21872 "Unable to access file. Cannot convert
   character." then rebuild; success → entry status |= 0x20, list redrawn, pop-up closed, Convert off.

### F2.8 Difficulty box (`0x00439780`)

Opened in the pop-up layer with p (status bits 8–12) and status & 0x20:

| Id | Art / text | Position (x, bottom y), size | Key | Action |
|---|---|---|---|---|
| 0xAC | `DifficultyLevels` | 237, 400, 326 × 200 | — | backdrop |
| 0xAB | text 10019 `SELECT DIFFICULTY`, colour 7 | 264, 260, 272 × 35 | — | title |
| 0xA8 | `WideButtonBlank`, 10018 `NORMAL` | 264, 297, 272 × 35 | R | `0x00439B80` → difficulty 0 |
| 0xA9 | 10017 `NIGHTMARE` | 264, 340 | N | `0x00439BA0` → 1 |
| 0xAA | 10016 `HELL` | 264, 383 | H | `0x00439BC0` → 2 |
| 0xAD | (no art) | 900, 900, 10 × 10 (off screen) | Esc | `0x00432EE0`: close the box (`0x0042F3F0`), back to the list |

1. Normal and Nightmare are always enabled (the box only opens when Nightmare is open, §F2.6 rule 3).
   Hell is created disabled and enabled when (classic character and p ≥ 8) or p ≥ 10. These are the
   loader thresholds of `formats/d2s.md` §2.2 rule 5.4; p is the progression field of `formats/d2s.md` §2.3.
2. No hardcore difference: hardcore only matters in §F2.6 rule 2.
3. A choice (`0x00439AF0`): with an entry at `sel`, session +0x210 := difficulty, the box is closed and the
   game starts (`0x00434A00`, as §F2.6 rule 4).

### F2.9 Control records and art

Record (`0x004F93C0`): +0x00 type (2 image, 3 animation, 4 text, 6 button), +0x04 x, +0x08 y (bottom),
+0x0C w, +0x10 h, +0x14 button key / text field a, +0x18 button string id / text field b, +0x1C → cel
global, +0x20 callback, +0x24 / +0x28 / +0x2C type-specific (text: font record, flags).

| Id | Type | x, y, w × h | Art (`data\global\ui\…`) | Notes |
|---|---|---|---|---|
| 0xA1 | image | 0, 599, 800 × 600 | `CharSelect\characterselectscreenEXP` (12 frames of 256 × 256 tiles) | background |
| 0xA2 | button | 627, 572, 128 × 35 | `FrontEnd\MediumButtonBlank` (2 frames) | OK (5102), Enter |
| 0xA3 | button | 33, 572, 128 × 35 | `FrontEnd\MediumButtonBlank` | EXIT (5101), Esc |
| 0xA4 | button | 33, 528, 168 × 60 | `CharSelect\TallButtonBlank` (2 frames) | flags 0x40 |
| 0xA5 | button | 233, 528, 168 × 60 | `CharSelect\TallButtonBlank` | |
| 0xA6 | button | 433, 528, 168 × 60 | `CharSelect\TallButtonBlank` | |
| 0xA7 | text + scroll bar | 564, 457, 34 × 371 | `FrontEnd\joingamescrollbars` (6 frames) | callback `0x00439DF0` |
| 0x84–0x8B | text | §F2.4 | — | a = 76, b = 3, font `0x007089C4` |
| 0x8C–0x93 | text | §F2.4 | — | click only |
| 0x96 | image | moved | `CharSelect\charselectbox` (2 frames, 256 × 93) | key handler `0x0043A0D0` |
| 0x9C | text | 85, 78, 466 × 42 | — | font `0x007089AC` |
| 0xD6 / 0xDA | image | 268, 350, 264 × 176 | `FrontEnd\PopUpOKCancel2` (2 frames) | |
| 0xD7 / 0xDB, 0xD8 / 0xDC | button | 281 / 421, 337, 96 × 32 | `FrontEnd\CancelButtonBlank` (2 frames) | NO / YES |
| 0xA8–0xAA | button | §F2.8 | `FrontEnd\WideButtonBlank` (4 frames) | |
| 0xAC | image | §F2.8 | `CharSelect\DifficultyLevels` (2 frames, 256 × 200) | |

Art is loaded once by `0x0042E6D0` (cel globals `0x00779734` screen, `0x0077973C` medium button,
`0x00779730` tall button, `0x00779778` box, `0x0077977C` grey box, `0x0077976C` difficulty, `0x00779738`
wide button, `0x00779784` pop-up, `0x007797B4` cancel button, `0x007797CC` scroll bar); the screen palette
is set by `0x0042F2E0` with the pair at `0x006D3A08` / `0x006D3A0C`. Frame counts and sizes from the
d2data / d2exp copies (a `Patch_D2` override was not checked). Drawing: `ui/panels.md`.
Menu sounds: deferred (`client/audio.md`).

Descriptor format, factory, adders and fonts: §F1.1 (part 1). Descriptor *i* = `0x00708D10 + 0x30·i`.
For an **animated image** (type 3, factory `0x00500850`) the fields differ from a button's: [5] =
frame-speed unit (0 = no animation), [6] = base draw mode (0 → 5 normal), [8] = click callback, [9] =
anim table (0 = single cel at [7]), [10] = hover callback. Draw modes: `render/blend-modes.md` §1
(3 modulate, 5 normal).

### F3.1 Character-create screen build (`0x00435580`)

Entered from character select "Create New" (`0x00435E00`), from the main menu when no save exists
(§F1.3), and again from the hardcore warning's CANCEL (§F3.6). Steps, in order:

1. `0x004326F0` loads the create art once (guard `[0x00779724]` = 0; freed by `0x0042EF50`); table in
   §F3.8. `0x0042F480` tears down the previous screen.
2. `[0x00708D0C]` (hovered class) := −1, `[0x0070CB80]` (selected class) := −1, creation flags word
   `[+0x1EF]` of the front-end state (`[0x007795D4]`) := 0 (hardcore, expansion, ladder all off).
3. Creates descriptors (list B): 177 background, 197 class-name text → `[0x007795D8]`, 198
   description text → `[0x007795DC]`, 174 title (set to string 5127 "Select Hero Class"); multiplayer
   only (`[0x007795EC]` = 1): 199 (strings 11143 / 11144, Phase 7+). Then 175 EXIT; 200 name-box art;
   201 label (string 5125 "Character Name"); 204 name edit box (`[0x0077934C]`, §F3.4); then the
   check boxes (§F3.5); then 176 OK (`[0x00779330]`).
4. Hidden at build (`0x004F9740(ctrl, 0)`): 200, 201, 204, 176, the hardcore pair, and (expansion
   installed) the expansion label / check box / grey box; multiplayer: the ladder pair. **No name entry
   is visible until a class is selected** (§F3.3 rule 6).
5. The class line-up and the fire (§F3.2) are created last; then tail-jump to `0x00430620` (OK-enable
   check, §F3.4 rule 4 — empty name → OK disabled).

| Desc | Type | x, y (y = bottom) | w × h | Art / text | Notes |
|---|---|---|---|---|---|
| 177 | 2 image | 0, 599 | 800×600 | `[0x00779724]` `CharacterCreate` / `charactercreationscreenEXP` | 12 frames (256² tiles) |
| 174 | 4 text | 0, 80 | 800×50 | 5127, Font30 (`0x007089B4`), flags 2 | title |
| 197 | 4 text | 0, 180 | 800×100 | class name, Font30, flags 2 | §F3.3 |
| 198 | 4 text | 250, 210 | 300×100 | description, Font16 (`0x007089C4`), flags 2 | §F3.3 |
| 175 | 6 button | 33, 572 | 128×35 | 5101 EXIT, `MediumSelButtonBlank` (`[0x00779744]`), hotkey 27 | `0x00430C30` |
| 176 | 6 button | 627, 572 | 128×35 | 5102 OK, `MediumSelButtonBlank`, hotkey 13 | `0x004369F0` |
| 200 | 2 image | 319, 519 | 169×26 | `[0x007797BC]` `textbox` (1 frame) | name-box frame |
| 201 | 4 text | 321, 512 | 200×32 | 5125, Font16 | label |
| 204 | 1 edit | 318, 510 | 157×16 | font FontFormal12 (`0x007089DC`) | [5]=8, [6]=4 (text inset, not traced) |
| 178 / 179 | 3 anim | 345, 470 / 345, 454 | 110×127 | `[0x00779728]` `fire`, speed 1, draw mode 3 | 178 expansion, 179 classic |

Language 9 (`0x00525150` = 9) moves the label y's up by 6 (`0x00432EF0`: desc 201 y 506, 202/191/211
y 555, 189/193 y 575, 195 y 595); English never takes this path.

### F3.2 Class line-up (positions, creation order)

Each class is one type-3 control, 88×184, anim table [9], click `0x00433BF0`,
hover callback [10]. Created in this order (list B order = draw order, later on top):

| Layout | Created (descriptor → global) | Fire |
|---|---|---|
| Classic (`0x00408F20` = 0) | 182 Barbarian (400,330) → `[0x0077956C]`; 181 Necromancer (301,333) → `[0x00779570]`; 183 Sorceress (521,344) → `[0x00779578]`; 180 Amazon (195,341) → `[0x00779574]`; 184 Paladin (610,359) → `[0x0077957C]` | 179 (345,454) |
| Expansion (`0x00408F20` ≠ 0) | 182 Barbarian (400,330) → `[0x0077956C]`; 186 Sorceress (626,353) → `[0x00779578]`; 187 Paladin (521,339) → `[0x0077957C]`; 181 Necromancer (301,333) → `[0x00779570]`; 209 Assassin (232,364) → `[0x00779584]`; 188 Amazon (100,337) → `[0x00779574]`; 210 Druid (720,370) → `[0x00779580]` | 178 (345,470) |

Left-to-right on screen: classic Amazon, Necromancer, Barbarian, Sorceress, Paladin; expansion
Amazon, Assassin, Necromancer, Barbarian, Paladin, Sorceress, Druid. Assassin and Druid appear iff the
expansion is installed (not a per-character choice); descriptor 185 (Necromancer at 217,360) is unused.
Class ids (`[0x0070CB80]`, save +0x28): 0 Amazon, 1 Sorceress, 2 Necromancer, 3 Paladin, 4 Barbarian,
5 Druid, 6 Assassin.

### F3.3 Class animation state machine (D2Win anim control `0x00500850`)

1. **Anim table** (`0x00708A00`, 0x50 bytes per class, 5 entries of {cel global, overlay cel global or 0,
   overlay draw mode, speed}); entry index = control state `[+0x50]`:
   0 idle (`nu1`), 1 hover (`nu2`), 2 step forward (`fw`), 3 selected idle (`nu3`), 4 step back (`bw`).
   Files and frame counts in §F3.8.
2. **Timing** (draw `0x005005B0`): frame duration = speed·1000/25 ms (speed 1 = 40 ms, 2 = 80 ms,
   3 = 120 ms), from `t0` = `[+0x48]` (GetTickCount). States 0, 1, 3 loop:
   `frame = ((now − t0) / dur) mod (frames − 1)` (the last frame is never shown in a loop). States 2, 4
   play once: `frame = min((now − t0) / dur, frames − 1)`. Animation runs only when speed ≠ 0, frames > 1
   and `[+0x5C]` ≠ 0.
3. **Draw**: base cel frame `frame` at (x, y) with draw mode `[+0x54]` (5); then, if the entry has an
   overlay, the overlay cel's same frame at the same (x, y) with the entry's overlay mode (3 or 5).
   Placement of DC6 frames: `render/sprite-placement.md`.
4. **Update** (`0x00500480`, every frame before draw): in state 0 or 1 the state := 1 if the pointer
   is inside the hit box, else 0; a change calls the hover callback [10] (t0 is **not** reset, so nu1↔nu2
   keep their frame phase). State 2 → 3 and state 4 → 1 once `frame ≥ frames − 2`, resetting frame and
   t0 (4 → 1 then falls to 0 on the next update if the pointer is outside).
5. **Hit box** (`0x005003A0`, only when an anim table exists): frame 0 of the current state's cel;
   inside iff `x ≤ mx − ox < x + w` and `y − h ≤ my − oy < y` (w, h, ox, oy = DC6 frame-0 width,
   height, x / y offset, `formats/dc6.md`). So the hit box follows the art, not the 88×184 descriptor size.
6. **Click** (`0x00500790` calls [8] = `0x00433BF0`; if it returns ≠ 0: state 3 → 4, state 1 → 2, both
   resetting frame and t0; other states unchanged). `0x00433BF0`:
   1. s := clicked control's state (`0x00500310`).
   2. For every class control in creation order (5 classic / 7 expansion) `0x00500340`: state 3 → 4
      (walk back, t0 reset); if a control is in state 2 or 4 → return 0 (click ignored; controls
      already visited stay switched).
   3. prev := `[0x0070CB80]`; `[0x0070CB80]` := −1; if prev ≠ −1 stop the selection sounds (`0x00515020`).
   4. Set the clicked class's name and description (`0x004307A0`, rule 7 strings).
   5. If clicked ≠ prev and s = 1: play its select sound, selected := clicked. Else if s ∈ {2,3,4}:
      selected stays −1 (clicking the selected hero deselects it; it walks back). Else selected := clicked.
   6. Visibility: selected = −1 (and s ≠ 1) → hide 200, 201, 204, 176, the hardcore pair, the expansion
      trio; focus none (`0x004F91C0(0)`). Otherwise show 200, 201, (`[0x00779DA4]` ≠ 0) hardcore pair,
      204 (focused), 176.
   7. Expansion installed: selected ∈ {5, 6} → show expansion label, hide its check box, show the grey
      box `joingameclickboxgrey`; other class → show label and check box, hide grey box, set the box
      checked (`0x005009A0(box, 1)`). In both cases flag 0x20 (expansion) := set — **every new
      selection re-checks Expansion**.
   8. Return 1.
7. **Hover text** (hover callbacks `0x00430840` Sorceress, `0x00430870` Amazon, `0x00430920`
   Necromancer, `0x00430950` Barbarian, `0x00430980` Paladin, `0x004308E0` Druid, `0x004308A0` Assassin
   — the last two return early without expansion; worker `0x004307A0`): on entering a class while
   nothing is selected (`[0x0070CB80]` = −1): text 197 := name, 198 := description, `[0x00708D0C]` :=
   class. On leaving while nothing selected and `[0x00708D0C]` = that class: clear 197 and 198. While a
   class is selected, hovering others changes nothing.

| Class | Name id | Description id (text, string.tbl / expansionstring.tbl) |
|---|---|---|
| Amazon | 4011 "Amazon" | 5128 "Skilled with the spear and the bow, she is a very versatile fighter." |
| Sorceress | 4010 "Sorceress" | 5131 "She has mastered the elemental magicks -- fire, lightning, and ice." |
| Necromancer | 4009 "Necromancer" | 5129 "Summoning undead minions and cursing his enemies are his specialties." |
| Paladin | 4008 "Paladin" | 5132 "He is a natural party leader, holy man, and blessed warrior." |
| Barbarian | 4007 "Barbarian" | 5130 "He is unequaled in close-quarters combat and mastery of weapons." |
| Druid | 10097 | 22518 "Commanding the forces of nature, he summons wild beasts and raging storms to his side." |
| Assassin | 10098 | 22519 "Schooled in the Martial Arts, her mind and body are deadly weapons." |

PROVISIONAL: 10097 shows "Druid" and 10098 "Assassin" (because the d2exp base `patchstring.tbl` maps
10097 "Assassin" / 10098 "Druid", the reverse of the code's use, so the 1.14d `Patch_D2.mpq` table must
differ); settled by REC-208.

### F3.4 Name entry (edit box, descriptor 204)

1. Max length: `0x004FDD50(box, 0x10)` sets `[+0x48]` = 16; the edit control rejects input once
   `length > [+0x48] − 1` (`0x004FEAE0`) → **at most 15 characters**.
2. Character filter (`0x00430590`, set by `0x004FDAD0`): accept `A–Z`, `a–z`; accept `-` or `_` only if
   the filter's second argument ≠ 0 and the current text holds no `-` and no `_`; reject everything else
   (digits, space, accented letters, other punctuation). PROVISIONAL: the second argument is the caret
   position, so a separator cannot be typed as the first character (because only that reading fits the
   later first-char rule); settled by REC-209.
3. Change callback `0x00433BD0` (set by `0x004FDB00`) → `0x00430620` after every edit.
4. OK enable (`0x00430620`): OK (176) enabled iff `2 ≤ len ≤ 15`, first char ∉ {`-`,`_`}, last char ∉
   {`-`,`_`}, and count(`-`) + count(`_`) < 2; else disabled. A disabled OK ignores clicks and Enter.
5. Enter in the box runs the same action as OK (`0x004369F0`, field [9] of descriptor 204).
6. No reserved-name list exists on the single-player path; case is kept as typed.

### F3.5 Check boxes (hardcore, expansion; ladder named only)

Check box = type-6 button with flag 2 (toggle), art `clickbox` (`[0x007797C0]`, 2 frames 15×16:
unchecked / checked). Each click toggles one bit of the creation flags `[+0x1EF]`.

| Control | Classic | Expansion installed | Click | Bit |
|---|---|---|---|---|
| Hardcore box | 203 (319,540) | 190 (319,560) | `0x00430730` | 0x04 |
| Hardcore label (5126 "Hardcore", Font16) | 202 (339,561) 100×32 | 189 (339,581) 100×32 | — | — |
| Expansion box | — | 212 (319,540) | `0x00430770` (no-op without expansion) | 0x20 |
| Expansion label (22731 "EXPANSION CHARACTER") | — | 211 (339,561) | — | — |
| Expansion grey box (Assassin/Druid) | — | 213 (319,540) image `joingameclickboxgrey` | — | — |
| Ladder box / label (multiplayer, Phase 7+) | 191–196, 2-row variants | same | `0x00430750` | 0x40 |

1. Defaults: hardcore unchecked (flags := 0 at build); expansion checked on each class selection
   (§F3.3 rule 6.7); Assassin/Druid cannot clear it (grey box, and `0x004365B0` forces 0x20 for classes
   5/6).
2. The hardcore pair exists only when `[0x00779DA4]` ≠ 0 (else created disabled and never shown).
   `[0x00779DA4]` is set to 1 unconditionally by character select (`0x0043B080`) and the other entry
   paths (`0x00438AD0`, `0x0043AE30`, `0x0043B9A0`, `0x0043BF60`). Hardcore is always offered in 1.14d
   single player: the xref scan of `[0x00779DA4]` (2026-10-08) finds five writers, all storing 1
   (`0x00438E51`, `0x0043AE4F`, `0x0043B113`, `0x0043B9B0`, `0x0043BFA6`), and three readers
   (`0x00433EF3`, `0x004357A5`–`0x00435869`); no unlock condition exists. REC-211 only verifies.

### F3.6 OK / Cancel behaviour and the new save

1. **EXIT / Esc** (`0x00430C30`): stop sounds, tear down, character select (`0x0043B080`). Nothing written.
2. **OK / Enter** (`0x004369F0`), only while OK is enabled:
   1. Free space of the save drive < 5000 bytes (GetDiskFreeSpaceA) → popup 5149 "There is not enough
      room on your Hard Disk. Please free up space and then try again." (`0x00433460`); stay.
   2. OK disabled; `0x004365B0`: `<save dir><name>.d2s` exists (`0x00431F70`, OPEN_EXISTING; Windows
      file names are case-insensitive) and not Battle.net → popup 5165 "That character name is already
      taken. Try another."; stay (OK re-enables on the next edit).
   3. Class 5 or 6 → flag 0x20 forced. Name copied to the state (+0xBD).
   4. Hardcore bit set → warning popup `0x00430520` (list A): 205 image (268,350) 264×176
      `[0x00779780]`, 206 CANCEL (281,337) 96×32 → `0x00436590`, 207 OK (421,337) → `0x00436360`,
      208 text (268,320) 264×120 Font16 = 5303 "WARNING: Once a Hardcore character dies, it cannot be
      played again. Are you sure you wish to create a Hardcore character?". CANCEL pops the popup and
      **rebuilds the whole create screen** (`0x00435580`: selection, name and flags lost). OK continues
      with step 5.
   5. Single player (`[0x007795EC]` = 0): tear down, fill the stub (`0x0043C540`), write it
      (`0x0043C6B0`), set the game-start code: `[+0x209]` = 4, | 0x800 hardcore, | 0x100000 expansion;
      leave the front-end loop (`0x004F9190`) → game load (`client/model.md` §7 r9). Modes 2 / 3 (Open
      Battle.net / TCP/IP) also write the stub; Battle.net (1) does not: Phase 7+.
3. **The stub save**: file `"%s%s.d2s"` = save dir (`0x00407050`) + name as typed, created with
   CREATE_ALWAYS, exactly 0x14F = 335 bytes, written once, at OK (after the hardcore confirmation).
   Layout and the fields it sets (status bit 0 "new", class, level 1, flags 0x04 / 0x20): `formats/d2s.md`
   §2.6. Loading it and creating the start items / StartSkill: `formats/d2s-load.md`, `items/generation.md`
   §10.3. The front end writes nothing else.

### F3.7 Sounds (deferred)

Selection start / stop goes through `0x00514EC0` / `0x00515020` per class; files are not named in
these functions (sound ids in registers). `client/audio.md`.

### F3.8 Art (`0x004326F0`, all `data\global\ui\FrontEnd\…`; frames from the 1.14d MPQs, 1 direction)

| Global | File | Frames | Frame 0 w×h, offset | Archive |
|---|---|---|---|---|
| `0x00779744` | `MediumSelButtonBlank` | 2 | 128×35 | d2data |
| `0x00779724` | `CharacterCreate` / `charactercreationscreenEXP` | 12 | 256² tiles | d2data / d2exp |
| `0x00779728` | `fire` | 30 | 110×127, (−34, 132) | d2data |
| `0x007797BC` | `textbox` | 1 | 169×26 | d2data |
| `0x007797C0` | `clickbox` | 2 | 15×16 | d2data |
| `0x007797C4` | `joingameclickboxgrey` (expansion only) | 1 | 15×16 | d2exp |
| `0x00779780` | hardcore popup background (loaded by `0x0042E6D0`, part 1) | — | 264×176 | — |

Class files (table `0x00708A00`; each row nu1, nu2, fw, nu3, bw; "spd" = speed units of 40 ms;
overlay "+file mode"):

| Class (table) | nu1 | nu2 | fw | nu3 | bw |
|---|---|---|---|---|---|
| Assassin `0x00708A00` (expansion only) | `assassin\asnu1` 31f spd 3 | `asnu2` 31f spd 3 | `asfw` 91f spd 1 | `asnu3` 31f spd 2 | `asbw` 51f spd 1 |
| Druid `0x00708A50` (expansion only) | `druid\dznu1` 21f spd 2 | `dznu2` 21f spd 2 | `dzfw` 121f spd 1 | `dznu3` 21f spd 2 | `dzbw` 40f spd 1 |
| Amazon `0x00708AA0` | `amazon\amnu1` 26f spd 3 | `amnu2` 26f spd 3 | `amfw` 54f spd 1 | `amnu3` 18f spd 2 | `ambw` 30f spd 1 |
| Necromancer `0x00708AF0` | `necromancer\nenu1` 12f spd 3 | `nenu2` 12f spd 3 | `nefw` 38f spd 1 +`nefws` mode 3 | `nenu3` 12f spd 2 +`nenu3s` mode 3 | `nebw` 28f spd 1 +`nebws` mode 3 |
| Barbarian `0x00708B40` | `barbarian\banu1` 16f spd 2 | `banu2` 16f spd 2 | `bafw` 64f spd 1 +`bafws` mode 5 | `banu3` 26f spd 1 | `babw` 19f spd 1 |
| Sorceress `0x00708B90` | `sorceress\sonu1` 32f spd 2 | `sonu2` 32f spd 2 | `sofw` 52f spd 1 +`sofws` mode 3 | `sonu3` 12f spd 1 +`sonu3s` mode 3 | `sobw` 30f spd 1 +`sobws` mode 3 |
| Paladin `0x00708BE0` | `paladin\panu1` 26f spd 2 | `panu2` 26f spd 2 | `pafw` 80f spd 1 +`pafws` mode 5 | `panu3` 9f spd 2 | `pabw` 40f spd 1 |

Overlay frame counts equal their base file's. `amazon\AMFWs.DC6` exists in d2data but 1.14d never
loads it. Assassin / Druid files load only when the expansion is installed (their controls are not
created otherwise).

## Constants & data dependencies

- Descriptor table `0x00708D10` (0x30 stride), anim tables `0x00708C30` /
  `0x00708C80`, font records `0x007089AC`–`0x007089EC`, version format
  `0x006D48D4`, save mask `0x006D41D4`.
- Strings (ENG, 1.14d `Patch_D2` patchstring): 5101 EXIT, 5102 OK, 5103
  CANCEL, 5106–5111 as above, 5115 OPEN BATTLE.NET, 5116 TCP/IP GAME, 10016
  HELL, 10017 NIGHTMARE, 10018 NORMAL, 10832 CREATE NEW, 5272 DELETE, 22732
  CONVERT TO. (The d2exp copy of patchstring.tbl gives other texts for
  10016–10018; the Patch_D2 copy wins.)
- Difficulty buttons (descriptors 168–170, WideButtonBlank, x 264, w×h
  272×35): Normal y 297 hotkey 'R' (82), Nightmare y 340 'N' (78), Hell y
  383 'H' (72); background 172 `DifficultyLevels` (237,400) 326×200.
- Character select OK / Exit: (627,572) / (33,572) 128×35 MediumButtonBlank;
  Create New (33,528) 168×60 TallButtonBlank.

| Constant | Value | Source |
|---|---|---|
| visible slots | 8 (2 × 4) | `0x0043AF6F` |
| row bottom y | 178 + 93 × row | `0x0043AFCA` |
| column x (text / figure / box) | 37, 309 / 237, 509 / 37, 309 | control table 0x84–0x93, `0x004390EE` |
| double click | 500 ms | `0x0043A9D0` |
| name length | 2..15 | `0x00439021` |
| header read | ≤ 0x2000 bytes | `0x0043C8A0` |
| strings | 5101, 5102, 5163, 5166, 5167, 5272, 5304, 10016–10019, 10832, 21796, 21872, 22730–22736 | `string.tbl`, 1.14d `Patch_D2` `patchstring.tbl` (ids 10016–10019 and 10832 are one higher than in the d2exp copy), `expansionstring.tbl` |

| Constant | Value | Source |
|---|---|---|
| Descriptors (create screen) | 174–213 | `0x00708D10` table |
| Anim tables | 7 × 0x50 bytes at `0x00708A00` (order as, dz, am, ne, ba, so, pa) | data |
| Frame duration unit | 1000/25 = 40 ms | `0x005005B0` |
| Name length | 2..15 | `0x00430620`, `0x004FEAE0` |
| Free-space minimum | 5000 bytes | `0x004369F0`, `0x00434E00` |
| Stub size | 0x14F | `0x0043C6B0` |
| Creation flag bits `[+0x1EF]` | 0x04 hardcore, 0x20 expansion, 0x40 ladder | `0x00430730/50/70` |
| Strings | 4007–4011, 5101–5103, 5125–5132, 5149, 5165, 5303, 10097, 10098, 22518, 22519, 22731 | string / patchstring / expansionstring .tbl |

## Randomness

None. The front end draws no game-seed values; its animations run on wall-clock time (Inputs).

## Edge cases & original bugs

1. Logo frame 29 is never displayed (`mod (frames−1)`, `0x005005B0`).
2. Timers fire on whole-second boundaries of GetTickCount/1000, so the
   Blizzard screen would last between 8 and 9 s depending on the start phase (§F1.1 r7; the screen
   itself is not built in 1.14d, §F1.3 note).
3. Single Player with no `.d2s` in the save directory goes straight to
   character create; Esc there lands on an empty character select, not the
   main menu (`0x00430C30`).
4. The difficulty popup's Esc is an off-screen button (900,900); clicking
   cannot reach it, only the hotkey.

1. Two files whose names differ only after the first `.` (`a.d2s`, `a.x.d2s`) both load `a.d2s`: two
   identical entries.
2. The name shown is the file name: a renamed `.d2s` shows the new name, and the game start passes that
   name (`0x00434A00`), not header +0x14.
3. End with n > 8 and n odd sets an odd `first`: rows then pair entries across the normal column split;
   the box column still follows `sel` parity relative to slot index (`(sel − first) & 1` in `0x004390A0`).
4. Clicking the slot right after the last entry selects an empty index (k = n): no box text, buttons off.
5. Delete matches `<name>.*`: a different character named `<name>` with a longer file name is not touched,
   but any non-save file named `<name>.<anything>` is deleted.
6. A dead hardcore character can still be deleted; it cannot be converted only if already expansion.

1. Clicking any hero while one is walking forward or back is ignored entirely (§F3.3 rule 6.2).
2. Selecting a second hero while one is selected: the first walks back, the second forward, in one click.
3. Clicking the selected hero: it walks back, nothing selected, name entry hidden (typed name kept in
   the hidden box).
4. Unchecking Expansion then clicking another hero re-checks it.
5. Name "a-b_c" can't be typed (second separator rejected); "ab-" types but OK stays disabled.
6. A same-named save in a different case ("BOB" vs "bob") counts as taken (file system).
7. Hardcore CANCEL resets the whole screen, not just the popup.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| logo created at t=0, now = 1,000 ms | frame 25 | F1.5 r3 |
| now = 1,160 ms | frame (29 mod 29) = 0 | F1.5 r3 |
| mouse (264, 289) / (264, 324) on Single Player (y 324, h 35) | inside / outside | F1.1 r3 |
| Wide button 272×35 | tiles 2: up frames 0–1, pressed 2–3 | F1.1 r4 |
| version text | "v 1.14d" | F1.4 r4 |
| character select, 11 saves (bar range 2), `first` 0, bar 0; wheel delta −120 three times | `first` 2, 4, 4 (bar 1, 2, 2); `sel` unchanged | F2.5 r5 |
| then wheel delta +120; then +60 | `first` 2 (bar 1); then no change (k = 0) | F2.5 r5 |
| character select, 8 saves, any wheel | nothing (no range) | F2.5 r5 |
| button h = 25 | font 10, k = 2 | F1.1 r5 |

| Input | Expected |
|---|---|
| Save folder with `Bob.d2s` (written last), `Al.d2s`, `_x.d2s`, `A.d2s`, `toolongname1234567.d2s`, `Ann-.d2s` | list: Bob, Al; `sel` = 0 (Bob) |
| expansion char p = 5, OK | difficulty box; Normal, Nightmare on; Hell off |
| classic char p = 8, OK | box; Hell on |
| expansion char p = 4, OK | game starts at Normal, no box |
| status 0x0C, OK | message 5304 |
| n = 11, Down from `sel` = 7 (first 0) | `sel` = 9, `first` = 2 |
| n = 11, End | `sel` = 10, `first` = 3 |

| Input | Expected |
|---|---|
| Expansion, nothing hovered | 7 heroes idle, fire at (345,470); no name box, no OK |
| Hover Paladin (none selected) | 197 "Paladin", 198 = 5132; Paladin plays `panu2` at the same phase |
| Click Paladin | `pafw` 80 frames × 40 ms then `panu3` loop; name box, Hardcore box (319,560) unchecked, Expansion box (319,540) checked appear; OK disabled |
| Type "Zz" | OK enabled; "Z" → disabled; "Zz-" → disabled |
| Type "7" or space | rejected |
| OK with hardcore checked | popup 5303; OK → `<save>\Zz.d2s` 335 bytes written, game load |
| Click Druid | grey box instead of Expansion box; flag 0x20 set |

## Provenance

- Descriptor table and fields: bytes of `0x00708D10`… read from the 1.14d
  image; factory `0x004F93C0`; ctors `0x00500850`, `0x00501290`,
  `0x004FC7A0`, `0x004FD9D0`; draws `0x005005B0`, `0x00500DB0`, `0x00500C50`,
  `0x004FD860`.
- Screens: `0x004336C0`, `0x0043C4F0`, `0x0042FB20`, `0x00434D90`,
  `0x00434DC0`, `0x00434DD0`, `0x00435CD0`, `0x00430BC0`, `0x00430C30`,
  `0x00431C30`, `0x00432EE0`, `0x00439840`, `0x00439780`, `0x00439B80`;
  list adders `0x0042F3A0` / `0x0042F430`.
- Preload `0x0042E6D0` / `0x004326F0`; frame counts and offsets from the
  DC6 headers in the MPQs; strings from `string.tbl`, Patch_D2
  `patchstring.tbl`, `expansionstring.tbl`; sounds from Sounds.txt.
- Riiablo `MenuScreen.java` used as a hint only (it agrees: black + fire
  layers, fire blended).

1.14d `Game.exe` (Ghidra exports + `tools/ghidra/disasm.py`, 2026-10-08): save folder `0x00407050`,
`0x00406D30`, `0x004067D0`, `0x00414930`, `0x00414990`, `0x00414B00`; scan `0x00438F70`; header read
`0x0043C8A0`; entry `0x00438AD0`, `0x00437D20`; screen build `0x0043AE30`, `0x0043ADB0`; list draw
`0x004380F0`; selection `0x004390A0`; click `0x0043A9D0`; keys `0x00439E90`; scroll `0x00439DF0`; OK
`0x00439840`; difficulty `0x00439780`, `0x00439AF0`, `0x00439B80`–`0x00439BC0`, `0x00432EE0`; delete
`0x00439CC0`, `0x0043B6B0`, `0x0043C750`, `0x0043B080`; convert `0x00439BE0`, `0x0043B520`; exit
`0x00434DD0`; create `0x00435E00`; game start `0x00434A00`; control table `0x00708D10` read from the image
(ids 0x84–0xAD, 0xD6–0xDD), record use `0x004F93C0`; art `0x0042E6D0`. Strings: `string.tbl` (d2data),
`patchstring.tbl` (Patch_D2, read with a PKWARE-explode MPQ reader in the scratchpad), `expansionstring.tbl`
(d2exp). DC6 headers from d2data / d2exp. D2MOO not used.

`0x00435580` (build), `0x004326F0` (art), `0x00432EF0` (lang-9 offsets), `0x00433BF0` (class click),
`0x004307A0` + hover callbacks `0x00430840–0x00430980`, `0x00431E70` (class byte +0x1EE),
D2Win anim control `0x00500850` / `0x005005B0` / `0x00500480` / `0x005003A0` / `0x00500790` /
`0x00500340` / `0x00500310`, edit box `0x005001D0` / `0x004FDD50` / `0x004FEAE0`, `0x00430590`
(filter), `0x00430620` (OK enable), check boxes `0x00430730/50/70`, `0x005009A0`, OK `0x004369F0` →
`0x004365B0`, `0x00431F70`, `0x00430520`, `0x00436360`, `0x00436590`, stub `0x0043C540` / `0x0043C6B0`,
`[0x00779DA4]` writers listed in §F3.5. Data: descriptors 174–213 at `0x00708D10`, anim tables
`0x00708A00`. Frame counts / sizes: DC6 headers of the files in d2data.mpq / d2exp.mpq (read with
`tools/mpq-tool` into a scratch dir). Strings: ENG `string.tbl` (d2data), `expansionstring.tbl` and
`patchstring.tbl` (d2exp). Hints only: riiablo `CreateCharacterScreen` / `CharacterCreateButton`.

## Open questions

- Wheel on front-end buttons (§F2.5 r5): which D2Win buttons carry flag 8 / 0x10 at +0x40 (set by
  `0x00501290` from the descriptor or later); a read of the button constructor and its setters
  settles it.
- **REC-200** Game exit target. Capture: in single player, Save and Exit
  from a game. Steps: hook `0x0044B8A0` return and the next screen builder
  called (`0x0043B080` vs `0x004336C0`). Settles: which screen return
  value 4 maps to (expected character select).
- **REC-201** Control draw order. Capture: break in the D2Win list draw
  while the main menu shows; log the control order (types 2, 3, 3, 6…).
  Settles: creation order = draw order (logo over background, buttons
  over logo).
- **REC-203** Character-select OK condition. Capture: press OK with a new
  character and with one that finished Normal; log whether `0x00439780`
  or `0x00434A00` runs and the session values compared at
  `0x00439A79`–`0x00439A97`. Settles: when the difficulty popup appears
  (owned by part 2 if it covers character select).

- **REC-204** Mouse wheel on character select. Capture: 1.14d, ≥ 11 characters, wheel up / down over the list
  and over the scroll bar; log `[0x00779DC8]` and `[0x0070CC00]` per event (hook `0x00439DF0`, `0x00439E90`).
  Verifies §F2.5 r5 (settled from the binary 2026-10-08: one notch = scroll bar ±1 = `first` ∓ 2).
- **REC-205** Dead hardcore figure. Capture: a hardcore save with status 0x0C (male and female class);
  screenshot the slot; hook `0x005066C0` args. Settles: what class' 8 / 9 draws (token, mode, palette) and
  the draw flags from `0x006CE278`.
- **REC-206** Legacy `Save Path` migration. Capture: registry with `Save Path` → a folder with saves, no
  `NewSavePath`; start the game; read `NewSavePath` after; repeat with an empty folder. Hook `0x00406DE0`
  return. Settles: which folder is kept.
- **REC-207** Slot level line text. Capture: screenshot one slot per class; or read the string id pushed
  to the `D2Lang_GetStringByIndex` call after `" %d "` in `0x004380F0` (`disasm.py fn 0x004380F0`). Settles: the
  exact level / class line text and colour.

- **REC-208** — class names 10097 / 10098. Capture: extract `data\local\lng\eng\patchstring.tbl` from
  `Patch_D2.mpq` (by name; the archive has no listfile) or, in the 1.14d game with expansion, hover the
  Druid and the Assassin on the create screen and screenshot text 197. Settles which id shows which name.
- **REC-209** — filter argument 2. Capture: on the create screen, select a class, type `-` into the empty
  box, then `a-`; hook `0x00430590` and log its 3 arguments. Settles whether a leading separator is
  rejected (arg = caret position) or the argument means something else.
- **REC-210** — animation timing. Capture: record the create screen at ≥ 50 fps; click the Paladin;
  count frames from click to `panu3` start (expect 79 × 40 ms ≈ 3.2 s) and check that loops skip the
  last frame. Settles §F3.3 rule 2/4 as implemented.
- **REC-211** — hardcore availability. Capture: fresh install, empty save folder, Single Player → create
  screen, select any class; check the Hardcore box is shown (and log `[0x00779DA4]` at `0x00435580`).
  Verifies §F3.5 rule 2 (settled from the binary 2026-10-08: always offered).
