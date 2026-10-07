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

## Still open

- UP-6 (add buttons with 0 points; damage block, name / class, hovers): no new read (`panels.md` OQ 3).
- UP-7, UP-8, UP-9, UP-10, UP-16, UP-17, UP-21, UP-22, UP-23, UP-28: not reached in this pass.
- UP-14 rest: Cain's count reset `0x004B5640` (note: `0x004B4830` itself rewrites the identify record's count to 2 / 3, `menus.md` §2.3).
- UP-15 rest: talk message bytes (`0x004B6C70` sends nothing; it opens the talk sub-menu `0x004B5890`; the 0x2F / 0x30 senders are `0x004B6E87` / `0x004B6E45`).
- UP-19 rest: remap `k` per state (`panels.md` OQ 4, capture).
- New: `panels.md` OQ 11 (mode 3 pixels of the transmute), OQ 12 (value overflow / draw order), OQ 13 (cube-gone close: one or two 0x4F 0x17); `menus.md` OQ 1 (hire row text), OQ 2 (confirm dialog `0x004B2F50`), OQ 3 (callers' flags), OQ 4 (captures).

## CODE-TABLE CHANGE commits

None (no TSV changed; the per-class close offsets are prose, the
`panel-layout.tsv` `cond` grammar has no class word).

## Cross-file requests

- to PC 1 (`crates/d2-client/tests/game_panels.rs`, C71): `menu\horadric` must not be held to zero offsets: expect frames 0 and 30 = 2 × 2 at (0, 0), frame 1 = 92 × 121 at (−205, 17), frame 15 = 239 × 255 at (−280, 82) (`ui/panels.md` §12.4, §Test vectors); then rerun C71 so the 7 `skltree_?_back` counts are reached.
- to PC 1 (`d2-client::ui::panels` impl, `impl-ui-panels.md`): `panels.md` §8.7 changed: level (12) uses the cmp color and `%ld`, not the grouping; skill-tree close offsets for all classes (§10.6); cube close and waypoint input now specified (`panels.md` §12.7, `menus.md` §1).
- to PC 2 vendors (`world/vendors.md` §7.1): the 1.14d client puts the item's mode (unit +0x10) in bits 16–31 of the 0x32 u32 @9 (`0x004B2760`–`0x004B2763`); with store items in mode 0 the bytes are 0, but the "unused (bits 16–30)" wording should say what the client writes. §8.1: the client's 0x35 one-item u32 @13 is the item's stat 72 value (`0x004B27F1`), so bit 31 is clear unless durability ≥ 2³¹.
- to PC 2 npc / hirelings (`world/npc.md` §4): the client sends 0x38 action 3 when the NPC menu opens for classes 252, 198, 515, 150, with the player's GUID (−1 without a player) in the u32 @9 (`0x004B48C9`–`0x004B48E8`); 0x36 merc id is sent as a u32 (u16 zero-extended, `0x004B1E94`).

## Recording list

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
