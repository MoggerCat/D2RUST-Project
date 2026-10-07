# Spec: UI — Controls (command table, default keys, key files, input dispatch)

- **Status:** draft (2026-10-07, RE on 1.14d `Game.exe` and the 1.14d
  `default.key` / `string.tbl` files; no capture, no packet trace yet).
  The default table is a byte read of the binary; behaviour is from the
  disassembly; unverified until the §Test vectors checks run.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::controls` (the `original` preset and the
  `Action` list, `client/ui.md` §A6), `d2-client::input` (event → command)
- **Related specs:** `client/ui.md` §A4 / §A6 / §B4 (the d2rs design this
  answers: this whole spec is the owner of row §B4; code TODOs that say
  `ui/controls.md §B4` mean this spec, and the original-defaults check is
  §B4 below), `ui/panels.md` (§2 `SetUIState`, ui ids; its §Open
  questions 2 is answered here), `ui/ui-states.tsv`, `ui/inventory.md`
  (belt and grid clicks), `items/inventory.md` §3 (belt slots),
  `formats/tbl.md` (labels)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 39–50 |
| Inputs | 51–59 |
| Outputs / state changes | 60–67 |
| Rules | 68–69 |
|   1. Binding table | 70–88 |
|   2. Key files | 89–121 |
|   3. Commands and default keys | 122–223 |
|   4. Dispatch | 224–293 |
|   5. Key-config screen assignment | 294–309 |
|   B4. Original-defaults check (`client/ui.md` §B4) | 310–328 |
| Constants & data dependencies | 329–335 |
| Randomness | 336–339 |
| Edge cases & original bugs | 340–352 |
| Test vectors | 353–371 |
| Provenance | 372–383 |
| Open questions | 384–408 |
<!-- /index -->

## Summary

The 1.14d client binds 57 commands (ids 0–56) to keys or mouse buttons
through a table of 114 bindings (two per command). The table is loaded at
game start from the character's `.key` file, else from `default.key`,
else from a default table compiled into `Game.exe`; it is written back to
both files when the defaults were used. Keyboard events reach the commands
through one key-down and one key-up window handler; the middle and X
buttons and the wheel through per-button handlers; the left and right
buttons are not configurable. Three held/toggled modifiers (Run, Toggle
Run/Walk, Stand Still) become flag bits on every world click.

## Inputs

| Name | Type | Source |
|---|---|---|
| Win32 key messages | `WM_KEYDOWN`/`WM_SYSKEYDOWN` (0x100/0x104), `WM_KEYUP`/`WM_SYSKEYUP` (0x101/0x105); VK in wParam, repeat bit 30 of lParam | game window, handler table `0x00712944` |
| Win32 mouse messages | 0x201/0x202 left, 0x204/0x205 right, 0x207/0x208 middle, 0x20B/0x20C X buttons, 0x20A wheel | handler table `0x0070F2C4` |
| character `.key` file | 0x476 bytes (§2.1) | save directory |
| `default.key` | 0x47A bytes (§2.2) | save directory, then archive `DATA\LOCAL\CMD\<lang>\default.key` |

## Outputs / state changes

Live binding table `0x007A6F90` (114 × 10 bytes, ends `0x007A7404`);
mouse-button handler slots (`0x007A6B10` … `0x007A7414`, §4.2); modifier
flags `0x007A065C` (Run held), `0x007A0660` (run lock), `0x007A0664`
(Stand Still held); UI state changes through `SetUIState` and the command
effects of §3; files of §2.

## Rules

### 1. Binding table

1. One binding is 10 bytes: i32 command id (0–56), u16 key, i32 slot
   (0 or 1). Key 0xFFFF = unbound. The table has 114 entries
   (0x474 bytes); the live copy is at `0x007A6F90`, the compiled defaults
   at `0x00712220` (initialised data; overwritten in memory by a valid
   `default.key`, §2.3).
2. Key values below 0x100 are Windows virtual-key codes. 0x100 = middle
   button, 0x101 = X button 1, 0x102 = X button 2, 0x103 = wheel up
   (positive delta), 0x104 = wheel down (§4.2).
3. Lookups: key of (command, slot) `0x00469AA0` (first entry with that
   command and slot; 0xFFFF if none); "is bound" `0x00469D90`; unbind
   (command, slot) `0x00469D70` (every matching entry). The key-config
   screen snapshots the live table to `0x007A6B18` on open (`0x00469D20`)
   and restores it on cancel (`0x00469D50`).
4. The command table `0x00712698` has 57 records of 12 bytes: key-down
   handler, key-up handler (either may be null), and a flag (u32) that
   lets the command run in key mode 2 (§4.1 r4). §3 lists them.

### 2. Key files

1. **Character file** `<save>\<name>.key` (`0x00469650`): when the account
   string `0x007A0500` is non-empty the path `<save><account>\<name>.key`
   is tried first, then `<save><name>.key` (name `0x007A05C4`). Content:
   u16 version 0x25, then the 114-entry table (0x476 bytes, no magic).
2. **`default.key`**: u16 magic 0x5357 (`"WS"`), u16 version 0x25, u16
   size 0x47A, then the table (0x47A bytes in total).
3. **Load at game start** (`0x0046AAE0`, from `0x0046AC70`):
   - Read the character file. It is accepted when exactly 0x476 bytes are
     read, the version is 0x25, every command 0–56 occurs as the command
     of at least one entry, and no two entries hold the same key other
     than 0xFFFF. The slot field is not checked. Accepted: the table
     becomes the live table; done (nothing is written).
   - Otherwise (`0x004698D0` with 1): open `<save>default.key`; if it
     opens and 0x47A bytes are read, that buffer is used, else the archive
     file `DATA\LOCAL\CMD\<lang>\default.key` (`<lang>` from
     `0x00525260`). A buffer with magic 0x5357, version 0x25 and size
     0x47A is copied over the compiled defaults `0x00712220`; any other
     buffer is dropped (the save-directory file, once read, is never
     followed by the archive file).
   - The live table := `0x00712220`; the mouse slots are rebuilt (§4.2);
     then both files are written (§2.4).
4. **Write** (`0x00469780`): only when the character file can be created
   (`CREATE_ALWAYS`): it gets version 0x25 + the live table; then
   `<save>default.key` is created with the 6-byte header + the live
   table. So the last character that fell back to the defaults defines
   the defaults of the next new character.
5. The 1.14d archives' `default.key` files carry version 0x22
   (`d2data.mpq`, 886 bytes) and 0x24 (`d2exp.mpq`, 1126 bytes) (measured):
   both fail the header test, so on a clean install the compiled table of
   §3 is the default.

### 3. Commands and default keys

Read from `0x00712220` (bindings) and `0x00712698` (handlers). "Gate"
= the handler does nothing while `0x0044DA30` or `0x00463DF0` returns
non-zero: `0x0044DA30` returns the client's exit flag `[0x007A0620]`
(`client/model.md` §1), `0x00463DF0` is true when there is no local
player or its mode is 0x11 (dead). `0x0044DB30` (command 2) returns the
game type `[0x007A0610]` (0 = single player, `client/msg-ui.md`), so the
party screen key works only in multiplayer games; `0x0044DCC0` is the
expansion flag `[0x007A04F4]`. `SetUIState(ui, mode, jump)` modes
0 on / 1 off / 2 toggle (`ui/panels.md` §2). Labels are the string keys
of the key-config screen's menu tables (§3.3). Keys in slot 1 / slot 0;
"—" = unbound. M2 = flag of §1 r4. The same data in machine form:
`key-commands.tsv` (§3.4).

<!-- rows -->
| Cmd | Label (tbl key) | Slot 1 | Slot 0 | Down handler → effect | Up handler | M2 |
|---|---|---|---|---|---|---|
| 0 | CfgCharacter | A | C | `0x00468940`: SetUIState(2, toggle, 1) | — | 0 |
| 1 | CfgInventory | I | B | `0x00468950`: SetUIState(1, toggle, 1) | — | 0 |
| 2 | CfgParty | P | — | `0x00468960`: if `0x0044DB30`: SetUIState(0x16, toggle, 1) | — | 0 |
| 3 | CfgMessageLog | M | — | `0x00468980`: SetUIState(0x18, toggle, 0) | — | 0 |
| 4 | CfgQuestLog | Q | — | `0x00468990`: gate; `0x004A3FE0(1)` | — | 0 |
| 5 | CfgChat | Enter | — | `0x004689B0`: if not `0x0044DA30`: SetUIState(5, toggle, 0) | — | 1 |
| 6 | CfgHelp | H | — | `0x004689D0`: gate; SetUIState(0x21, toggle, 0); registry `Diablo II\Help Menu` := 1 if absent or 0 | — | 0 |
| 7 | CfgAutoMap | Tab | middle button | `0x00468A30`: SetUIState(0xA, toggle, 0); if ui 0xA is now closed: `0x00457640(0)` | — | 0 |
| 8 | CfgAutoMapCenter | F9 | — | `0x00468A60`: `0x00457640(1)` | — | 0 |
| 9 | CfgAutoMapFade | F10 | — | `0x00468A70`: value `0x004576C0` := (value + 1) mod 4 (`0x004576F0`) | — | 0 |
| 10 | CfgAutoMapParty | F11 | — | `0x00468A90`: toggle `0x004577B0` / `0x004577C0` | — | 0 |
| 11 | CfgAutoMapNames | F12 | — | `0x00468AB0`: toggle `0x004577E0` / `0x004577F0` | — | 0 |
| 12 | CfgSkillTree | T | — | `0x00468AF0`: SetUIState(4, toggle, 1) | — | 0 |
| 13 | CfgSkillPick | S | — | `0x00468B00`: SetUIState(3, toggle, 0); `0x004A8CE0(0)` | — | 0 |
| 14–21 | CfgSkill1–8 | F1–F8 | — | `0x00468B90` + 0x30·k (k = 0–7; cmd 21 at `0x00468CE0`): gate; hotkey k (§3.1) | — | 0 |
| 22 | CfgBeltShow | ` (0xC0) | — | `0x00468F00`: if the player has an item at body location 8 and `0x00629BB0(item, 0x13)`: SetUIState(0x1F, toggle, 0) | — | 1 |
| 23–26 | CfgBelt1–4 | 1–4 | — | `0x00468F40`/`F50`/`F60`/`F70` → `0x00498C50`/`C90`/`CD0`/`D10`: use belt column 0–3 (§3.2) | — | 0 |
| 27–33 | CfgSay0–6 | NumPad 0–6 | — | `0x00468F80` + 0x10·k: send C→S 0x3F with value 0x19 + k (`0x004785B0`) | — | 1 |
| 34 | CfgRun | Ctrl | — | `0x00469000`: Run held := 1; if the player is in mode 2 (walk) and `0x00625480(P, 0xA, 0)`: mode 3, send C→S 0x53 | `0x004691C0`: Run held := 0; if run lock off and the player is in mode 3: mode 2, send C→S 0x54 | 0 |
| 35 | CfgRunLock | R | X button 2 | `0x00469060`: run lock := not run lock | — | 0 |
| 36 | CfgStandStill | Shift | — | `0x00469080`: Stand Still := 1 | `0x00469200`: Stand Still := 0 | 0 |
| 37 | CfgShowItems | Alt | X button 1 | `0x00469090`: SetUIState(0xD, on, 0) | `0x00469210`: SetUIState(0xD, off, 0); `0x00466FE0` | 0 |
| 38 | CfgClearScreen | Space | — | `0x004690A0` → `0x0044C6B0` | — | 0 |
| 39 | Cfgskillup | wheel up | — | `0x00469100`: gate; `0x004AA740(−1)` | — | 0 |
| 40 | Cfgskilldown | wheel down | — | `0x00469120`: gate; `0x004AA740(+1)` | — | 0 |
| 41 | Cfgcleartextmsg | N | — | `0x004691B0` → `0x004A01E0` | — | 1 |
| 42 | CfgSnapshot | Print Screen | — | none | `0x004FA7A0` | 1 |
| 43 | CfgTogglePortraits | Z | — | `0x00493840` | — | 0 |
| 44 | Cfgswapweapons | W | — | `0x00469140`: unless ui 0xC, 0x17 or 0x19 is open: `0x0048A730` | — | 1 |
| 45 | CfgToggleminimap | V | — | `0x00468AD0`: toggle `0x00457770` / `0x00457780` | — | 1 |
| 46–53 | CfgSkill9–16 | — | — | `0x00468D10` + 0x40·k (k = 0–7): expansion only (`0x0044DCC0`); gate; hotkey 8 + k | — | 0 |
| 54 | Cfghireling | O | — | `0x00469170`: expansion only; if the player has a hireling (`0x00478F20(P, 7)` ≠ −1) and `0x00408F20`: SetUIState(0x24, toggle, 1) | — | 0 |
| 55 | CfgSay7X | NumPad 7 | — | `0x00468FF0`: send C→S 0x3F with value 0x20 | — | 1 |
| 56 | — (not listed) | Esc | — | `0x004690B0`: unless `0x004B34A0` or `0x004A0000`: if ui 9 is open `0x0047E200(1)`; else if `0x00456300(0, 1)` = 0: `0x0047E090(1, 0)` | — | 0 |

