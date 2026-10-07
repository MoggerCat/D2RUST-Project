# Spec: UI — Panels, part 3 (cursor, character and skill-tree inputs, waypoint rows, scroll panels)

- **Status:** draft (2026-10-07, RE on the 1.14d `Game.exe` and the
  1.14d cursor / scroll DC6 headers; no capture yet). Continues
  `ui/panels.md` and `ui/panels-2.md`; section numbers continue their
  numbering (§23–§27). Owner of `client/ui.md` §B6 (cursor).
- **Target version:** 1.14d, English install
- **Crate/module:** `d2-client::ui` (cursor, `panels::character`,
  `panels::skilltree`, `panels::waypoint`, scroll panels),
  `d2-client::ui::original` (`PENDING` inputs)
- **Related specs:** `ui/panels.md` (§1 coordinates `W`, `H`, `sx`, `sy`,
  §2 `SetUIState`, §5 pass order, §8 character, §10 skill tree, §13
  waypoint), `ui/panels-2.md` (§17, §19, §22), `ui/menus.md` §1
  (waypoint input), `ui/inventory.md` §8 (item graphic), `ui/text.md`,
  `render/capture.md` §3.3 (recorded cursor globals),
  `render/sprite-placement.md` §2, `render/blend-modes.md`,
  `skills/levels.md` (`skill_level`, `bonus_level`), `world/waypoints.md`
  §2 (record bits), `client/msg-ui.md` (S→C 0x63, quest flags),
  `sim/rng.md` (generator step).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 42–54 |
| Inputs | 55–66 |
| Outputs / state changes | 67–72 |
| Rules | 73–74 |
|   23. Mouse cursor (`client/ui.md` §B6; takes the rule of `render/capture.md` §3.3) | 75–180 |
|   24. Character panel inputs (`panels.md` §8.7–§8.9; the `PENDING` character values) | 181–225 |
|   25. Skill tree inputs (`panels.md` §10.3–§10.5; the `PENDING` icons and levels) | 226–261 |
|   26. Waypoint rows (`panels.md` §13 r2–r7; the `PENDING` waypoint panel) | 262–296 |
|   27. Scroll and other panels (`panels.md` OQ 9) | 297–370 |
|   28. Gold dialog box and controls (`panels-2.md` §21 r9; answers `panels-2.md` OQ 6) | 371–531 |
| Constants & data dependencies | 532–546 |
| Randomness | 547–552 |
| Edge cases & original bugs | 553–566 |
| Test vectors | 567–591 |
| Provenance | 592–619 |
| Open questions | 620–636 |
<!-- /index -->

## Summary

The mouse cursor is a small state machine over seven cursor pictures,
stepped on wall-clock time after each draw, replaced by the cursor item's
inventory graphic while an item is held (§23). The character panel's
value colors read state masks built from `states.txt` flags and the
difficulty's resist penalty (§24). The skill tree shows each class skill's
level with bonuses and decides per frame which icons can take a point
(§25). The waypoint menu rebuilds its rows from the act's level range and
the waypoint record of S→C 0x63 (§26). The scroll panels (Inifuss,
Horadric, recipe) are left-slot pictures with a few animated symbols or
one text line (§27).

## Inputs

| Name | Type | Source |
|---|---|---|
| mouse | position and button events (u16 x +0x0C, y +0x0E) | window messages |
| wall clock | `GetTickCount` ms | cursor and scroll animation |
| local player P | unit, stats, states, skill list, room / level | client model |
| player client seed | unit +0x20 / +0x24 | `render/capture.md` §7 |
| waypoint record | 16 bytes of S→C 0x63 (`[0x007BF081]`) | `client/msg-ui.md`, `world/waypoints.md` §2 |
| client quest flags | `0x004B32D0()` record | `client/msg-ui.md` OQ 4 |
| tables | `states.txt`, `skills.txt`, `skilldesc.txt`, `levels.txt`, `difficultylevels.txt` | `data/fields.tsv` |

## Outputs / state changes

Cursor globals (§23 r2), draws, C→S messages named per rule, the
skill-tree icon state tables `[0x007C0A38]` / `[0x007C0838]`, the
waypoint row table `[0x007BF03C]`.

## Rules

### 23. Mouse cursor (`client/ui.md` §B6; takes the rule of `render/capture.md` §3.3)

1. **Types.** Table `0x00712010`, 7 records of 0x1C bytes: +0x00
   animated, +0x04 loops, +0x08 frame count, +0x0C step (1/256 frame per
   step), +0x10 animates on press, +0x14 draw function, +0x18 name
   (count `[0x007120D4]` = 7, asserted):

   | Type | Name | Animated | Loops | Frames | Step | Press | Draw |
   |---|---|---|---|---|---|---|---|
   | 0 | Gaunt | 0 | 0 | 1 | 0 | 1 | `0x004683C0` |
   | 1 | grasp | 1 | 0 | 8 | 0 | 1 | `0x004683C0` |
   | 2 | ohand | 1 | 0 | 8 | 0x40 | 1 | `0x004683C0` |
   | 3 | orotate | 1 | 1 | 8 | 0x20 | 1 | `0x004683C0` |
   | 4 | ppress | 1 | 1 | 8 | 0x40 | 1 | `0x004683C0` |
   | 5 | protate | 1 | 1 | 8 | 0x40 | 1 | `0x004683C0` |
   | 6 | buysell | 0 | 0 | 1 | 0 | 0 | `0x00468460` |

   Cel `[0x007A6AC0 + 4t]` = `DATA\GLOBAL\UI\CURSOR\<name>` (loaded at
   game start by `0x004680B0`, freed by `0x00468170`). Frames (d2data,
   measured): `gaunt` 1 × (34 × 30, offsets 0, 0); `grasp`, `ohand`,
   `orotate`, `ppress`, `protate` 8 frames of about 32 × 26–30 with
   offsets x −1 / −2, y 23 / 24; `buysell` 10 frames of 32 × 40, offsets
   0. The draw point is the mouse position, so the offsets place the
   picture (`render/sprite-placement.md` §2).
