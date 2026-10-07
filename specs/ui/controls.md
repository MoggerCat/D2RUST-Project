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
| Summary | 41–52 |
| Inputs | 53–61 |
| Outputs / state changes | 62–69 |
| Rules | 70–71 |
|   1. Binding table | 72–90 |
|   2. Key files | 91–123 |
|   3. Commands and default keys | 124–194 |
|   4. Dispatch | 195–264 |
|   5. Key-config screen assignment | 265–280 |
|   6. World clicks (left / right button; answers OQ 2 in part) | 281–558 |
|   7. Gates and belt use (answers OQ 3, OQ 4, OQ 5) | 559–600 |
|   B4. Original-defaults check (`client/ui.md` §B4) | 601–610 |
| Constants & data dependencies | 611–617 |
| Randomness | 618–621 |
| Edge cases & original bugs | 622–634 |
| Test vectors | 635–659 |
| Provenance | 660–678 |
| Open questions | 679–719 |
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
non-zero (meaning: §Open questions 3). `SetUIState(ui, mode, jump)` modes
0 on / 1 off / 2 toggle (`ui/panels.md` §2). Labels are the
`string.tbl` / `expansionstring.tbl` keys of the key-config screen,
matched by effect (§Open questions 1). Keys in slot 1 / slot 0;
"—" = unbound. M2 = flag of §1 r4.

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
| 9 | ? | F10 | — | `0x00468A70`: value `0x004576C0` := (value + 1) mod 4 (`0x004576F0`) | — | 0 |
| 10 | ? | F11 | — | `0x00468A90`: toggle `0x004577B0` / `0x004577C0` | — | 0 |
| 11 | ? | F12 | — | `0x00468AB0`: toggle `0x004577E0` / `0x004577F0` | — | 0 |
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
| 41 | ? | N | — | `0x004691B0` → `0x004A01E0` | — | 1 |
| 42 | CfgSnapshot | Print Screen | — | none | `0x004FA7A0` | 1 |
| 43 | ? | Z | — | `0x00493840` | — | 0 |
| 44 | Cfgswapweapons | W | — | `0x00469140`: unless ui 0xC, 0x17 or 0x19 is open: `0x0048A730` | — | 1 |
| 45 | CfgToggleminimap? | V | — | `0x00468AD0`: toggle `0x00457770` / `0x00457780` | — | 1 |
| 46–53 | CfgSkill9–16 | — | — | `0x00468D10` + 0x40·k (k = 0–7): expansion only (`0x0044DCC0`); gate; hotkey 8 + k | — | 0 |
| 54 | Cfghireling | O | — | `0x00469170`: expansion only; if the player has a hireling (`0x00478F20(P, 7)` ≠ −1) and `0x00408F20`: SetUIState(0x24, toggle, 1) | — | 0 |
| 55 | CfgSay7 | NumPad 7 | — | `0x00468FF0`: send C→S 0x3F with value 0x20 | — | 1 |
| 56 | ? | Esc | — | `0x004690B0`: unless `0x004B34A0` or `0x004A0000`: if ui 9 is open `0x0047E200(1)`; else if `0x00456300(0, 1)` = 0: `0x0047E090(1, 0)` | — | 0 |

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
   messages: §6 (the remaining predicates: §Open questions 8).

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

### 6. World clicks (left / right button; answers OQ 2 in part)

1. **Events → action kind.** The fixed button handlers call the click
   dispatcher `0x00462D00(kind ECX, x EDX, y, mods)` only while in game
   (`[0x007A061C]` ≠ 0) and with a local player unit (type 0); `mods` =
   §4.3 r1 flag word (8 run, 4 Stand Still) except where noted:

   | Kind | Handler | Event | Position | Result kept in |
   |---|---|---|---|---|
   | 0 | `0x0044BF40` | left down (0x201) | event | `[0x007A0650]` (left held) |
   | 1 | `0x0044C000` | left held (r6) | current mouse | — |
   | 2 | `0x0044C060` | left up (0x202; also the focus-loss release, §4.3 r3) | current mouse | `[0x007A0650]` := 0 first |
   | 3 | `0x0044C180` | right down (0x204) | event | `[0x007A0654]` (right held) |
   | 4 | `0x0044C2C0` | right held (r6) | current mouse | — |
   | 5 | `0x0044C370` | right up (0x205) | event; `mods` = the event's wParam (MK_SHIFT 4, MK_CONTROL 8) | `[0x007A0654]` := 0 first |

   Down events are consumed (event +0x18 := 1). Right down is ignored in
   open mode 3, and not dispatched (not consumed) when the click lies in
   an open panel's half: open mode 2 with 0 < x < W / 2 and 0 < y < H −
   47, or open mode 1 with W / 2 < x < W and 0 < y < `[0x007A521C]`
   (`0x0044C180`). Left down / up reach the dispatcher only when no
   panel consumed the event first (`ui/panels.md` §4.4).
