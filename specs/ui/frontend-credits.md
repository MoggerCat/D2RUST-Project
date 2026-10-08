# Spec: UI — Front-end videos, credits and cinematics (startup videos, credits screen, cinematics menu)

- **Status:** draft (2026-10-08, RE on the 1.14d `Game.exe` merged front end; credits files, DC6 headers and
  video file lists read from `d2data.mpq`, `d2exp.mpq`, `d2video.mpq`, `D2XVIDEO.MPQ`; no capture yet). Points
  that need a capture are PROVISIONAL with a REC id (METHODS M22).
- **Target version:** 1.14d, English install
- **Crate/module:** `d2-client::ui::front_end::credits`
- **Related specs:** `ui/frontend-menus.md` (§F1.1 descriptors and buttons, §F1.3 flow, §F1.4 main menu, §F1.5
  logo, §F1.6 palette and sounds), `ui/text.md` (§5 colours, §6 widths, §7 draw call), `ui/text-fonts.tsv`
  (FontFormal10, Font24), `ui/frontend-loading.md` L10 (in-game act videos: this spec owns their playback,
  skip and progress byte), `render/blend-modes.md` §1 (draw modes), `formats/dc6.md`, `client/assets.md`
  (archives), `client/audio.md` (sounds, deferred).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 40–50 |
| Inputs | 51–62 |
| Outputs / state changes | 63–71 |
| Rules | 72–73 |
|   C0 Front-end tick | 74–81 |
|   C1 Startup sequence (`0x004359D0` → `0x00435230`) | 82–116 |
|   C2 Trademark screen input (adds to `ui/frontend-menus.md` §F1.3) | 117–129 |
|   C3 Credits screen (`0x004312C0`) | 130–180 |
|   C4 Credits text file and parsing (`0x00430CD0`, `0x00431050`, `0x00430EF0`, `0x00430C80`) | 181–223 |
|   C5 Video hook (all videos; stub in d2rs) | 224–254 |
|   C6 Cinematics progress byte N | 255–273 |
|   C7 Palette | 274–280 |
|   C8 Cinematics menu (`0x00431600`) | 281–320 |
|   C9 Sounds (deferred; owner `client/audio.md`) | 321–327 |
|   C10 640 × 480 | 328–334 |
| Constants & data dependencies | 335–352 |
| Randomness | 353–356 |
| Edge cases & original bugs | 357–374 |
| Test vectors | 375–394 |
| Provenance | 395–417 |
| Open questions | 418–433 |
<!-- /index -->

## Summary

What the front end shows around the main menu besides the menus of `ui/frontend-menus.md`: the startup video
chain (two Blizzard logo videos, the classic intro on the first run, the expansion intro once for an old
progress value) ending on the trademark screen; the Credits screen (a three-column text list read from
`Credits.txt` / `ExpansionCredits.txt` that scrolls up over a full-screen picture); and the Cinematics menu
(5 classic / 7 expansion buttons, enabled by a progress byte the game stores in the registry, each playing
one video and returning to the menu). Video playback is a stub in d2rs: every hook logs the video path and
returns at once, keeping every side effect of the original caller. Timings are in front-end ticks (C0).
Sounds are named only.

## Inputs

| Input | Source |
|---|---|
| Art: `CharSelect\creditsbckg` / `creditsbckgexpand`, `FrontEnd\CinematicsSelection` / `CinematicsSelectionEXP`, `FrontEnd\WideButtonBlank`, `FrontEnd\MediumButtonBlank`, title screen and logo cels | the user's MPQs (`client/assets.md`), preloaded by `0x0042E6D0` |
| Credits text `DATA\LOCAL\UI\ENG\Credits.txt` (d2data) / `ExpansionCredits.txt` (d2exp) | the user's MPQs (C4) |
| Strings 5101, 5103, 5113, 5114, 4137–4140, 21797–21803 | `.tbl` tables (`ui/text.md` §2) |
| Cinematics progress byte N | original: registry value `Aux Battle.net`; d2rs: client settings (C6) |
| Expansion installed (`0x00408F20` ≠ 0) | `client/model.md` Inputs `expansion_installed` |
| Pointer and keys | client input edge (`ui/controls.md`) |
| Front-end tick (40 ms) | C0 |

## Outputs / state changes

| Output | Where |
|---|---|
| Screen drawn each tick (800 × 600 frame) | front-end draw pass |
| One log line per video request (stub) | C5 rule 6 |
| Progress byte N written (first run default, startup expansion intro, in-game video requests) | C6 |
| Return to the main menu (`0x004336C0`) or to the cinematics menu after a video | C3, C8 |

## Rules

### C0 Front-end tick

1. The front-end loop (`0x004FA590`) keeps a time budget: whenever it is ≤ 0 it adds 40 ms and runs one
   update + draw of all control lists (`0x004F98E0`); a debt larger than 1,000 ms is dropped; between
   passes it sleeps at most 20 ms. One pass = one **front-end tick** = 40 ms (25 per second).