Every slot-0 entry not named above is 0xFFFF. In table order the bindings
of command 21 (F8) sit at entries 90–91, after command 45; order is not
otherwise significant except for the first-match rules of §4.1.

#### 3.1 Skill hotkeys

1. Hotkey k (0–15) with ui 3 (skill speed bar) open: `0x004A9E60(k)`
   assigns the hovered skill to k; otherwise `0x004AA030(k)` uses it.
2. Use (`0x004AA030`): if the skill of k (`0x007C06C8[k]`) is not −1 and
   `0x004A9FC0` allows: last hotkey `0x007C0708` := k, `0x004786A0` with
   `0x007C0768[k]`, then the skill becomes the left skill
   (`0x00643BC0`) when `0x007C07B8[k]` ≠ 0, else the right skill
   (`0x00643C50`).
3. Commands 46–53 (hotkeys 9–16) have no default key in 1.14d.

#### 3.2 Belt keys

1. Use belt column c (`0x00498C50` + 0x40·c): gate; only when the byte
   `0x007BEFB0` = 1; calls `0x00498A90` with the player, its inventory,
   column c and "Shift down" = `GetAsyncKeyState(VK_SHIFT) & 0x8000`.
   Which item of the column is used and what Shift does: §Open
   questions 4 (belt slot numbering: `items/inventory.md` §3).