2. **Globals** (`render/capture.md` §3.3 keeps the recorded list): drawn
   `[0x007A6B08]`, mouse x `[0x007A6AB0]`, y `[0x007A6AAC]`, x adjust
   `adj` `[0x007A6AB4]`, type `t` `[0x007A6ADC]`, frame `f` `[0x007A6AE0]`
   (8.8 fixed point, except type 6), state `s` `[0x007A6AF0]`, last step
   time `[0x007A6AEC]`, idle start `[0x007A6AE8]`, cursor item
   `[0x007A6ABC]`.
3. **Init** `0x004680B0` (ECX = `adj`; the only caller `0x0044F3ED`
   passes 0): registers the cursor's window handlers (`0x00451DB0(3)`),
   loads the cels, drawn := 1, s := 1, t := 5, f := 0, idle := now, mouse
   := (W / 2, H / 2).
4. **Mouse move** `0x00468840`: mouse := the event's position, drawn :=
   1. When `0x004F6270()` ≠ 0 and `0x00407FF0()` = 0 and the position is
   outside [0, W − 1] × [0, H − 1], it is clamped to that range, the OS
   cursor is moved there (`SetCursorPos`) and the handler returns.
   Otherwise idle := now and, when s is 4 or 2: s := 3, t := 2, f := 8 ×
   256 − 256 (the last frame of `ohand`, run backwards by r8).
5. **Button down** `0x00467F20`: mouse := the event's position, idle :=
   now; when s ≠ 5 and the current type's press flag is set: s := 5, t :=
   4, f := 0. The event is not consumed.
6. **Button up** `0x00467FA0`: mouse, idle as above; s = 5 → s := 1, t :=
   5, f := 0. Not consumed.
7. **Cursor item** `0x00468070(item)` (every change of the cursor item:
   callers `0x0047EDDD`, `0x00487BD0`, `0x00487C10`, `0x00488446`,
   `0x00488960`, `0x00488CE2`, `0x00488D3C`, `0x00488DD1`, `0x00488F95`,
   `0x0048A650`, `0x00490347`, `0x004909E3`, `0x00490E08`): item := it, t
   := 5, f := 0, s := 4 with an item, else 1. Shop cursors: `0x00468010(u,
   f0)` item := u, t := 6, f := f0, s := 6; `0x00468040(f0, flag)` item :=
   0, t := 6, f := f0, s := 7 + (flag ≠ 0) (callers in the shop mouse
   `0x00488B00`, `0x00488F10`, `ui/menus.md` §4).
8. **Step** `0x00468310` (run after every `0x004683C0` draw; `now` =
   `GetTickCount()`): if the type is animated and `now > last + 16`:
   last := now and
   - s = 3 (`0x004682C0`): f −= 0x40; if f < 0: a looping type adds
     back frames × 256, any other → s := 1, t := 5, f := 0;
   - else (`0x004681C0`, only with a local player): s = 1 → the player's
     client seed takes one step of the D2 generator (`sim/rng.md`; low
     word × 0x6AC690C5 + high word, 64-bit) and f += 0x20 when bits 0–5
     of the new low word are below 16; other states → f += step. Then if
     f ≥ frames × 256: a looping type subtracts frames × 256; else s = 2
     → s := 4, t := 3, f := 0; s = 5 → s := 1, t := 5, f := 0; any other
     state is a fatal error.
   Then, in s = 1 with `now > idle + 5000`: s := 2, t := 2, f := 0.
9. **Draw** `0x004684C0` (once per drawn frame after the UI pass,
   `render/draw-order.md`; nothing while drawn = 0). With an item unit
   (type 4) on the cursor and s < 6: the item's inventory graphic
   (`ui/inventory.md` §8) with its top-left at (`adj + mx − gw / 2`, `my
   − gh / 2`), `gw` × `gh` = the graphic frame's size, halves rounded
   down (unsigned); no step runs, so the cursor frame stays where it
   was. Otherwise the type's draw function.
10. `0x004683C0` (types 0–5): x = clamp(`mx + adj`, `adj`, `W − adj −
    1`), y = clamp(`my`, 0, `H − 1`); cel draw of frame `f >> 8` at (x,
    y), light 0xFF, draw mode 5, no remap; then the step (r8).
11. `0x00468460` (type 6): frame `f` (not shifted) of `buysell` at (`adj
    + mx`, `my + 33`), draw mode 5; no clamp, no step.
12. `0x004685C0` is a dead copy of r9 (no caller; it reads the never
    written `[0x007A6AA8]`, `[0x007A6AB8]`): not reproduced.
13. So an untouched mouse shows `protate` advancing by chance for 5 s,
    then `ohand` once (32 steps), then `orotate` looping every 64 steps; a
    move during `ohand` / `orotate` plays `ohand` backwards (29 steps from
    0x700) back to `protate`; a press shows `ppress` until the release.
    The seed draws of r8 change the client player seed (`render/capture.md`
    §7: recorded per frame).

