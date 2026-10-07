# Spec: UI — Panels (slots, open/close, view shift, panel art, hit rects, intents)

- **Status:** draft (2026-10-07, RE on the 1.14d `Game.exe` and the 1.14d
  panel DC6 / `inventory.bin` / string files; no capture yet). Positions,
  frames, tables and intents are read from the code and data named per
  rule; pixel results are unverified until the capture cases of §Test
  vectors run.
- **Target version:** 1.14d, English install
- **Crate/module:** `d2-client::ui` (`PanelRules`, panel draw lists, hit
  tests), `d2-client::ui::layout` (loads `panel-layout.tsv`,
  `ui-states.tsv`, `npc-menus.tsv`)
- **Related specs:** `client/ui.md` §A2, §B1, §B2 (design this answers),
  `ui/text.md` (every text call, fonts, colors, centering), `render/camera.md`
  §1 (view rectangle and `shiftX` per open mode), `render/sprite-placement.md`
  §2 (where a cel drawn at (X, Y) lands), `render/blend-modes.md` (draw
  modes 3 and 5, rectangle mode 2), `render/draw-order.md` (where the
  UI pass sits in the frame), `render/capture.md` §3.1, §8 (recorded panel
  globals, capture cases), `formats/dc6.md`, `items/inventory.md` (grids,
  item intents 0x16–0x2A), `world/npc.md` (0x2F/0x30/0x34/0x36/0x38/0x62
  handlers), `world/vendors.md` (0x32/0x33/0x35), `world/cube.md` (0x2A,
  0x4C, 0x4F 0x17/0x18), `world/waypoints.md` (0x49, S→C 0x63),
  `combat/vitals.md` §2 (0x3A), `skills/levels.md` §6.4 (0x3B),
  `sim/client-messages.tsv` (layouts). Machine tables: `ui-states.tsv`,
  `panel-layout.tsv`, `npc-menus.tsv` (this spec, §16). Continued in
  `ui/panels-2.md` (§14, §17–§22) and `ui/panels-3.md` (§23–§27).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 58–77 |
| Inputs | 78–90 |
| Outputs / state changes | 91–95 |
| Rules | 96–97 |
|   1. Screen layout model | 98–134 |
|   2. UI states and the open/close call | 135–174 |
|   3. The conflict gate (`0x00453910`) | 175–203 |
|   4. Slots, open mode and the view shift | 204–255 |
|   5. UI pass order (`0x00456EE0`) | 256–295 |
|   6. 800 × 600 border and control panel art (`0x00499450`) | 296–316 |
|   7. Shared panel parts | 317–334 |
|   8. Character panel (ui 2, left; `0x004A7D00`) | 335–437 |
|   9. Inventory panel family (`0x0048EDF0`) | 438–498 |
|   10. Skill tree (ui 4, right; `0x004AC690`) | 499–562 |
|   11. Stash (ui 0x19, full; inventory modes 0x0C / 0x0D) | 563–598 |
|   12. Horadric Cube (ui 0x1A, full; inventory mode 0x0E) | 599–649 |
|   13. Waypoint menu (ui 0x14, left; `0x0049C9C0`) | 650–702 |
|   14. NPC menu (ui 8) and NPC shop (ui 0x0C) | 703–708 |
|   15. Event → intent summary | 709–736 |
|   16. Machine tables | 737–771 |
| Constants & data dependencies | 772–792 |
| Randomness | 793–797 |
| Edge cases & original bugs | 798–818 |
| Test vectors | 819–857 |
| Provenance | 858–898 |
| Open questions | 899–977 |
<!-- /index -->

## Summary

The 1.14d client keeps one open flag per UI state (38 states: inventory,
character, skill tree, stash, waypoint, NPC menu, …). One call opens,
closes or toggles a state; before opening it consults a 38 × 38 table
that says, for each state already open, whether to ignore it, close it,
or refuse. Opening or closing a panel sets the screen open mode
(0 none, 1 right panel, 2 left panel, 3 both or full), which shifts the
world view by a quarter of the screen width (`camera.md` §1) and can
move the mouse cursor with it. Panels are 320 × 432 pictures stored as
four DC6 frames, drawn at fixed offsets from the screen size; at 800 × 600
every panel coordinate is the 640 × 480 one plus (80, 60), and a stone
border fills the rest of the half screen. This spec owns the state table
and open/close rules, the open mode, the panel art placement, the hit
rectangles and which C→S message each panel click sends. What the server
does with the message is the owner spec named in §15. Code hooks
`TODO(spec: ui/panels.md §B1 / §B2 / §B6)` (`client/ui.md` §B): button
frames, label pens, wheel step, image requests, shading and blend, open
mode: `panels-2.md` §22; cursor: `panels-3.md` §23.

## Inputs

| Name | Type | Source |
|---|---|---|
| frame size W × H | 640 × 480 or 800 × 600 | `camera.md` §1 (`0x0071146C`, `0x00711470`) |
| resolution mode | 0 (640 × 480) or 2 (800 × 600) | D2GFX `ResolutionMode` `0x007C8CB8` (`0x004F5160`) |
| video mode | 1 GDI, 3 DirectDraw, … | D2GFX `0x007C8CB0` (`0x004F5140`) |
| game type | classic or expansion game | `0x0044DCC0` (`[0x007A04F4]`); expansion installed `0x00408F20` (`d2exp.mpq` present) |
| mouse | (x, y) in frame pixels, button down/up | `0x00468730` / `0x00468740`; event record `+0x0C` x, `+0x0E` y (u16) |
| player | unit: class `[0x007A0522]`, stats, mode, inventory | bridge snapshot |
| panel art | DC6 files of §16 `panel-layout.tsv` | `client/assets.md` archive order |
| tables | `ui-states.tsv`, `panel-layout.tsv`, `npc-menus.tsv` | this spec |

## Outputs / state changes

The 38 UI flags, the screen open mode and `shiftX` (`camera.md` §1), the
cursor position (cursor jump, §4.3), UI `DrawItem`s, C→S messages (§15).

## Rules

### 1. Screen layout model

1. **Panel shift.** Before drawing the UI each frame the client sets
   `ScreenShiftX` `0x007A2858` = `sx` and `ScreenShiftY` `0x007A285C` =
   `sy`: (80, −60) when the resolution mode is 2, else (0, 0)
   (`0x00456EE0`). These are not the world view shift (`shiftX`
   `0x007A5214`, §4).
2. Every panel position in this spec and in `panel-layout.tsv` is an
   expression in `W`, `H`, `sx`, `sy` (grammar §16.2). Bottom-anchored
   positions use `H + sy − d`, top-anchored ones `c − sy`, left panels
   `sx + c`, right panels `W − sx − c`. With 800 × 600 every one of them
   equals the 640 × 480 value plus (80, 60): **the 800 × 600 layout is the
   640 × 480 layout moved by (80, 60)**, plus the border of §6 and the
   wider control panel. The mouse tests use the same expressions, so hit
   rectangles move with the art.
3. Panel art positions are cel draw positions (X, Y): the frame covers
   columns `X … X + w − 1`, rows `Y − h + 1 … Y` (`sprite-placement.md`
   §2; every panel frame has offsets 0, measured on all files of §16,
   except `menu\horadric` (§12.4) and `cursor\pentspin` (`ui/menus.md` §2.5), whose
   frames carry non-zero offsets that the cel draw applies as usual).
4. **Panel quads.** A full panel is 320 × 432 pixels, stored as four
   frames `f … f + 3` of sizes 256 × 256, 64 × 256, 256 × 176, 64 × 176
   (measured on every panel file of §16). With panel left edge `X0` they
   are drawn at `(X0, H + sy − 224)`, `(X0 + 256, H + sy − 224)`,
   `(X0, H + sy − 48)`, `(X0 + 256, H + sy − 48)`, draw mode 5, light
   0xFF, no palette remap (`D2GFX_DrawCelContext` `0x004F6480`, e.g. at
   `0x004A7D3E`). So a panel covers rows `H + sy − 479 … H + sy − 48`:
   1–432 at 640 × 480, 61–492 at 800 × 600.
5. **Slots.** The left panel has `X0 = sx` (columns 0–319 / 80–399), the
   right panel `X0 = W − sx − 320` (columns 320–639 / 400–719).
6. Draw-call conventions used below: "cel draw" = `0x004F6480` (mode 5,
   no remap) unless a remap `k` is named, then the colored cel draw
   `0x004F64B0` (`text.md` §4.1, palette argument `k`); text = `text.md`
   §7 `DrawText(text, x, y, k, centered = 0)` in the font named.
   "Centered in [a, b]" means: span `s = b − a + 1`, width A of `text.md`
   §6; if width < s, pen x = `a + ((s − width) >> 1)`, else `a`.

### 2. UI states and the open/close call