2. d2rs: the front end runs on a fixed 40 ms tick. Controls whose timing reads GetTickCount (timers
   `ui/frontend-menus.md` §F1.1 r7, animations §F1.5) keep their ms rules, sampled once per tick.

### C1 Startup sequence (`0x004359D0` → `0x00435230`)

1. On the first entry of the front end in a process (front-end state byte `[0x007795D4]+0x35F` = 0) and
   without the "Skip To Open" option (`+0x35D` = 0; that path is Phase 7+, named only), the entry runs
   `0x00435230` once and sets `+0x35F` := 1. Every later entry (back from a game) goes straight to the main
   menu.
2. `0x00435230`, in order:
   1. `0x004FA410(0)`, tear down (`0x0042F480`), input flush (C5 r4).
   2. If the video backend reports no video (`0x004F9050` = 0) → step 6.
   3. Play `Data\Local\Video\New_BLIZ640x480.bik`, then `Data\Local\Video\BlizNorth640x480.bik` (mode 0).
      The `…640x240.bik` pair (mode 2) is chosen only when `0x0040EB10` ≠ 0; in 1.14d it always returns 0.
   4. Progress byte missing (`0x0042FA40` writes the default, C6 r1) = **first run**: if `d2video.mpq`
      opens (Storm open, priority 100), play the classic intro `d2intro` (`0x00433640`, C5) and close the
      archive. Step 5 is skipped on the first run.
   5. Byte present, expansion installed and N has bit 0x10 but not bit 0x80 (`0x0042FAB0`) → play the
      expansion intro (`0x004334E0`, C5 r1) and write N |= 0x80. No 1.14d writer produces that value
      (C6 r3), so this only fires for a value left by an older install.
   6. `0x004F9060(1)` and the trademark screen `0x0042FB20` (`ui/frontend-menus.md` §F1.3). On the first run
      the trademark screen is built twice (at the end of `0x00433640`, then again here): the second build
      replaces the first, and its 9 s timer counts from the second build.
3. 1.14d shows no Blizzard-logo DC6 screen; the rows "start → Blizzard logo" and "Blizzard
   logo → trademark" of `ui/frontend-menus.md` §F1.3 (descriptors 0 `blizno` + 1 timer 8 s, callback
   `0x00434D90`) are dead data, and the logo is the two videos of step 3 (because no caller of the adders
   `0x0042F430` / `0x0042F3A0` passes 0 or 1, and `0x00434D90` is referenced only by those two
   descriptors; confirmed by the call-site scan of `ui/frontend-menus.md` §F1.3, 2026-10-08). REC-229
   only verifies.
4. Order in d2rs (stub, C5 r6), expansion installed:

| Case | Log lines (in order) | N after |
|---|---|---|
| first run (no N) | `Data\Local\Video\New_BLIZ640x480.bik`, `Data\Local\Video\BlizNorth640x480.bik`, `DATA\LOCAL\video\ENG\d2intro640x292.bik` | 0x22 |
| N = 0x32 | the two logo videos, `DATA\LOCAL\video\ENG\D2x_Intro_640x292.bik` | 0xB2 |
| N = 0x22 (or any other) | the two logo videos | unchanged |
| return from a game | none (main menu) | unchanged |

### C2 Trademark screen input (adds to `ui/frontend-menus.md` §F1.3)

1. Descriptor 2 (the trademark image) has key handler [10] = `0x00434DC0` (main menu). The D2Win key
   router (`0x004F9BF0`, key table `0x0072DCA8`) hands it the key-down of: Backspace, Tab, Enter, Space,
   Page Up, Page Down, End, Home, ←, ↑, →, ↓, Delete, H, N, R, F1, and Esc (`0x004FA760` falls through to
   the router when no control holds Esc as hotkey). Each goes to the main menu at once. Other keys
   (other letters, digits, F2–F12, Shift / Ctrl / Alt alone) do nothing.
2. The timer (descriptor 3) also takes Space key-down (`0x004FD970`) and runs its callback, the same
   `0x00434DC0`.
3. A left click on the image (anywhere in 800 × 600) → main menu (§F1.3).
4. The timer fires on the first tick with ⌊now/1000⌋ − ⌊t0/1000⌋ > 9 (whole seconds of GetTickCount,
   §F1.1 r7): 9.0 to 10.0 s after the build, ticks 226 to 250.

### C3 Credits screen (`0x004312C0`)

1. Build order: tear down; palette (C7); descriptor 41 background; 42 EXIT; the three text columns A, B, C
   (classic 43, 44, 45; expansion 47, 48, 49); 46 timer; load the text (C4); set the top row of C, A and B
   to 20 (`0x004FBA30`). The timer is switched to periodic mode (`0x004FDA50`: callback every ≥ 50 ms) and
   its Space handler removed (`0x004FD830`).

