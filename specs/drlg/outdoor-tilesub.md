# Spec: DRLG — lvlsub tile substitution (DrlgTileSub)

- **Status:** draft: rules read from the 1.14d disassembly; the border
  substitution draws of Blood Moor and Cold Plains and one Blood Moor
  room's sub-theme pick and substitutions (draw counts per pick) match
  `20261005-232125-rng.jsonl` exactly.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::drlg::outdoor::tilesub`
- **Related specs:** `drlg/outdoor.md` (owner of the callers: border
  substitution per act §7–§11, room creation §12); `drlg/rooms.md` (room
  seed, reset `0x0066EE40`, tile fill that consumes the grids);
  `drlg/preset.md` and `formats/ds1.md` (DS1 loading, substitution
  groups); `data/runtime-maps.md` §9 (lvlsub first-row index);
  `data/fields.tsv` (lvlsub layout); `sim/rng.md`.

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 35–46 |
| Inputs | 47–55 |
| Outputs / state changes | 56–61 |
| Rules | 62–63 |
|   1. Rows and files | 64–87 |
|   2. Border substitution (`0x00670750`, D2MOO `DRLGTILESUB_AddSecondaryBorder`) | 88–166 |
|   3. Sub-theme pick (`0x006706A0`, D2MOO `DRLGTILESUB_PickSubThemes`) | 167–174 |
|   4. Room substitution (`0x006707A0`, D2MOO `sub_6FD8AA80`) | 175–241 |
| Constants & data dependencies | 242–258 |
| Randomness | 259–266 |
| Edge cases & original bugs | 267–278 |
| Test vectors | 279–293 |
| Provenance | 294–311 |
| Open questions | 312–322 |
<!-- /index -->

## Summary

LvlSub.txt rows point at small DS1 "substitution" files. Each file holds
groups: a match pattern plus N variants laid out to its right. Two uses:
**border substitution** replaces coarse border cells of an outdoor level
grid with other border presets (Act 1, 2, 4 wilderness borders, Act 5
barricade walls), drawing from the **level seed**; **room substitution**
pastes tile patterns (stones, trees, puddles, waypoints, shrines) into an
outdoor room's tile grids, drawing from the **DRLG room seed**: a
sub-theme pick when the room is created, and the pastes when the room's
tiles are built.

## Inputs

| Name | Type | Source |
|---|---|---|
| lvlsub rows of a Type | 0x15C-byte records | `runtime-maps.md` §9 first-row index, `fields.tsv` |
| substitution DS1 | groups, layers | `ds1.md`, loaded once per row |
| level seed / room seed | seeds | `outdoor.md`, `rooms.md` §2 |
| level / room grids | `outdoor.md` §1, §12.2 | |

## Outputs / state changes

Border substitution: outdoor grids 0 and 2 (stamps, blanks). Room
substitution: the room's floor, wall and tile-type grids, roof/shadow
tiles, preset units; room DT1 mask (+0x50) and picked-theme mask.

## Rules

### 1. Rows and files

1. Rows of type t (`0x0061FA90`): row index := first-row index[t]
   (`runtime-maps.md` §9; t < 0 or a missing table is a fatal error);
   callers iterate rows while the row's `Type` equals t (record stride
   0x15C).
2. Record fields used (offsets): `Type` +0x00, `File` +0x04, `CheckAll`
   +0x40, `BordType` +0x44, `Dt1Mask` +0x48, `GridSize` +0x4C; runtime:
   DS1 file +0x50, tile-type grids [4] +0x54, wall grids [4] +0xA4, floor
   grid +0xF4, shadow grid +0x108; `Prob0..4` +0x11C, `Trials0..4` +0x130,
   `Max0..4` +0x144 (index = sub theme 0..4).
3. File load on first use (`0x006704E0`): load the DS1 named by `File`
   (`0x00665F40`, `preset.md`); a file without groups is a fatal error.
   Grids are views over the DS1 layers, (DS1 width + 1) × (height + 1):
   per wall layer k: tile-type grid k and wall grid k; wall grid k ≥ 1
   ORed with k << 18; floor grid from floor layer 0 if any; shadow grid
   if present. Files stay loaded for the session.
4. DS1 file fields used: group count +0x4C, groups +0x50 (24 bytes: x, y,
   w, h, —, N +0x14), wall-layer count +0x14, floor-layer count +0x18,
   method +0x00 (1 fixed, 2 random), preset-unit list +0x54. N is the
   group's last DS1 value (`ds1.md` group `unknown`, D2MOO: variant
   count). Variant v ∈ 1..N of a group starts at x offset v·(w + 1) from
   the group box; offset 0 is the match pattern.

### 2. Border substitution (`0x00670750`, D2MOO `DRLGTILESUB_AddSecondaryBorder`)

#### 2.1 Context and callers

Context (built by `0x006752A0` for Acts I, II, IV; `0x0067E0E0` for Act
V): level, grid size (gw, gh), grid 0, grid 2, base preset B, skip style
S := −1, lvlsub type T, callbacks:

| Callback | Acts I/II/IV | Act V (T = 12, B = 0) |
|---|---|---|
| link test | "not a link" (`0x006741E0`) | — |
| fit test | preset fits 1×1 (`0x00674230`) | — |
| custom test | — | `0x0067E080` |
| style map | — | `0x0067E000` |
| keep cell | grid 0 := 0, grid 2 := 0 (`0x00674160`) | same |
| blank cell | grid 0 := 0, grid 2 := 0x100 (`0x006741A0`) | same |
| stamp | `outdoor.md` §5.1 (`0x006743C0`) | same |

Calls (`outdoor.md`): Act I levels 2..7: T = 0, 1, 2, 3 at the points of
§7 step 3; level 39: 0..3; B = 4. Act II: T = 2, 1, 3, B = 364. Act IV
(104–106): T = 1, 2, 3, B = 799. Act V (not 110): T = 12.

#### 2.2 Algorithm

For each row of type T in order (`0x0066F990`), with its file loaded:

1. If S = −1: S := 62.
2. If `BordType` = 0: g0 := **roll(group count)** on the level seed
   (`0x0066F9C9`); else g0 := 0 (no draw).
3. For j in 0..count−1, group G := (g0 + j) mod count (`0x0066F690`):
   - Off := −1 if T = 1 and outdoor flags & 0xC, else 1.
     W := gw − GridSize·G.w + Off; H := gh − GridSize·G.h + 1;
     A := W·H. If A ≤ 0: next group (no draws).
   - Small := T = 1 and the level is 2..7 and W < 6 and H < 6.
   - Shuffle: entry k := (k mod W, k div W); for k in 0..A−1: a :=
     **roll(A)**, b := **roll(A)** (level seed, sites `0x0066F78D` /
     `0x0066F7B8` for a, `0x0066F7E9` for b), swap.
   - For entries (x, y) in order: skip (2, 2) when Small; if the test
     (§2.3) passes: v := **roll(N)** (`0x0066F8DB` / `0x0066F905`;
     N ≤ 0: v = 0, no draw); replace (§2.3) with x offset (v + 1)·(G.w +
     1); then `BordType` 0: stop this row (return to the next row) ;
     `BordType` 1: stop this group (next group); `BordType` 2: continue
     with the next entry.
4. Next row.

So `BordType` 0 = "one replacement per row" (and the only kind with a
start-group roll), 1 = "one per group", 2 = "everywhere it fits".

#### 2.3 Test and replace (`0x0066F3B0`, `0x0066F520`)

The candidate (x, y) is snapped down to a multiple of `GridSize` (signed
remainder). Group cell (i, j) maps to level cell (x + i·GridSize, y +
j·GridSize); pattern values: floor f (floor grid, 0 if the file has no
floor layers), wall w (wall grid 0, 0 if no wall layers), at (G.x + i +
xoff, G.y + j) with xoff = 0 for the test.

Test, per cell, rows j outer, columns i inner; value c := grid 0 at the
level cell:
- custom test present: it must return true (Act V: m := style map(w >>
  20 & 0x3F, w >> 8 & 0xFF); m = −5 → true; m = c → "not a link" at the
  cell; else false);
- else if w & 1: style s := (w >> 8 & 0xFF) − 1; unless s = S, c must
  equal B + s; and the cell must not be a link;
- else if f & 2: the cell must fit a 1×1 preset (in grid, spawn valid).

Replace, per cell (xoff = variant offset):
- w & 1: s := (w >> 8 & 0xFF) − 1; P := style map(…) if present, else
  B + s; if P ≠ −5 and s ≠ S: stamp P at the cell with file −1 and the
  border flag set (`outdoor.md` §5.1; build-list roll on first use of P);
- else f & 2: keep cell; else: blank cell.

Act V style map (`0x0067E000`; style must be 48 or 49, else fatal): the
first row (style, lo, hi, P, P snow) of `0x006F2038` with v in lo..hi
gives P + v − lo (P snow for level 117); rows: (49, 1, 16, 915, 987),
(49, 31, 46, 915, 987), (48, 1, 1, 883, 959), (48, 2, 3, 881, 957),
(48, 4, 4, 884, 960), (48, 5, 5, 895, 971), (48, 6, 7, 893, 969),
(48, 8, 8, 896, 972), (48, 30, 30, 0, 0), (48, 31, 31, −5, −5). No row:
fatal.

### 3. Sub-theme pick (`0x006706A0`, D2MOO `DRLGTILESUB_PickSubThemes`)

At outdoor room creation (`outdoor.md` §12.2), with sub type t :=
leveldefs `SubType`, theme h := `SubTheme`: if t = −1 or h = −1: mask 0,
no draws. Else for each row k (0, 1, …) of type t: one **room-seed step**
(site `0x006706D7`), r := lo' mod 100; if r < `Prob[h]`: mask |= 1 << k,
room DT1 mask |= row `Dt1Mask`. Every row draws, also with `Prob` 0.

### 4. Room substitution (`0x006707A0`, D2MOO `sub_6FD8AA80`)

Runs when the room's grids are built, right after the room seed reset
(`rooms.md` §9.2), three times (`outdoor.md` §12.2): waypoint rows
(theme 0), shrine rows (theme 0), then the room's sub type with its
theme. Arguments: type t (−1: nothing), theme h, mask. For each row k of
type t while mask bits remain (bit k = row k; mask shifts right per row):
if the bit is set: load the file; `CheckAll` ≠ 0 → §4.1; else §4.2.

Room context: room tile size (w, h) = (8, 8) for outdoor rooms; grids:
floor, wall layer 0 (only one wall grid is passed), tile-type grid of the
room's outdoor data; wall-layer count 1.

#### 4.1 CheckAll (`0x0066FF50`)

For each group G in file order: W := w − G.w + 1, H := h − G.h + 1; skip
if W ≤ 0 or H ≤ 0.
- Method 1 (fixed): for y in 1..H−1 (outer), x in 1..W−1: if the fixed
  test (§4.3) passes at (x, y): apply variant 0 (offset 0). No draws.
- Method 2 (random): for y in 0..H−1, x in 0..W−1: if the random test
  passes: one **room-seed step**, r := lo' mod 100; if `Prob[h]` < r: v :=
  **roll(N)**; apply with offset (v + 1)·(G.w + 1). (The comparison is
  the opposite of §3: a higher `Prob` means fewer pastes.)

#### 4.2 Scattered (`0x00670170`, D2MOO `DRLGTILESUB_DoSubstitutions`)

If the file has no groups: nothing. Repeat `Max[h]` times:
1. G := group[**roll(count)**] (room seed, site `0x006701DB`).
2. aw := w − G.w, ah := h − G.h; if aw ≤ 0 or ah ≤ 0: next repetition
   (the group roll was still drawn).
3. `Trials[h]` = −1: shuffle aw·ah entries ((k mod aw, k div aw); pairs
   **roll(A)**, **roll(A)** at sites `0x006702A4`/`0x006702C3`,
   `0x006702EB`); first entry with the fixed test passing at (x+1, y+1):
   apply (offset 0).
4. `Trials[h]` > 0: up to `Trials[h]` times: x := **roll(aw)** + 1 (site
   `0x00670408`), y := **roll(ah)** + 1 (`0x0067044D`); fixed test passes →
   apply (offset 0), stop. `Trials[h]` ≤ 0 other than −1: nothing.

#### 4.3 Tests

Fixed test (`0x0066FCF0`) at room cell (x, y): for each group cell where
the pattern floor has bit 2 or (the file has a wall grid 0 and) the
pattern wall has bit 1: the room floor cell must have bit 2 and no bit of
0x3F0FF00, and no room wall layer may have bit 1 there.
Random test (`0x0066FE00`): for each group cell: the pattern tile type
must equal the room's tile type; if the pattern floor (when the file has
floor layers) has bit 2: the room floor must have bit 2 and agree with
it on 0x3F0FF00; if the pattern wall (when the file has wall layers) has
bit 1: the room wall must have bit 1 and agree on 0x3F0FF00.

#### 4.4 Apply (`0x0066FAD0`)

At room cell (x, y) with x offset o into the pattern:
1. Count pattern shadow cells with bit 0x8000000 in the group box (at
   offset o) and grow the room's roof list by that many (`0x0066EFB0`,
   `rooms.md`).
2. Per group cell: floor (if floor layers) with bit 2 → room floor :=
   value | 0x80; per wall layer k < file count: wall value with bit 1 and
   a room wall grid k → overwrite; tile type ≠ 0 and a room tile-type grid
   k → overwrite; shadow with 0x8000000 → shadow tile at (room tile x + x
   + i, room tile y + y + j) (`0x0066E060`).
3. Preset units (`0x0066FA10`): in subtile coordinates (×5), every unit
   of the file with x strictly between G.x and G.x + G.w and y strictly
   between G.y and G.y + G.h — of the **match pattern** box, not the
   chosen variant — becomes a room preset unit at (x + ux − G.x, y + uy −
   G.y) (`0x0066BF30`, `preset.md` owns the unit records).

## Constants & data dependencies

| Item | Value |
|---|---|
| lvlsub record | 0x15C bytes; Types 0..12 in 1.14d (first rows 0, 1, 2, 3, 4, 6, 10, 16, 17, 21, 28, 31, 33) |
| default skip style S | 62 |
| style map table | `0x006F2038`, 10 × 5 ints |
| room substitution grids | floor, wall 0, tile type 0 |

Types used: 0 Border-Cliffs (BordType 1), 1 Border-Middle (0), 2
Border-Corner (1), 3 Border-Border (2), 4 Act 1 waypoints, 5 Act 1
shrines, 6 Act 1 scatter (Stone, Trees, Puddles, Swamp Big, Swamp Small,
Wild Objects), 7 Act 2 waypoint, 8 Act 2 shrines, 9 Act 2 scatter, 10/11
Act 5 scatter, 12 Barricade (BordType 2, GridSize 2). Leveldefs
`SubType`/`SubTheme`/`SubWaypoint`/`SubShrine` select them (Blood Moor 6
/ 0 / 4 / 5, Cold Plains 6 / 1 / 4 / 5).

## Randomness

| When | Seed | Draws in order |
|---|---|---|
| border substitution (level build) | level seed | per row: [BordType 0: roll(groups)]; per group: A pairs roll(A), roll(A); per replacement roll(N) |
| outdoor room creation | room seed (after its init step, `rooms.md` §2) | one `lo' mod 100` per row of the sub type |
| room tile build, after the reset | room seed | waypoint rows, shrine rows, own rows (§4): CheckAll random: per passing cell `lo' mod 100` [+ roll(N)]; scattered: per repetition roll(groups) [+ shuffle pairs, or ≤ Trials × (roll(aw), roll(ah))] |