2. **Dispatcher** `0x00462D00`: returns 0 at once while ui 9 (game menu)
   is open (`0x004538D0(9)`), and while the per-pass latch
   `[0x007A5264]` is set unless x = y = 0; else latch := 1 (cleared once
   per client loop pass at `0x0044F24C`, before the draw). It builds a
   click record `C`: +0 flags, +4 P, +8 the hovered unit `U`
   (`0x00467A10`, `client/model.md` hover), +0xC / +0x10 the click's
   world position (`0x0045AFF0` maps the screen x, y in place,
   `render/camera.md`), replaced by `U`'s position (`0x0045ADF0`,
   `0x0045AE20`) when `U` is an object (type 2) or an item (type 4),
   +0x14 / +0x18 the walk codes (r4), +0x1C the skill. Flags: kind 0, 1,
   2 → 1 (left), kinds 3, 4, 5 → 2 (right); kind 0, 3 → 4 (press), 1, 4
   → 8 (held), 2, 5 → 0x10 (release); `mods` & 4 → 0x20 (Stand Still);
   `mods` & 8 and P's stamina (stat 10) ≠ 0 → 0x40 (run). Kinds 0, 3
   first call `0x00467A70` with a hovered unit, else `0x00466FE0`; kinds
   2, 5 call `0x00466FE0`. Kind > 5 → returns 1.
3. **Press / held / release filter** `0x00462930(C)`: release → the
   press latch `[0x007A526C]` := 0, no action; press → `[0x007A526C]` :=
   1 + (`U` ≠ none), action; held → action only when `[0x007A526C]` ≠ 0
   and (no `U`, or `U` is a player, monster or type-5 unit and P's mode
   is not 2, 3 or 6 (walk, run, town walk)).