14. **Which messages reach the cursor** (answers §Open questions 4).
    The init registers the 3 entries of table `0x00711FEC` on the game
    window (kind 0 = window message): `WM_MOUSEMOVE` (0x200) → the move
    r4; `WM_NCMOUSEMOVE` (0xA0) → `0x00467EF0`: drawn := 0 (the cursor
    is not drawn while the mouse is over the non-client area), and when
    the hit-test code (event +8) is 2 (`HTCAPTION`) the OS cursor call
    `0x004F59F0(0)` (video-mode dependent; an app edge); the event is
    left unconsumed (+0x18, +0x1C := 0); `WM_LBUTTONUP` (0x202) → the up
    r6. The press r5 is **not** a window handler: only three panel
    presses call it, the character panel's button press (`0x004A7720`,
    a stat button or the close button armed) and the control panel's
    new-stats / new-skills press (`0x004A66E0`, `0x004A6790`,
    `ui/control-panel.md` §8 r4). So a world click, a right click or any
    other panel press leaves the cursor type unchanged; the `ppress`
    animation shows only on those buttons. The up transition is also
    called directly by `0x0048B7C0`, `0x004A6840`, `0x004A6920`,
    `0x004A78C0` (a second call is harmless: s is 1 by then). No
    right-button message reaches the cursor.

### 24. Character panel inputs (`panels.md` §8.7–§8.9; the `PENDING` character values)

1. **State masks.** `0x0063A130(unit EAX, mask EBX)` is 1 when the
   unit's state bit array (`0x00625BB0`; no array → 0) and the mask share
   a bit, over (state count `[data +0xC4]` + 31) / 32 words. The masks
   are the data tables' per-flag state masks at `+0xCC + 4b`, `b` = the
   `states.txt` flag bit of `data/fields.tsv` (`states` `bit` column):

   | Test | Data offset | Flag (bit) | Used for |
   |---|---|---|---|
   | `0x0063A550` | +0x118 | `armblue` (19) | defense blue |
   | `0x0063A570` | +0x11C | `rfblue` (20) | fire resist blue |
   | `0x0063A590` | +0x120 | `rcblue` (21) | cold resist blue |
   | `0x0063A5B0` | +0x124 | `rlblue` (22) | lightning resist blue |
   | `0x0063A5D0` | +0x128 | `rpblue` (23) | poison resist blue |
   | `0x0063A5F0` | +0x130 | `armred` (25) | defense red |
   | `0x0063A610` | +0x134 | `rfred` (26) | fire resist red |
   | `0x0063A630` | +0x138 | `rcred` (27) | cold resist red |
   | `0x0063A650` | +0x13C | `rlred` (28) | lightning resist red |
   | `0x0063A670` | +0x140 | `rpred` (29) | poison resist red |

   So "a raising / lowering effect" of `panels.md` §8.9 = the player has
   at least one state whose `states.txt` row sets that column.
2. **Resist penalty** (`panels.md` §8.9 "stat − difficulty penalty",
   exact): `v` = stat + `ResistPenalty` of the `difficultylevels.txt`
   record of the client's difficulty (`0x0044DCD0`, record `0x00611D30`,
   index clamped to [0, count − 1]) in an expansion game; in a classic
   game `v` = stat − 20 on difficulty 1, stat − 50 on difficulty 2, stat
   on 0. Live values: classic file 0 / −20 / −50, expansion file 0 / −40
   / −100 (`d2exp`, `patch_d2` `difficultylevels.txt`, measured). Cap,
   floor and colors: `panels.md` §8.9.
3. **Defense color** (stat 31, `panels.md` §8.8; `0x004A86BB`–
   `0x004A8713`): value = total defense `0x006223F0(P)`; color := 3 when
   `armblue` (r1); := 3 when P has an equipped shield
   (`0x0063C8F0(inventory, 0)`, item type 51) and state 101 `holyshield`
   (`0x00639DF0`); then := 1 when `armred`. Text `%ld`; the Font8 switch
   of `panels.md` §8.8 at ≥ 1,000.
4. **Language** of the language-6–9 rules (`panels.md` §8.1, §8.8,
   `panels-2.md` §17.5): the client's language id `0x00525150` (0 =
   English; the id `client/msg-ui.md` compares chat languages with).
   English never takes them.
5. **Popup width** of `panels.md` §8.8 = `0x00502520` = width A of
   `ui/text.md` §6 over the formatted value in the font then set
   (Font16); string lookup by id = `ui/text.md` §2 (`0x00524A30`).

### 25. Skill tree inputs (`panels.md` §10.3–§10.5; the `PENDING` icons and levels)

1. **Icon file and frame**: `panels.md` §10.3 (prefix `CC` read from
   `0x00724AC0`; frame = `skilldesc` `IconCel` + 1 while pressed).
2. **Learnability** (each draw with free points, `0x004AC200`, per class
   skill `i` with id `s`, record `R` = `skills.txt` row): learnable when
   all hold, else state `[0x007C0A38 + 4i]` := −1 and remap
   `[0x007C0838 + 4i]` := 5:
   - `0x006447D0(P, s)`: P's level (stat 12) ≥ the required level
     (`0x00644750`, `reqlevel`) and each set `reqskill1`–`reqskill3`
     (`R` +0x17E, +0x180, +0x182) names a skill P has natively with base
     ≥ 1 (`0x006446C0`, `0x006447B0`, `0x0045C4B0`);
   - base `b` = `skill_level(P, e, 0)` (`skills/levels.md`, `e` = P's
     native entry, `0x006439B0(P, s, −1)`; no entry → 0) < max level
     (`0x004AA8B0`: `maxlvl` +0x12C, 20 when < 1);
   - `0x00644920(P, s)`: `R`'s `InGame` flag (+5 & `[0x006CE270]` = 4,
     bit 10) set, P's level ≥ the required level and P's strength,
     dexterity, energy, vitality (stats 0, 2, 1, 3, full values) ≥
     `reqstr`, `reqdex`, `reqint`, `reqvit` (+0x176, +0x178, +0x17A,
     +0x17C);
   - free points (base stat 5) ≥ the cost: `skpoints` (+0x170) ≠ −1 →
     `skillcalc(P, skpoints, s, b)` (`0x00646CA0`), else 1.
   Learnable and state ≠ 1 → state := 0, remap := 0 (a pressed icon
   stays pressed). Without free points the icons only path
   (`0x004AC4D0`) is used (`panels-2.md` §19.6).