## Edge cases & original bugs

1. Every sub-type row draws in §3, even rows with `Prob` 0.
2. CheckAll random pastes when `Prob` < r (inverse of §3).
3. Preset units come from the match pattern box, not the variant box.
4. The row loop of §1.1 reads the record after the last row of the
   highest type (Type 12 in 1.14d) to test its `Type`; the value read is
   whatever follows the table.
5. `roll(N)` with N ≤ 0 draws nothing (variant 0 offset = w + 1).
6. In the scattered pass the group roll is drawn even when the group does
   not fit the room.

## Test vectors

Recorded (`20261005-232125-rng.jsonl`):

| Case | Expected | Source |
|---|---|---|
| Blood Moor border substitution | type 0: 550 pairs (BordType 1, no group roll); type 1: roll(9) = 0 at seq 3672 then shuffles; types 1+2: 393 pairs, 3 variant rolls; type 3: 668 pairs, no variant roll | seq 2572–5800 |
| Cold Plains | type 0: 745 pairs; roll(9) = 0 (seq 8395); types 1+2: 477 pairs + 2 variant rolls; type 3: 924 pairs + 2 variant rolls | seq 6905–11330 |
| Blood Moor room, room seed {223305360, 666} (alloc seq 6644), init step 2795816810 | sub-theme draws lo' 310527139, 1584266672, 2804909076, 4172721383, 3426375614, 3941659325 → r 39, 72, 76, 83, 14, 25 vs Prob0 30, 50, 90, 0, 50, 20 → mask 0b010100 (Puddles, Swamp Small); DT1 mask 0x44103 \| 0x40001 \| 0x400001 | seq 6646–6651 |
| same room, client copy, reset seq 32973 {2795816810, 666} | scattered: Puddles `Max0` 8 then Swamp Small `Max0` 2 → 10 group rolls (`0x006701DB`); trials per pick 1, 14, 7, 20, 20, 20, 5, 20 (Trials0 20) and 3, 5 (Trials0 5); 115 (x, y) pairs | seq 32974–33381 |
| per level copy | 198 (Blood Moor) and 222 (Cold Plains) sub-theme draws = 6 rows × 33 / 37 outdoor rooms | `outdoor.md` §Test vectors |