4. **Per kind** (all start with r3):
   - 0 left down (`0x004629A0`): skill := P's left skill (`0x00620190`).
     If `0x00464600(P, skill)` = 0 (P holds a cursor item or is not a
     player; or P's mode is 0, 4, 7–12, 17 or 19; or mode 13 with class
     3, mode 14 with class 6, modes 15–16 with classes 4–6; mode 18 as
     `0x004645B0(skill)` decides): cursor state 6 → `0x00453EC0` (C→S
     **0x27** with the cursor unit's GUID in both fields); cursor state 8
     → C→S **0x4C** [−1]; then, holding a cursor item → C→S **0x17**
     [item GUID] (drop it, `DropItem`), returns 0. Else flags |= 0x80 and
     the action (r5); returns 1.
   - 1 left held (`0x00462A20`): skill := left skill; nothing unless
     `0x00464600` ≠ 0. "Speed changed" = the path's speed
     (`0x006486C0`) differs from P's walk speed × stat 67 / 100 while
     not running, or equals it while running. With a unit (+8): when it
     is not P's current target (`0x004648F0`) or the speed changed, P's
     path is re-targeted (`0x00648B90`) and flags |= 0x80; the action
     (r5) runs either way. Without a unit: Stand Still clear and P's
     player data +0x154 ≠ 0 (an interaction in progress) → nothing; else
     (Stand Still clear) flags |= 0x80; the action runs.
   - 2 left up (`0x00462B40`): when P has a hireling (`0x00478F20(0)` ≠
     −1), a cursor unit and that unit's mode record +8 = 2:
     `0x00467410(0)`; then `0x00462370`: P not busy (`0x004648F0` = 0) in
     mode 2 or 6 → `0x00461840(1)`, in mode 3 → `0x00461840(3)` (the
     path end sent as walk / run code 1 / 3, `0x00481030`); returns 0.
   - 3 right down / 4 right held (`0x00462BA0` / `0x00462C20`): skill :=
     P's right skill (`0x006201D0`); when `0x00464600` ≠ 0: flags |=
     0x80 if the skill's `range` (+0x14) is 1 (`h2h`) and run is not
     set, then the action (r5).
   - 5 right up (`0x00462CA0`): the hireling / cursor-unit step of kind 2
     only; returns 0.
5. **Action** `0x004625B0(C)` after the walk-code set-up `0x004621D0`:
   walk codes := 1 / 2 (walk to a point / to a unit), or 3 / 4 with run
   (flag 0x40); a right click with a skill that is not usable in town
   (skills +5 & `[0x006CE268]` = 0) while P's room is in town
   (`0x0061AB00`) plays the player event sound (`0x004CB9C0`) and stops;
   a `passive` right skill (+4 & 0x10) stops; P's pending interaction
   (player data +0x154) is cleared (0x13: `0x0045C470` with a cursor
   item); no skill → stop; the target is re-picked (`0x00467880(&x, &y,
   flags, …)`, Stand Still or a right click without a unit forces a
   location target) into +8; then cursor state 6 / 8 → as kind 0. The
   decision then picks one sender, each ending in the mode request and
   message of r7: on a unit → interact (`0x00461DC0`, code 0x13 for NPCs,
   objects, items, warps), attack / skill on the unit (`0x00461700(1)`),
   walk to it (`0x00461C70`, `0x00461890`, `0x004619E0`); on a point →
   skill at the point (`0x00461700(0)`) or walk / run there
   (`0x00461840`). The exact predicate order of `0x004625B0` and of
   those senders (melee range, `0x00645460`, `0x00643860`,
   `0x00465C60`, `0x004610C0`, `0x00462560`, `0x004623C0`): §Open
   questions 8.
6. **Held repeat.** Every client loop pass (`0x0044EFA0`, before the
   receive; `audio/sound-table-2.md` §14.2) runs kind 1 while
   `[0x007A0650]` ≠ 0 and `[0x007A0658]` = 0 (`0x0044F039`), then kind 4
   while `[0x007A0654]` ≠ 0 (`0x0044F046`), at the current mouse
   position. `[0x007A0658]` := 1 when an interaction starts
   (`0x0044BEF0`, from `0x00461DC0`) stops the left repeat until the next
   left down / up. The per-pass latch (r2) allows one positioned dispatch
   per pass. Repeats are therefore per loop pass, not per server tick;
   the send ticks of a held button stay a recording case (OQ 2).
7. **Skill codes** (`0x00461700(onUnit)`, only while mouse y ≤
   `0x00454970()`): on a unit (target +8; none → type 6, GUID −1): left
   → 0x06, or 0x09 held; with Stand Still 0x07 / 0x0A held; right →
   0x0D / 0x10 held, Stand Still 0x0E / 0x11 held. At a point (+0xC,
   +0x10): left 0x05 / 0x08 held; right 0x0C / 0x0F held. The code goes
   to `0x00481030(code, P, a, b)`: the client mode request
   (`0x00480C10`, `client/model.md` §8) and, through `0x00480B40`, the
   C→S message whose id **is** the code (codes 1–0x11; layouts
   `sim/client-messages.tsv`: point codes `[x u16][y u16]`
   `0x004785D0`, unit codes `[type u32][GUID u32]` `0x004786A0`); code
   0x13 sends nothing there (the interact message is sent by its own
   path). So a left click on the ground without Stand Still is a walk
   (code 1 / 3), never 0x05.

8. **Decision order of `0x004625B0(C)`** (answers OQ 8 for the action;
   notation: P = C+4, U = C+8, skill = C+0x1C, F = C+0 flags, T / g =
   U's type / GUID, row = the skill's `skills.txt` record (skill entry
   +0, `0x00644140`), rng = `range(P, skill)` (`0x00645460`,
   `skills/use.md` §3 r6: 0 none, 1 h2h, 2 rng, 4 loc; `both` already
   resolved), SS = F & 0x20 (Stand Still), R = F & 2 (right button).
   Flag columns by `skills.txt` bit (`data/fields.tsv`; mask table
   `0x006CE268`, entry n = 1 << n): `passive` 4, `InTown` 8,
   `SearchEnemyXY` 21, `SearchEnemyNear` 22, `SearchOpenXY` 23,
   `TargetCorpse` 24, `TargetPet` 25, `TargetAlly` 26, `TargetItem` 27,
   `AttackNoMana` 28, `interrupt` 31.
   1. Set-up `0x004621D0` (r5 first part): with a skill, R and rng ≠ 1:
      no row → stop; row lacks `InTown` and P's room is in town → event
      sound, stop; `passive` → stop. Walk codes (r5). Pending
      interaction cleared (r5). **No skill → stop** (so the action below
      always has a skill). Re-pick (r10) into U. Cursor state 6 → C→S
      0x27 (as kind 0); state 8 → C→S 0x4C [−1]; either way the action
      goes on.
   2. **Unit or point.** No row, or no U → *point* (step 4). Else, when
      R and rng ≠ 1: U an object or item (type 2 / 4): with `TargetItem`
      remember "item skill" (b := 1) and go on to the corpse test below;
      without it → *point*. U type 5 (a tile / warp): F |= 0x100, →
      *point*. Then (all other cases): U a monster (type 1) that is dead
      (`0x00464820`: flag 0x10000, or mode 0 / 12) and the row lacks
      `TargetCorpse` → *point*; else → *unit* (step 3).
   3. **Unit** (T, g):
      1. Not b, U of type 2, 4 or 5, R and rng ≠ 1 → nothing.
      2. Skill mode (entry +8, `0x00643860`) = 0: SS → nothing; else
         go to 3.6.
      3. Row has `TargetItem`: the use check (r9.1) fails → nothing;
         else **skill on unit** (`0x00461700(1)`, r7).
      4. "Act on it" := U's flag +0xC4 bit 2 is set and (row has
         `TargetPet` or `TargetAlly`, or the hostility test r9.7 is
         true). When true: T = 2 → SS: nothing, else the object sender
         (r9.4); T = 0 → both U's and P's rooms in town: the town
         player sender (r9.5), else the attack sender r9.3 with type 0;
         other T → U's room in town and the row (by id, `0x00462560`)
         lacks `InTown` → nothing; else the attack sender r9.3 (T, g).
      5. Not "act on it": SS → the use check, then **skill at the
         point** (`0x00461700(0)`); no SS → 3.6.
      6. T ≠ 1 → the interact sender r9.2 (T, g). T = 1 (monster): only
         on a press (F & 4): its `monstats` row has `interact` (byte
         +0xD bit 1) → the interact sender (1, g); else **walk** to it:
         code C+0x18 (2 walk / 4 run to unit) with (1, g). A held or
         released click on a monster here sends nothing.
   4. **Point.** When SS, or R and rng ≠ 1: the use check (r9.1) with
      its use-state output s; fails → nothing; then **skill at the
      point** (`0x00461700(0)`, then `0x00467A70(0)`) unless the skill
      (after the r9.1 fallback) is id 0 (Attack), s = 1 (no mana) and
      not SS, which falls through to the walk. Walk: the step clamp
      r9.6 passes → `0x00461840` (r9.8) with walk code C+0x14 (1 / 3)
      to the clamped point, then `0x00467A70(0)`; else nothing.

   So: left on ground → walk / run; Stand Still + left / right on ground
   → skill at point; right with an h2h skill on ground → walk; right
   with a ranged skill on ground → skill at point; left on a hostile
   monster → attack sender; left on an NPC (`interact`) → interact
   sender; left on a non-hostile, non-interact monster → walk to unit.
9. **Senders** (each ends in `0x00481030(code, P, type, GUID)`, r7, or
   in nothing):
   1. **Use check** `0x004610C0(&s)` (EDI = P, EBX = &skill): P not the
      local player → passes. No skill or no row → fails. s := use_state
      (`0x004D9FC0` = `skills/use.md` §2 `0x00647960`, plus code 8 for
      the local player while `[0x007A0498]` < `[0x007A04FC]`). s ∈ {1,
      2, 4} and the row has `AttackNoMana` → the skill becomes the
      Attack entry (`0x006439F0`) and the state is recomputed. Final
      state 0 or 5 → passes; else the refusal sound of that state in
      table `0x00711DDC` (u16 per state, count `[0x00711EF0]`, else fatal
      0x2CB) plays when non-zero (`0x004CB9C0`), fails.
   2. **Interact** `0x00461DC0(T, g)`, by U's type (jump table
      `0x004621AC`: 0 → `0x00461F61`, 1 → `0x00462030`, 2 →
      `0x00461DF2`, 3 → `0x00462057`, 4 → `0x00461EE5`, 5 →
      `0x00461FB7`); d = unit distance (`0x00641530`), "clear" =
      `0x00622B50(P, U, 0x804)` = 0; "pend" = pending interaction
      (player data +0x150..+0x15C := 1, 0x13, T, g; `0x00460780`);
      "stop repeat" = `[0x007A0658]` := 1 (`0x0044BEF0`, r6); code
      **0x13** = the interact mode request (`client/model.md` §8; the
      C→S 0x13 leaves from the mode machine, `0x00480930`):
      - player: d ≤ 4 and clear → 0x13; else code C+0x18 (T, g), pend.
      - monster: reach 2 when its `monstats` row has `interact`, else 5;
        then the tail.
      - missile / other / none: reach 5, the tail.
      - tail: a monster with `npc` and `interact` (byte +0xD bits 0, 1):
        its path stops (`0x00648730`), C→S **0x59** [type][g][x][y] with
        its current position (`0x00478700`, `MakeEntityMove`), its
        monster data +0x28 |= 1, `0x00480E70(U, 1)`. Then d > reach →
        code C+0x18 (T, g) and pend; else 0x13 (T, g).
      - object: held (F & 8) → nothing. notify := 1, except class 59
        (town portal) while P has state 102 (`just_portaled`) → 0.
        `0x00623660(P, U)` (P stands at the object) and clear → notify:
        0x13 and stop repeat; else nothing. Otherwise: already pending
        on the same (T, g) (`0x00461D60`) → nothing; else walk
        (`0x00461840`, code C+0x14) to the object's position, then when
        notify: pend and stop repeat.
      - item: held → nothing. d ≤ 4 and clear → 0x13 and stop repeat.
        Else already pending (T, g) → nothing; else code C+0x18 (T, g),
        pend, stop repeat.
      - tile (warp, type 5): `[0x007A048C]` < `[0x007A04C8]` → nothing.
        d > 4 → code C+0x18 (T, g) and pend; else 0x13 (T, g). Then
        `[0x007A04C8]` := `[0x007A048C]` + 500.
   3. **Attack** `0x00461C70(T, g)`: use check fails → nothing. The
      target by (g, T) (`0x00463990`); none → nothing. P in melee range
      of U (`0x00622C40(P, U, 1 if U is moving)`, moving =
      `0x00622D00`) → skill on unit. Else: the row's `srvdofunc` (+0x2E)
      = 0x13 (Inferno, Arctic Blast) → the approach r9.9. rng 1 or 4:
      SS → skill at the point; else code C+0x18 (T, g), then pending
      (6, T, g) when F & 1 and pending (0xD, T, g) when F & 2. Other rng
      → skill on unit.
   4. **Object with a skill** `0x00461890(g)`: object (g, 2) lookup;
      none → nothing; no `objects.txt` row → fatal 0x4C4. notify as in
      r9.2; P at the object and clear → notify: code 0x13 (2, g); else
      walk (code C+0x14) to the object and, when notify, pending (0x13,
      2, g).
   5. **Player in town** `0x004619E0` (EDI = g): d < 3 and clear → code
      0x13 (0, g); else code C+0x18 (0, g) and pending (0x13, 0, g).
   6. **Walk clamp** `0x004623C0(P)` (EBX = C): p = P's position (static
      position for types 2, 4, 5, else the path position); dx, dy =
      C+0xC − px, C+0x10 − py. |dx| ≥ 0x100 or |dy| ≥ 0x100 → fail; dx =
      dy = 0 → fail. len := `0x00474080(dx, dy)` = (max(|dx|, |dy|) ×
      0x3D7 + min × 0x197) >> 10, at least 1. Minimum t: press with
      run 5; press without run 3; held 4, or 5 with run; release 0.
      len < t → f := (float32)(t / len), dx := trunc(dx × f), dy :=
      trunc(dy × f) (x87, `__ftol2` truncation). C+0xC, C+0x10 := p +
      (dx, dy). Then with n = `[0x007A0498]` (client update counter):
      P a player, F & 8 (held), P's mode 2, 3 or 6 and n −
      `[0x007A5268]` < 7 (unsigned) → fail; else `[0x007A5268]` := n,
      pass. So a held walk re-sends at most every 7 client updates.
   7. **Hostility** `0x00465C60(P, U)` → 1 "act on it". a / b := (type,
      GUID) of P / U (P none → (6, −1)).
      - P's room in town: U a dead player (flag 0x10000, mode 0 or
        0x11) and `0x0047A4F0(a GUID, U GUID)` = 0 → 0. P a player, U a
        monster with an owner in the pet list (`0x00479150`,
        `[0x007BB5BC]`, `client/model.md` §14) → 1 only when
        `0x00464EC0(P, U)` (P's left or right skill row has `TargetPet`
        and U's owner is P's GUID). Else 1.
      - not in town: a monster side with a pet-list owner becomes (0,
        owner); P with flag-2 (+0xC8) bit 10 and U +0x94 = 0 → a := (0,
        P +0x98) (the code tests U's +0x94 here); U likewise with its own
        +0x94 / +0x98. a = b → 0. Both type 0: U dead (`0x00464820`) and
        `0x0047A4F0(a GUID, U GUID)` ≠ 0 → 1; else the relation flag
        8 (hostile) of `0x004DC440(a GUID, b GUID, 8)`. b a monster:
        `monstats2` `alSel` (`0x004638A0(class, 4)`) → 1; `noSel` (bit 5)
        → 0. a a player and b an object or item → 1. Else 1 when the
        alignments differ (`0x00650D70(P, U)` = 0).
   8. **Walk to a point** `0x00461840(x, y, code)` (EDI = P): P's path
      target := (x, y) (`0x00648AD0`), path compute (`0x00649970(path,
      P, 0)`, `sim/pathing.md` §3); no path → nothing; else code with
      the path's end point (`0x00648A40`, `0x00648A60`).
   9. **Approach** `0x00461B40` (`srvdofunc` 0x13): SS → skill on unit.
      range := `0x00646CA0(P, row +0x64, row id, skill level
      0x006442A0(P, skill, 1))`, dist := `0x006416D0(P, U)`; dist ≤
      range → skill on unit. Else pending (6, T, g) when F & 1, (0xD,
      T, g) when F & 2; then walk (r9.8, code C+0x14) to p + (U − p) ×
      (dist − range) / dist (per axis, integer, truncating).
10. **Target re-pick** `0x00467880(&x, &y, F, force)` (force = SS, or R
    without U): with U: a dead player → keep U. U dead and the row lacks
    `TargetCorpse` → the hover is dropped (`0x00466DE0`), (x, y) := U's
    position, U := none. With no U and (x, y) = P's position →
    `0x004C51E0(&y)`. Held left without SS while P moves
    (`0x00622D00`) → keep. Then only when use_state = 0: row
    `SearchEnemyNear` → `0x00467490`; no U, `SearchEnemyXY` and force →
    `0x00467660`; `SearchOpenXY` → `0x004677B0(&x)`; these return the
    new U (their search rules: §Open questions 8).

### 7. Gates and belt use (answers OQ 3, OQ 4, OQ 5)

1. **Gates** of §3: `0x0044DA30` = `[0x007A0620]`, the game-exit flag
   (set to 1 by `0x0044D520` from the gate `0x00453910` (`0x004539D3`),
   `0x004A2CB0`, `0x004B4380`, `0x004B9180`, `0x004B9210`, and by
   `0x0044DD60`, `0x0044E380`; set at game start from `0x0044BBA0`;
   never cleared in a game); `0x00463DF0` = no local player, or the
   local player in mode 0x11 (dead). `0x0044DB30` = the game type
   `[0x007A0610]` (0 single player, `ui/control-panel.md` Inputs): the
   party command (cmd 2) works only in a multiplayer game.
2. **Belt keys** (cmds 23–26, `0x00498C50` + 0x40·c): gate (r1); only
   when the column-ready byte `[0x007BEFB0 + c]` = 1 (written by
   `0x00498D50(c, v)`, c < 4, from the item handlers `0x004C4130`,
   `0x004C42A0`, `0x004C4C70`: `client/msg-stats-items.md`); then
   `0x00498A90(inventory, P, c, shift)` with shift =
   `GetAsyncKeyState(VK_SHIFT) & 0x8000`.
3. **Belt use** `0x00498A90`: shift := 0 in a classic game; nothing
   with a cursor item or with ui 9 open. The item is belt slot `c`
   (`0x0063C7F0(inventory, c)`: the bottom row, `items/inventory.md` §3);
   none → nothing. Its `items` record (missing → fatal 0xA64) passes
   `0x00498A20` when `quest` (+0x12A) and `unique` (+0x129) are not 1
   (else `0x0049FF90` and stop) and `useable` (+0x11D) is 1 (else the
   player event sound `0x004CB9C0` and stop); and `0x004C2240(item)` =
   0 (`ui/inventory.md` §9 r2). Then C→S **0x26** [item GUID u32]
   [shift u32: 0 or 0x8000][0 u32] (`0x004786D0`; `UseBeltItem`),
   `0x004C21F0(item)`; with shift and a hireling (`0x00478F20(P, 7)` ≠
   −1): hireling sound `0x004CBDE0(P, 1, 0x54)` when the item is of type
   0x4C, 0x51 or 0x50, else `0x004CBDE0(P, 1, 0x55)`; without:
   `0x004C1E20(inventory, 0, 0, 0)` and sound `0x004B9A00(0, 0, 0)`. The
   belt hover `[0x007BEF94]` := 0 in each send path.
4. **Key mode around the latch `[0x007A27B4]`** (§4.1 r5): the gold
   dialog (`ui/panels-2.md` §21.6–§21.7): open → key mode 0 (key-up
   kept), close → key mode 1.
5. **Pointer button meanings** for the d2rs input (`client/ui.md` §B4;
   code `PointerButton`, `UiFrame::unhandled`): Left and Right are the
   fixed world buttons of §6 (never rebindable); Middle and the X
   buttons run whatever command is bound to keys 0x100–0x102 (§4.2;
   default middle = command 7, automap). Shift / Ctrl / Alt meaning
   comes only from the bindings of commands 36 / 34 / 37 (§3, §4.3),
   except right up, which reads the event's MK_SHIFT / MK_CONTROL (§6
   r1). An event no panel consumed becomes a §6 dispatch.