#### 3.3 Key-config menu tables

The key-config screen lists commands from one of two tables of 10-byte
rows (i32 command, u16 string id, i32 0; command 57 = separator row with
no text), chosen by `0x004A43E0`: expansion game → `0x00724468`, 62 rows
(separators at rows 7, 28, 35, 42, 49, 58); classic → `0x00724268`, 51
rows (separators at 6, 19, 25, 32, 38, 47); the row count goes to
`[0x007C0274]` and the table pointer to `[0x00724264]`. Commands 44–54
are listed only in the expansion table; command 56 (Esc) is in neither.
String ids: `string.tbl` 3924–3985, `patchstring.tbl` (1.14d
`Patch_D2.mpq` copy) 10833 `CfgSkillPick` and 11083 `CfgSay7X`,
`expansionstring.tbl` 22717–22727 (decoded from the English 1.14d
tables). `CfgMiniMap` ("Micromap", 3932), `CfgSkill*`-free ids 3951–3958
(`CfgBelt5`–`12`) and 21804 `CfgSay7` are in no menu row: unused.

#### 3.4 `key-commands.tsv`

One row per command 0–56: `cmd`; `string_id`, `string_key` (§3.3, `-`
for 56); `key1`, `key2` (default key of slot 1 / slot 0 as
`0xNNN:Name`, `-` = none); `down`, `up` (handler addresses of
`0x00712698`, `-` = none); `full_ok` (the M2 flag); `file_pos` (position
of the command's two entries in the default table: entries 2p (slot 1)
and 2p + 1 (slot 0)); `menu_classic`, `menu_exp` (row in the §3.3
tables, `-` = not listed). Generated from the 1.14d binary; the §B4
check reads it.