3. **Level number** (`0x004ABF60` third argument): `L` = `skill_level(P,
   e, 1)` (base + `bonus_level`, at least 0, capped by `0x00611830(0)`,
   `skills/levels.md`), 0 without an entry. Drawn when `b` ≠ 0 or `L` ≠
   0, color by `bonus_level(P, s)` (`0x00644300`: > 0 → 3, < 0 → 1, else
   0) (`panels.md` §10.4).
4. **Remap `k`** (exact order, `0x004ABF60`; corrects the last clause of
   `panels.md` §10.3): `k` := the remap table value (r2); `k` := 3 when
   the mouse is strictly inside the icon; `k` := 1 when `R`'s `InGame`
   flag is clear; `k` := 3 when `b` = 0, `bonus_level(P, s)` ≠ 0 and the
   state ≠ −1. The colored cel draw uses `k` (`ui/text.md` §4 r3–r4).

### 26. Waypoint rows (`panels.md` §13 r2–r7; the `PENDING` waypoint panel)

1. **Open** (`0x0049CF90(record)`, from S→C 0x63): `SetUIState(0x14, on,
   1)`; only if it returned 1: `0x0044DA40`; tab := the act of P's level
   (`0x006427F0`) through the tab setter (`ui/menus.md` §1 r4), 0 without
   a player, room or level; rows rebuilt (r2); waypoint GUID
   `[0x007BF07D]` := record +1; the 16-byte record +5 is copied to
   `[0x007BF081]` (`0x00661030`, `world/waypoints.md` §2 r5); latch
   `[0x007BF085]` := 0; rows rebuilt again.
2. **Row rebuild** `0x0049C7F0` (open, tab change): rows `n`
   `[0x007BF08A]` := 0, other-known count `[0x007BF08E]` := 0, all 9 rows
   cleared (level u32 `[0x007BF03C + 5r]`, used byte `[0x007BF040 +
   5r]`); current level `c` = P's room's level (`0x0061A1B0`). Tab `t` >
   5 → latch := 0, done. Tab table `0x007224AC` (stride 11: u8 cached, i32
   first level, i32 last level, u8 lowest index (initial 0xFF), u8
   highest index): level ranges 1–38, 40–74, 75–102, 103–107, 109–136
   for tabs 0–4. On the tab's first use (cached = 0, once per process):
   cached := 1 and, for each level of the range that has a waypoint
   (`0x00660E00`: its `levels.txt` `Waypoint` index), lowest := that
   index if lowest is still 0xFF, highest := max(highest, index). Then
   for each index `w` from lowest to highest: known (`0x00660E50`
   (record, w), `world/waypoints.md` §2 r2) → level := the level of `w`
   (`0x00660D90`, first match); a row (level, used := 0 if level = `c`,
   else 1 and other-known += 1); unknown → a row (level, used 0) when the
   index maps to a level. Each mapped index adds one row in index order.
3. **Draw and input**: `panels.md` §13.3–§13.6 (title uses the
   other-known count, row colors use the used byte and `c`), `ui/menus.md`
   §1 (hit tests skip used = 0 rows, so the current level's row cannot be
   chosen).
4. **Tab gates**: the tab draw tests client quest records 7, 15, 23, 26
   bit 0 for tabs 1–4 (`panels.md` §13.3), the setter records 7, 15, 23,
   28 (`ui/menus.md` §1 r4), on the client quest record `0x004B32D0()`
   (`0x0065C310` bit test, `world/quests.md` record layout). Which S→C
   message fills that record: `client/msg-ui.md` OQ 4 (not this spec).

### 27. Scroll and other panels (`panels.md` OQ 9)

1. **Scroll panel** (ui 0x10, left; `0x0049FF10`): drawn only while the
   scroll item `[0x007BF228]` is set; by its code (item unit +0x80):
   `bks` (Scroll of Inifuss) → the base (r2) only; `bkd` (deciphered) →
   the base and the symbols (r3) while `[0x007BF254]` = 0, else the base;
   `tr1` (Horadric Scroll) → the base; any other code → nothing.
2. **Base** `0x0049FA10`: a filled rectangle `0x004F6300(sx, H + sy −
   224, W / 2, H − 48, color 0, mode 5)` (opaque black, `render/blend-
   modes.md` §8 r2; the bottom edge has no `sy`), then `UI\MENU\scroin`
   (4 frames: 256 × 256 and 64 × 256 with y offset −115, 256 × 176 and
   64 × 176 with offset 0; d2data, measured) frames 0–3 at (`sx`, `H + sy
   − 109`), (`sx + 256`, `H + sy − 109`), (`sx`, `H + sy − 49`), (`sx +
   256`, `H + sy − 49`), then the close button frame 10 (`panels.md`
   §7.2; no pressed frame here) at (`sx + 277`, `H + sy − 61`); draw mode
   5, light 0xFF, no remap.
3. **Symbols** `0x0049FBA0(1)` (after the base; cels `UI\menu\scroin2`
   5 frames 40 × 25 / 20 × 25, `UI\menu\scroin3` 21 frames, loaded on
   first use): counter `n` `[0x007BF247]`: on the first call stamp
   `[0x007BF243]` := now, `n` := 0; later calls with now > stamp + 50
   ms: stamp := now, `n` += 1. For stone `i` = 0–4 with start `S[i]` =
   0, 12, 24, 36, 48 (`0x00722F08`) and `S[i]` < `n`: `d` = `n − S[i]`;
   draw mode 0 when `d` < 5 (and click sound 0 when `d` = 1), 1 when `d`
   < 10, 2 when `d` < 15, else 5; `u` = the stone's symbol slot
   `[0x007BF098 + 2i]` (u16; `u` ≥ 6 → skipped, `u` = 5 fatal); frame
   `i` of `scroin2` at (`sx + X1[u]`, `Y1[u] − sy`), then frame (`d` if
   `d` ≤ 20, else 0) of `scroin3` at (`sx + X2[u]`, `Y2[u] − sy`) in draw
   mode 3. `X1`, `Y1` (`0x00722EB8`): (242, 104), (255, 222), (148, 310),
   (47, 222), (75, 104); `X2`, `Y2` (`0x00722EE0`): (303, 161), (322,
   242), (254, 290), (190, 238), (211, 162). The writers of
   `[0x007BF098]` and `[0x007BF254]`: §Open questions 1 (answered: r7).