Comparison (exact): the (site, seed after) sequence of every draw, and
the resulting tile grids (via the tile-fill check of `rooms.md`).

## Provenance

- **1.14d `Game.exe`**: `0x00670750`, `0x0066F990`, `0x0066F690`,
  `0x0066F3B0`, `0x0066F520`, `0x006752A0`, `0x0067E0E0`, `0x0067E000`,
  `0x0067E080`, `0x006706A0`, `0x006707A0`, `0x0066FF50`, `0x00670170`,
  `0x0066FCF0`, `0x0066FE00`, `0x0066FAD0`, `0x0066FA10`, `0x006704E0`,
  `0x00670630`, `0x0061FA90`, callers `0x0067D540`, `0x0067D2D0`;
  table `0x006F2038` read from the image. Offsets cross-checked with
  `fields.tsv` (Prob0 284 = 0x11C, Trials0 304, Max0 324, Dt1Mask 72,
  GridSize 76, BordType 68, CheckAll 64).
- **D2MOO** `DrlgTileSub.cpp`, `DrlgOutSiege.cpp` (style map) as a map;
  1.14d agrees. 1.14d detail: the per-group loop is a separate function
  (`0x0066F690`) and returns to the row loop on `BordType` 0 or 1 after a
  replacement; the row loop then stops for `BordType` 0.
- **Recorded**: as §Test vectors; the room vector (mask, Max/Trials
  structure) was predicted from the rules and patch_d2 `LvlSub.txt`
  before counting.

## Open questions

1. Confirm that the 1.14d DS1 loader (`0x00665F40`, `preset.md`) stores
   the DS1 group `unknown` value at group +0x14 (used here as the
   variant count) and what +0x10 holds.
2. Waypoint and shrine substitution (types 4/5, `Trials` −1 shuffles,
   theme 0) are not in the recording (no Act 1 waypoint/shrine room
   built): record entering Cold Plains far enough to build those rooms.
3. CheckAll rows (none in 1.14d LvlSub: all `CheckAll` = 0) — the §4.1
   path is untested by data; keep it for mods.