### 4. Dispatch

#### 4.1 Keyboard

1. The key handlers `0x0046A840` (down, messages 0x100 and 0x104) and
   `0x0046A940` (up, 0x101 and 0x105) are registered while the key mode
   `0x007A7418` (set by `0x0046AA20`) is non-zero. Mode 0 with the
   "keep key-up" argument set registers the key-up handler alone
   (table `0x00712974`), so held commands can still be released.
2. Key down: ignored when lParam bit 30 is set (auto-repeat: a held key
   fires once). Otherwise the first entry (table order) whose key equals
   the VK, whose command is < 0x39 and whose command has a down handler
   is taken; if its key is F4 (0x73) and Alt is down
   (`GetAsyncKeyState(VK_MENU)` < 0), nothing happens.
3. Key up: the first entry whose key equals the VK and whose command has
   an up handler; the repeat bit is not tested.
4. In key mode 2 only commands whose M2 flag is 1 run (both directions).
   A handler address that fails `IsBadCodePtr` is a fatal error.
5. Key mode changes (UI open / close hooks, `ui/panels.md` §2 r6):
   - game start `0x0046AC70`: 1; game end `0x0046ACB0`: 0;
   - ui 5 (chat) opens: 0, key-up kept; ui 23 (player trade) opens: 0,
     key-up kept;
   - ui 12 (NPC shop), 25 (stash), 26 (cube) open: 2;
   - ui 12, 23, 25, 26 close: 1; ui 5 closes: 1 unless one of ui 12, 23,
     25, 26, 27, 28, 29, 30, 32 is open;
   - the key-config screen opens (`0x004A5200`): 0, key-up not kept;
   - `0x00453EE0` / `0x00454150` around the latch `0x007A27B4`: 1 / 0
     (§Open questions 5).