1. The client has 38 UI states, ids 0–37 (`[0x0070F9CC]` = 0x26 is
   asserted). State `i` is open when the u32 at `0x007A27C0 + 4i` is 1.
   Names follow D2MOO `D2C_UIvars` (1.10f; the 1.14d ids are confirmed by
   the panel each flag's draw call draws, §5, and by the community labels
   `IsGameMenuOpen` 0x09, `IsAutomapOpen` 0x0A, `IsHelpScreenOpen` 0x21).
   The list with flag addresses, slot kind and conflict row is
   `ui-states.tsv` (§16.1).
2. `SetUIState(ui, mode, jump)` (`0x00455F20`, ui ECX, mode EDX, jump on
   the stack) is the only writer of the flags besides start-up resets.
   `ui > 0x25` or `mode > 2` is a fatal error. Modes: 0 on, 1 off,
   2 toggle (D2MOO `UI_TURNON` / `UI_TURNOFF` / `UI_TOGGLE`).
3. States 0x23, 0x24, 0x25 can change only in an expansion game with
   the expansion installed; otherwise the call returns 0.
4. The gate (§3) runs next; if it refuses, the call returns 0 and nothing
   changes.
5. Flag update, with `P` the local player (`[0x007A6A70]`) and "P alive"
   = P is a player unit (type 0) whose mode is neither 0 (death) nor 0x11
   (dead) and whose unit flag bit 0x10000 (`+0xC4`) is clear
   (`0x00464820`):
   - on: flag := 1 if there is no P or P alive; else unchanged;
   - off: flag := 0;
   - toggle: flag := !flag if there is no P or P alive; else, if ui = 5
     (chat box), the chat flag is toggled anyway; else unchanged.
6. Transition hooks: closed → open calls the open hook `0x00455720(ui)`,
   and for ui 5 also `0x0047BDC0`; open → closed calls the close hook
   `0x00455AE0(ui)`. The hooks register / unregister the state's window
   message handlers (`0x00451DB0` / `0x00451E10` with the window handle
   `0x004F59A0`) and run per-state set-up or clean-up (e.g. stash open
   loads the stash art `0x00489E50`, cube open `0x0048A4B0`, skill tree
   close `0x004ABF10`, waypoint close `0x0049CF50`). Ui 1 (inventory)
   additionally calls `0x00487990` before the slot logic, whatever the
   transition.
7. Then the open mode is recomputed from the slot kind of `ui` (§4).
   The call returns 1.
8. Mode "on" for a state that is already open still runs the gate, and
   most states refuse themselves (`ui-states.tsv` diagonal = 2), so
   "on" twice returns 0 the second time.

### 3. The conflict gate (`0x00453910`)

Run only for mode on, or for toggle while the state is closed (mode off
skips it and passes).

1. Requests for ui 5 (chat) or 9 (escape menu):
   - ui 5 is refused while `0x004B85E0()` is set and `[0x007A2820]` = 0;
   - both are refused while `GetTickCount() < [0x007A2790]`;
   - ui 9 with no player is refused; ui 9 while the player is dead (mode
     0x11) does not open the menu: it runs the respawn path
     (`0x004647D0`, C→S 0x41 via `0x00478590`) and returns 0.
2. While `[0x007BF0A4]` ≠ 0 (a modal text screen, `0x004A0000`) only
   ui 0x0A, 0x13, 0x11, 6 and 7 may open; any other request is refused.
3. Then, for every open state `i` (0 … 37 in order), the action
   `C[i][ui]` (`ui-states.tsv` column `conflicts`, digit `ui`; table of
   row pointers `0x00711130`, rows of 38 u32) applies:

   | `C[i][ui]` | Action |
   |---|---|
   | 0 | nothing |
   | 1 | close `i`: `SetUIState(i, off, 0)` (full call, hooks and open mode included), continue the loop |
   | 2 | refuse: return 0 at once (states closed earlier in the loop stay closed) |
   | 3 | refuse (return 0); for ui 0 a fatal error |
   | 4 | if an NPC interaction is active (`[0x007C0D29]`), end it (`0x004B3C20`), continue |

4. Row 0 (state 0, "game") points into zero-filled `.data` (`0x007A28A0`,
   past the section's raw size): all 0.
5. If the loop ends, the gate passes.

### 4. Slots, open mode and the view shift

1. Slot kinds (`0x00455F20` switch at `0x00456079`, jump table
   `0x004562BC` / index bytes `0x004562D8`):

   | Kind | States |
   |---|---|
   | right | 1 inventory, 4 skill tree |
   | left | 2 character, 0x0F quest log, 0x10 Inifuss scroll, 0x14 waypoint, 0x16 party, 0x24 mercenary, 0x25 recipe scroll |
   | full | 0x0C NPC shop, 0x19 stash, 0x1A cube, 0x1B, 0x1C, 0x1D, 0x20 |
   | anvil | 0x0E |
   | none | every other state (no open-mode change) |

2. New open mode `m` after the flag update (`v` = new flag of `ui`;
   "left open" = any of the flags of 2, 0x14, 0x0F, 0x10, 0x24, 0x25,
   0x16 is set; "right open" = flag 1 or 4 is set):

   | Kind | `v` = 1 | `v` = 0 |
   |---|---|---|
   | right | 3 if left open, else 1 | 2 if left open, else 0 |
   | left | 3 if right open, else 2 | 1 if right open, else 0 |
   | full | 3 | 0 |
   | anvil | 1 | 0 |

   (Ui 0x0B, key configuration: on close calls `0x004A5DE0`; no mode
   change.) The mode is applied by `SetScreenOpenMode(m)` `0x0045AEA0`:
   `shiftX` and the view rectangle per `camera.md` §1, then `0x004F52A0`;
   mode 3 also calls `0x0044DA70`.
3. **Cursor jump** (only when `jump` ≠ 0 and the left/right rule set a
   mode of 0, 1 or 2; mouse (x, y) read before the mode change;
   `0x00468770(x', y)` moves the cursor):

   | Case | Condition | New x |
   |---|---|---|
   | right opened, no left open | x > W / 4 (signed, rounded toward 0) | x − W / 4 |
   | right closed, no left open | x < W / 2 (C division) | x + W / 4 |
   | left opened, no right open | x < W − W / 4 | x + W / 4 |
   | left closed, no right open | x > W / 2 | x − W / 4 |

   No jump when the other side is open (modes 2/3 and 1/3) or for full
   and anvil kinds. In 1.14d the only callers that pass `jump` = 1 are
   the waypoint menu (0x14 on, `0x0049CF90`; off, `0x0049CEC0`,
   `0x0049D160`) and `0x004A2840` (ui 0x0F off) (scan of all 136 call
   sites). Hot keys pass 0 (§Open questions 2).
4. The play area for clicks: a click inside an open panel's area is
   consumed by the panel (§8–§14); the world input path is not reached.
   The right panel's area is the inventory record's `inv` rectangle
   (`[0x007BCAA0]` left, `[0x007BCAA4]` right exclusive, `[0x007BCAA8]`
   top, `[0x007BCAAC]` bottom, from `inventory.bin`, §9.2); the
   character panel's area is x in [`sx`, `W / 2 − 1`], y in [`sy`,
   `H + sy − 49`] (`0x004A7720`).

### 5. UI pass order (`0x00456EE0`)

Runs once per drawn frame (`draw-order.md`), returns at once while
`[0x007A2808]` ≠ 0. Steps (flag `[i]` = state `i` open):

1. Set `sx`, `sy` (§1.1); `0x00502280(0, 0, 0)` (clears the pending
   hover text); `[9]` → `0x0047E3D0`; `[0x0B]` → `0x004A5270`;
   `0x00467A10` → `0x00454F30`; `[0x23]` → `0x00452D20`.
2. If none of `[1]`, `[0x0C]`, `[0x0E]`: `0x00497FE0`, `0x00497E00`
   (equipment warning icons).
3. `[0x0A]` and open mode ≠ 3 → automap `0x0045AD60`.
4. `0x00455510`, `0x00493340`; `[0x13]` → `0x00494020`; `[0x24]` →
   mercenary panel `0x004929F0`; `[0x25]` → `0x0048BC10`; `[0x0F]` →
   quest log `0x004A34F0`.
5. Inventory family `0x0048EDF0` (§9) if any of `[1]`, `[0x0C]`,
   `[0x0E]`, `[0x19]`, `[0x1A]`, `[0x1C]`, `[0x1D]`; `[0x17]` (trade) →
   `0x004B8730` then `0x0048EDF0`.
6. `0x00453470`; `[4]` → skill tree `0x004AC690`; `[2]` → character
   `0x004A7D00`; `[0x16]` → party `0x0049C3D0`; `[0x007A27A0]` →
   `0x004B8100`; `[0x10]` → `0x0049FF10`; `[0x14]` → waypoint
   `0x0049C9C0`; `[0x21]` → `0x00494320`, `0x00494EB0`.
