# Spec: UI — Controls (key configuration, default bindings, input → command)

- **Status:** draft (2026-10-07, RE on the 1.14d `Game.exe`, the 1.14d
  `default.key` files and 16 per-character `.key` files of a 1.14d
  install; no input trace yet). The command list, the default table and
  the file formats are read from data and checked byte for byte
  (§Provenance); the world mouse-click path (which C→S message a click
  sends, hold repeat) is §Open questions 1.
- **Target version:** 1.14d, English install
- **Crate/module:** `d2-client::controls` (preset `original`, `.key`
  import), `d2-client::input` (key / button → command dispatch)
- **Related specs:** `client/ui.md` §A4, §A6, §B4 (design this answers),
  `ui/panels.md` §2–§4 (`SetUIState`, conflict gate, cursor jump), §15
  (panel event → message), `skills/use.md` §1 (0x05–0x11 server side),
  `items/inventory-moves.md` (belt use), `sim/client-messages.tsv`
  (0x26, 0x3C, 0x3F, 0x51, 0x53, 0x54, 0x60 layouts), `formats/tbl.md`
  (string ids). Machine table: `key-commands.tsv` (§7).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 42–55 |
| Inputs | 56–64 |
| Outputs / state changes | 65–70 |
| Rules | 71–72 |
|   1. Key codes | 73–98 |
|   2. Binding table and files | 99–127 |
|   3. Load, defaults and save | 128–160 |
|   4. Rebinding rules (key configuration panel) | 161–185 |
|   5. Dispatch | 186–234 |
|   6. Commands | 235–279 |
|   7. `key-commands.tsv` | 280–296 |
|   8. Hard-wired input (not configurable) | 297–315 |
| Constants & data dependencies | 316–326 |
| Randomness | 327–330 |
| Edge cases & original bugs | 331–350 |
| Test vectors | 351–375 |
| Provenance | 376–398 |
| Open questions | 399–415 |
<!-- /index -->

## Summary

The 1.14d client has 57 configurable commands (ids 0–56). Each command
has two binding records (key one, key two); a record holds one key code
(a Windows virtual-key code, or 0x100–0x104 for the middle button, the
two extra buttons and the wheel). A key may be bound to one record only.
The table lives in memory as 114 records of 10 bytes, is loaded per
character from `<save>\<name>.key` at game start, falls back to a
default table, and is written back on save. Key presses run the
command's press handler, releases its release handler; auto-repeat is
ignored. Left and right mouse buttons are not configurable. This spec
owns the command list, the default bindings, the file formats, the
binding rules and the key → command dispatch.

## Inputs

| Name | Type | Source |
|---|---|---|
| key / button events | Windows message, code, lParam bit 30 (repeat) | window message chain (`0x00712944` handler table) |
| per-character key file | `<save>\<name>.key`, 1142 bytes | §2.3 |
| default key file | `<dir>\default.key`, 1146 bytes; `data\local\cmd\<lng>\default.key` in the archives | §2.2 |
| expansion game flag | `[0x007A04F4]` (`0x0044DCC0`) | `client/model.md` |

## Outputs / state changes

The current binding table `0x007A6F90` (114 × 10 bytes); the mouse
dispatch slots (§3.4); calls of the command handlers (§6), which change
UI states (`ui/panels.md` §2) or queue C→S messages.

## Rules

### 1. Key codes

1. A key code is a u16. Codes below 0x100 are Windows virtual-key codes
   (`VK_*`: 0x08 Backspace, 0x09 Tab, 0x0D Enter, 0x10 Shift, 0x11
   Ctrl, 0x12 Alt, 0x1B Esc, 0x20 Space, 0x2C PrintScreen, 0x30–0x39
   digits, 0x41–0x5A letters, 0x60–0x69 numpad 0–9, 0x70–0x87 F1–F24,
   0xC0 backquote, …). 0xFFFF = no key.