### B4. Original-defaults check (`client/ui.md` §B4)

The `original` preset of `d2-client::controls` must list the 57 commands
of §3 with exactly the slot-1 / slot-0 keys of the §3 table (114
bindings, compiled table `0x00712220`), mapping VK codes and 0x100–0x104
to the portable `Key` names. The check compares the preset with a fresh
read of `0x00712220` (§Test vectors). The other §B4 items: pointer
button meanings and modifiers §7 r5; events no panel takes → world
intents §6; repeat while held §6 r6 (send ticks: §Open questions 2).

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
| left down on open ground, no modifier | a walk (code 1, C→S 0x01 `[x][y]`), not 0x05 (path through §6 r5: to confirm with OQ 8 / the OQ 2 trace) | §6 r7 |
| left down on open ground 1 subtile east of P (dx = 1, dy = 0), no run | walk clamp: len 1 < t 3 → f = 3, target dx = 3 | §6 r9.6 |
| left held on ground while P walks, 3 client updates after the last walk send | nothing sent (n − last < 7) | §6 r9.6 |
| left down on an `interact` NPC 8 subtiles away | C→S 0x59 [1][g][x][y], then walk-to-unit code 2 and a pending interaction (0x13, 1, g) | §6 r9.2 |
| right button held on open ground, P standing | press code 0x0C, then code 0x0F on each client loop pass that passes §6 r2–r3 (to confirm, OQ 2) | §6 r6, r7 |
| right down at (100, 200), open mode 2 (character panel) | not dispatched, not consumed | §6 r1 |
| left down with an item on the cursor over the ground | C→S 0x17 `[item GUID]` | §6 r4 |
| key 1, column ready, Shift, hireling present, healing potion | C→S 0x26 `[GUID][0x8000][0]` | §7 r3 |
| key 1, classic game, Shift | C→S 0x26 `[GUID][0][0]` | §7 r3 |