7. Control panel and border `0x00499450` (§6).
8. `[3]` → skill select `0x004AA7E0`; `[0x1F]` → belt rows `0x00498E90`;
   `0x004A64C0`; `[0x22]` → `0x00495180`; new-stats button (`[6]`
   `0x004A6B30` else `0x004A6A70`); new-skills button (`[7]`
   `0x004A6E60` else `0x004A6DA0`); `[0x11]` → `0x004A2A80`;
   `0x00498120`; `[0x15]` → mini panel `0x0047F710`; `[5]` → chat
   `0x0047B720`.
9. Help-screen toggling block (only when `[0x21]`, ends with
   `0x004966C0`); `[0x0D]` and mode ≠ 3 → item names `0x004C0810`;
   `[8]` → NPC menu `0x004B4380` (§14); `[0x0E]` → anvil `0x004C01E0`.
10. `0x00496B10`, `0x004A0E70`, `0x00503000`, `0x00454A70`,
    `0x00453060`, `0x004554B0`, `0x00453B30`, `0x00453100`,
    `0x0046C060` (hover text, cursor and remaining overlays; owners
    listed in §Open questions 1).

Later draws overwrite earlier ones; the character panel (step 6) is
drawn after the inventory family (step 5), the control panel (step 7)
after both.

### 6. 800 × 600 border and control panel art (`0x00499450`)

1. Only in resolution mode 2: if the open mode is 2 or 3, the left border
   (`Panel\800BorderFrame`, frames 0–4, `0x00498630`); if it is 1 or 3,
   the right border (frames 5–9, `0x00498700`). Positions are absolute
   (800 × 600 only), in `panel-layout.tsv` rows `border`. The left border
   covers columns 0–400, the right 400–799; panel art drawn earlier in
   the frame is under it only where the border has pixels.
2. Control panel base: resolution mode ≠ 2: `Panel\CtrlPnl7` frames 0–4
   at x = 0, W/2 − 155, W/2 − 27, W/2 + 101, W − 117; resolution mode 2:
   `Panel\800CtrlPnl7` frames 0–5 at x = 0, W/2 − 235, W/2 − 107,
   W/2 + 21, W/2 + 149, W − 117 (`0x004983D0`). y = H for all, except
   frames 0 and the last one, which are drawn at y = H − 1 under
   DirectDraw (video mode 3) and at y = H otherwise. The reference driver
   is GDI (`composition.md`), so y = H. Switching resolution frees the
   other file's cels.
3. Then `0x00496F80`, `0x00497110`, `0x00498EA0`, `0x00497480`,
   `0x004975B0`, `0x004977C0`, `0x00499040`, `0x00496CF0`, `0x00497A40`,
   `0x00498340` (globes, belt, skill buttons, run / menu buttons, level
   name timer): §Open questions 1.

### 7. Shared panel parts

1. Panel cels are loaded by name relative to `DATA\GLOBAL\UI\` through
   `0x004520C0` (`"%s\UI\"` + name) or with an explicit
   `"%s\ui\panel\…"` path; the archive is chosen by the normal archive
   order (`client/assets.md`). Which file of a classic / expansion pair is
   used is per panel below.
2. **Close button.** `%s\ui\panel\buysellbtn` (`0x00454600`, cached in
   `[0x007A287C]`): frame 10 released, 11 pressed. 1.14d reads the
   23-frame `d2exp.mpq` copy before the 18-frame `d2data.mpq` one; frames
   10 and 11 exist in both.
3. A press sets the panel's pressed flag (drawn frame 11); the release
   hit test decides the action. Several release handlers do not check that
   the press began on the button (§8.3, §9.3): reproduce.
4. Hover tool tips are queued with `0x00502280(text, x, y)` and drawn in
   step 10 of §5 (owner: §Open questions 1). Strings named here are
   string-table ids (`text.md` §2).

### 8. Character panel (ui 2, left; `0x004A7D00`)

1. Art: `Panel\InvChar6` (expansion game and expansion installed) else
   `Panel\InvChar`, frames 0–3 as left quads (§1.4) (loader `0x004967F0`
   into `[0x007BEF64]`; languages 6–9 use `DATA\LOCAL\UI\InvChar6` /
   `InvChar` instead).
2. Close button: §7.2 at (`sx + 128`, `H + sy − 60`), frame 10 + pressed
   (`[0x007C02F4]`). Hover x in [`sx + 128`, `sx + 160`], y in
   [`H + sy − 92`, `H + sy − 60`] → tool tip `strClose` (4144 = 0x1030) at
   (`sx + 143`, `H + sy − 95`).
3. Mouse down (`0x004A7720`) in the close rectangle sets pressed. Mouse
   up (`0x004A78C0`) clears pressed; if the release is in the close
   rectangle: `SetUIState(2, off, 0)` (no check that the press was on
   the button).
4. If stat 4 (`statpts`, unshifted) ≠ 0:
   - `Panel\skillpoints` frame 0 at (`sx + 3`, `H + sy − 116`);
   - Font6, color 1: `strchrstat` (4075) centered in [`sx + 11`,
     `sx + 88`] at y `H + sy − 125`, `strchrrema` (4076) same span at y
     `H + sy − 117`;
   - Font16, color 0: the value (`%i`) centered in [`sx + 92`,
     `sx + 127`] at y `H + sy − 120`;
   - four add buttons, table `0x00724A48` (stride 14: x i32, y i32,
     pressed i32, stat u16): (117, 105, str 0), (117, 167, dex 2),
     (117, 253, vit 3), (117, 315, energy 1). With `b = H + sy − 480 + y`:
     `Panel\Levelsocket` frame 0 at (`sx + x + 5`, `b + 5`), then
     `Panel\Level` frame `pressed ? 1 : 0` at (`sx + x + 8`, `b + 1`).
5. Add-button hit: `sx + x < mx < sx + x + 40` and `b − 22 < my < b`
   (strict). Mouse down there (any stat points left) sets that button's
   pressed field. Mouse up there: count = 1, or, with Shift held
   (`GetKeyState(VK_SHIFT)` < 0), count = current `statpts`; then
   messages C→S 0x3A are queued in chunks of at most 32 (release order:
   `panels-2.md` §17.2):
   `[0x3A][stat][n − 1]` for each chunk `n` (`0x004A7A15`;
   `combat/vitals.md` §2). Mouse up clears every pressed field first, so
   a release on a button other than the pressed one also spends.
6. Labels: 15 entries, table `0x00724818` (stride 18: x1 i32, y i32,
   x2 i32, string id u16; the u16 at +0x0E is not read), Font6, color 0,
   centered in [`sx + x1`, `sx + x2`] at y `H + sy − 480 + y`. A label
   whose string holds a LF before its end is split there: the part
   before the LF at `y − 4`, the part after at `y + 4`, each centered on
   its own (`0x004A8287`). Entries in `panel-layout.tsv` (`label0` …
   `label14`).
7. Values: 18 entries, table `0x00724928` (stride 16: x1, y, x2, stat),
   Font16, centered in [`sx + x1`, `sx + x2`] at y `H + sy − 480 + y`.
   Value = stat value of the player (`0x00625480`), base = unmodified
   value (`0x006253B0`). Stats 6–11 (life, mana, stamina, current and
   max) are shown `>> 8`; stat 6 is shown as at least 1 while the player
   is alive. Color: 3 (blue) when value > base, 1 (red) when value <
   base, else 0, for stats 0, 2, 3, 1 (attributes), 7, 9, 11 (max
   life/mana/stamina), 12 (level), 31 (defense) and the resistances
   (§8.9); experience (13) and next-level (30) are formatted by
   `0x00525350` (thousands grouping, §8.11) in color 0. Per-stat paths
   (switch `0x004A8448`, index bytes `0x004A897C` for stats 6–45, jump
   table `0x004A8950`): 6 → `>> 8`, min 1 while alive, color 0; 7, 9,
   11 → color compares the **unshifted** value and base, then `>> 8`;
   8, 10 → `>> 8`, color 0; 13, 30 → §8.11; 31 → §8.8; 37–45 (odd) →
   §8.9; every other stat (0–3, 12) → color compare, text `%ld`
   (`wsprintfA`, format `0x006D9BE8`). All non-grouped values use
   `%ld` (signed decimal).
8. Font fallback: for the stats 6–11 and, for defense, when its value
   is ≥ 1,000 (or language 6), the value is drawn in Font8 (font 0)
   instead of Font16 if it is ≥ 1,000 or its popup width (`0x00502520`)
   is ≥ `x2 − x1`; the font is restored after the value. The popup width
   is the max width (`text.md` §6, `0x00501840`) of the formatted value
   in Font16; `x1`, `x2` are the table values (no `sx`); the ≥ 1,000
   test is on the shown (shifted) value (`0x004A8893`–`0x004A88C5`).
   Stats 12, 13, 30, the attributes and the resistances never switch
   font.