2. Mouse codes (`0x004694A0`, `0x0044C400`–`0x0044C520`):

   | Code | Input | Window message |
   |---|---|---|
   | 0x100 | middle button | WM_MBUTTONDOWN / UP (`0x0044C4A0` / `0x0044C470`) |
   | 0x101 | extra button 1 | WM_XBUTTONDOWN / UP, high word of wParam = 1 (`0x0044C4D0` / `0x0044C520`) |
   | 0x102 | extra button 2 | same, high word = 2 |
   | 0x103 | wheel forward (positive delta) | WM_MOUSEWHEEL (`0x0044C400`) |
   | 0x104 | wheel backward (negative delta) | WM_MOUSEWHEEL |

3. **Bindable codes** (`0x00469AE0`): 0x100–0x104; the codes 0x08, 0x09,
   0x0C, 0x0D, 0x10–0x14, 0x20–0x24, 0x2A, 0x2C–0x2F, 0x60–0x87, 0x91,
   0xBA–0xC0, 0xDB–0xDE; and any other code < 0x105 for which the C
   `isalpha` or `isdigit` is true (0x30–0x39, 0x41–0x5A, 0x61–0x7A).
   Every other code is refused, among them 0x01–0x04 (left, right,
   cancel, middle as virtual keys), 0x15, 0x17–0x19, 0x1B (Esc),
   0x1C–0x1F, 0x25–0x29 (arrows, select), 0x2B, 0x5B–0x5D (Windows and
   menu keys), 0x90 (NumLock), and every code ≥ 0x105.

### 2. Binding table and files

1. **Record** (10 bytes, little-endian, packed): `cmd` u32 (0–56), `key`
   u16 (§1, 0xFFFF none), `slot` u32 (1 = key one, 0 = key two). The
   table is 114 records = 1140 (0x474) bytes; every command has exactly
   one record per slot. Record order is the file order of §7 column
   `file_pos` (record `2p` = slot 1, `2p + 1` = slot 0 of the command at
   position `p`). The display order sorts by `cmd`, slot 1 first
   (comparator `0x00469450`).
2. **Default file** (`default.key`, 1146 = 0x47A bytes): u16 `0x5357`
   ("WS"), u16 version `0x25`, u16 size `0x47A`, then the 1140-byte
   table. A file is accepted only with all three header values and a
   read size of exactly 0x47A (`0x004698D0`, `0x004699E6`).
3. **Per-character file** (`<save>\<name>.key`, 1142 = 0x476 bytes): u16
   version `0x25`, then the 1140-byte table. Measured: all 16 `.key`
   files of the test install are this layout and their tables equal
   the default table.
4. Archive copies (measured): `d2data.mpq`
   `data\local\cmd\eng\default.key` is 886 bytes, version 0x22, 88
   records (commands 0–43, F5–F8 on commands 8–11); `d2exp.mpq`
   `data\local\cmd\eng\default.key` is 1126 bytes, version 0x24, 112
   records (no command 55 / 56). Both fail the 1.14d header check
   (§2.2), so neither is ever used by 1.14d (§3.2).
5. **The 1.14d default table** is compiled into `Game.exe` at
   `0x00712220` (`.data`, file offset 0x312220, 1140 bytes, SHA-256
   `a711045f3efb1de993c3890c6dbf1f7dad5750df857370cfe1fb0f1d1241cfdd`).
   Its bindings are `key-commands.tsv` columns `key1` / `key2`. The
   install's `default.key` equals header + this table (measured).

### 3. Load, defaults and save

1. **Game start** (`0x0046AC70` → `0x0046AAE0`): open
   `<save dir>\<character name>.key` (`0x00469650`, name
   `[0x007A05C4]`; it tries the sub-folder `[0x007A0500]` first when
   that string is non-empty). The file is accepted when the read
   returns exactly 0x476 bytes, the version is 0x25, every command id
   0–56 occurs in at least one record, and no key other than 0xFFFF
   occurs in two records. Accepted: the current table := the file's
   table. Then §3.4.
2. Otherwise (no file or a failed check): load defaults
   (`0x004698D0(1)`): read `<dir>\default.key` (`0x00406BA0` gives
   `<dir>`; the install folder on the test machine); if accepted (§2.2)
   its table replaces the default table `0x00712220`. Else read
   `DATA\LOCAL\CMD\<lng>\default.key` from the archives (`<lng>` from
   `0x00525260`, "eng"); in 1.14d this fails the header check (§2.4)
   and the compiled table stays. Then current := default table, §3.4,
   and save (§3.3).