6. Windows keys (VK 0x5B, 0x5C, 0x5D) are swallowed (`0x0044C5C0`).
   `WM_SYSCOMMAND` `SC_KEYMENU` and `SC_SCREENSAVE` are swallowed; `SC_MOVE`
   when `0x004F5A30` is non-zero (`0x0044C570`).

#### 4.2 Configurable mouse buttons and wheel

1. After every table change `0x004694A0` scans the live table: for each
   entry with key 0x100–0x104 and command < 0x39 it stores the command's
   handlers in the button slot: 0x100 down `0x007A6B10` / up `0x007A740C`;
   0x101 `0x007A7414` / `0x007A6B14`; 0x102 `0x007A7410` / `0x007A6F8C`;
   0x103 `0x007A7404`; 0x104 `0x007A7408` (wheel: down handler only). A
   wheel binding to a command that has an up handler is removed (key
   := 0xFFFF). Several entries on one button: the last in table order
   wins.
2. Middle down / up (0x207 / 0x208, `0x0044C4A0` / `0x0044C470`) and X
   button down / up (0x20B / 0x20C, `0x0044C4D0` / `0x0044C520`; high word
   of wParam 1 → 0x101, 2 → 0x102) call the stored handler; only while
   `0x007A061C` ≠ 0. No key-mode test applies to mouse buttons.
3. Wheel (`0x0044C400`): while `0x007A061C` ≠ 0, the signed wheel delta
   is added to an accumulator `0x007A06F8`; when |acc| > 0x77 the 0x103
   handler runs if acc > 0, the 0x104 handler if acc < 0, and acc := 0
   (one action per event, however large the delta).

#### 4.3 Left and right buttons, modifiers

1. Left (0x201 `0x0044BF40`, 0x202 `0x0044C0F0`) and right (0x204
   `0x0044C180`, 0x205 `0x0044C370`) are fixed. Each world action passes
   a flag word to `0x00462D00`: 8 when Run held or run lock is set (no
   inversion: Run with the lock on still runs), plus 4 when Stand Still
   is held (`0x0044BEC0`).