9. Resistances (stats 39 fire, 43 cold, 41 lightning, 45 poison): shown
   value = stat − difficulty penalty (classic game: 20 in nightmare, 50
   in hell, `[0x0044DCD0]`; expansion game: the difficulty table value
   `0x00611D30`), clamped to [−100, cap], cap = min(75 + the matching
   max-resist stat (40, 44, 42, 46), 95). Blue when a resist-raising
   effect is active (`0x0063A570`… family), red when a lowering one is.
   Exact order (`0x004A8570`–`0x004A86A0`): color := 3 if the raising
   test (fire `0x0063A570`, cold `0x0063A590`, lightning `0x0063A5B0`,
   poison `0x0063A5D0`) is true, then := 1 if the lowering test
   (`0x0063A610`, `0x0063A630`, `0x0063A650`, `0x0063A670`) is true
   (lowering wins). Then with `v` = stat − penalty: if max(`v`, −100) ≥
   cap, shown := cap (so a cap below −100 wins over the −100 floor) and
   color := 4 unless it is 3; else shown := max(`v`, −100) and, if shown
   < 0, color := 1 (also over 3). Text `%ld`.
10. The damage / attack-rating block (`0x004EDA20`, `0x004A7340`,
    `0x004A74A0`, `0x004A7AE0`, `0x004A7180`), the name and class lines
    (Font16 at y `H + sy − 455`) and the hover texts of the stat lines:
    §Open questions 3.
11. **Experience and next level** (`0x004A87F7`, `0x004A8750`). Value:
    experience = stat 13 (`0x00625480`); next level: with `L` = the
    player's base level (`0x006253B0`, stat 12) and `c` = its class
    (unit +4; outside 0–6 read as 0), if `L` ≠ `experience.txt` MaxLvl of
    `c` (`0x00611830`), the value is the `experience.txt` entry of `c` on
    the row whose `Level` is `L` (`0x00611800`: record `L + 1`, i.e. the
    total experience that ends level `L`; level 1 → 500), else the
    player's stat 30 value. Text: `0x00525350(buf, value, 128)`: the
    value as **unsigned** decimal (`_ultoa`), with `,` inserted after
    every 3 digits counted from the right (only when there are more than
    3 digits; no locale), e.g. 1234567 → `1,234,567`, 999 → `999`; if
    the result does not fit the buffer it is `*` (unreachable with 128).
    Drawn at once: Font16, color 0, centered in [`sx + x1`, `sx + x2`]
    (width A) at y `H + sy − 480 + y`; no Font8 fallback.
12. Zero points, name / class lines, damage block, chance popups, draw
    order: `ui/panels-2.md` §17.

### 9. Inventory panel family (`0x0048EDF0`)

1. Mode `[0x007BCBF0]` picks what is drawn besides the inventory: 0
   none, 1–9 NPC trade states (§14.3), 0x0B player trade, 0x0C / 0x0D
   stash (§11), 0x0E cube (§12), 0x13 other, 10 draws nothing at all.
2. Layout records: on a change of `InventoryArrangeMode` `[0x007A5218]`
   (0 for 640 × 480, 1 for 800 × 600) the client reloads the panel
   rectangles from `inventory.bin` (`0x004835B0`): the player's record
   (`0x00621050`, `items/inventory.md` §1.3) and record 13 (hireling)
   with arrange-mode offset 16 for 800 × 600 (records 16–31): `inv`
   rectangle (`0x0065C180`), grid (`0x0065C1F0`), and the ten equipment
   rectangles (`0x0065C270`, slot order rArm, torso, lArm, head, neck,
   rHand, lHand, belt, feet, gloves) into `0x007BCCA8`, `0x007BCC94`,
   `0x007BCCBC`, `0x007BCC6C`, `0x007BCC80`, `0x007BCCD0`, `0x007BCCE4`,
   `0x007BCCF8`, `0x007BCD0C`, `0x007BCD20` (each: left, right, top,
   bottom, w, h). The 1.14d values (measured, `inventory.bin`, d2exp)
   are §Test vectors.
3. Art: `InvChar6` / `InvChar` (§8.1) frames 4–7 as right quads
   (`X0 = W − sx − 320`). Close button §7.2 at (`W − sx − 302`,
   `H + sy − 64`), pressed `[0x007BCE90]`. Close rectangle x in
   [`W − sx − 302`, `W − sx − 270`], y in [`H + sy − 96`, `H + sy − 64`]
   (`0x00486E10`). Mouse down there sets pressed (`0x00489190`); mouse
   up there (`0x00486EF0`) calls `SetUIState(1, off, 0)` without
   checking pressed (exact order: `panels-2.md` §18.1).
4. Empty equipment slots get a background picture (only if no item is
   in that body location, `0x0063BDE0`); position = (slot `left` + dx,
   slot `bottom` + dy), offsets table `0x007220B0`:

   | Body loc | File, frame | (dx, dy) |
   |---|---|---|
   | 3 torso | `Panel\inv_armor` 0 | (2, −2) |
   | 8 belt | `Panel\inv_belt` 0 | (0, −1) |
   | 9 feet | `Panel\inv_boots` 0 | (−1, −4) |
   | 1 head | `Panel\inv_helm_glove` 1 | (0, −1) |
   | 10 gloves | `Panel\inv_helm_glove` 0 | (−1, −2) |
   | 2 neck | `Panel\inv_ring_amulet` 0 | (0, −1) |
   | 6 right ring | `Panel\inv_ring_amulet` 1 (rHand rect) | (0, −1) |
   | 7 left ring | `Panel\inv_ring_amulet` 1 (lHand rect) | (0, −1) |
   | 5 left hand | `Panel\inv_weapons` 0 (lArm rect) | (0, −2) |
   | 4 right hand | `Panel\inv_weapons` 0 (rArm rect) | (0, −2) |

   A hand's picture is drawn when that hand is empty and the other hand
   is empty or does not hold a two-handed weapon (`0x0063D340` = 2).
   Draw order is the table order.
5. Weapon-swap tabs (`Panel\invchar6Tab`) and the extra (4, −4) hand
   offset depend on `[0x007BCC4C]`, which the loader sets to 0 and no
   1.14d instruction sets non-zero (all.asm scan): the tab draws never
   run. A second hand-background pass after them (`0x0048F950`) can never
   draw. Both: reproduce as no-ops.
6. Then the grid (`0x00483FF0`, page 0) and the equipped items
   (`0x004845A0`: the equipment boxes, not buttons; `ui/inventory.md`
   §3–§6), the shop extras in modes 1–9 (`0x004886C0`), the gold line and
   gold button (`0x00488100(1)`, `panels-2.md` §21), the close button,
   then the hover box (`0x0048DD90`); the cursor item: `panels-3.md`
   §23. (Corrected 2026-10-07: `0x004845A0` was named here as the gold
   buttons.)
7. Clicks on the grid and body locations send the item intents (§15);
   their validation is `items/inventory-moves.md` §7.
8. Mouse-up order, the area test (bottom inclusive) and the belt test:
   `ui/panels-2.md` §18.

### 10. Skill tree (ui 4, right; `0x004AC690`)

1. Art: `Spells\skltree_<c>_back` for the player class (`a` ama, `s`
   sor, `n` nec, `p` pal, `b` bar, `d` dru, `i` ass; table `0x00724BF0`,
   stride 0x22: cel u32, path), frames 0–3 as right quads, then frames
   `4t … 4t + 3` at the same places, `t` = current tab `[0x00724BEC]`
   (1–3, initial 1) (`0x004AAAE0`).
2. Tabs (mouse down, `0x004AB7E0`; only when my ≤ H − 48): x in
   [`W − sx − 88`, `W − sx`] and y strictly inside
   (`H + sy − 372`, `H + sy − 265`) → t = 3,
   (`H + sy − 264`, `H + sy − 157`) → t = 2,
   (`H + sy − 156`, `H − 49`) → t = 1 (the last bound has no `sy`).
   A change plays the click sound (`0x004B9A00(0, 0, 0)`). No message.
3. Skill icons: for each class skill (`0x00646140` count, skills.txt
   row → skilldesc row via skills +0x194) whose skilldesc page (+2)
   equals `t`: column (+4) 1, 2, 3 → x = `W − sx − 305`, `− 236`,
   `− 167`; row (+3) 1–6 → y = `H + sy − 418`, `− 350`, `− 282`,
   `− 214`, `− 145`, `− 77` (`0x004AA9C0`); file = class icon file (§16
   rows `icon_*`, `0x004A8C80`): `Spells\<CC>Skillicon` with `CC` = `Am`,
   `So`, `Ne`, `Pa`, `Ba`, `Dr`, `As` for classes 0–6 (table
   `0x00724AC0`, stride 0x22: cel u32, path; the skill tree passes no
   skill, so the player's class `[0x007A0522]` is used; with a skill the
   skill's class `0x00645040` is used, loaded on first use by
   `0x004520C0`, and a class > 6 gives `Spells\Skillicon`
   `[0x007C07F8]`), frame = skilldesc `IconCel` (+7), + 1
   while pressed. Remap `k` (`0x004F64B0`): start 5 (grey: skill level
   0 and a point would not be accepted) or 0 (learnable / learned,
   `0x004AC4D0`), 3 if the mouse is strictly inside the icon, 1 if the
   skill's flag byte (+5) has no bit of `[0x006CE270]`, 3 if the player
   has no free points and the skill has a level and is not pressed
   (exact order and last clause: `panels-3.md` §25 r4).