3. **Save** (`0x00469780`): write the version + current table to the
   per-character file (1142 bytes), then `<dir>\default.key` with the
   header (1146 bytes, `0x4697F2`–`0x004698AE`).
4. **Mouse slots** (`0x004694A0`): clear the five mouse slots, then for
   each record in table order whose key is ≥ 0xE0 and whose `cmd` < 57:
   key 0x100 → middle press := press handler, middle release := release
   handler; 0x101 / 0x102 → the extra button's press and release; 0x103
   / 0x104 → wheel forward / backward := press handler, unless the
   command has a release handler, in which case the record's key is
   cleared to 0xFFFF. Later records overwrite earlier ones.
5. "Default" in the key configuration panel copies the default table
   into the current table (`0x00469D50` → §3.4); "Cancel" restores the
   copy taken when the panel opened (`0x00469D20` keeps it at
   `0x007A6B18`). Panel layout and its other buttons: §Open questions 3.

### 4. Rebinding rules (key configuration panel)

`Rebind(cmd, slot, key)` (`0x00469C20`) returns 1 when the key was
placed, 0 otherwise, with an optional message string id:

1. `cmd` ≥ 56 → refused (command 56, Esc / game menu, is fixed).
2. Key not bindable (§1.3) → refused, message 3979 `CantAssignKey`.
3. Key 0x103 / 0x104 (wheel) and the command has a release handler
   (commands 34, 36, 37, 42) → refused, message 3978 `CantAssignMW`
   (`0x00469420`).
4. Key 0x2C (PrintScreen) and the command has a press handler → refused,
   message 3979 (PrintScreen can go only to command 42, the only one
   without a press handler).
5. Otherwise: the first record holding `key` loses it (key := 0xFFFF);
   then the record (`cmd`, `slot`) gets `key`, return 1. If no record
   (`cmd`, `slot`) exists, the key goes back to the record it was taken
   from and the call returns 0.
6. Clearing a slot (`0x00469D70`, string 3980 `CfgClearKey`): key :=
   0xFFFF on every record (`cmd`, `slot`).
7. Commands 44–54 appear in the panel only in an expansion game
   (`key-commands.tsv` `menu_classic` = `-`; `0x004A43E0` picks the menu
   table and its row count: 51 classic, 62 expansion, separators
   included). Their bindings stay active in a classic game; their
   handlers check the game type themselves (§6).

### 5. Dispatch

1. **Hot-key mode** `[0x007A7418]` (`0x0046AA20(mode, swap)`,
   `0x0046AC70` sets 1 at game start, `0x0046ACB0` sets 0 at game end):

   | Mode | Window handlers registered | Effect |
   |---|---|---|
   | 1 | press and release (`0x00712944`: WM_KEYDOWN / WM_SYSKEYDOWN → `0x0046A840`, WM_KEYUP / WM_SYSKEYUP → `0x0046A940`) | all commands |
   | 2 | same | only commands with `full_ok` = 1 (§7) run, on press and on release |
   | 0 | release only (`0x00712974`) | presses do nothing; release handlers still run |

   Set by the UI open / close hooks (`ui/panels.md` §2.6): opening ui 5
   (chat) or 0x17 (player trade) → 0; opening 0x0C (NPC shop), 0x19
   (stash) or 0x1A (cube) → 2; closing 0x0C, 0x19, 0x1A or 0x17 → 1;
   closing ui 5 → 1 only if none of the states 0x0C, 0x17, 0x19, 0x1A,
   0x1B, 0x1C, 0x1D, 0x1E, 0x20 is open (else unchanged). The key
   configuration panel (ui 0x0B) sets 0 while it waits for a key and 1
   otherwise (`0x004A5200`).