| Desc | Type | x, y (y = bottom) | w × h | Art / text | Notes |
|---|---|---|---|---|---|
| 41 | 2 image | 0, 599 | 800×600 | `[0x00779720]` `CharSelect\creditsbckg` (classic) / `creditsbckgexpand` (expansion), 12 frames: 256² tiles, 4 × 3 (widths 256, 256, 256, 32; heights 256, 256, 88) | no click, no key handler |
| 42 | 6 button | 33, 578 | 128×35 | 5101 `EXIT`, `FrontEnd\MediumButtonBlank` (`[0x0077973C]`, 2 frames), hotkey 27 | `0x00435020` → main menu |
| 43 / 47 | 4 text | 560 / 400, 615 | 250×610 | column A, FontFormal10 (`0x007089D4`), flags 0x90 | right-aligned, scrolls |
| 44 / 48 | 4 text | 570 / 410, 615 | 250×610 | column B, same font, flags 0x80 | left-aligned, scrolls |
| 45 / 49 | 4 text | 440 / 280, 615 | 250×610 | column C, same font, flags 0x82 | centred, no wrap, scrolls |
| 46 | 8 timer | [1] = 50 | — | — | `0x004341F0` every ≥ 50 ms |

2. Column draw (`0x004FBF30`), per tick, for each column:
   - row pitch P = font height 15 + line gap `[0x0072DE90]` = 4 (English) = **19 px**;
   - row k (k = 0, 1, …, counted from the top row) has baseline y = (615 − 610) + 15 + 19·k + `off` =
     20 + 19·k + `off`; rows are drawn while 610 − 19·k ≥ 19, so **32 rows** (k = 0…31; baselines 20…609
     at `off` 0); rows below y 600 fall off the screen;
   - x: column A: pen x = x − width A of the row (`ui/text.md` §6): the text ends at x (560 / 400). Column
     B: pen x = x (570 / 410). Column C: pen x = x + max(0, ⌊(250 − width A)/2⌋): centred in x … x + 250
     (440–690 classic, 280–530 expansion; centre 565 / 405); the centring offset uses the whole row's
     width A and is never negative. The general text-control layout (margins, alignment flags, row
     clipping, wrap) is `ui/frontend-menus.md` §F1.1 r8; for these columns the margins are 0;
   - clip (`0x004FBEE0`): each row is cut to its longest prefix whose width B is < 250 px before it is
     drawn; after the C4 r5 wrap no shipped row is cut;
   - colour = the row's k (C4 r4); draw call `ui/text.md` §7 (not centred).
3. Scroll (flag 0x80, inside the draw): first `off` −= 2; if `off` < −19 then `off` := 0 and the top row
   := the next row. Then the rows are drawn. So after draw t (t = 1, 2, …): top row = 20 + ⌊t/10⌋, `off` =
   −2·(t mod 10); a row rises 19 px per 10 ticks (nine 2-px steps, then a 1-px step): 47.5 px/s. When the
   top row has no next row the column draws nothing more and stops scrolling. The three columns hold the
   same number of rows above every text row (C4 r3), so they stay in step.
4. Entry state: top row 20, `off` 0; rows 0–49 are blank, so the first text row (50) starts at baseline
   20 + 19·30 = 590 and reaches the top baseline (20) after 300 ticks (12 s).
5. Leave: EXIT click or Esc (hotkey 27) → `0x00435020` → main menu (`0x004336C0`, which rebuilds it,
   §F1.4). A click on the picture or the text does nothing; Space does nothing (timer key handler removed);
   no other key is handled.
6. End: the timer callback (`0x004341F0`; with 40 ms ticks it runs on every second tick) goes to the main
   menu iff `[0x0077996C]` ≤ (top index of C) + 10. `[0x0077996C]` is the top index of C at the end of
   loading (row count − visible rows, `0x004FBAB0`, the index the list's auto-follow reached while rows
   were added); the top index is set to 20 at entry and the scroll moves only the top-row pointer, never
   the index. So a credits list longer than about 62 rows never returns by itself: after the last row
   ("The End") leaves the top, the picture and EXIT stay until the player leaves. PROVISIONAL: no
   automatic return with the shipped files (because the scroll never updates the index the check reads);
   settled by REC-225. A list of ≤ ~62 rows in C returns on the first timer call (edge case 4).
7. With the shipped English files (C4 r6): "The End" (C row R) is the top row from draw (R − 20)·10 and
   disappears at draw (R − 19)·10: classic R = 1,232 → draw 12,130 (485.2 s); expansion R = 1,800 → draw
   17,810 (712.4 s).

### C4 Credits text file and parsing (`0x00430CD0`, `0x00431050`, `0x00430EF0`, `0x00430C80`)