4. Level number (if level > 0 or the hard points ≠ 0): `%d`, Font16
   (Font formal10 for ≥ 10, then x − 4) at (`X + 48`, `Y + 12`), color 3
   when the bonus part (`0x00644300`) > 0, 1 when < 0, else 0.
5. Icon hit: `X < mx < X + 48`, `Y − 48 < my < Y` (strict). With free
   skill points (stat 5 > 0): mouse down marks the icon pressed; mouse up
   on the pressed icon checks the required level (`0x00646CA0`) and the
   skill's max level (`0x004AA8B0`, `0x004AA900`) and sends
   `[0x3B][skill u16]` (`0x004785B0`, `skills/levels.md` §6.4); release
   anywhere clears pressed. Without free points a mouse down on an icon
   whose skill flag (+4) has no bit of `[0x006CE278]` sends a 9-byte
   message through `0x004786A0` (−1 argument) and calls
   `SetUIState`: §Open questions 4.
6. Close button: §7.2 at (`W − sx + o`, `H + sy − 63`), `o` from table
   `0x00724CE4` index `3 × class + t` (−149, −220 or −305; e.g.
   amazon tabs 1–3: −149, −220, −305). The whole table (i32 × 22, entry
   0 = 0 unused), `o` for tabs 1, 2, 3: amazon −149, −220, −305;
   sorceress −305, −305, −149; necromancer −305, −149, −305; paladin
   −305, −220, −305; barbarian −149, −305, −149; druid −149, −149, −149;
   assassin −220, −149, −305 (read from the 1.14d image, `0x004AB549`).
   `0x004AB530` sets the variant `[0x00724CE0]` := 1, 2, 3 for `o` =
   −149, −220, −305; the hit test `0x004AB630` uses the variant's fixed
   x. Frame 10, + 1 while pressed (`[0x007C0C38]` ≠ 0). Its hover / release rectangle is
   [`W − sx + o`, `+ 0x22`] × [`H + sy − 63 − 0x22`, `H + sy − 63`]
   (`0x004AB630`, `0x004AB5F0(0x22, 0x22)`). Release there:
   `SetUIState(4, toggle, 0)` (`0x004ABD42`).
7. Free-points box (`0x004AC200` / `0x004AC4D0`) and tab tool tips
   (`0x004AB310`, hover x in [`W − sx − 89`, `W − sx + 1`]): §Open
   questions 4.
8. Clicks at x ≤ W / 2 are not consumed by the skill tree's release
   handler.
9. Mouse down / up order, the no-points 0x3C, the free-points number,
   tab tool tips, draw order: `ui/panels-2.md` §19.

### 11. Stash (ui 0x19, full; inventory modes 0x0C / 0x0D)

1. Open: through the stash object (server interaction, `world/cube.md`
   §interaction rules); `0x00489E00` calls `SetUIState(0x19, on)`, sets
   inventory mode 0x0C and clears the button states. The open hook loads
   `%s\ui\panel\bank` and, with the expansion installed,
   `%s\ui\panel\TradeStash` (`0x00489E50`).
2. Art: `TradeStash` in an expansion game, `bank` otherwise, frames 0–3
   as left quads; the right half shows the inventory (§9).
3. Text: `GoldMax` (4051, "Gold Max: %d") with the stash gold cap
   (`0x00623460`) at (`sx + 78`, `H + sy − 415`) in an expansion game,
   (`sx + 78`, `H + sy − 219`) in a classic game, color 0, current font.
4. Close button §7.2 at (`sx + 272`, `H + sy − 64`) expansion /
   (`sx + 275`, `H + sy − 65`) classic (`0x00486AE0`, `0x00486B00`),
   pressed `[0x007BCE38]`; hover x in [`X`, `X + 40`], y in [`Y − 35`,
   `Y + 5`] → `strClose` tool tip.
5. Closing (button release `0x00489AC0` / `0x00489CE0`, or any close of
   ui 0x19 via the close hook `0x00489EE0`): `SetUIState(0x19, off, 0)`,
   inventory mode 0, and C→S `0x4F` button 0x12 (p1 = p2 = 0). Server
   meaning of 0x12: §Open questions 6.
6. Grid clicks: stash page 4 intents (§15). Stash gold line, button
   and dialog: `panels-2.md` §21.
7. **How many 0x4F 0x12.** The close hook `0x00489EE0` sends one only
   while the inventory mode is 0x0C or 0x0D (it sets mode 0 first and
   calls `SetUIState(0x19, off, 0)` again, a no-op); there is no latch.
   Mouse up `0x00489AC0`: in the inventory close rectangle
   (`0x00486E10`) with no cursor item → `SetUIState(0x19, off, 0)` only
   (hook: **one** message); in the stash close rectangle (`0x00489980`)
   with the button pressed (`[0x007BCE38]` ≠ 0; not pressed → nothing)
   → pressed := 0, `SetUIState(0x19, off, 0)` (hook: one message), then
   0x4F 0x12 again at `0x00489B92`: **two** messages. The key handler
   `0x00489CE0` (mode 0x0C, chat ui 5 closed) does the same: `SetUIState`
   then a second send at `0x00489D21` (two), and resets the stash
   selection globals (`0x00721E3C`–`0x00721E50` := −1).
8. Press / release rectangles: `ui/panels-2.md` §20.1.

### 12. Horadric Cube (ui 0x1A, full; inventory mode 0x0E)

1. Open: `0x0048A460` → `SetUIState(0x1A, on)`, mode 0x0E; the open
   hook loads `%s\ui\panel\supertransmogrifier` (`0x0048A4B0`).
2. If the cube item is gone (`0x0044DA30` or `0x00463DF0`) while drawn:
   `SetUIState(0x1A, off)`, then C→S 0x4F button 0x17 (`0x0048F183`)
   and nothing more is drawn that frame.
3. Art: frames 0–3 as left quads; cube grid (page 3) via `0x00483FF0`.
   Close button at (`sx + 275`, `H + sy − 65`) frame 10 + `[0x007BCE40]`;
   transmute button `Panel\miniconvert` frame `[0x007BCE48]` (0/1) at
   (`sx + 144`, `H + sy − 188`).
4. Transmute animation: while `[0x007BCC10]` ≠ 0, `%s\ui\menu\horadric`
   frame `n` at (W / 2, H / 2 − 1) in draw mode 3; `n` += 1 when more
   than 70 ms passed since the last step (`GetTickCount`); at `n` = 30
   the animation stops and the cel is freed. This is wall-clock timed.
   Exact (`0x0048F03E`–`0x0048F0C0`): each drawn frame, if more than 70 ms
   passed, the stamp := now and `n` += 1; if `n` reaches 30 the flag and
   (if loaded) the cel are cleared and **nothing is drawn** that frame,
   so frames 0–29 are drawn and frame 30 never. Otherwise frame `n` is
   drawn by `0x004F6480` at (`W / 2`, `H / 2 − 1`) (C division), light
   0xFF (pushed as −1, as for §1.4), draw mode 3, no remap. Start (`0x0048A540`): flag
   := 1, `n` := 0, stamp := now, cel loaded if not yet. The file's frames
   are not centered on the draw point: frames 0 and 30 are 2 × 2 with
   offsets (0, 0); frames 1–29 have widths 92–239, heights 121–256,
   offset x −287 … −205 and offset y 17 … 82 (frame 1: 92 × 121 at
   (−205, 17)), all measured on `d2data` `menu\horadric.dc6`; the cel
   draw applies them (`sprite-placement.md` §2), so frame 1 covers
   columns `W / 2 − 205 … W / 2 − 114`, rows `H / 2 − 104 … H / 2 + 16`.
   While the animation runs with `n` < 14 the cube grid (page 3,
   `0x00483FF0`) is not drawn (`0x0048EF92`–`0x0048EFA7`); from `n` = 14
   on, and when no animation runs, it is drawn with `[0x007BCC04]` := 3
   for the call.
5. Hover over the buttons → `strUiMenu2` "Transmute" (3341 = 0xD0D) or
   `strClose` tool tips (y `H + sy − 100` / `H + sy − 223`).
6. Messages: transmute button release → C→S 0x4F button 0x18
   (`0x0048A28B`); close → 0x4F button 0x17 (`0x0048A07D`,
   `0x0048F183`); items in / out: §15 (`world/cube.md`).