4. **Recipe scroll** (ui 0x25, left; `0x0048BC10`, expansion game and ui
   0x25 open): `0x0048BBE0(text, color)` (from the 0x26 type 7 consumer
   `0x0049F490`, `ui/messages.md` §4) copies the text to `[0x007BCEA8]`,
   keeps the color in `[0x007BCEA4]` and opens ui 0x25 (`SetUIState`).
   Draw: `menu\recipescroll` (`0x004520C0`, d2exp, 4 frames 256 / 64 ×
   256 / 176, offsets 0) frames 0–3 as left quads (`panels.md` §1.4),
   then font 4 and the text at pen (`sx + 80`, `130 − sy`), color
   `[0x007BCEA4]`, not centered; the font is restored.
5. **Guild states** 0x1B, 0x1C, 0x1D, 0x20 (full slot, `panels.md` §4.1)
   have no draw call of their own in the UI pass (`panels.md` §5); 0x1C
   and 0x1D only make the inventory family draw (§5 step 5). No opener
   of them was found: §Open questions 2 (answered: r8).
6. **Anvil** (ui 0x0E, `0x004C01E0`) is the item-socket dialog of
   `ui/messages.md` §11. **Player trade** (ui 0x17, inventory mode 0x0B,
   `%s\ui\panel\trade`, `0x004B8730`) is multiplayer only and not
   specified (Phases 7+): §Open questions 3.

7. **Scroll reading and the symbol slots** (answers §Open questions 1).
   The scroll item is set only by `0x0049FF90(u ECX, item EDX)` (callers: the
   inventory use `0x00487963` and the belt use `0x00498A77`):
   `[0x007BF228]` := item; ui 0x10 is toggled (`SetUIState(0x10, toggle, 0)`);
   `0x0044DA40`; then for code `bkd ` the flag `[0x007BF254]` := 1, C→S
   **0x3E** [u's GUID u32, −1 when u is none] (`0x00478680`,
   `ActivateInifussScroll`) and `[0x007BF1E0]` := 0; any other code:
   flag := 0, `[0x007BF1E0]` := 1. No other instruction writes the flag
   (its only reader is the r1 test), and no instruction writes the
   symbol slots `[0x007BF098]`–`[0x007BF0A1]` (scan of every absolute
   reference in the image; the one block clear of the area, `0x0049D440`,
   zeroes `0x007BF1B0`–`0x007BF24E`, item and counters included, not the
   slots or the flag). The slots are zero-initialized data, so every
   slot reads 0. Consequence in 1.14d: a `bkd` scroll is always drawn
   with flag 1, i.e. the base only; `bks` draws the base only (r1); the
   symbol pass of r3 is unreachable. d2rs draws the base for both and
   keeps r3 only for fidelity of the code path (slots 0).