## Provenance

1.14d `Game.exe`: `.\UI\CmdTbl.cpp` functions `0x004694A0`–`0x0046ACB0`,
handler bodies `0x00468940`–`0x00469210` (disassembly,
`tools/ghidra/disasm.py`), window handler tables `0x00712944` and
`0x0070F2C4` read from the binary, command and binding tables dumped with a
script (outside the repo). Mouse handlers `0x0044BEC0`–`0x0044CE90`.
World clicks `0x00462D00` (jump table `0x00462EC0`), `0x00462930`,
`0x004629A0`, `0x00462A20`, `0x00462B40`, `0x00462BA0`, `0x00462C20`,
`0x00462CA0`, `0x004621D0`, `0x004625B0`, `0x00461700`, `0x00481030`,
`0x00480B40`, `0x00464600`, `0x00453EC0`, `0x0044C000`, held repeats in
`0x0044EFA0` (`0x0044F039`, `0x0044F046`), latch clear `0x00462920`;
gates `0x0044DA30`, `0x00463DF0`, `0x0044DB30`; belt `0x00498C50`,
`0x00498A90`, `0x00498A20`, `0x00498D50`, `0x004786D0`.
UI hooks `0x00455720` / `0x00455AE0` (jump tables `0x00455A40` /
`0x00455E80` read from the binary; `index/switches.tsv` renumbers their
cases). Labels from the 1.14d `string.tbl` / `expansionstring.tbl`;
archive `default.key` headers measured. No D2MOO code used.