2. Stand Still is also re-sampled from the keys bound to command 36
   (both slots, `GetAsyncKeyState`) by `0x0044CE90` (caller `0x0044EFA0`).
3. On `WM_ACTIVATEAPP` (`0x0044C5E0`): wParam ≠ 0 calls `0x00455F20`
   (register arguments not read), clears Stand Still and calls
   `0x00466FE0` and `0x004FB130`; wParam = 0 clears Run held when the
   key of command 34 is bound and not physically down, and runs the
   left-button release (`0x0044C060`) when the left button state
   `0x0070F234` ≠ 0x10 (the right one likewise from `0x0070F2BC`).
4. What each button does on the world, its repeat while held (the
   right-button repeat `0x0044C2C0`, caller `0x0044F046`) and the C→S
   messages: §Open questions 2.

### 5. Key-config screen assignment

1. Assign key K to (command c, slot s) (`0x00469C20`): c must be < 0x38
   (command 56, Esc, cannot be reassigned). K must be allowed
   (`0x00469AE0`): any value ≥ 0x100; letters and digits; and VK 8, 9,
   0xC, 0xD, 0x10–0x14, 0x20–0x24, 0x2A, 0x2C–0x2F, 0x60–0x87, 0x91,
   0xBA–0xC0, 0xDB–0xDE; VK 1–4, 0x15, 0x17–0x19, 0x1B–0x1F, 0x25–0x29,
   0x2B, 0x5B–0x5D, 0x90 are refused. Else error string 3979
   `CantAssignKey`.
2. Wheel (0x103 / 0x104) on a command with an up handler: error 3978
   `CantAssignMW`. Print Screen (0x2C) on a command with a down handler:
   error 3979.
3. Then the first other entry holding K is unbound, and K is written to
   the (c, s) entry (if no such entry exists, K is written back where it
   was).

### B4. Original-defaults check (`client/ui.md` §B4)

The `original` preset of `d2-client::controls` must list the 57 commands
of §3 with exactly the slot-1 / slot-0 keys of the §3 table (114
bindings, compiled table `0x00712220`), mapping VK codes and 0x100–0x104
to the portable `Key` names; our `Action` names are the `string_key`
column of `key-commands.tsv` (command 56: `GameMenu`). Runnable check:

1. Unit (CI): build the 1140-byte table from `key-commands.tsv` (entries
   in `file_pos` order, slot 1 then slot 0; i32 cmd, u16 key or 0xFFFF,
   i32 slot) and compare with the preset's bindings.
2. Game file (`#[ignore]`, `D2_GAME_DIR`): the same 1140 bytes equal
   `Game.exe` bytes at file offset 0x312220 (`.data` raw offset 0x305000
   + 0xD220; SHA-256
   `a711045f3efb1de993c3890c6dbf1f7dad5750df857370cfe1fb0f1d1241cfdd`).
   Measured 2026-10-07: equal; the install's `default.key` equals header
   + table, and the 16 character `.key` files of the test machine equal
   version + table.

## Constants & data dependencies

Version 0x25, magic 0x5357, sizes 0x476 / 0x47A, 114 entries, 57
commands; wheel threshold 0x77; labels from `string.tbl` 3921–3977 and
`expansionstring.tbl` (`CfgSay7`, `CfgSkill9`–`16`, `CfgToggleminimap`,
`Cfgswapweapons`, `Cfghireling`).

## Randomness

None.

## Edge cases & original bugs

1. A held key never repeats a command (bit 30 test); a key that is bound
   twice runs only the first entry's command.
2. Several bindings on one mouse button: the last table entry wins (§4.2
   r1), the opposite of the keyboard's first match.
3. A valid character file is never rewritten at load; `default.key` in
   the save directory silently replaces the compiled defaults for every
   later character with no valid file.
4. The wheel accumulator is reset to 0, not reduced by 120: one event of
   delta 240 is one step.