1. File: `%s\UI\%s\%s` (`0x006D41E4`) with `DATA\LOCAL`, the language folder (`0x00525260`, `ENG`) and
   `ExpansionCredits.txt` (expansion installed) or `Credits.txt` (classic), read whole through the archive
   layer (`0x004068E0`). 1.14d English copies: `data\local\ui\eng\Credits.txt` in d2data.mpq (24,718 bytes,
   8-bit text, CR LF, 1,619 lines + the empty piece after the last CR LF) and
   `data\local\UI\ENG\ExpansionCredits.txt` in d2exp.mpq (66,498 bytes, UTF-16LE with BOM, CR LF, 2,136
   lines). Patch_D2.mpq holds neither; `MacCredits.txt` (d2data) is never opened.
2. Decoding (`0x00430CD0`): ≤ 2 bytes → no text; first u16 0xFEFF → UTF-16LE, BOM dropped; 0xFFFE →
   UTF-16 with the bytes of every unit swapped (`0x0042E5D0`); otherwise every byte widened to one unit
   (`0x00526F20`). Every CR and LF unit then becomes NUL. No text (missing or ≤ 2-byte file) is a fatal
   assertion (`0x00431050`, `MainMenus.cpp`); d2rs: a fatal error naming the path.
3. Parse: a line = the units up to the next NUL; the walk always steps over the line plus 2 units (it
   assumes CR LF) and stops when the remaining unit count reaches 0. Before the first line each column gets
   50 blank rows (order C, A, B per row). Then:
   - **Heading**: a line starting with `*` → A += blank, B += blank, C += the line without `*` (colour 1);
     then balance.
   - **Block** (any other line, until the next `*` line or the end), entry by entry: the entry's first
     line L1 must start with an ASCII letter (unit < 0x80 and `isalpha`), **else the whole parse stops**
     (the rest of the file is dropped; C += blank, blank). L2 = the next line if any remains and it does not
     start with `*` (blank or `(`… allowed), else none.
     - L1 and L2 both non-empty → A += L1, B += L2, C += blank (colour 4).
     - else (no L2, or L2 blank) → C += L1 (colour 4), C += blank.
     After the block: balance.
   - **Balance** (`0x00430EF0`): pad the shorter columns with blank rows up to the longest, then add one
     blank row to each.
   After the walk (`0x00430C80`): C += 50 blank rows, then C += `The End` (literal at `0x006D41DC`, colour
   4); `[0x0077996C]` := top index of C.
4. Colours (`k` of `ui/text.md` §5, text-colour maps of the sky PL2, C7): headings k = 1, every other row
   k = 4. The shipped files hold no `ÿc` code (0 found).
5. Wrap: all three columns wrap (flags 0x90 / 0x80 / 0x82 all have bit 0x20 clear; the add `0x004FC9B0`
   skips the wrap only when 0x20 is set): a row whose width B over its first 255 units is ≥ 250 px is
   split by `0x004FCDA0` into rows (`ui/frontend-menus.md` §F1.1 r8 (d)), each added with the row's
   colour, no indent (the parse passes indent mode 0). No pair line of the shipped files reaches 250 px
   (widest 236 classic, 193 expansion, FontFormal10 advances from `fontformal10.tbl`). Six C rows do, all
   headings (2 classic: 268 and 406 px; 4 expansion: 287, 406, 268, 406): each splits into exactly two
   rows (k = 1), so each adds one row to C, and the balance after the heading then adds one blank row
   to A and B. (Corrected 2026-10-08: an earlier draft said column C never wraps.)
6. Shipped English files, by these rules: classic 93 headings, 730 pairs, 62 single lines, 2 wrapped
   headings → C has 1,233 rows (`The End` = row 1,232); expansion 184 headings, 913 pairs, 119 single
   lines, 4 wrapped headings → 1,801 rows (`The End` = row 1,800). The parse reads every line of both
   files (no early stop).

### C5 Video hook (all videos; stub in d2rs)

1. Path (`0x0042FD10`): `%s\video\%s\<name>%s.bik` with `DATA\LOCAL`, the language folder (`ENG`) and the
   size suffix `640x292` (`0x0040EB10` = 0 → `0x006D421C`; `640x146` with mode 2 otherwise). If that file
   does not open, the same with `ENG` is tried. Front-end names: `d2intro` (`0x006D48B4`), `Act02start`,
   `Act03start`, `Act04start`, `Act04end` (`0x006D4BC4`–`0x006D4C24`), `D2x_Intro_` (`0x006D4894`),
   `D2x_Out_` (`0x006D4C40`). The startup expansion intro (`0x004334E0`) falls back to `D2x_Intro%s`
   (`0x006D4878`, no underscore) with `ENG`, a name no archive holds (original bug, unreachable in English).
   Files: classic videos in `d2video.mpq` (`data\local\video\Eng\…`), expansion ones in `D2XVIDEO.MPQ`
   (`Data\Local\Video\ENG\…`), each in a 640x292 and a 640x146 variant; logo videos in `d2video.mpq`
   (640x480, 640x240, 320x480).