8. **Guild states 0x1B, 0x1C, 0x1D, 0x20 are never opened** (answers
   §Open questions 2). All 136 `SetUIState` call sites resolve to a
   constant ui id (register tracking over each call's set-up) except the
   gate's own close (`0x00453A4F`, mode 1 off, `panels.md` §3). None
   passes 0x1B, 0x1C, 0x1D or 0x20; the only other writer of their flags
   (`0x007A282C`, `0x007A2830`, `0x007A2834`, `0x007A2840`) is the
   reset `0x00456970` (stores 0). They stay closed in 1.14d; d2rs need
   not draw or open them.

### 28. Gold dialog box and controls (`panels-2.md` §21 r9; answers `panels-2.md` OQ 6)

The controls are the UI classes `ButtonImplementation`,
`EditBoxImplementation`, `SpinnerImplementation` of `.\UI\`, each behind
a wrapper object whose method k calls the implementation's method k
(method tables `0x006DA6F4`, `0x006DA77C`, `0x006DA83C`). Coordinates
are the 640 × 480 values passed (`panels-2.md` §21 r6); "inside" is
inclusive unless noted.

1. **Box** `0x004B7CD0(x 215, y 140, p1 0x00453FC0, p2 0, art 0, p6
   1)`: a menu box object (`ui/menus.md` §2.1 layout; creation time
   +0 := `GetTickCount`, style 2, nothing selected) whose art is
   `data\global\ui\menu\` + table `0x007274AC`[art] (0 →
   `dialogbackground`, 210 × 158, 1 frame, offsets 0; d2data,
   measured); size := the cel size; it is drawn by `ui/menus.md`
   §2.5 (frame 0 at (215, 140 + 158), mode 5), then the controls,
   then the prompt items. More than one box at a time is fatal
   0x3C8; no cel is fatal 0x3B3. Controls are kept in a list that
   `0x004B7C00` **prepends** to, so with the add order spinner, edit
   box, OK, Cancel the list (draw and offer order) is Cancel, OK,
   edit box, spinner.
2. **Box input** (window handlers: 14 entries of `0x007273F0` on the
   game window, plus the wheel entry `0x007273E4` (0x20A →
   `0x004B7610`); kind 0 = window message, kind 3 = key down by VK).
   "Offer" = call that method of each control in list order and stop
   at the first that returns 1 (WM_CHAR: non-zero). Every event
   within 80 ms of the box's creation is only consumed.
   - mouse down (0x201 / 0x204): offer method 3; taken → consumed.
     Else the box's pressed flag (+0x10) := 1, the hovered item is
     re-picked (`0x004B7200`); outside the box grown by 4 pixels on
     each side: when ui 0x15 (mini panel) is open and the mouse is in
     its strip (W/2 − 74 < x < W/2 + 71, H − 69 < y < H − 50,
     `0x0047F190`) the event passes on untouched; else p1 runs
     (`0x00453FC0` = close, `panels-2.md` §21 r7: a click outside cancels). Consumed.
   - mouse up (0x202 / 0x205): passes on when ui 0x15 is open and the
     mouse is in that strip; else consumed; box not pressed → offer
     method 4; pressed → the item path (`ui/menus.md`; the gold box
     has no selectable item) and pressed := 0.
   - mouse move (0x200): offer method 5; taken → consumed; else the
     hovered item update.
   - wheel: |delta| ≥ 120 → offer method 2; none → the item step
     `0x004B7100` (nothing with < 2 selectable items). Consumed.
   - Esc key down (two entries; at each box creation `0x004B7C60`
     rewrites their VKs to the keys of command 38 `CfgClearScreen`,
     slot 1 then slot 0, when bound and < 0xE0, so by default Space
     and Esc): nothing while ui 5 (chat) is open; else consumed and,
     once the box was drawn more than 4 times (+0x54 > 4), p1 runs
     (close) and `0x00453AE0`. A third Esc entry (`0x004B76D0`) does
     the same without the chat test.
   - arrow keys Up 0x26 / Down 0x28 / Right 0x27 / Left 0x25:
     consumed; offer methods 7 / 8 / 9 / 10; none → the item step.
   - Enter key down 0x0D: consumed; acts only on a selected item (none
     here).
   - WM_CHAR (0x102): offer method 6 (non-zero stops). Else, unless
     the character is one of command 38's keys (`0x004B7970`): CR →
     the Enter key path; Esc → the third Esc path; any other →
     consumed (p6 = 1).
3. **Buttons** `0x004BB0F0(x, y, type, callback)`: art
   `data\global\ui\panel\buysellbtn` (`0x00454600`, cached in
   `[0x007A287C]`; d2exp copy, 23 frames of 32 × 32, offsets 0,
   measured); frame and caption from table `0x007275A8` (u16 frame,
   u16 string id per type): type 0 = frame 16, 3401 `ok`; type 1 =
   frame 10, 4142 `lowercasecancel`. OK = (250, 287, 0, `0x00454080`)
   with Enter enabled (method 12 := 1); Cancel = (355, 287, 1,
   `0x00454140`). Size (w, h) = the frame's size. Rectangle: x in
   [x, x + w], y in [y − h, y].
   - draw (method 1): frame + pressed (OK 16 / 17, Cancel 10 / 11) at
     (x, y), light 0xFF, mode 5 (bottom-left anchor); while the cursor
     (`0x00468730`, `0x00468740`) is in the rectangle: font 1, the
     caption at (x + (w − width A) / 2 (C division), y − h − 5),
     color 0, then the previous font.
   - mouse down (method 3): in the rectangle → pressed := 1, UI sound
     4 (`0x004B9A00(4, 0, 0, 0)`), taken.
   - mouse up (method 4): not pressed → 0; in the rectangle →
     pressed := 0 and the callback (its result is returned); outside
     → pressed := 0, 0.
   - WM_CHAR (method 6): Enter enabled, not a repeat (lParam bit 30
     clear) and character 0x0D → the callback. So Enter = OK.
   - methods 2, 5, 7–10: 0.
4. **Edit box** `0x004BBD80(258, 228, width 100, lines 1, max, cap
   10, callback 0x00453FD0, font 1)`: text buffer of cap + 1 u16
   units (+0x28), caret (+0x36), length (+0x3A), digits-only flag
   (+0x20, set by the constructor), key repeat allowed (+0x24, set
   to 1 after creation, method 17), caret blink counter (+0x46) and
   visible flag (+0x4A, starts 1). Lines ≤ 0 would draw a frame
   (`0x0046EFD0(x − 9, y − 16, width, 18, 0, 2)`; not here). The
   constructor also loads `ui\menu\textslid` (12 × 13, 17 frames,
   measured), unused by the gold dialog.
   - draw (method 1): counter += 1; visible: toggled to 0 when
     counter % 20 = 0; hidden: toggled to 1 when counter % 10 = 0
     (so the caret shows for the first 19 draws, then 10 off / 10
     on). Font 1. First visible character k: the smallest k with
     width A of text[k..] ≤ width (0 when the text fits). The text
     from k at (x + 1, y), color 0. Caret `_` when caret < cap,
     visible and caret ≤ length: at (x + 1 + width A of the first
     `caret` units of the **whole** text, y), color 0 (with k > 0 the
     caret is misplaced; the gold amounts never scroll).
   - WM_CHAR (method 6), c = the character:
     - a repeat while key repeat is off → 1 (nothing); c > 0x7F →
       consumed, 1.
     - ok := isdigit(c) (digits-only; else the font has the glyph,
       `0x00502550`, and CR / Esc call the callback).
     - Backspace (8): when length > 0: if caret ≠ length and caret ≠
       0 the units from caret − 1 shift left by one; then the last
       unit is cleared and length −= 1 (so Backspace with the caret
       at 0 deletes the **last** character); caret −= 1 when > 0.
       Consumed.
     - `.` (0x2E): when length > 0 and caret ≠ length: the unit at
       the caret is deleted (shift left), length −= 1. Consumed (so
       `.` acts as Delete and is never typed).
     - not ok: space, Esc, CR → 0 (left to the box); any other →
       consumed, 1.
     - ok and length < cap − 1 (at most 9 digits): caret = length →
       append, caret += 1, length += 1; else the units from the caret
       shift right, c is written at the caret, length += 1 (the caret
       does **not** advance). Then, when the text wraps to more lines
       than `lines` at `width` (`0x00502970`), the last unit is
       removed (length −= 1, caret −= 1). Then v := `atol(text)`; v >
       max (unsigned) → the text := `%d` of max, caret := length :=
       its length. Consumed, 1.
   - mouse down (method 3): pressed := 1, returns 0 (not taken);
     mouse up (method 4): when pressed: blink counter := 0, pressed
     := 0; returns 0.
   - Right / Left (methods 9 / 10): caret += 1 (≤ length) / −= 1 (≥
     0); taken. Up / Down, wheel, move: 0.
   - value (method 11, read by `panels-2.md` §21 r7): empty → 0; else `atol(text)`, 0
     when > max. Set value (method 15): `%d`, cut to cap units,
     caret := length := its length. Kind 3's pre-fill (`panels-2.md` §21 r6) uses it.
5. **Spinner** `0x004BC480(223, 219, 0, callback 0x00453FE0)`: art
   `ui\menu\spinner` (4 frames 15 × 12, offsets 0, measured); (w, h)
   = frame 0's size. Orientation 0 (vertical): up arrow at (x + 1,
   y) = (224, 219), down arrow at (x + 1, y + h + 1) = (224, 232)
   (orientation ≠ 0: down at (x + w, y)). Up / down pressed flags,
   held count n, press time t0, last step time t1, wheel pulse flag.
   - draw (method 1): when up or down is pressed: n += 1 and the
     callback runs (before drawing); then frame 2 × up at the up
     point and 2 × down + 1 at the down point, mode 5.
   - mouse down (method 3): mouse-held := 1; up rectangle x in (ux,
     ux + w), y in (uy − h, uy] → t0 := now, up := 1, the callback
     runs once with "first" set, taken; down rectangle x in (dx, dx +
     w), y in (dy − h, dy) (exclusive) → the same with down; else 0.
   - mouse up (method 4): when mouse-held: n := 0, mouse-held := 0;
     clears up (else down) and returns 1 when one was set.
   - wheel (method 2): delta > 0 → up := 1, down := 0; delta < 0 →
     down := 1, up := 0; either → pulse := 1; consumed, 1.
   - direction (method 11): 1 up, 2 down, 0 none. Step (method 12,
     `0x004BBFB0`): none pressed → 0; pulse → clears up, down and
     pulse, 1; "first" → t1 := now, 1; now − t1 < 70 ms → 0; else
     t1 := now and, with d = now − t0: d > 4096 → (d >> 11) × n; d >
     3000 → n >> 2; d > 2000 → n >> 4; d > 1000 → n >> 5; else 1
     (times: `GetTickCount`; n counts draws since the press).
   - callback `0x00453FE0`: dir, step as above; v := the edit box
     value (into `[0x007A2A68]`); m := P's full stat 15 (kind 4) or
     14; dir 1: v := min(v + step, m) (unsigned); dir 2: step > v →
     0, else v − step; `[0x007A2A68]` := v and the edit box's set
     value (v).
6. **Keys and close summary.** Enter (WM_CHAR CR) → OK (`panels-2.md` §21 r8); Esc →
   close only (as Cancel); the Clear Screen key (default Space) →
   close; a click outside the box → close; typing digits edits the
   amount, clamped to the max as typed.

## Constants & data dependencies

- Cursor type table `0x00712010` (7 × 0x1C), cels
  `DATA\GLOBAL\UI\CURSOR\{Gaunt, grasp, ohand, orotate, ppress, protate,
  buysell}`; idle 5,000 ms; step gate 16 ms; seed multiplier 0x6AC690C5.
- State mask table data +0xCC + 4·bit; `states.txt` columns `armblue`,
  `rfblue`, `rcblue`, `rlblue`, `rpblue`, `armred`, `rfred`, `rcred`,
  `rlred`, `rpred`; state 101; `difficultylevels.txt` `ResistPenalty`.
- `skills.txt` `maxlvl`, `skpoints`, `reqlevel`, `reqstr`, `reqdex`,
  `reqint`, `reqvit`, `reqskill1`–`3`, `InGame`; mask `[0x006CE270]` = 4.
- Waypoint tab table `0x007224AC`; `levels.txt` `Waypoint`.
- Scroll tables `0x00722EB8`, `0x00722EE0`, `0x00722F08`; item codes
  `bks`, `bkd`, `tr1`; files `menu\scroin`, `scroin2`, `scroin3`,
  `recipescroll`.

## Randomness

The idle cursor (§23 r8, state 1) steps the local player's **client**
seed (unit +0x20) once per cursor step; no server RNG is used. Everything
else here is wall-clock or input driven.

## Edge cases & original bugs

Reproduced by default.

- The idle cursor consumes the client player seed on wall-clock steps
  (§23 r8): any other client user of that seed sees a draw count that
  depends on the frame rate.
- With an item on the cursor the cursor animation freezes (§23 r9).
- The current level's waypoint row has used byte 0: drawn, never
  clickable, not counted as "other known" (§26 r2).
- The waypoint tab ranges are computed once per process (§26 r2).
- The scroll base fills down to `H − 48` without `sy` (§27 r2).
- A deciphered scroll's symbol slot 5 is a fatal error (§27 r3).

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| init, no input for 4,999 ms | type 5 `protate`, state 1 | §23 r3, r8 |
| s = 1, left down on open ground, then left up | t stays 5, s stays 1 (no press transition; the up acts only in s 5); idle restarts | §23 r14 |
| idle > 5,000 ms | state 2, type 2 `ohand`, frame 0; after 32 steps state 4, type 3 | §23 r8 |
| `ohand` frame 0 (32 × 26, offsets (−1, 24)) at mouse (100, 100), adj 0 | covers columns 99–130, rows 99–124 | §23 r10, `sprite-placement.md` §2 |
| mouse at (900, 50) at 800 × 600, no clip test | drawn at x 799 | §23 r10 |
| press during state 1 | state 5, type 4 `ppress`, frame 0; release → state 1, type 5 | §23 r5, r6 |
| move during state 4 | state 3, type 2, frame 0x700; 29 steps of −0x40 later state 1, type 5 | §23 r4, r8 |
| 2 × 3 armor on the cursor, graphic 56 × 84, mouse (300, 200) | graphic top-left (272, 158) | §23 r9 |
| fire resist 50, hell, expansion | `v` = −50 → shown −50, color 1 | §24 r2, `panels.md` §8.9 |
| fire resist 50, hell, classic | `v` = 0 → shown 0, color 0 | §24 r2 |
| defense 1,200, Holy Shield active with a shield | color 3, Font8 | §24 r3 |
| skill base 0, `bonus_level` +1, learnable | `L` = 1 drawn color 3; remap 3 | §25 r3, r4 |
| skill base 20, `maxlvl` 20 | state −1, remap 5 | §25 r2 |
| act 1 waypoint menu, record with indices 0, 1 known, P in Rogue Encampment | row 0 (level 1) used 0, row 1 used 1; other-known 1; title `waypointsheader` | §26 r2, `panels.md` §13.6 |
| deciphered scroll, `n` = 13 | stone 0 at `d` 13 (mode 2), stone 1 at `d` 1 (mode 0, sound 0) | §27 r3 |
| gold dialog, max 500, type `9`, `9`, `9` | text `99`, then `500` (clamped at the third digit) | §28 r4 |
| gold dialog text `123`, caret 0, Backspace | text `12` (the last digit goes), caret 0 | §28 r4 |
| gold dialog, Enter (WM_CHAR 0x0D) | OK callback `0x00454080` | §28 r3 |
| gold dialog, mouse down at (100, 100) | close (cancel), consumed | §28 r2 |
| spinner up held, draws n = 40, 2,500 ms after press, 70 ms since last step | step 40 >> 4 = 2 | §28 r5 |

## Provenance

1.14d `Game.exe` (exports `re/exports/funcs`, `tools/ghidra/disasm.py`):
cursor `0x004680B0`, `0x00468170`, `0x00468840`, `0x00467F20`,
`0x00467FA0`, `0x00468070`, `0x00468010`, `0x00468040`, `0x004684C0`,
`0x004683C0`, `0x00468460`, `0x00468310`, `0x004681C0`, `0x004682C0`,
`0x00467E40`, `0x00467E90`, `0x004685C0` (no caller), type table
`0x00712010` dumped from the image; state masks `0x0063A130`,
`0x0063A550`–`0x0063A670` (data offsets read in each wrapper),
`0x00625BB0`; character values `0x004A8520`–`0x004A8713`, `0x00611D30`,
`0x0044DCD0`, `0x0063C8F0`; skill tree `0x004AC200`, `0x004ABF60`,
`0x006447D0`, `0x00644920`, `0x004AA8B0`, `0x006442A0`, `0x00644300`,
`0x006439B0`, mask table `0x006CE268`; waypoint `0x0049CF90`,
`0x0049C7F0`, table `0x007224AC`; scrolls `0x0049FF10`, `0x0049FA10`,
`0x0049FBA0`, tables `0x00722EB8`, `0x00722EE0`, `0x00722F08`; recipe
`0x0048BC10`, `0x0048BBE0`; gold dialog box `0x004B7CD0`,
`0x004B7C00`, `0x004B7C60`, handlers `0x004B7270`–`0x004B7A90` (tables
`0x007273E4` / `0x007273F0` read from the image), buttons
`0x004BAF20`, `0x004BB000`, `0x004BAC70`, `0x004BACD0`, `0x004BAD40`
(table `0x007275A8`), edit box `0x004BBBB0`, `0x004BB210`, `0x004BB620`,
`0x004BB560`, `0x004BB4D0`, spinner `0x004BC340`, `0x004BBE70`,
`0x004BC050`–`0x004BC160`, `0x004BBF90`, `0x004BBFB0`, callbacks
`0x00453FC0`–`0x00454140`; `dialogbackground`, `buysellbtn`, `spinner`,
`textslid` DC6 headers (`mpq-tool extract`). DC6 headers of the cursor and scroll files
(d2data / d2exp) and `difficultylevels.txt` read with Python scripts
outside the repo. The state-mask offsets match the `states.txt` flag bit
numbers of `data/fields.tsv` for all ten wrappers. No D2MOO code used.

## Open questions

1. **Answered** (2026-10-07, §27 r7: no writer; the flag is set to 1
   for every `bkd` read, so the symbol pass is unreachable). Was: Who writes the deciphered-scroll symbol slots `[0x007BF098]` (5 × u16)
   and the flag `[0x007BF254]` (§27 r3): no direct store was found;
   likely a block copy in a quest message handler. Read the S→C quest /
   0x5D-family handlers for a copy into `0x007BF090`–`0x007BF260`.
2. **Answered** (2026-10-07, §27 r8: none does). Was: Whether any 1.14d path opens ui 0x1B, 0x1C, 0x1D or 0x20 (§27 r5):
   the 136 `SetUIState` call sites pass most ui ids in registers set
   earlier; a register-tracking scan settles it.
3. Player trade panel (ui 0x17, §27 r6): art, both players' names and
   gold, buttons 2, 4, 7, 8 (`0x004B8730`–`0x004B9110`). Multiplayer
   only; not needed for Phases 0–6.
4. **Answered** (2026-10-07, §23 r14). Was: The cursor handler table of `0x00451DB0(3)` (§23 r3): which window
   messages reach `0x00468840`, `0x00467F20`, `0x00467FA0` (move; left /
   right down; up), read from the table the call registers.
