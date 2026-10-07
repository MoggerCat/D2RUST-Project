# Handoff: PC 2 spec worker — UI (`specs/ui/*` except `text.md`)

Branch `claude/spec-ui` (from `origin/claude/local-pc2-integration`
`b707ad1`), 2026-10-07. Evidence: the 1.14d `Game.exe` (Ghidra exports,
`tools/ghidra/disasm.py`), table values and DC6 frame headers read with
Python scripts in `C:\Users\zffit\Desktop\D2test\scratch-ui\` (outside the
repo). New spec `specs/ui/menus.md` (split out of `ui/panels.md`, which
was at the 60 KB limit).

## Answered

| Question | Spec § | Answer |
|---|---|---|
| HANDOFF §5 C71 (`menu\horadric` frame 1 offset (−205, 17), test expected (0, 0)) | `panels.md` §1.3, §12.4, §Test vectors | The file is right, the spec / test were wrong: frames 0 and 30 are 2 × 2 at (0, 0), frames 1–29 carry offsets x −287 … −205, y 17 … 82 (all 31 headers measured). The draw (`0x0048F0C0`) is a normal cel draw at (W / 2, H / 2 − 1), mode 3, so the offsets apply; frame 30 is never drawn. |
| UP-1 (thousands grouping `0x00525350`) | `panels.md` §8.7, §8.11 | Unsigned decimal, `,` every 3 digits from the right, no locale, `*` on overflow (unreachable). Only experience (13) and next level (30) use it; **level (12) does not** (cmp color, `%ld`). Next level = `experience.txt` entry on row `Level` = base level (`0x00611800`), stat 30 at MaxLvl. |
| UP-2 (popup width `0x00502520`) | `panels.md` §8.8 | Max width (`0x00501840`) of the value in Font16, compared ≥ `x2 − x1` (table values). |
| UP-3 (number format) | `panels.md` §8.7 | `%ld` (`0x006D9BE8`, `wsprintfA`) for every non-grouped value. |
| UP-4 (cmp shifted?, colors of 6, 8, 10) | `panels.md` §8.7 | 7, 9, 11 compare the unshifted values; 6, 8, 10 are color 0. |
| UP-5 (resist color priority, clamp order) | `panels.md` §8.9 | Lowering wins over raising; a value at the cap is color 4 unless blue; a cap below −100 wins over the −100 floor. |
| UP-12 (cube close also `SetUIState(0x1A, off)`?) | `panels.md` §12.7 | Yes: `0x0048A050` = `SetUIState(0x1A, off, 0)` + latched 0x4F 0x17 (`[0x007BCC54]`), one message per open. |
| UP-13 (Horadric frame 30) | `panels.md` §12.4 | Never drawn (stop at `n` = 30 before the draw); mode 3, light 0xFF confirmed. The `ImageRequest` field is an implementation choice. Also: the cube grid is hidden while `n` < 14. |
| UP-14 part (Resurrect slot `0x004B6440`) | `panels.md` §14.2 | Inserted before the hire slot, else after the last option; placeholder id 0x1507 captioned "Resurrect %s: %d" (`menus.md` §2.3). |
| UP-15 part (hire sender, menu box) | `menus.md` §2, §3 | 0x38 action 3 is sent when the NPC menu opens for 150, 198, 252, 515 (`0x004B48E8`); choosing a hire row sends 0x36 [NPC GUID][merc id u16→u32] (`0x004B1E9E`). Box object, auto layout, anchor (NPC pixel − tile origin, 150 up, y ≥ 20), draw. |
| UP-18 (buy / sell / repair fields) | `menus.md` §4 | Kind, price function arguments per NPC class, the 0x32 / 0x33 / 0x35 u32 @9 and @13 (0x35 one item: @13 = durability), timers, confirm vs immediate. |
| UP-19 (icon prefix `CC`) | `panels.md` §10.3, §16.2 | `Spells\<CC>Skillicon`, `CC` = Am So Ne Pa Ba Dr As (`0x00724AC0`). |
| UP-20 (close offsets other classes) | `panels.md` §10.6 | Full 7 × 3 table from `0x00724CE4`. |
| UP-24 (waypoint tab / row click rects, tab switch) | `menus.md` §1.2–§1.4 | Tab: y' ≤ 30, x' bands 64 / 80; rows strict 17 < x' < 297, hy < y' < hy + 30; the tab setter falls back by quest records 28 / 23 / 15 / 7 (draw uses 26 for tab 4). |
| UP-25 (self-close mode / jump, latch reset) | `menus.md` §1.5, §1.7 | `SetUIState(0x14, off, 0)`, draw continues; latch cleared only at open, `0x0049C6A0`, `0x0049C7F0` (tab > 5). |
| UP-26 (close via `0x0049CEC0` twice?) | `menus.md` §1.5, §1.8 | Once: every level-0 send shares the latch `[0x007BF085]`. |
| UP-27 part (tab on open) | `menus.md` §1.4 | The act of the player's level, then the quest fallback. |
| UP-11 (stash close: one or two 0x4F 0x12) | `panels.md` §11.7 | The close hook sends one (no latch, only in inventory mode 0x0C / 0x0D); the stash close-button release (`0x00489B92`) and the key handler (`0x00489D21`) send a second one: **two**. A release on the inventory close button sends one. |
| UP-24 rest (filled hover rectangle extent) | `panels.md` §13.4 | `0x004F6300(sx + 287 − s, 370 − sy, sx + 294 + s, 387 − sy, 0, 2)`, `s` = half the text width; text pen (`sx + 292 − s`, `385 − sy`). |
| UP-27 rest (color of an unknown selected row) | `panels.md` §13.5 | Always 5: the row hit skips unknown rows, so they are never pressed; 3 = pressed row (close not pressed) or current level. |
| `panels.md` OQ 7 | `menus.md` §1 | Answered (marked in place). |
| `panels.md` OQ 4, OQ 8 | §10.3, §10.6; `menus.md` §2–§3 | Partly answered (marked in place). |
| UP-6 (0 points, damage block, name / class, hovers) | `panels-2.md` §17 | 0 points: nothing drawn, presses nothing; mouse-up stops at the first hit button (corrects §8.5); class line (charstats +0) and name line (font by UTF-8 length); damage block `0x004ED570` entries `0x0072D840`; chance-to-hit / to-be-hit formulas and popups (`0x004A7340`, `0x004A74A0`, `0x004A7AE0`, `0x004A7180`); descdam / descatt functions left to a skills spec (`panels-2.md` OQ 1). |
| UP-7 | `panels-2.md` §18.1, §18.4 | Yes, `0x00486EF0` clears `[0x007BCE90]` first; inv_* rows not added (slot rectangles, grammar cannot name them; §9.4 is the source). |
| UP-8 | `panels-2.md` §18.2 | Right edge exclusive, bottom **inclusive** in all 10 copies of the test. |
| UP-10 | `panels-2.md` §20.1–§20.3 | Stash close press / release strict, hover inclusive; stash gold, cube close, transmute rectangles. |
| UP-14 rest | `panels-2.md` §14.10 | `0x004B5640` on the transaction result in menu state 10: Cain records 16–19, 38 count := 2, rebuild. |
| UP-15 rest | `panels-2.md` §14.9 | Talk option sends nothing; 0x2F [type][GUID], 0x30 [1][GUID] / [a4][GUID], 0x31 [GUID][msg id]. |
| UP-16 | `panels-2.md` §14.7, §14.8 | First record wins, unknown class → record 0; flag byte = reopen the menu after talking. |
| UP-17 | `panels-2.md` §14.11–§14.13 | Button records per NPC class, mode always 3, `0x004B3500` always 0, tab pages / delay, tab and button hit rects. |
| UP-21, UP-22, UP-23 | `panels-2.md` §19 | No-points 0x3C [skill][−1] + close; free-points number; tab tool tips (left-aligned bug); press / release order, points re-checked; draw order. |
| `panels.md` OQ 11, 12, 13 | `panels.md` OQ; `panels-2.md` §17.10, §20.4 | Mode 3 = additive (blend-modes §1); draw order; cube-gone close sends **two** 0x4F 0x17. |
| `menus.md` OQ 1, 2, 3 | `menus.md` §3.5, §4.4, §4.5 | Hire row text; confirm dialog; click callers (wParam = flags, Shift → 0x32 bit 31 on quick paths). |
| Split | `panels.md` §14 → `panels-2.md` §14 | panels.md 61.0 → 58.9 KB; §14 had no coverage claims. |
| xpc-to-pc2 `ui/*` lines (0x26, 0x27, 0x4E, 0x50, 0x58, 0x8A, 0x91 consumers) | new `ui/messages.md` §1–§14 | Screen message list (add / wrap / 18-line cap / 10 s expiry / draw), chat line formats of types 1, 2, 4, 6 (+ 5, 7: recipe scroll), overhead bubbles (text pass, per-unit placement, 16-slot overlap search, bubble draw), NPC text list (sorted list, talk topic box, caption table), dialog panel (open, speed line, scroll, speech wait, end, skip input, 0x31 on close), timed box, hire popup (`SetUIState(0x23)`, `PopupHireling` → hireling panel once), 0x50 code effects (3 dead, 4 Inifuss stones), item-socket dialog (state 0x0E: orifice / imbue / sockets / personalize, 0x44 / 0x38 sends), 0x8A balloon overlay, intro table +0x12 (return greeting + C→S 0x4D). |
| UP-28 (control panel overlays) | new `ui/control-panel.md` §1–§10 | Draw order of `0x00499450`; globes (smoothing `0x00496DD0`, 80-row fill, potion overlays, poison frame, covers, HP / MP numbers and toggles); experience and stamina bars; run / walk and menu buttons; belt (type from body location 8, `belts.bin` record, pop-up rows, slot boxes, key labels, hit area, pop-up on hover); skill buttons; new-stats / new-skills buttons per resolution; mini panel (7 / 8 buttons by game type, three layouts, functions, tool tips, press / release); control panel mouse down / up. |
| UP-9 (GoldMax font) | `panels-2.md` §20.6, OQ 4 | Font 1: the belt draw `0x00499040` sets it every frame in step 7 and never restores it; every other font setter reached before the next frame's stash draw saves and restores (static read of the 176 `0x00502EF0` call sites); exception: a refused overhead bubble leaves font 13. |
| `ui/messages.md` OQ 3 | `ui/messages.md` OQ 3 | Binding 7 = automap (mini-panel tool tip table `0x0047F490`). |
| `client/msg-ui.md` OQ7 (writer part) | `ui/messages.md` §14 | All 12 writes of `[0x007C0D25]` / `[0x007C0D29]` are UI code (`0x004B1640`, `0x004B66B0`, `0x004B6DD0`, `0x004B3C20`, `0x004B3E10`): UI state; the bridge needs a UI-keyed lookup rule (cross-file request). |

## xpc-to-pc2 lines done

- `docs/handoff/xpc-to-pc2.md` line 16 (`ui/*`, owner of the UI consumer, `sim/intents-events.md` OQ18: a UI rule per entry point for 0x26, 0x27, 0x4E, 0x50, 0x58, 0x8A, 0x91) — done in `ui/messages.md` §1–§14, commit `a82a04c` (fix-ups in the next commits).
- `docs/handoff/xpc-to-pc2.md` line 17 (`ui/*` follow-up, PC 1 area 3: chat formats and colours, overhead draw and record rule, NPC text list / dialog panel / box, hire popup and list, 0x58 dialogs, intro table +0x12, interact-NPC writer, `client/msg-ui.md` OQ7) — done in `ui/messages.md` §2–§14, commit `a82a04c`; OQ7 writer part answered (§14), the bridge rule is a cross-file request.

## Still open

- UP-19 rest: remap `k` per state (capture).
- `ui/control-panel.md` OQ 1–6 (x87 precision of the globe smoothing; belt hover / click; skill icon overlays; level-change timer; new-stats / new-skills input; captures).
- `panels-2.md` OQ 1 (descdam / descatt functions), OQ 2 (skill-tree tab captions), OQ 3 (result note caption), OQ 5 (button fields +0x0A, +0x10).
- `panels.md` OQ 8 rest (runtime inserts), `menus.md` OQ 4 (captures).

- `ui/messages.md` OQ 1–6 (recording; allocator state of the bubble box; binding 7; gossip text records; 0x50 code 36; inventory handlers / item checks of the socket dialog).

## CODE-TABLE CHANGE commits

None (no TSV changed; the per-class close offsets are prose, the
`panel-layout.tsv` `cond` grammar has no class word).

## Cross-file requests

- to PC 1 (`ui/text.md`): the wide formatter `0x005269D0` takes one argument for every conversion, `%%` included (caller `0x004A7180` passes (block, chance, name, chance) for "%d%%
… %s … %d%%"); only `%d`, `%u`, `%s`, `%%` are handled (others fatal). Add as a rule.
- to PC 1 (skills description spec): `panels-2.md` OQ 1, the `descdam` / `descatt` function tables `0x0072D768` / `0x0072D7F8` drawn by the character panel `0x004ED570`.
- to PC 1 (`d2-client::ui::panels`): `panels.md` §14 moved to `panels-2.md` §14 (numbers unchanged); `panels.md` §8.5 / §9.3 corrected (release order, no cursor-item action); new §17–§20.

- to PC 1 (`crates/d2-client/tests/game_panels.rs`, C71): `menu\horadric` must not be held to zero offsets: expect frames 0 and 30 = 2 × 2 at (0, 0), frame 1 = 92 × 121 at (−205, 17), frame 15 = 239 × 255 at (−280, 82) (`ui/panels.md` §12.4, §Test vectors); then rerun C71 so the 7 `skltree_?_back` counts are reached.
- to PC 1 (`d2-client::ui::panels` impl, `impl-ui-panels.md`): `panels.md` §8.7 changed: level (12) uses the cmp color and `%ld`, not the grouping; skill-tree close offsets for all classes (§10.6); cube close and waypoint input now specified (`panels.md` §12.7, `menus.md` §1).
- to PC 2 vendors (`world/vendors.md` §7.1): the 1.14d client puts the item's mode (unit +0x10) in bits 16–31 of the 0x32 u32 @9 (`0x004B2760`–`0x004B2763`); with store items in mode 0 the bytes are 0, but the "unused (bits 16–30)" wording should say what the client writes. §8.1: the client's 0x35 one-item u32 @13 is the item's stat 72 value (`0x004B27F1`), so bit 31 is clear unless durability ≥ 2³¹.
- to PC 2 npc / hirelings (`world/npc.md` §4): the client sends 0x38 action 3 when the NPC menu opens for classes 252, 198, 515, 150, with the player's GUID (−1 without a player) in the u32 @9 (`0x004B48C9`–`0x004B48E8`); 0x36 merc id is sent as a u32 (u16 zero-extended, `0x004B1E94`).

- to PC 1 (`client/msg-ui.md` OQ7, `client/bridge.md` §10 r3): every writer of the interact NPC `[0x007C0D25]` / `[0x007C0D29]` is UI code (`ui/messages.md` §14: `0x004B1640`, `0x004B66B0`, `0x004B6DD0`, `0x004B3C20`, `0x004B3E10`), so it stays UI state: add the bridge rule for UI-keyed lookups at delivery (0x50 code 3, 0x8A) and mark OQ7 answered.
- to PC 1 (`client/msg-ui.md` §7 code 2): `0x004939B0` never reads the hire-table entry `0x004B3340` leaves in EAX; the UI effect is `SetUIState(0x23, on)` + the `PopupHireling` flag (`ui/messages.md` §9 r2). Code 3: `[0x007C0D39]` / `[0x007C0D3D]` have no reader in `Game.exe` (§10).
- to PC 1 (`client/msg-ui.md` §5 r2.3): with a type other than 1 or 2 the list is freed without a null test (`0x004A170D` → `0x006612A0`, reads [0 + 8]); note the 1.14d crash and the d2rs choice (`ui/messages.md` §Edge cases).
- to PC 1 (`client/model.md` / `client/msg-ui.md`, model writes inside UI code): the interaction end `0x004B3C20` sets the NPC's unit flag +0xC4 |= 2 (`0x004B3CAE`) and clears the local player's data +0x150…+0x15C (`0x004B3C42`–`0x004B3C60`); the NPC menu open `0x004B66B0` clears +0xC4 bit 1 (`0x004B6794`) and sets it again when state 8 cannot open (`0x004B6890`).
- to PC 1 (`client/msg-ui.md` §8 code 0): the "dialog" is the item-socket dialog, UI state 0x0E; refusal sends C→S 0x44 [player GUID or −1][object GUID][0][2] (`0x004C03F4`), `ui/messages.md` §11.
- to PC 1 (`client/msg-ui.md` OQ9): the filter object's method +0x18 also receives every one-line color-4 screen message as 8-bit text (`0x0049E4E6`–`0x0049E52D`, `ui/messages.md` §2 r2).

- to PC 1 (`client/model.md` / skills): the control panel skill-button draw `0x00496CF0` replaces a left / right skill whose level is ≤ 0 (`0x006442A0(P, skill, 1)`) in the model (`0x00643CE0`, `0x006470F0`, `0x00643BC0(P, 0, −1)` / `0x00643C50(P, 0, −1)`), every frame: a model write inside a UI draw; the bridge needs it as a rule or a model-side check (`ui/control-panel.md` §7 r1).

## Recording list

- A globe refilling after a potion, frame by frame (smoothing precision), and the control panel at 640 × 480 and 800 × 600 with the belt popped and the mini panel in its three layouts (`ui/control-panel.md` OQ 1, OQ 6).
- Chat lines (whisper, whisper echo, broadcast) at 800 × 600; two monsters with overhead text at once (bubble moved); an NPC dialog panel scrolling with frame times; a timed box (`ui/messages.md` OQ 1).

- Stash open at 800 × 600, expansion and classic: the `GoldMax` line, to identify its font (`panels-2.md` OQ 4).

- Transmute in the Horadric Cube at 800 × 600 and 640 × 480: frames with
  animation steps 1, 15, 29 and the step after 29 (no frame), to prove
  the offsets and draw mode 3 (`panels.md` §12.4, OQ 11).
- Character panel of a level-99 character (experience 3,520,485,254) and
  of a level-1 character: the grouped strings and the next-level value
  (`panels.md` §8.11).
- Akara's menu, Charsi's menu and Kashya's hire list at 800 × 600 (box
  position, item rows, highlight) (`menus.md` OQ 1, OQ 4).
- Waypoint menu with each tab clicked and a row hovered (`menus.md` §1,
  `panels.md` §13).