7. **Close** `0x0048A050`: animation flag := 0, `SetUIState(0x1A, off,
   0)`, then C→S 0x4F button 0x17 (p1 = p2 = 0) only if the latch
   `[0x007BCC54]` is 0, setting it (the open `0x0048A460` clears it),
   then a pending cursor-item restore (`[0x007BCC50]`, `0x00463990` /
   `0x00480930`). Callers: the close-button release `0x0048A190` (pressed
   `[0x007BCE40]` and the release in the button rectangle, `0x0048A257`),
   `0x0048A3E0`, and the close hook `0x0048A500` (only while the
   inventory mode is still 0x0E; it sets mode 0 first). When a button
   close reaches the hook, the nested `0x0048A050`'s `SetUIState` finds
   the state closed (no hook again) and the latch lets only the first of
   the two calls send: **one** 0x4F 0x17 per open, whichever path closes.
8. Button rectangles and the cube-gone close (two 0x4F 0x17):
   `ui/panels-2.md` §20.2–§20.4.

### 13. Waypoint menu (ui 0x14, left; `0x0049C9C0`)

1. Open: S→C 0x63 → `0x0045E670` → `0x0049CF90`: `SetUIState(0x14, on,
   jump = 1)` (`world/waypoints.md` §5). If the menu is drawn while the
   player is gone or in a state checked by `0x00463DF0`, it closes itself
   (`SetUIState(0x14, …)`) and, once, sends C→S 0x49 with level 0
   (close) (`[0x007BF085]` latch).
2. Art: `waygatebackground` frames 0–3 as left quads (loader
   `0x0049C580`: `waygatebackground`, `expwaygatetabs` in an expansion
   game else `waygatetabs`, `waygateicons`).
3. Tabs, y = `34 − sy` (expansion) / `33 − sy` (classic): expansion 5
   tabs at x = `sx` + 5, 67, 129, 191, 253 (63 × 31 frames), classic 4
   tabs at `sx` + 3, 81, 159, 237 (78 × 30). The current tab
   `[0x007BF086]` draws frame 2t; another tab draws 2t + 1 if tab 0, or
   if its act is reachable (quest checks `0x004B32D0` / `0x0065C310` on
   records 7, 15, 23 and 26 for tabs 1–4); otherwise nothing.
4. Close button at (`sx + 273`, `417 − sy`), frame 10 + `[0x007BF06C]`.
   Hover x in [`sx + 273`, `sx + 308`], y in [`387 − sy`, `420 − sy`] →
   a filled rectangle (`D2GFX_DrawRectangle`, color 0, mode 2) and the
   `strUiMenu1` "Cancel" (4130) tool tip at y `385 − sy`, centered
   on `sx + 291`. Exact (`0x0049CDA6`–`0x0049CE33`, drawn after the
   rows): with `s` = width A of the string / 2 (C division), the
   rectangle `0x004F6300(left sx + 287 − s, top 370 − sy, right
   sx + 294 + s, bottom 387 − sy, color 0, mode 2)`, then the text at
   pen (`sx + 292 − s`, `385 − sy`), color 0, current font.
5. Rows: `[0x007BF08A]` rows (≤ 9) of the current tab, table
   `0x007224E8` (stride 24: icon x, icon y, text x, text y, 2 more i32):
   icon x/y = (17, 89 + 36 r) except row 4 onward (234, 270, 306, 342,
   378), text (80, 84, 119, 154, 189, 224, 259, 294, 329, 364). Known
   row: `waygateicons` frame `sel` (row = selected `[0x007BF06D]`) if it
   is the current level, else `3 + sel`; the current level also gets
   frame 0 drawn again. Text: level name (`0x00453E70`) at (`sx + tx`,
   `ty − sy`), Font16, color 5 unknown, 0 known, 3 if hovered-selected
   or current level. Exact (`0x0049CD10`–`0x0049CD8B`): unknown row
   (used byte 0): no icon, color 5; known: 0; then 3 if the close button
   is not pressed and the row is the pressed row (`[0x007BF06D]`, set by
   mouse down, `ui/menus.md` §1.2; an unknown row can never be pressed,
   the row hit skips it); then 3 if the row's level is the current
   level (the second icon draw, frame 0). Text = level name
   (`0x00453E70(level)`).
6. Title: `waypointsheader` (3990) if any other waypoint is known
   (`[0x007BF08E]`), else `nowaypoints` (3991), Font16, color 0, at
   x = `sx + 160 − width / 2`, y = `48 − sy`.
7. Messages: choosing a row sends C→S 0x49 `[wp GUID u32][level u16 of
   the row]` (`0x0049D0F3`; `[0x007BF07D]` = waypoint GUID); every close
   path sends 0x49 with level 0 (`0x0049C6C0`, `0x0049C700`,
   `0x0049CEC0`, `0x0049CF50`, `0x0049D2B5`). Travel rules:
   `world/waypoints.md` §6–§7. Tab and row hit rectangles: §Open
   questions 7.
8. Mouse-down / mouse-up handling, the tab and row hit tests, the tab
   setter, the close latch, the self-close and the key close:
   `ui/menus.md` §1.

### 14. NPC menu (ui 8) and NPC shop (ui 0x0C)

Moved unchanged (rules 1–6, same numbers) to `ui/panels-2.md` §14,
with rules 7–13 added there (record lookup, flag byte, talk messages,
Cain reset, shop buttons, tabs and mouse).

### 15. Event → intent summary

From a scan of every call of the C→S queue helpers `0x00478590`–
`0x00478890` in `all.asm` (message id = the `mov cl, id` before the
call):

| UI | Event | Message (owner) |
|---|---|---|
| character | release on an add button | 0x3A × ⌈n / 32⌉ (`combat/vitals.md` §2) |
| skill tree | release on a pressed icon, points left | 0x3B (`skills/levels.md` §6.4) |
| inventory, stash, cube, trade pages | grid click (`0x0048FFE0`) | 0x19 lift, 0x18 place, 0x1F swap, 0x20 use, 0x21 stack, 0x27 use on item, 0x28 socket, 0x29 scroll to tome, 0x2A to cube, 0x33 sell, 0x63 to belt, 0x4C (`items/inventory-moves.md` §7) |
| inventory | body location click (`0x00490780`, `0x00490BA0`, `0x00490FC0`) | 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x21, 0x27, 0x4C (`items/inventory-moves.md` §7) |
| inventory | socket fill (`0x004912A0`) | 0x28 |
| inventory | weapon swap (`0x0048A730`) | 0x60 |
| mercenary | item on merc (`0x0048B7C0`, `0x004936E0`) | 0x61 |
| belt | `0x00498870` / `0x00498A90` | 0x23 / 0x24, 0x26 |
| stash | close | 0x4F 0x12 |
| cube | transmute / close | 0x4F 0x18 / 0x4F 0x17 (`world/cube.md`) |
| waypoint | row / close | 0x49 (`world/waypoints.md` §6) |
| NPC menu | options | §14.1 |
| shop | buy / sell / repair | 0x32 / 0x33 / 0x35 (`world/vendors.md`) |
| player trade | `0x004B8730`–`0x004B9110` | 0x4F buttons 2, 4, 7, 8 (gold, p1/p2 = amount halves) |
| quest log | `0x004A2760`, `0x004A34F0` | 0x58 |
| skill select | `0x004A9BD0`, `0x004AA030` / hot keys `0x004A9D70`… | 0x3C / 0x51 |

Exact grid-to-message rules (which cell, which item state) are
`items/inventory-moves.md` §7 and `ui/inventory.md` (§Open questions 1).

### 16. Machine tables

#### 16.1 `ui-states.tsv`

One row per state 0–37. Columns: `id`, `name` (D2MOO), `flag` (address
of the u32), `slot` (`right` | `left` | `full` | `anvil` | `none`, §4.1),
`exp_only` (1: §2.3), `conflicts` (38 characters, one decimal digit per
requested state 0–37: `C[id][j]`, §3.3). Read from the 1.14d tables
`0x00711130` (row pointers) and `0x0070FAA0` + 0x98 × (id − 1).

#### 16.2 `panel-layout.tsv`

One row per draw, text or hit. Columns:

| Column | Meaning |
|---|---|
| `panel` | ui id (decimal) or `border` / `ctrlpnl` |
| `item` | name, unique per panel and kind with `cond` |
| `kind` | `draw` (cel at draw position x, y), `text` (pen x, y), `hit` (rectangle) |
| `file` | `draw`: path under `data\global\ui\` without extension (`C` = class letter of §10.1, `CC` = class icon prefix of §10.3: `Am`, `So`, `Ne`, `Pa`, `Ba`, `Dr`, `As`); else `-` |
| `frame` | `draw`: frame index, or a word named in §8–§14 (`iconcel`, `tabstate`, `rowstate`); `text`: string id, `value`, `level`, `tabstr`, or `a/b` alternatives |
| `x`, `y` | expression: terms joined by `+` / `-`, a term is a decimal integer, `W`, `H`, `W2` (= W / 2), `sx`, `sy`; no spaces |
| `w`, `h` | `hit`: size, the rectangle covers `x … x + w − 1`, `y … y + h − 1` (`h` `-` for `tab1_bottom`: bottom row `H − 50`); `text`: `w` = centering span when `cond` has `centered`; else `-` |
| `color` | `text`: color index `k`, `cmp` (§8.7) or a state word; else `-` |
| `font` | `text`: font id (`text-fonts.tsv`); else `-` |
| `cond` | comma list of: `always`, `res2`, `res_not2`, `mode_l` (open mode 2 or 3), `mode_r` (1 or 3), `exp`, `classic` (game type, §8.1), `released`, `pressed`, `statpts`, `tab1` – `tab3`, `skill_here`, `tab1_bottom`, `tab_shown`, `tab_visible`, `row_used`, `row_known`, `centered`, `split_lf` (§8.6), `half_centered` (pen x = x − width / 2) |
| `addr` | 1.14d function that draws or tests it |

#### 16.3 `npc-menus.tsv`

One row per static record of §14.1: `record`, `npc` (class id),
`count`, `options` (comma list of `string_id:kind`, kind ∈ `talk`,
`trade`, `gamble`, `hire`, `travel_west`, `sail_west`, `identify`,
`resurrect`), `flag` (byte @0x26, meaning §Open questions 8).

## Constants & data dependencies

- Panel shift (80, −60) at resolution mode 2 (§1.1); panel quad offsets
  256 / −224 / −48 (§1.4); UI state count 38.
- DC6 files (archive / frames measured 2026-10-07): `panel\invchar`
  (d2data, 8), `panel\invchar6` (d2exp, 8), `panel\bank` (d2data, 4),
  `panel\TradeStash` (d2exp, 4), `panel\supertransmogrifier` (4),
  `panel\buysell` (4), `panel\buysellbtn` (d2exp 23, d2data 18),
  `panel\buyselltabs` (8, 79 × 31), `panel\800BorderFrame` (d2exp, 10),
  `panel\ctrlpnl7` (6), `panel\800ctrlpnl7` (d2exp, 7), `panel\level`
  (3, 30 × 30), `panel\levelsocket` (1, 35 × 36), `panel\skillpoints`
  (1, 135 × 23), `panel\miniconvert` (2, 32 × 32), `panel\inv_*` (§9.4),
  `spells\skltree_?_back` (16 each; `d`, `i` in d2exp), class skill
  icons 48 × 48, `menu\waygatebackground` (4), `menu\waygatetabs`
  (8, 78 × 30), `menu\expwaygatetabs` (d2exp, 10, 63 × 31),
  `menu\waygateicons` (5, 30 × 30), `menu\horadric` (31, non-zero frame offsets, §12.4). `Patch_D2.mpq`
  holds no UI file (listfile).
- `inventory.bin` records 0–31 (fields `data/fields.tsv` `inventory`).
- Strings (English): 3990, 3991, 4036–4039, 4051, 4057–4076, 4130, 4144,
  3341, 3381–3398, 4020, 22695.

## Randomness

None. The cube animation (§12.4) and the skill-tree / waypoint hover
states depend on wall-clock time and mouse position, not on the game RNG.

## Edge cases & original bugs

Reproduced by default.

- Release handlers of the character and inventory close buttons and the
  stat buttons do not check where the press began (§8.3, §8.5, §9.3).
- The weapon-swap tab draws and a second hand-background pass are dead
  code in 1.14d (§9.5).
- The skill tree's tab-1 hit rectangle ignores `sy` for its bottom edge
  (§10.2).
- Shift-click on a stat button spends all points in chunks of 32, one
  message per chunk (§8.5).
- "On" for an already open state usually returns 0 because most states
  refuse themselves (§2.8); toggling is the normal key path.
- A state closed by the gate (action 1) stays closed when a later row
  refuses (§3.3).
- Under DirectDraw the two outer control-panel pieces sit one row higher
  (§6.2); the GDI reference does not show it.
- The 800 × 600 border is drawn only when a panel is open on that side,
  so with mode 0 the screen edges show the world (§6.1).

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| 800 × 600, char panel open | quads at (80, 316), (336, 316), (80, 492), (336, 492): rows 61–492, columns 80–399 | §1.4 |
| 640 × 480, inventory open | invchar6 frames 4–7 at (320, 256), (576, 256), (320, 432), (576, 432) | §1.4, §9.3 |
| 800 × 600, inventory open | frames 4–7 at (400, 316), (656, 316), (400, 492), (656, 492); close button (418, 476) | §9.3 |
| `SetUIState(1, on)` with nothing open | flag 1 set; open mode 1; `shiftX` −200 at 800 × 600 (`camera.md` §1) | §4.2 |
| inventory open, `SetUIState(2, on)` | `C[1][2]` = 0: both open; mode 3 | §3, §4.2 |
| inventory open, `SetUIState(4, on)` | `C[1][4]` = 1: inventory closed (mode 0), then skill tree open (mode 1) | §3.3 |
| stash open, `SetUIState(1, toggle)` | `C[0x19][1]` = 2: refused, returns 0 | §3.3 |
| quest log open, `SetUIState(2, on)` | `C[0x0F][2]` = 1: quest log closed, character open, mode 2 | §3.3 |
| right panel opened at 800 × 600, jump = 1, mouse x 500 | cursor x 300 | §4.3 |
| left panel closed at 640 × 480, jump = 1, mouse x 600 | cursor x 440 | §4.3 |
| shift-click "Vitality +" with 70 points | three 0x3A messages: `3A 03 1F`, `3A 03 1F`, `3A 03 05` | §8.5 |
| click "Strength +" without shift | `3A 00 00` | §8.5 |
| label `strchrfir` (4071, "Fire⏎Resistance"), 640 × 480 | "Fire" centered in [190, 268] at y 342, "Resistance" at y 350 | §8.6 |
| `inventory.bin` record 0 (amazon, 640) | inv (320, 640, 0, 441); grid 10 × 4 at (339, 626, 255, 368), box 29 × 29; head (455, 509, 8, 59); torso (453, 509, 77, 159) | §9.2, d2exp `inventory.bin` |
| record 16 (amazon, 800) | inv (400, 720, 60, 501); grid (419, 706, 315, 428); head (535, 589, 68, 119) = record 0 + (80, 60) | §9.2 |
| record 12 (expansion stash, 640) / 28 (800) | grid 6 × 8 at (74, 244, 82, 313) / (154, 324, 142, 373) | §11, `items/inventory.md` §1.3 |
| empty head slot, record 16 | `inv_helm_glove` frame 1 at (535, 118) | §9.4 |
| amazon skill tree tab 1, 800 × 600 | frames 0–3 then 4–7 at (400, 316) …; close button at (571, 477) | §10.1, §10.6 |
| skill in column 2, row 3 at 800 × 600 | icon at (484, 258); hit 485–531 × 211–257 | §10.3, §10.5 |
| Akara (148) menu | talk, trade, cancel (count 3) | §14.1 |
| capture `placement-0001` inventory frames | inventory pixels equal the CPU reference of §9.3 (frames 4–7 and the close button); camera fields per `camera.md` | capture, queued (LOCAL-RUN batch 6) |
| capture `ui-0001` (to add to `capture.md` §8): open in turn character, inventory, skill tree, stash, cube, waypoint, at 800 × 600 | each frame equals the CPU reference of §6–§13 | capture, queued |
| capture `ui-0002`: same at 640 × 480 (`-w` with the 640 option) | same | capture, queued |
| `menu\horadric` frame 1 (92 × 121, offset (−205, 17)) at 640 × 480 | drawn at (320, 239): columns 115–206, rows 136–256 | §12.4, d2data DC6 header |
| `menu\horadric` frame offsets (game-file test) | frames 0 and 30: 2 × 2 at (0, 0); frames 1–29 non-zero offsets as measured (frame 1 (−205, 17), frame 15 (−280, 82)); 31 frames | §12.4 |
| Horadric animation, 30 steps done | `n` = 30: nothing drawn, flag 0; only frames 0–29 were ever drawn | §12.4 |
| experience 1234567 | `1,234,567`, color 0 | §8.11 |
| level 1 character, next level | `500` (experience.txt row `1`) | §8.11 |
| experience 999 / 1000 | `999` / `1,000` | §8.11 |
| sorceress skill tree tab 3, 800 × 600 | close button at (W − sx − 149, H + sy − 63) = (571, 477); variant 1 | §10.6 |
| druid, any tab | `o` = −149 | §10.6 |
| fire resist 80, max-fire 0, normal, no effect | cap 75: shown 75, color 4 | §8.9 |
| fire resist −150, max-fire −200 (cap −125) | shown −125, color 4 | §8.9 |
| cold resist −10, lowering and raising effects both active | shown −10, color 1 | §8.9 |

## Provenance

1.14d `Game.exe` (exports `re/exports/funcs`, disassembly via
`tools/ghidra/disasm.py` and `all.asm`): UI draw pass `0x00456EE0`;
`SetUIState` `0x00455F20` (switch table `0x004562BC` / `0x004562D8`), gate
`0x00453910` (action table `0x00453A7C`), conflict rows `0x00711130`,
hooks `0x00455720` / `0x00455AE0`, `SetScreenOpenMode` `0x0045AEA0`;
border `0x00498630` / `0x00498700`, chooser `0x00499450`, control panel
`0x004983D0`; character panel `0x004A7D00` (tables `0x00724818`,
`0x00724928`, `0x00724A48`), handlers `0x004A7720` / `0x004A78C0`;
inventory family `0x0048EDF0`, layout `0x004835B0`, cels `0x004868B0`,
`0x004967F0`, close `0x00486E10` / `0x00486EF0` / `0x00489190`, offsets
`0x007220B0`; skill tree `0x004AC690`, `0x004AAAE0`, `0x004AA9C0`,
`0x004ABF60`, `0x004AB7E0`, `0x004ABC30`, `0x004AB530` / `0x004AB630`,
tables `0x00724BF0`, `0x00724CE4`; stash `0x00489E00`, `0x00489E50`,
`0x00489AC0`, `0x00489CE0`, `0x00489EE0`; cube `0x0048A460`, `0x0048A4B0`,
`0x0048A540`; waypoint `0x0049C9C0`, `0x0049C580`, table `0x007224E8`;
NPC menu `0x00726C48`, `0x004B2250`, `0x004B66B0`, `0x004B6410`,
`0x004B6440`, `0x004B4830`, `0x004B8100`; shop `0x00488400`,
`0x004B23E0`, tables `0x00722110`, `0x00722168`; message helpers
`0x00478350`, `0x00478590`–`0x00478890` (scan of all callers). Community
labels (`game\LoD 1.14D.txt`): `ScreenOpenMode`, `ScreenShiftX/Y`,
`InventoryArrangeMode`, `IsGameMenuOpen`, `IsAutomapOpen`,
`IsHelpScreenOpen`, `ResolutionMode`, `VideoMode`. D2MOO (1.10f)
`D2Constants.h` gave the UI state names only; every id was confirmed by
the 1.14d draw call the flag gates (§5) and by the labels. Riiablo panel
code (`screen/panel/*.java`) hinted that the character page uses
`invchar6` frames 0–3 and the inventory frames 4–7; confirmed by
`0x004A7D00` and `0x0048EDF0`. Game files: DC6 headers of all
`data\global\ui\{panel,menu,spells,cursor}` files (d2data, d2exp;
Patch_D2 has none), `inventory.bin` (d2exp), English string tables;
probes with Python, outputs outside the repo. No capture yet.
2026-10-07 additions: value switch `0x004A8448` (index bytes
`0x004A897C`, jump table `0x004A8950`, resist table `0x004A89A4`),
grouping `0x00525350`, next-level lookups `0x00611800` / `0x00611830`;
icon files `0x00724AC0`, close offsets `0x00724CE4` read from the image;
Horadric animation `0x0048F03E`–`0x0048F0C0`, start `0x0048A540`, frame
headers of d2data `menu\horadric.dc6` (all 31 frames, Python, outside the
repo; HANDOFF §5 C71 found frame 1 at (−205, 17)); cube close
`0x0048A050`, `0x0048A190`, `0x0048A500`; Resurrect insert `0x004B6440`.

## Open questions

1. *Partly answered* (2026-10-07): the control panel overlays, the
   mini panel and the new-stats / new-skills buttons are
   `ui/control-panel.md`. Was: Owners still to write (listed so the work is not lost): control panel
   overlays (globes and their fill rule, belt, skill buttons, mini panel
   `0x0047F710` / button handler `0x0047EC50`, run and menu buttons,
   new-stats / new-skills buttons), cursor (`client/ui.md` §B6,
   `capture.md` §3.3), hover text / item boxes (`0x00502280` queue),
   inventory grid and item drawing (`ui/inventory.md`), automap
   (`ui/automap.md`), quest log, party, mercenary panel, help screen,
   escape menu, chat. Ghidra reads of the addresses in §5. Quest log
   states, icons and the 0x58 replay: `world/quests-status.md` §3, §5
   (`0x004A34F0`, `0x004A3220`, `0x004A23D0`, `0x004A27D0`); only its
   layout, hit rectangles and draw order belong here.
2. **Answered** (2026-10-07, `ui/controls.md` §3: command table
   `0x00712698`, compiled default keys `0x00712220`; each command's
   `SetUIState(ui, mode, jump)` is in its row). Was: Which key toggles
   which state and with which `jump` (CmdTbl `0x004A5…`, `default.key`):
   owner `ui/controls.md` (`client/ui.md` §B4). Read of the command
   table.
3. **Answered** (2026-10-07, `panels-2.md` §17; the `descdam` /
   `descatt` functions: `panels-2.md` OQ 1). Was: Character panel: damage / attack-rating block, name and class lines,
   per-stat hover texts (§8.10). Disassembly of `0x004A7D00` after
   `0x004A818C` and `0x004A7340`–`0x004A7AE0`.
4. Skill tree (no-points message, free points, tab tool tips answered
   2026-10-07 in `panels-2.md` §19; partly answered 2026-10-07: the icon file `CC` §10.3 and
   the close offsets per class §10.6 are read from the image; still
   open as below): the free-points box (`0x004AC200`), the tab tool tips
   (`0x004AB310`), the no-points mouse-down message (§10.5), the exact
   remap `k` per state (verify by capture).
5. **Answered** (2026-10-07, `panels-2.md` §21: gold lines
   `0x00488100`, buttons `0x00486DA0` / `0x00489920`, dialog
   `0x00454150`; `0x004845A0` is the equipment draw, `ui/inventory.md`
   §6; the dialog's generic controls: `panels-2.md` OQ 6). Was: Stash
   gold buttons, gold dialog and the inventory gold button
   (`0x004845A0`, `0x00489580`, `0x004891xx`). Ghidra read.
6. **Answered** (2026-10-07, `world/vendors-2.md` §10.2: button 0x12
   closes the stash interaction and recounts scrolls / tomes; it sends
   nothing). Was: Server meaning of C→S 0x4F button 0x12 (stash close) and of the
   player-trade buttons 2, 4, 7, 8: `0x0054C7C0` → `0x00568060`.
7. **Answered** (2026-10-07, `ui/menus.md` §1: `0x0049D160` mouse down,
   `0x0049D010` mouse up, `0x0049C490` tab hit, `0x0049C510` row hit,
   `0x0049C760` tab set, latch `[0x007BF085]`). Was: waypoint tab and
   row click rectangles and the tab switch
   (`0x0049D010`, `0x0049D160`): Ghidra read.
8. **Partly answered** (2026-10-07): box position and item metrics,
   the hire sender (C→S 0x36 `0x004B1E9E`; the 0x38 action 3 request
   at menu open `0x004B48E8`) and the Resurrect insert are in
   `ui/menus.md` §2–§3 and §14.2; still open: the remaining runtime inserts
   (the flag byte: `panels-2.md` §14.8). Was: NPC menu: box position and item metrics (`0x004B7EB0`, `0x004B85F0`),
   the record flag byte, the remaining runtime option inserts (imbue,
   add sockets, personalize, go east / sail east), the hire sender
   (`0x004B5C60`). Ghidra read; check against a capture of Akara's and
   Charsi's menus.
9. *Partly answered* (`panels-3.md` §27: scroll, recipe, guild,
   anvil; trade stays open). Trade panel (ui 0x17, inventory mode 0x0B: `%s\ui\panel\trade`
   `[0x007BCB04]`, both players' names and gold at `0x1B − sy` /
   `0xF2 − sy`), anvil (ui 0x0E), Inifuss scroll, recipe scroll, guild
   panels (0x1B, 0x1C): not specified here.
10. Pixel proof: capture cases `ui-0001`, `ui-0002` and the inventory
    frames of `placement-0001` (§Test vectors).
11. **Answered** (2026-10-07): mode 3 is the additive blend of
    `render/blend-modes.md` §1; the pixels stay a capture case
    (recording list). Was: The Horadric animation draws with draw mode 3 (§12.4) and light 0xFF:
    which blend that is for a cel with these offsets is `render/blend-
    modes.md`'s; confirm the pixels with a capture of a transmute
    (frames 1, 15, 29).
12. **Answered** (2026-10-07, `panels-2.md` §17.10; pixels: capture).
    Was: Character panel draw order of the 18 values against the labels and
    the add buttons when a value is wider than its span (overlap):
    capture `ui-0001` with a level-99 character (experience
    `3,520,485,254`).
13. **Answered** (2026-10-07, `panels-2.md` §20.4): two. Was: Cube-gone close (§12.2): `0x0048F183` sends 0x4F 0x17 directly and
    `SetUIState(0x1A, off)` runs the close hook `0x0048A500` → `0x0048A050`
    with its latched send (§12.7): one or two 0x4F 0x17 messages? Read
    whether `[0x007BCC54]` or the inventory mode is changed before the
    `SetUIState` call at `0x0048F160`–`0x0048F183`.