2. Player (`0x004F5D90` → backend; Bink loop `0x0050FDF0`, same in `0x005137E0`): the frame is centred in
   the window (x offset rounded up to a multiple of 4). Each video frame the loop removes the pending
   window messages: left, right or middle button-down (0x201, 0x204, 0x207) or any character message
   (WM_CHAR 0x102: a key that types a character, so letters, digits, Space, Enter, Esc, Backspace, Tab;
   not arrows, F-keys or Shift / Ctrl / Alt alone) **ends the video at once**; a quit message ends it once
   500 ms have passed since its start. Otherwise it plays to its last frame. A skip ends only the current
   video (a held key's auto-repeat may also skip the next one of the startup chain).
3. After the player closes: `0x00515CE0(0)`, `0x00515CE0(0xFF)` (sound, named only).
4. Input flush (`0x004317D0`): at startup and before and after every cinematics-menu video, all pending
   mouse-button, character and quit messages are removed unprocessed; other messages are dispatched. The
   click that picked a cinematic cannot skip it, and the skip key never reaches the menu.
5. Archive juggling, named only: `0x004FAD70` (unload), `0x004FAD10` / `0x004FAEC0` (load `d2video.mpq` /
   `d2Xvideo.mpq`, with "insert disc" pop-ups `0x00431A00` / `0x00434220`, descriptors 58–60, strings
   5168–5170, 21880 when an archive is missing). A complete 1.14d install never shows them. d2rs: no-op
   (all archives open, `client/assets.md`).
6. **d2rs stub:** no decoder. A video request logs one line `video stub: <path>` (info level; the path of
   rule 1 with `ENG` and `640x292`, or the logo path) and returns at once, as if skipped on its first
   frame. Every side effect of the caller still happens in the original order (progress byte writes,
   input flush, screen rebuild); no tick passes.

### C6 Cinematics progress byte N

1. Original storage: `Software\Blizzard Entertainment\Diablo II`, value `Aux Battle.net`, the string
   `216.148.246.N` (`"%d.%d.%d.%ld"`); only the low byte of the last field is tested. Missing → the menu
   uses level 1 and writes `216.148.246.34` (`0x00431600`; at startup `0x0042FA40`).
2. Level L(N) (`0x004313A0`, inline in `0x00431600` and `0x00482CA0`): bit 0x01 → 7; else 0x80 → 6; else
   0x10 → 5; else 0x08 → 4; else 0x40 → 3; else 0x04 → 2; else 1. Default 0x22 → 1.
3. Writers:
   - In game, at every video request (`0x00482EF0` → `0x00482CA0`, before playback, whether or not the
     video plays; ids of `ui/frontend-loading.md` L10): id → level 2→2, 3→3, 4→4, 5 (`ACT04END`)→5, 6
     (`D2X_INTRO_`)→5, 7 (`D2X_OUT_`)→7, other→1. If that level > L(N): N := 0x26 (level 2), 0x62 (3),
     0x2A (4), 0xB2 (5), 0x23 (7). 0xB2 decodes as level 6, so Act04end (Diablo's death) or the Act V
     intro opens both TERROR'S END and SEARCH FOR BAAL. With the value missing the in-game writer compares
     against 34 and writes nothing.
   - Startup expansion intro: N |= 0x80 (C1 r2.5).
   - Default 0x22 when missing (r1).
4. d2rs: no registry (as `ui/frontend-menus.md` §F2.1 r5). N is one byte in the d2rs client settings; absent
   means "never run". Same readers, writers and values.

### C7 Palette

Credits (`0x004312C0`), expansion cinematics (`0x004313D0`), trademark (`0x0042FB20`) and every screen
built on `0x0043C4F0` (main menu, classic cinematics) load `data\global\palette\sky\pal.dat` + `pal.pl2`
(`0x0042F2E0` with `0x006D39BC` / `0x006D39A8`, or the pair at `0x006D3A08` / `0x006D3A0C` naming the same
files). This is the front-end palette set by code (`ui/frontend-menus.md` §F1.6 rule 1).

### C8 Cinematics menu (`0x00431600`)

1. Entry: `0x00515F50(0xB4)`, `0x00514930`, `0x00515CE0(0xFF)` (sound, named only); `0x0043C4F0` (tear
   down; background 8, logo 6 / 7, sky palette); read N (C6), L = L(N).
2. Expansion installed → `0x004313D0(L)`: tear down again (the logo goes), clear the pop-up layer, sky
   palette, descriptor 8 (`gameselectscreenEXP`), then 61–70. Classic → 50–57 on top of 8, 6, 7.

| Desc | Type | x, y (y = bottom) | w × h | Art / string | Action |
|---|---|---|---|---|---|
| 61 | 2 image | 237, 505 | 326×427 | `FrontEnd\CinematicsSelectionEXP` `[0x00779774]`, 4 frames (256 + 70) × (256 + 171) | — |
| 62 | 4 text | 262, 153 | 272×35 | 5114 `SELECT CINEMATICS`, Font24 (`0x007089BC`), flags 2 | — |
| 63 | 6 button | 262, 181 | 272×35 | 21797 `THE SISTER'S LAMENT`, `WideButtonBlank` `[0x00779738]` | `0x00434320` d2intro |
| 64 | 6 button | 262, 224 | 272×35 | 21798 `DESERT JOURNEY` | `0x00434380` Act02start |
| 65 | 6 button | 262, 268 | 272×35 | 21799 `MEPHISTO'S JUNGLE` | `0x004343E0` Act03start |
| 66 | 6 button | 262, 310 | 272×35 | 21800 `ENTER HELL` | `0x00434440` Act04start |
| 67 | 6 button | 262, 353 | 272×35 | 21801 `TERROR'S END` | `0x004344A0` Act04end |
| 68 | 6 button | 262, 396 | 272×35 | 21802 `SEARCH FOR BAAL` | `0x00434500` D2x_Intro_ |
| 69 | 6 button | 262, 439 | 272×35 | 21803 `DESTRUCTION'S END` | `0x00434560` D2x_Out_ |
| 70 | 6 button | 334, 488 | 128×35 | 5103 `CANCEL`, `MediumButtonBlank` `[0x0077973C]`, hotkey 27 | `0x004345C0` |
| 50 | 2 image | 237, 555 | 326×341 | `FrontEnd\CinematicsSelection` `[0x00779770]`, 4 frames (256 + 70) × (256 + 85) | classic |
| 51 | 4 text | 262, 283 | 272×35 | 5114, Font24, flags 2 | classic |
| 52–56 | 6 button | 262; 316, 359, 402, 445, 488 | 272×35 | 4137 `Act 1`, 4138 `Act 2`, 4139 `Act 3`, 4140 `Act 4`, 5113 `Epilogue` | same callbacks as 63–67 |
| 57 | 6 button | 334, 538 | 128×35 | 5103 `CANCEL`, hotkey 27 | `0x004345C0` |

3. Enabled entries: expansion: entries 1…L enabled; entries L+1…7 disabled (`0x004F96F0(ctl, 0)`) and their
   label replaced by an empty one in font 10 (`0x005009D0`). Classic: entries 1…min(L, 5) enabled, the
   others disabled with their labels kept. Disabled buttons ignore the mouse and draw as
   `ui/frontend-menus.md` §F1.1 r4 (up frames, draw mode 1; these buttons have no 0x20 flag). PROVISIONAL: a
   disabled classic label is drawn in the same colour as an enabled one (because the label draw
   `0x00500C50` was not traced for the disabled state); settled by REC-228.
4. Input: no entry has a hotkey; no keyboard focus or arrow navigation; Enter does nothing; Esc = CANCEL.
   No hover state; while the mouse is held on a button its pressed frames show and the label moves down
   2 px (§F1.1 r4–r5).
5. Entry click: archive juggling (C5 r5), then `0x00431870`: tear down, clear pop-ups, input flush, play the
   video if the backend has video (`0x004F9050`), `0x004F9060(1)`, input flush, rebuild the menu
   (`0x00431600`, which re-reads N). The return target is always the cinematics menu.
6. CANCEL / Esc (`0x004345C0`): archive restore, `0x00514D80` ≠ 0 → `0x005148F0(1)` (music, named only),
   main menu `0x004336C0`.
7. Playing a video from this menu never changes N (only the writers of C6 r3 do).

### C9 Sounds (deferred; owner `client/audio.md`)

Button clicks (EXIT, CANCEL, Credits, Cinematics, entries) use the front-end button sound
(`ui/frontend-menus.md` §F1.6 r2). Cinematics entry `0x00515F50(0xB4)`, `0x00514930`, `0x00515CE0(0xFF)`; after
every video `0x00515CE0(0)`, `0x00515CE0(0xFF)`; CANCEL `0x005148F0(1)`. Video audio: none in d2rs (stub).
The credits screen starts no sound.

### C10 640 × 480

The descriptors of C3 and C8 are fixed 800 × 600 coordinates; no 640 × 480 variant exists for these
screens. PROVISIONAL: with the 640 × 480 game resolution the front end, credits and cinematics menu still
draw in the 800 × 600 frame with the same positions (because no front-end path reads the resolution
option and the backgrounds are 800 × 600 art); settled by REC-226.

## Constants & data dependencies

| Constant | Value | Source |
|---|---|---|
| Front-end tick | 40 ms; debt cap 1,000 ms; sleep ≤ 20 ms | `0x004FA590` |
| Descriptors | 41–49 (credits), 50–57 (classic cinematics), 61–70 (expansion), 58–60 (disc pop-up, unused) | table `0x00708D10` |
| Credits text font | FontFormal10 (id 4, height 15) via record `0x007089D4` | `ui/text-fonts.tsv` |
| Row pitch | 15 + 4 = 19 px (`[0x0072DE90]` = 4, English) | `0x004FC7A0`, `0x004FBF30` |
| Scroll step | 2 px per draw; row change when `off` < −19 | `0x004FBF30` |
| Start top row / lead blanks / tail blanks | 20 / 50 / 50 | `0x004312C0`, `0x00431050`, `0x00430C80` |
| End check | every ≥ 50 ms; `[0x0077996C]` ≤ top index + 10 | `0x004341F0` |
| Credits files | `%s\UI\%s\%s`, `Credits.txt` (`0x006D4208`), `ExpansionCredits.txt` (`0x006D41F0`) | `0x00430CD0` |
| Video formats | `0x006D48B4`, `0x006D4BC4`–`0x006D4C40`, `0x006D4894`, `0x006D4878`; logos `0x006D4C84`–`0x006D4CFC`; suffixes `640x292` / `640x146` (`0x006D421C` / `0x006D4214`) | C5 |
| Video skip messages | 0x201, 0x204, 0x207, 0x102; quit after 500 ms | `0x0050FDF0` |
| Progress value | `Aux Battle.net` (`0x006D4164`), `216.148.246.N`, default N 0x22 | C6 |
| In-game video table | 8 × 12 bytes at `0x00721D98` (id, name, flags) | `ui/frontend-loading.md` L10 |
| Strings | 4137–4140, 5101, 5103, 5113, 5114, 5168–5170, 21797–21803, 21880 | `string.tbl`, `expansionstring.tbl` |

## Randomness

None.

## Edge cases & original bugs

1. A credits entry whose first line does not start with an ASCII letter (a blank line, `(`, a digit, an
   accented letter) ends the parse: the rest of the file never shows. The shipped files only have such
   lines as second lines of pairs.
2. A pair whose second line is blank turns the first line into a centred single row.
3. Rows ≥ 250 px wrap at load (C4 r5), in every column; the draw also cuts any row to < 250 px (C3 r2).
4. A credits list of ≤ ~62 rows in column C returns to the main menu on the first timer call; a longer one
   never returns by itself (C3 r6).
5. The scroll runs in the draw, so it advances one step per drawn tick; a stalled front end (debt dropped
   after 1,000 ms) loses scroll steps rather than catching up.
6. The startup expansion intro's fallback path has no underscore (C5 r1).
7. Act04end writes N = 0xB2, which reads as level 6: SEARCH FOR BAAL opens with TERROR'S END (C6 r3).
8. On the first run the trademark screen is built twice (C1 r2.6).
9. A held key auto-repeats WM_CHAR and can skip several startup videos in a row.
10. `0x004334E0` reads N into an uninitialised local when the value is missing; unreachable, because it
    runs only when N exists.

## Test vectors

| Input | Expected | Source |
|---|---|---|
| N = 0x22 (default), expansion | L 1: entry 63 enabled; 64–69 disabled, blank labels | C6 r2, C8 r3 |
| N = 0xB2 | L 6: 63–68 enabled, 69 disabled; classic: all of 52–56 enabled | C6 r2, C8 r3 |
| N = 0x23 / 0x62 / 0x2A / 0x26 | L 7 / 3 / 4 / 2 | C6 r2 |
| In game N = 0x22, video id 5, then id 3 | N 0xB2, then unchanged | C6 r3 |
| Startup, N = 0x32, expansion | logs New_BLIZ640x480, BlizNorth640x480, `DATA\LOCAL\video\ENG\D2x_Intro_640x292.bik`; N 0xB2 | C1 r2, r4 |
| Startup, no N | logs the two logo videos and `DATA\LOCAL\video\ENG\d2intro640x292.bik`; N 0x22; trademark | C1 r2, r4 |
| Credits (expansion), draw 1 / 9 / 10 | row 50 baseline 588 / 572 / 571; top row 20 / 20 / 21 | C3 r3–r4 |
| Credits row 50 = pair L1 "Ab", L2 "Cd" (expansion) | "Ab" drawn ending at x 400; "Cd" starting at x 410 | C3 r2 |
| Synthetic heading `*AAAA BBBB`, width B of `AAAA` 240 px, of `AAAA B` 252 px | two C rows `AAAA` and `BBBB` (the leading space dropped), both k 1; A and B get one extra blank row | C4 r5, §F1.1 r8 |
| Synthetic centred row `AAAA` of width 100 px (expansion) | pen x 280 + 75 = 355 | C3 r2 |
| Lines `*H`, `Al`, `Bo`, (blank), `Cy` | C: …, `H` (k 1), blank, blank, … ; A/B: `Al` / `Bo` on the row after the heading's blank; `Cy` never shown | C4 r3 |
| Shipped expansion file | C rows 1,801; `The End` gone at draw 17,810 | C3 r7, C4 r6 |
| Trademark: key Q / key N / Space | stays / main menu / main menu | C2 |
| Video playing: ← key / `a` / right click | plays on / ends / ends | C5 r2 |
| Credits: Space, Enter, click on text | nothing; Esc → main menu | C3 r5 |

## Provenance

1.14d `Game.exe` (Ghidra exports, `tools/ghidra/disasm.py` via `re/scripts/da.py`, image reads, 2026-10-08):
front-end entry `0x004359D0`; startup `0x00435230`, `0x00433640`, `0x004334E0`, `0x0042FA40`, `0x0042FAB0`,
`0x0040EB10`, `0x004F9050`, `0x004F9060`; trademark `0x0042FB20`, `0x00434DC0`, `0x00434D90`; key router
`0x004F9BF0`, `0x004F9B60`, `0x004FA760`, table `0x0072DC48`–`0x0072DDB8`; image ctor `0x004FD6C0`, click
`0x004FD5D0`; timer `0x004FD9D0`, `0x004FD860`, `0x004FD8F0`, `0x004FD970`, `0x004FD830`, `0x004FDA50`;
credits `0x004312C0`, `0x00431050`, `0x00430CD0`, `0x00430EF0`, `0x00430C80`, `0x004341F0`, `0x00435020`;
text control `0x004FC7A0`, `0x004FC9B0`, `0x004FCF80`, `0x004FD060`, split `0x004FCDA0`, clip `0x004FBEE0`
(2026-10-08 re-read: wrap in all columns, heading add `0x0043113C` colour 1, mode 0), draw `0x004FBF30`, `0x004FB950`,
`0x004FBA30`, `0x004FB6A0`, font height `0x00501A40`; front-end loop `0x004FA590`; cinematics `0x00431600`,
`0x004313D0`, `0x004313A0`, jump tables `0x00431550`, `0x004317AC`, entry callbacks
`0x00434320`–`0x00434560`, `0x004345C0`, `0x00431870`, `0x0042FD10`, `0x004317D0`, `0x005009D0`,
`0x004F96F0`; archives `0x004FAD10`, `0x004FAD70`, `0x004FAEC0`, pop-ups `0x00431970`, `0x00431A00`,
`0x00431570`, `0x00434220`; Bink loops `0x0050FDF0`, `0x005137E0` (imports `binkw32.dll`); in-game
`0x00482EF0`, `0x00482CA0`, tables `0x00482DCC`, `0x00482DE4`; palette `0x0042F2E0`, `0x0043C4F0`; preload
`0x0042E6D0` (globals `0x00779720`, `0x00779770`, `0x00779774`, `0x0077973C`, `0x00779738`). Descriptors
41–70 read from `0x00708D10`. Files: `Credits.txt`, `MacCredits.txt` (d2data), `ExpansionCredits.txt`
(d2exp) read with `tools/mpq-tool` into the scratchpad (structure counted, text not copied); row counts
from a scratch simulation of C4; widths from `fontformal10.tbl` (d2data); DC6 headers of the four
backgrounds; video lists of `d2video.mpq` / `D2XVIDEO.MPQ`. Strings: ENG `string.tbl`,
`expansionstring.tbl`, 1.14d `patchstring.tbl`. D2MOO not used.

## Open questions

- **REC-225** Credits end and scroll rate. Capture: 1.14d expansion, Credits; record at ≥ 50 fps from the
  click to 13 minutes; log `[0x0077996C]` and C's `[+0x58]` / `[+0x9C]` each tick (hook `0x004341F0`,
  `0x004FBF30`). Settles C3 r3 (19 px per 10 ticks), r4 (first row at y 590) and r6 (no automatic return
  after "The End").
- **REC-226** Front end at 640 × 480. Capture: set the 640 × 480 video option, restart, screenshot the main
  menu, Credits and Cinematics. Settles C10 (800 × 600 frame kept or not).
- **REC-228** Disabled cinematics buttons. Capture: N = 0x22 (registry), classic and expansion menus;
  screenshot; compare the disabled buttons' pixels and labels against C8 r3. Settles the disabled label
  colour (classic) and the blank labels (expansion).
- **REC-229** Startup chain. Capture: delete `Aux Battle.net`, start 1.14d, hook `0x004F5D90` (log paths)
  and `0x0042F430` (log ids); repeat with the value present. Verifies C1 r3 (settled from the binary
  2026-10-08: no `blizno` screen) and settles the first-run intro.
- Folded into `ui/frontend-menus.md` 2026-10-08: §F1.3 rows 1–2 note (REC-229), §F1.6 rule 1 (sky palette), edge case 2 (8–9 s).