## Open questions

1. Command → label mapping of the key-config screen (its row table was
   not found): commands 9, 10, 11, 41, 43, 45, 56 are unnamed, and
   `CfgMiniMap` ("Micromap") vs `CfgToggleminimap` is unassigned. Read
   the config-screen draw `0x004A5270` / `0x004A47C0`.
2. *Partly answered* (2026-10-07, §6: action kinds, click record,
   filters, held repeat per loop pass, codes → C→S ids; open: §Open
   questions 8 and the send ticks). Was: Left / right button semantics:
   `0x00462D00`'s first argument, what a click on a unit / the ground /
   with Shift does, the held-button repeat and its tick; which C→S
   messages go out. Needs a read of `0x00462D00`, `0x0044C2C0`,
   `0x0044F046` plus a packet trace of a scripted click (`client/ui.md`
   §B4 "send ticks"): Needs recording.
3. **Answered** (2026-10-07, §7 r1). Was: The gates `0x0044DA30` and
   `0x00463DF0` (likely game-paused / player not controllable) and
   `0x0044DB30` (party screen condition).
4. **Answered** (2026-10-07, §7 r2–r3; the writers of `[0x007BEFB0 +
   c]`: `client/msg-stats-items.md`). Was: Belt use `0x00498A90`: which
   slot of column c, the Shift branch (give to hireling?), and the byte
   `0x007BEFB0`.