2. **Press** (`0x0046A840`): ignored when lParam bit 30 is set (key
   already down: auto-repeat never repeats a command). Find the first
   record, in table order, with `key` = the virtual-key code, `cmd` <
   57 and a press handler. None → not consumed. If the key is 0x73
   (F4) and Alt is down (`GetKeyState(0x12)` < 0) → nothing (Alt+F4 is
   left to Windows). If mode = 2 and the command's `full_ok` = 0 →
   nothing. Else call the press handler and consume the message.
3. **Release** (`0x0046A940`): the same search for a record with a
   release handler (no repeat test, no F4 test), the mode-2 filter,
   then the release handler.
4. **Mouse codes** (§1.2) are dispatched from the mouse messages while
   in game (`[0x007A061C]` ≠ 0) through the slots of §3.4; the hot-key
   mode does not apply to them. The wheel adds the message's delta to an
   accumulator `[0x007A06F8]`; when |accumulator| ≥ 120 the slot of its
   sign (forward ≥ 120, backward ≤ −120) runs once and the accumulator
   resets to 0 (the remainder is dropped; a delta of 240 runs once).
5. Release handlers are what make three commands "held": Run (34), Stand
   Still (36), Show Items (37); Screen Shot (42) acts on release.
6. **Stand-still polling**: every pass of the client loop
   (`0x0044EFA0` at `0x0044F04C`) sets the stand-still flag to 1 if a
   key bound to command 36 (slot 1, then slot 0) is down by
   `GetAsyncKeyState`, else 0 (`0x0044CE90`). The press / release
   handlers of command 36 are therefore overridden at the next loop
   pass.
7. **WM_ACTIVATEAPP** (`0x0044C5E0`): wParam ≠ 0 → `SetUIState(0x0D,
   off, 0)` (item labels), stand-still flag := 0, `0x00466FE0`,
   `0x004FB130(0)`. wParam = 0 → the run flag := 0 unless the slot-1
   key of command 34 is down (`GetAsyncKeyState`), then the pending
   left / right button handlers are closed (`0x0044C060` when
   `[0x0070F234]` ≠ 0x10, likewise for `[0x0070F2BC]`).

### 6. Commands

"Guard" = the handler does nothing while the client's exit flag
`[0x007A0620]` is set or the local player is missing or dead (mode 0x11)
(`0x0044DA30`, `0x00463DF0`). `SetUIState(ui, mode, jump)` is
`ui/panels.md` §2 (mode 2 = toggle). Handlers by command id:

| Cmd | Name | Action |
|---|---|---|
| 0 | Character Screen | `SetUIState(2, toggle, jump 1)` |
| 1 | Inventory Screen | `SetUIState(1, toggle, 1)` |
| 2 | Party Screen | multiplayer only (game type `[0x007A0610]` ≠ 0): `SetUIState(0x16, toggle, 1)` |
| 3 | Message Log | `SetUIState(0x18, toggle, 0)` |
| 4 | Quest Log | guard; `0x004A3FE0(1)` (quest panel; owner `ui/panels.md` OQ 1) |
| 5 | Chat | not while exiting: `SetUIState(5, toggle, 0)` |
| 6 | Help Screen | guard; `SetUIState(0x21, toggle, 0)`; then registry value `Diablo II\Help Menu` := 1 if it is missing or 0 (`0x00414F10` / `0x004150E0`) |
| 7 | Automap | `SetUIState(0x0A, toggle, 0)`; if the automap is now closed, `0x00457640(0)` |
| 8 | Center Automap | `0x00457640(1)` |
| 9 | Fade Automap | fade level := (level + 1) mod 4 (`0x004576C0` / `0x004576F0`) |
| 10 | Party on Automap | flag `0x004577B0` toggled (`0x004577C0`) |
| 11 | Names on Automap | flag `0x004577E0` toggled (`0x004577F0`) |
| 12 | Skill Tree | `SetUIState(4, toggle, 1)` |
| 13 | Skill Speed Bar | `SetUIState(3, toggle, 0)`; `0x004A8CE0(0)` |
| 14–21 | Skill 1–8 | guard; hot-key slot k = cmd − 14: if ui 3 (skill speed bar) is open, `0x004A9E60(k)` (bind the hovered skill, C→S 0x51), else `0x004AA030(k)` (select the slot's skill, C→S 0x3C) (`ui/panels.md` §15) |
| 46–53 | Skill 9–16 | as 14–21 with k = cmd − 38 (8–15), expansion game only |
| 22 | Show Belt | if the player has an item in body location 8 (belt) of item type 0x13 (`0x0063BDE0`, `0x00629BB0`): `SetUIState(0x1F, toggle, 0)` |
| 23–26 | Use Belt 1–4 | guard; only while `[0x007BEFB0]` = 1: use belt column cmd − 23 with "shift" = Shift down (`GetKeyState(0x10)` bit 15) (`0x00498C50` + 0x40 × column → `0x00498A90`; C→S 0x26, `items/inventory-moves.md`) |
| 27–33, 55 | Say 'Help' … 'Retreat' | C→S 0x3F with sound 0x19 + n, n = cmd − 27 (cmd 55: n = 7, 0x20) (`0x004785B0`) |
| 34 | Run (held) | press: run flag `[0x007A065C]` := 1; if the player is a player unit in mode 2 (walk) and `0x00625480(player, 0x0A, 0)` ≠ 0 (guard as above): `0x00480E70(player, 3)` and C→S 0x53. Release: flag := 0; if run-lock is off and the player is in mode 3 (run): `0x00480E70(player, 2)` and C→S 0x54 |
| 35 | Toggle Run/Walk | run-lock `[0x007A0660]` := not run-lock (no message) |
| 36 | Stand Still (held) | press: stand-still flag `[0x007A0664]` := 1; release: := 0 |
| 37 | Show Items (held) | press: `SetUIState(0x0D, on, 0)`; release: `SetUIState(0x0D, off, 0)` |
| 38 | Clear Screen | in game: close every closable open state (`0x00456300(0, jump 1)`, closable = table `0x006D6378`, §Constants); if none was closed: `0x00457640(0)` and close again including the automap (`0x00456300(1, 0)`) |
| 39 / 40 | Select Previous / Next Skill | guard; `0x004AA740(−1)` / `0x004AA740(1)` |
| 41 | Clear Messages | `0x004A01E0` |
| 42 | Screen Shot | release only: `0x004FA7A0` |
| 43 | Show Portraits | `0x00493840` |
| 44 | Swap Weapons | unless ui 0x0C, 0x17 or 0x19 is open: `0x0048A730` (C→S 0x60, `ui/panels.md` §15) |
| 45 | Toggle MiniMap | flag `0x00457770` toggled (`0x00457780`) |
| 54 | Hireling Screen | expansion game, the player has a hireling (`0x00478F20(player, 7)` ≠ −1) and `0x00408F20()` ≠ 0: `SetUIState(0x24, toggle, 1)` |
| 56 | Esc (fixed) | unless `0x004B34A0()` or a modal text screen (`0x004A0000`): if ui 9 (game menu) is open, close it (`0x0047E200(1)`); else close the closable states (`0x00456300(0, 1)`) and, if none was closed, open the game menu (`0x0047E090(1, 0)`) |

The run and stand-still flags are read by the left / right mouse
handlers (§8.2).

### 7. `key-commands.tsv`

One row per command 0–56. Columns:

| Column | Meaning |
|---|---|
| `cmd` | command id |
| `string_id`, `string_key` | label in the key configuration panel (`formats/tbl.md` id; 10833 and 11083 are 1.14d `Patch_D2.mpq` `patchstring.tbl` ids); `-` for 56 (not listed) |
| `key1`, `key2` | default key of slot 1 / slot 0: `0xNNN:Name` (§1) or `-` |
| `down`, `up` | 1.14d press / release handler (`0x00712698` + 12 × cmd, +0 / +4), `-` = none |
| `full_ok` | +8 of the same entry: 1 = runs in hot-key mode 2 (§5.1) |
| `file_pos` | position in the record order of the default table (§2.1) |
| `menu_classic`, `menu_exp` | row in the panel's menu table (`0x00724268`, 51 rows, separators at rows 6, 19, 25, 32, 38, 47; `0x00724468`, 62 rows, separators at 7, 28, 35, 42, 49, 58); `-` = not listed |

Our `Action` names for the `original` preset (`client/ui.md` §A6) are
the `string_key` values; command 56 is `GameMenu`.

### 8. Hard-wired input (not configurable)

1. Left and right mouse buttons, the cursor and Esc's binding are fixed
   (§1.3 refuses 0x01, 0x02, 0x04 and 0x1B).
2. **World clicks** (`0x0044BF40` and the other button handlers
   `0x0044C060`; `0x0044C2C0` runs from the client loop while
   `[0x007A0654]` ≠ 0, the held-button path): in game, the handler builds a flag word
   from the held commands: 8 when the run flag (§6 cmd 34) or the
   run-lock (cmd 35) is set, | 4 when the stand-still flag (cmd 36) is
   set, and passes it with the message's key state to `0x00462D00` for a
   player unit. Which C→S message (0x01–0x04 walk / run, 0x05–0x11
   skill, `skills/use.md` §1) and the hold repeat follow from that
   function: §Open questions 1.
3. Inside an open panel the click goes to the panel (`ui/panels.md`
   §4.4); Shift-clicks on items and stat buttons are the owners' rules
   (`ui/inventory.md`, `ui/panels.md` §8.5).
4. Text entry (chat box, ui 5) receives WM_CHAR while the hot-key mode
   is 0 (§5.1); the edit control is `ui/text.md` / §Open questions 4.

## Constants & data dependencies

- 57 commands, 114 records of 10 bytes, table 0x474 bytes; version 0x25;
  default file header "WS" / 0x25 / 0x47A.
- Closable states for Clear Screen / Esc (`0x006D6378`, 38 u32, 1 =
  closable): 1, 2, 3, 4, 5, 9, 11, 12, 13, 15, 16, 18, 20, 22–33, 36, 37.
  Not closable: 0, 6, 7, 8, 10 (automap, unless asked), 14, 17, 19, 21,
  34, 35.
- Strings: 3921–3985 (`Cfg*`), 3975–3980 (panel messages), 10833, 11083,
  22717–22727.

## Randomness

None.

## Edge cases & original bugs

Reproduced by default.

- Auto-repeat never repeats a command (§5.2).
- Alt+F4 is ignored only for the key code 0x73 itself, whatever command
  F4 is bound to (§5.2).
- Chat opened over the stash, shop or cube leaves hot keys off after
  the chat closes (§5.1: closing ui 5 does not restore mode 2).
- Release handlers run in mode 0: releasing Ctrl or Alt while the chat
  box is open still ends running or the item labels. Stand Still
  follows the polled key state in every mode (§5.6), so Shift works
  while typing.
- The wheel drops the remainder of its accumulator (§5.4).
- Rebinding a key steals it from its first holder before checking that
  the target record exists (§4.5).
- Toggle Run/Walk sends nothing; the change shows at the next move
  click (§8.2).
- The archive `default.key` copies are dead data in 1.14d (§2.4).

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| `key-commands.tsv` `key1`/`key2` written as records in `file_pos` order, slot 1 then slot 0 | 1140 bytes equal to `Game.exe` bytes at file offset 0x312220 (SHA-256 `a711045f…cfdd`) | §2.5; game test (`#[ignore]`, `D2_GAME_DIR`) |
| same table with header `57 53 25 00 7A 04` | equals `<D2_GAME_DIR>\default.key` when the user never rebound keys (1146 bytes) | §2.2 |
| record 0 | `00 00 00 00 41 00 01 00 00 00` (cmd 0, 'A', slot 1) | §2.1 |
| record 90 (`file_pos` 45) | cmd 21, F8 (0x77), slot 1 | §7 |
| default file with version 0x24 (the `d2exp.mpq` copy, 1126 bytes) | rejected; compiled table used | §2.2, §2.4 |
| per-character file of 1142 bytes, version 0x25, command 13 missing from every record | rejected; defaults loaded and both files rewritten | §3.1, §3.2 |
| per-character file where 'A' is bound to commands 0 and 3 | rejected (duplicate key) | §3.1 |
| `Rebind(56, 1, 'K')` | 0 | §4.1 |
| `Rebind(36, 1, 0x103)` | 0, message 3978 | §4.3 |
| `Rebind(0, 1, 0x2C)` | 0, message 3979 | §4.4 |
| `Rebind(3, 0, 'C')` on the default table | 1; command 0 slot 0 now none; command 3 slot 0 = 'C' | §4.5 |
| `Rebind(0, 1, 0x25)` (left arrow) | 0, message 3979 | §1.3 |
| WM_KEYDOWN 'I', lParam bit 30 clear, mode 1 | `SetUIState(1, toggle, 1)` | §5.2, §6 |
| same with bit 30 set | nothing | §5.2 |
| stash open (mode 2), WM_KEYDOWN 'I' | nothing; WM_KEYDOWN 'W' swaps nothing either (ui 0x19 open, §6 cmd 44) | §5.1, §6 |
| mode 2, WM_KEYDOWN Numpad0 | C→S `3F 19 00` | §6 cmd 27 |
| WM_MOUSEWHEEL +60, +60 | Select Previous Skill once, after the second | §5.4 |
| WM_MOUSEWHEEL −240 | Select Next Skill once | §5.4 |
| Ctrl down while walking, Ctrl up (run-lock off) | C→S `53`, then `54` | §6 cmd 34 |
| trace `controls-0001` (§Open questions 5) | identical C→S messages and ticks | trace, queued |

## Provenance

1.14d `Game.exe` (Ghidra export, `tools/ghidra/disasm.py`, `all.asm`):
`CmdTbl.cpp` functions `0x00469650` (per-character path), `0x00469780`
(save), `0x004698D0` (default load), `0x00469AE0` (bindable keys),
`0x00469C20` (rebind), `0x00469420`, `0x00469450`, `0x00469AA0`,
`0x00469D20`–`0x00469D90`, `0x004694A0` (mouse slots), `0x0046AAE0`
(game-start load and checks), `0x0046AA20` / `0x0046AC70` /
`0x0046ACB0` (hot-key mode), `0x0046A840` / `0x0046A940` (dispatch),
handler table `0x00712698` (57 × 12 bytes), window handler tables
`0x00712944`, `0x00712974`; mouse messages `0x0044C400`–`0x0044C520`,
`0x0044BF40`; command handlers `0x00468940`–`0x00469220` (disassembled
in full); UI hooks `0x00455720` (jump table `0x00455A40`), `0x00455AE0`
(`0x00455E80`); menu tables `0x00724268`, `0x00724468` (selector
`0x004A43E0`); closable table `0x006D6378`. Game files: `Game.exe`
`.data` (default table), `default.key` of the install, the
`default.key` copies in `d2data.mpq` / `d2exp.mpq`, 16 per-character
`.key` files (Saved Games, 2026-10-07), English `string.tbl`,
`expansionstring.tbl`, `patchstring.tbl` (`Patch_D2.mpq`); probes with
Python, outputs outside the repo. D2MOO (1.10f) gave only the record
struct name `D2KeyConfigStrc`; every field was confirmed from the 1.14d
code and files.

## Open questions

1. World mouse clicks: how `0x00462D00` and the other button handlers
   (`0x0044C060`, `0x0044C2C0`) turn the flag word (§8.2) into C→S
   0x01–0x11, and the hold-repeat timing (recording 015956 shows 0x10
   every 13 frames). Needs more than two function reads: recording
   `controls-0001` (`docs/handoff/pc2-rec-pc2-ui.md`).
2. Whether the in-game key panel shows slot 1 in the "Key/Button One"
   column (comparator order §2.1 suggests so): capture `controls-0002`.
3. Key configuration panel (ui 0x0B, `0x004A5270`, `0x004A5A40`–
   `0x004A63B0`): row layout, scroll, highlight and the
   accept / default / cancel buttons. Owner of its pixels: this spec,
   capture `controls-0002`.
4. Chat edit box (caret, selection, `ui/text.md` OQ 3 / 4): owner
   `ui/text.md` (PC 1); listed so it is not lost.
5. Input trace `controls-0001` settles §5 and §6 end to end.