5. Alt+F4 never reaches a command bound to F4.

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| fresh install, no `.key` files | live table = `0x00712220`; character `.key` (0x476 bytes) and `<save>default.key` (0x47A bytes) written | §2.3 |
| `.key` with version 0x24 | rejected → defaults | §2.3 |
| `.key` with I bound to commands 0 and 1 | rejected (duplicate key) | §2.3 |
| archive `d2exp.mpq` `default.key` | header version 0x24 → dropped | §2.5 |
| key A down, held, auto-repeat messages | ui 2 toggled once | §4.1 r2 |
| wheel delta +240 in one message | command 39 once | §4.2 r3 |
| Ctrl down with run lock on, then left click | flags 8 | §4.3 r1 |
| Shift + left click | flags 4 (| 8 if running) | §4.3 r1 |
| NPC shop open, press I | nothing (M2 = 0) | §4.1 r4 |
| chat open, press I | nothing (key-down not registered) | §4.1 r5 |
| `original` preset vs `0x00712220` | identical 114 bindings | §B4 |
| `key-commands.tsv` written as a table (§B4 r1) | 1140 bytes, SHA-256 `a711045f…cfdd`; entry 0 = `00 00 00 00 41 00 01 00 00 00`; entries 90–91 = command 21 (F8, slot 1; none, slot 0) | §B4, §3.4 |
| expansion game, key-config menu | 62 rows, row 3 = `Cfghireling` (command 54) | §3.3 |
| classic game, key-config menu | 51 rows, no Swap Weapons / Hireling / Skill 9–16 rows | §3.3 |

## Provenance

1.14d `Game.exe`: `.\UI\CmdTbl.cpp` functions `0x004694A0`–`0x0046ACB0`,
handler bodies `0x00468940`–`0x00469210` (disassembly,
`tools/ghidra/disasm.py`), window handler tables `0x00712944` and
`0x0070F2C4` read from the binary, command and binding tables dumped with a
script (outside the repo). Mouse handlers `0x0044BEC0`–`0x0044CE90`.
UI hooks `0x00455720` / `0x00455AE0` (jump tables `0x00455A40` /
`0x00455E80` read from the binary; `index/switches.tsv` renumbers their
cases). Labels from the 1.14d `string.tbl` / `expansionstring.tbl`;
archive `default.key` headers measured. Menu tables `0x00724268` / `0x00724468` and selector `0x004A43E0`, string ids decoded from the English 1.14d `string.tbl`, `expansionstring.tbl` and the `Patch_D2.mpq` `patchstring.tbl`; the compiled table compared byte for byte with the install's `default.key` and 16 character `.key` files (2026-10-07, `claude/pc2-ui`). No D2MOO code used.

## Open questions

1. **Answered** (2026-10-07, §3.3: the menu tables `0x00724268` /
   `0x00724468` give every label; `CfgMiniMap` is unused). Was: command
   → label mapping of the key-config screen.
2. ~~Left / right button semantics: `0x00462D00`'s first argument, what a
   click on a unit / the ground / with Shift does, the held-button repeat
   and its tick; which C→S messages go out.~~ More than two reads
   (`0x00462D00`, `0x0044C2C0`, `0x0044F046`): recording `controls-0001`
   (`docs/handoff/pc2-rec-pc2-ui.md`).
3. **Answered** (2026-10-07, §3 intro): `0x0044DA30` = exit flag,
   `0x00463DF0` = no player or player dead, `0x0044DB30` = game type
   (multiplayer).
4. Belt use `0x00498A90`: which slot of column c, the Shift branch
   (give to hireling?), and the byte `0x007BEFB0`.
5. **Answered** (2026-10-07): the latch `[0x007A27B4]` marks the gold
   amount dialog (`0x00454150` opens it and sets key mode 0, key-up
   kept; `0x00453EE0` closes it and sets 1; `ui/inventory.md` §9).
6. ~~When the key-config screen's Accept writes the files (no caller of
   `0x00469780` other than the load path).~~ Recording `controls-0002`
   (file times of `<save>\<name>.key` and `default.key` after Accept).
7. ~~Which of slot 0 / slot 1 the config screen shows as "Key/Button
   One" (`CfgPrimaryKey`).~~ The display sort `0x00469450` puts slot ≠ 0
   first; the column is settled by capture `controls-0002`.