5. **Answered** (2026-10-07, §7 r4: the gold dialog, `ui/panels-2.md`
   §21). Was: Key-mode 0 from `0x00454150` and 1 from `0x00453EE0`
   around the latch `0x007A27B4`: which UI state that is.
6. When the key-config screen's Accept writes the files (no caller of
   `0x00469780` other than the load path was found).
7. Which of slot 0 / slot 1 the config screen shows as "Key/Button One"
   (`CfgPrimaryKey`).
8. World-click predicates (§6 r5): the exact condition order of
   `0x004625B0` and of the senders `0x00461DC0`, `0x00461C70`,
   `0x00461890`, `0x004619E0`, `0x00461840` (which target types
   interact, melee range, the town and `0x00645460` / `0x00643860` /
   `0x00465C60` / `0x004610C0` / `0x00462560` / `0x004623C0` tests), and
   the target re-pick `0x00467880`. Disassembly read; then the packet
   trace of OQ 2. *Partly answered* (2026-10-07, §6 r8–r10: the full
   order of `0x004625B0`, every sender, the hostility test, the walk
   clamp and the re-pick). Open: the three searches of the re-pick
   (`0x00467490` `SearchEnemyNear`, `0x00467660` `SearchEnemyXY`,
   `0x004677B0` `SearchOpenXY`) and the nudge `0x004C51E0`; read
   them.
