# Spec: World — Object population (room objects from `levels` ObjGrp / `objgroup`)

- **Status:** draft: every rule is read from the 1.14d `Game.exe`
  disassembly (addresses inline) and the live 1.14d tables (`patch_d2`
  `levels.txt`, `objects.txt`, `objgroup.txt`); D2MOO 1.10f gave names
  only. No recording of a populated room exists yet (open question 1).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::objects` (population)
- **Related specs:** `world/objects.md` (§1 object data, §2 object
  control and the per-level regions, §3 allocation and init, §5.1 shrine
  pick, §8.3 traps), `world/object-functions.tsv` (populate table),
  `monsters/population.md` §1 (the room pass that calls this, its order
  against monster population) and §2.2 (monster region), `sim/tick.md`
  §4, `sim/units.md` §3.1 (allocation), `sim/rng.md` §3 (`roll`), §7
  (seeds), `sim/path-placement.md` §4 (box collision query
  `0x0064D800`), `drlg/rooms.md` §1 (DRLG room flags, active room
  fields), `drlg/levels.md` §11.5 (populated level, populated-room
  count), `drlg/outdoor.md` §1.2, §12 (outdoor room flags),
  `data/fields.tsv` (`levels`, `objgroup`, `objects` offsets).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 42–57 |
| Inputs | 58–68 |
| Outputs / state changes | 69–76 |
| Rules | 77–78 |
|   1. Object region (0x90 bytes, `objects.md` §2 rule 4) | 79–113 |
|   2. Entry and guard (`0x00552610` → `0x00552560`) | 114–139 |
|   3. Theme step (`0x00552400`) | 140–166 |
|   4. Group slots (`0x00552610` after the guard) | 167–188 |
|   5. Populate functions (`0x00731D00`) | 189–380 |
|   6. Trap monster (`0x005474C0`) | 381–401 |
| Constants & data dependencies | 402–418 |
| Randomness | 419–435 |
| Edge cases & original bugs | 436–462 |
| Test vectors | 463–483 |
| Provenance | 484–506 |
| Open questions | 507–516 |
<!-- /index -->

## Summary

The first time an active room is populated, the room pass runs object
population `0x00552610(game, room)` after the preset units and before
monster population (`monsters/population.md` §1). It updates the
level's object region (`objects.md` §2), may try a level theme (dead in
1.14d data), and then walks the eight `ObjGrp`/`ObjPrb` slots of the
room's `levels.txt` row: each slot that passes its percent roll picks
one `objgroup.txt` member and calls that object's populate function
(`objects.txt` `PopulateFn`, table `0x00731D00`). The nine populate
functions place clusters of caskets, urns and barrels, single shrines
and wells (with per-level limits and spacing), common objects and
corpse props. Placement uses the room's subtile rectangle and the box
collision query; objects are allocated in mode 0 and run their init
function (`objects.md` §3).

## Inputs

| Name | Type | Source |
|---|---|---|
| active room: subtile rect (x0, y0, w, h) at +0x4C, active-room seed +0x6C, DRLG room +0x10 | room | `drlg/rooms.md` §1 |
| `levels` `ObjGrp0`–`7` (+0xE5), `ObjPrb0`–`7` (+0xED), `Themes` (+0x210) | table | `data/fields.tsv` |
| `objgroup` `ID0`–`7` (+0x00), `DENSITY0`–`7` (+0x20), `PROB0`–`7` (+0x28) | table | `data/fields.tsv` |
| `objects` `SizeX` (+0xD0), `SizeY` (+0xD4), `Selectable0`–`7` (+0xC4), `Orientation` (+0x13C), `Parm1` (+0x17C), `XSpace` (+0x162), `YSpace` (+0x163), `SubClass` (+0x167), `Gore` (+0x172), `PopulateFn` (+0x1B2) | table | `data/fields.tsv` |
| object control seed and the level's object region | game state | `objects.md` §2 |
| monster region of the level (trap monster, §6) | game state | `monsters/population.md` §2.2 |

## Outputs / state changes

Object units (type 2) allocated with flags 1 (added to the room), mode 0,
no fixed GUID (`sim/units.md` §3.1), each running its init function and
PreOperate roll (`objects.md` §3); their selectable flag (unit +0xC4 bit
0x2) rewritten by the populate function; region counters and position
lists; draws on the control seed and on the active-room seed.

## Rules

### 1. Object region (0x90 bytes, `objects.md` §2 rule 4)

One per level id, zeroed at game creation except the three fields named
there. D2MOO `D2ObjectRegionStrc` names.

| Offset | Type | Field | Written by |
|---|---|---|---|
| +0x00 | u8 | act (levels `Act`) | creation |
| +0x04 | i32 | rooms visited | §2 rule 6 (+1 per populated room) |
| +0x08 | i32 | room count; 0x7FFFFFFF = not yet set | §2 rule 5 |
| +0x0C | i32 | theme gate count | never written: regions are reached only through `0x00546F90` (callers `0x00552560`, `0x00552610`) and the tests below, none of which writes it; stays 0 (§3) |
| +0x10 | i32 | health shrines | §5.2 |
| +0x14 | i32 | shrines placed (positions kept, at most 10) | §5.2 |
| +0x18 | i32 | wells placed (at most 4) | §5.8 |
| +0x1C | i32 | trap monster id; −1 = not chosen | §6 |
| +0x20 | 4 × (i32 x, i32 y) | well positions | §5.8 |
| +0x40 | 10 × (i32 x, i32 y) | shrine positions | §5.2 |

Region tests (each `fastcall(game, level id)`; region = control +0x48 +
4·level):

| Test | 1.14d | Result |
|---|---|---|
| **health needed** | `0x00547330` | room count > 0 **and** (visited · 128) / room count > 96 (signed division) **and** health shrines = 0 |
| **no more shrines** | `0x00547360` | shrines = 10, or shrines > room count / 8 (signed, toward 0) |
| **no more wells** | `0x00547300` | wells = 4, or wells > room count / 8 |
| **shrine spacing** (`x`, `y`) | `0x00547430` | false when some stored shrine has \|sx − x\| < 50 **or** \|sy − y\| < 50; else true |
| **well spacing** (`x`, `y`) | `0x005473A0` | the same over the wells with 100 |
| add shrine (`x`, `y`) | `0x00547490` | if shrines < 10: store at +0x40 + 8·shrines, shrines += 1 |
| add well (`x`, `y`) | `0x00547400` | if wells < 4: store at +0x20 + 8·wells, wells += 1 |
| add health shrine | `0x00547390` | health shrines += 1 |

The spacing tests reject a candidate that is close on **either** axis
(edge case 2).

### 2. Entry and guard (`0x00552610` → `0x00552560`)

`0x00552610(game ECX, room EDX)`:

1. L := the levels row of the room's level (`0x0061A1B0` → `0x0061DB70`);
   none → fatal 0xBEC.
2. Guard `0x00552560(game, L)` (room in EBX); 0 → no population.

Guard, in order:

3. The DRLG room has a waypoint flag (DRLG room flags +0x28 & 0x30000,
   `0x0061A210` → `0x0066BB60`) → 0.
4. lvl := populated level (`0x0061A1F0`, `drlg/levels.md` §11.5: 0 for a
   room with flag 0x800000); region := regions[lvl]. lvl = 0 → 0.
   Outdoor dirt path: the DRLG room has type 1 (+0x48) and its outdoor
   room data (+0x20) flags (+0x54) have bit 0x80 (the grid-2 "dirt path"
   bit, `drlg/outdoor.md` §1.2, §12.2; `0x0061ABB0` → `0x0066BA90` →
   `0x0067D7A0`) → 0. lvl ≥ levels row count (data tables +0xC5C) → 0.
   The room is in a town (`0x0061AB00`) → 0.
5. Room count (+0x08) = 0x7FFFFFFF → set it to the populated-room count
   of lvl (`0x0061ABF0(act record at game +0xBC + 4·region act, lvl)`).
6. Visited (+0x04) += 1.
7. `Themes` (L +0x210) ≠ 0 → the theme step (§3); it returns 1 only when
   a theme function ran and returned 1, which ends population (guard 0).
8. Guard 1.

### 3. Theme step (`0x00552400`)

Arguments: region (EAX), game, room, `Themes` mask.

1. Room count < 10 · theme gate count (+0x0C) → return 0 (no draw).
2. r := `lo' % 100` on the **control seed** (one step).
3. p := 5 if visited > room count / 2 (signed, toward 0), else 0; p +=
   5 if visited > room count − room count / 4 (signed, toward 0).
4. r ≥ p + 12 → return 0.
5. Room coordinates are read (`0x00619730`, no draw). List t := the bit
   indices i + 1 (i = 0..6, ascending) whose bit 1 << i is set in the
   mask; k := its length.
6. j := `roll(k)` on the control seed (`k < 1`: no draw, j = 0).
7. v := t[j]. A theme runs only if j < k **and v < k** and theme table
   `0x00731E68`[v] {active, function} has both non-zero; then its
   function's result is returned. Else return 0.

1.14d table `0x00731E68` (8 bytes per entry): 0 {0, 0}; 1 {1,
`0x00551FF0`}; 2 {1, `0x00552000`}; 3 {0, 0}; 4 {1, `0x00552140`}; 5 {1,
`0x005523C0`}; 6 {1, `0x005523E0`}. Live `Themes` values: 8 (bit 3:
t = {4}, k = 1), 48 (t = {5, 6}, k = 2), 60 (t = {3, 4, 5, 6}, k = 4).
The test v < k (edge case 1) then passes only for 60 with j = 0 (v = 3),
whose entry is inactive: **no theme function runs with the 1.14d
tables**, but rules 2 and 6 still draw for every themed room. The theme
functions are not specified (unreachable; a data patch that makes them
reachable needs them specified first).

### 4. Group slots (`0x00552610` after the guard)

seed := the active-room seed (room +0x6C). For i = 0 … 7:

1. g := `ObjGrp[i]` (u8). r := `lo' % 100` on the **room seed** (one
   step, for every slot, g = 0 included).
2. If **health needed** (§1) for the populated level **and** `objects`
   row g (the objects row whose index equals the group id, edge case 3)
   has `SubClass` ≠ 0: r := 100.
3. g = 0 or r > `ObjPrb[i]` → next slot.
4. G := `objgroup` row g (`0x006412F0`); none → **end population** (no
   further slots).
5. r2 := `lo' % 100` on the room seed. c := 0. For j = 0 … 7 while
   `ID[j]` ≠ 0: c += `PROB[j]`; if r2 < c **and** `objects` row `ID[j]`
   has `Gore` ≤ 2 (byte `0x00731BBC` = 2): take j and stop; else
   continue (a member whose `Gore` fails passes its share on).
6. Taken j: `PopulateFn` ≥ 10 → fatal 0xC43; populate table
   `0x00731D00`[`PopulateFn`] null (entry 0) → nothing; `DENSITY[j]` ≥
   128 → fatal 0xC47; else call it `(game ECX, room EDX, density
   DENSITY[j], class ID[j], chance 100)`. Its result is ignored.
7. Next slot.

### 5. Populate functions (`0x00731D00`)

Common pieces (the active room's subtile rect (x0, y0, w, h), `roll`
per `rng.md` §3, "x16" = the low 16 bits of x compared as an unsigned
value):

- **Count** n := ((w · h >> 7) · density) >> 8 (signed 32-bit, shifts
  arithmetic).
- **Gate:** every function draws r := `lo' % 100` on the seed named
  below (populate 6 through populate 3) and stops when r > chance; the chance is always 100,
  so the gate never stops anything, but the draw is made.
- **Selectable fix:** after a placement, unit flag 0x2 := `objects`
  `Selectable[unit mode]` of the **populate class's** row (the mode
  after init and PreOperate).
- **Spot P** (`0x00550380(game, class, sx, sy, room)`, control seed): w
  < 2 or h < 2 → none. Up to 5 tries: x := x0 + `roll(w − sx − 1)`, y :=
  y0 + `roll(h − sy − 1)`; accept when x ≠ 0, y ≠ 0, x0 + 1 ≤ x16 < x0 +
  w − 1, y0 + 1 ≤ y16 < y0 + h − 1, and the box query (`0x0064D800`:
  room, x, y, sx + 6, sy + 6, mask 0x3F11) is 0. Accepted → allocate
  (type 2, class, x, y, flags 1, mode 0); else none.
- **Test A** (`0x00550220`, position (x, y), size (sx, sy)): sx + 2 ≤ w,
  sy + 2 ≤ h, x0 + 1 < x16 < x0 + w − sx − 2, y0 + 1 < y16 < y0 + h −
  sy − 2, box (sx + 7, sy + 7, mask 0xC01) = 0 and box (sx, sy, mask
  0x3F11) = 0.
- **Test B** (`0x005502F0`): w > 1, h > 1, x0 + 2 < x16 < x0 + w − sx,
  y0 + 2 < y16 < y0 + h − sy, box (sx + 2, sy + 2, mask 0x3F11) = 0.
- **Directions:** d := `lo' & 7` of one step; DX = (−1, 0, 1, −1, 1,
  −1, 0, 1), DY = (−1, −1, −1, 0, 0, 1, 1, 1) (tables `0x00731B7C`,
  `0x00731B9C`).
- **Grow decision** (clusters): with k placed so far, if k >> 1 > 0 then
  `roll(k >> 1)` ≠ 0 ends the cluster; then the cluster also ends when
  the previous growth try failed.

#### 5.1 Clusters, populate 1 (`0x00550C20`, control seed)

1. Size (sx, sy) of the class argument's row. Gate.
2. Class list, base step B, spread S and tries T by the class argument:

| Argument | List (`0x00731D50`…, counts `0x00731D88`…) | B | S | Growth test | T |
|---|---|---|---|---|---|
| 1, 79 | 79, 53, 1 | 5 | 5 | A | 12 |
| 3 | 3, 28 | 5 | 5 | A | 18 |
| 4 | 4, 9, 52, 94, 95 | 1 | 0 | B | 12 |
| 89 | 89, 284 | 5 | 5 | A | 12 |
| 208, 209 | 208, 209 | 1 | 0 | B | 12 |
| other | — | | | | return none |

Live rows with populate 1 but no list (50, 51, 443–445, 467–471, 518,
535) place nothing after the gate draw.

3. n := Count; n < 1 → none.
4. While T > 0: class c := list[`roll(list count)`]; x := x0 + `roll(w
   − sx − 1)`, y := y0 + `roll(h − sy − 1)`; Test A with (sx, sy).
   Fail → T −= 1, repeat. Pass → allocate c at (x, y); k := 1; growth
   (rule 5); then n −= 1, T −= 1; n < 1 → stop.
5. Growth: grow decision; then up to 3 · max(n, 4) tries: d := direction
   step; x += (`roll(S)` + B) · DX[d] · 2; y += (`roll(S)` + B) · DY[d] ·
   2 (the two `roll(S)` draws in that order; S = 0 draws nothing); the
   growth test (A or B per the table) with (sx, sy) at the new (x, y).
   The position moves on every try, failed or not. A pass → class c' :=
   list[`roll(list count)`], allocate c' at (x, y), k += 1, back to the
   grow decision; all tries failed → back to the grow decision (which
   draws, then ends the cluster).
6. No selectable fix. Returns the last unit allocated by growth (0 if
   none).

#### 5.2 Shrine, populate 2 (`0x00552B50`)

1. Size of the row; lvl := populated level. One step on the **room
   seed** (r := `lo' % 100`).
2. **Health needed** (§1) → health := 1, tries := 30, no gate. Else
   health := 0, tries := 3, gate on r.
3. **No more shrines** → none.
4. Up to tries: spot Q (§5.9) with the shrine-spacing test. A spot →
   rule 5, return the unit; none after all tries → none.
5. Placed shrine U: selectable fix. If U's `InteractType` ≠ 2:
   - health = 1: U's class (unit +0x04) := `Parm1` of U's row if 1 …
     572, else 84 (`0x00552AC0`; the object data's record pointer is not
     changed); shrine pick (`objects.md` §5.1 rule 3) for class 2 with
     **level 1** and the room (`0x0054F770`, control seed; result
     discarded); `InteractType` := 2, shrine record := shrines row 2;
     add health shrine.
   - health = 0: nothing.
   If `InteractType` = 2 (rolled by init): add health shrine.
   Then add shrine (U's position, static path +0x0C, +0x10).

With live shrines, class 2 = {2, 4} and level 1 passes only id 2
(`LevelMin` 1; id 4 has 2): the discarded pick draws `roll(2)` until it
returns the index of 2, at most 8 times.

#### 5.3 Common objects, populate 3 (`0x00551470`, control seed)

Density ≥ 129 → fatal 0x864. Size of the row; gate; for n tries (n :=
Count): spot P; each placed unit gets the selectable fix. Returns the
last result of P (none when the last try failed).

#### 5.4 Barrels, populate 4 (`0x00551850`, control seed)

Size and spacing always from `objects` row 7 (barrel), whatever the
class argument (live: only row 7 uses populate 4). Density ≥ 129 →
fatal 0x955. Gate. n := Count, T := 2n, total := 0.

1. While n > 0 and T > 0: step, class c := 11 (exploding barrel) when
   `lo' % 3` = 0, else 7; x := x0 + `roll(w)`, y := y0 + `roll(h)`; Test
   B (row 7 sizes). Fail → T −= 1, repeat.
2. Pass → allocate c; total += 1; total ≥ 8 → return. k := 1.
3. Growth: grow decision; up to 15 tries: d := direction step; x +=
   `XSpace` · DX[d], y += `YSpace` · DY[d] (row 7: 2); Test B. Pass →
   step, class := 11 when `lo' & 3` = 0 else 7, allocate, k += 1, total
   += 1, total ≥ 8 → return; back to the grow decision. All 15 failed →
   back to the grow decision (ends).
4. Cluster end: n −= 1, T −= 1.

At most 8 barrels per call. No selectable fix.

#### 5.5 Crates, populate 5 (`0x00551C00`, control seed)

Size and spacing of the class argument's row. Density ≥ 129 → fatal
0x9D7. Gate. n := Count; n < 1 → none. T := 2n, G := 4n, added := 0.

1. While T > 0: class c := the argument; unless it is 46 (crate),
   c := list 4 of §5.1 (4, 9, 52, 94, 95)[`roll(5)`]. x := x0 +
   `roll(w)`, y := y0 + `roll(h)`; Test B. Fail → T −= 1; n < 1 →
   return; repeat.
2. Pass → unless c is 46, c := the same list[`roll(5)`] again (this
   pick is the one used). Allocate c. k := 1, last pass := 1.
3. Growth: grow decision; up to max(4, G) tries: d := direction step;
   x += `XSpace` · DX[d], y += `YSpace` · DY[d]; Test B. Pass →
   allocate c (same class) and, when a unit was made, added += 1, added
   > 7 → return; k += 1; back to the grow decision.
4. Cluster end: n −= 1, G −= 4, T −= 1; n < 1 → return.

The first crate of each cluster is not counted (at most 8 added ones
per call). No selectable fix. Live: only row 46 uses populate 5.

#### 5.6 Corpses, populate 6 (`0x00551690`)

Runs populate 3 (§5.3) with the same arguments; if it returns a unit,
the **fly spawn** `0x00551150(game, unit)`: r := `lo' % 100` on the
control seed; r > 70 → allocate class 103 (`Dummy`) at the unit's
position in the unit's room (flags 1, mode 0). Live rows: 54, 55, 56,
154, 178.

#### 5.7 Corpses on sticks, populate 7 (`0x00551200`, control seed)

1. Gate. p := `roll(4)` (count `0x00731E64`).
2. Size of the row. Up to 8 tries: x := x0 + `roll(w − sx − 1)`, y := y0
   + `roll(h − sy − 1)`; Test A with size (5, 5). All fail → none.
3. Pass → pattern p of table `0x00731EB4` {points pointer, count}. In
   the 1.14d image the four count words (`0x00731EB8` + 8p) are 0 and no
   instruction writes them (their only references are the reads in this
   function), so the function returns none here (open question 2 asks
   for a live memory read). The pattern points (`0x00731D9C`…, counts
   6, 5, 5, 7 stored after each list) are therefore unused.

For a non-zero count (not 1.14d): for each point (dx, dy): Test A at
(x + dx, y + dy) with the row's size; pass → step, class 57 + (`lo'` &
1), allocate, and the fly spawn of §5.6 on it.

#### 5.8 Wells, populate 8 (`0x005516C0`)

1. lvl := populated level. **No more wells** → none. Density ≥ 129 →
   fatal 0x8D0.
2. Size (sx, sy) of the row. Step the **room seed**, gate.
3. Spot W (`0x00550540`, room seed) with size (2·sx + 1, 2·sy + 1) =:
   (sx', sy'): w < 2, h < 2, w ≤ sx' or h ≤ sy' → none. Up to 5 tries:
   x := x0 + `roll(w − sx' − 1)`, y := y0 + `roll(h − sy' − 1)`; the
   bounds of spot P; box (sx' + 6, sy' + 6, mask 0x3F11) = 0; well
   spacing (§1) true. Accept → allocate.
4. Placed: selectable fix; add well (its position).

#### 5.9 Spot Q (`0x00550A30`, room seed; used by §5.2)

Arguments: game, class, sx, sy, room, `Orientation` o, lvl, test.
sx + 2 > w or sy + 2 > h → none. Up to 5 tries:

| o | x | y (draws in this order) |
|---|---|---|
| 1 | x0 + w/4 + `roll(w/2)` | y0 + 1 (`0x004BC500(1, 2)`: one step) |
| 2 | x0 + 1 (one step) | y0 + h/4 + `roll(h/2)` |
| other | x0 + `roll(w − sx − 1)` | y0 + `roll(h − sy − 1)` |

(w/2, w/4: signed, toward 0.) Accept with the bounds of spot P, the box
(sx + 6, sy + 6, mask 0x3F11) = 0 and the test (game, lvl, x, y) true →
allocate.

#### 5.10 Trapped souls, populate 9 (`0x00551580`)

Size of the row. Step the room seed, gate. For n tries (n := Count):
spot P (control seed); the first placed unit gets the selectable fix and
is returned (one at most).

### 6. Trap monster (`0x005474C0`)

Used by casket, barrel and trap handlers 8–9 (`objects.md` §8.2, §8.3).
Argument: the operate record (game, object). lvl := level of the
object's room (`0x0061A1B0`), region := regions[lvl].

1. Cached id (+0x1C) in 0 … monstats count − 1 → return it.
2. A := 96 (`mummy1`) when the level's act (`0x006427F0`) is Act II
   (index 1), else 5 (`zombie1`). B := (0 `skeleton1`, 170
   `sk_archer1`, 274 `skmage_pois1`, 379 `skmage_cold1`, 383
   `skmage_fire1`, 387 `skmage_ltng1`).
3. Cache := 234 (`flyingscimitar`).
4. For each entry j of the level's monster region (`0x00547BB0(game +
   0xF0, lvl)`; `monsters/population.md` §2.2) in order, j < monster
   count (+0x10): c := entry class (i16). A ≤ c < A + 5 → cache := A,
   return. Else the first b of B with b ≤ c < b + 4 → cache := b,
   return.
5. Return the cache (234).

No draw. The answer is fixed per level for the game once chosen.

## Constants & data dependencies

| Constant | Value | Where |
|---|---|---|
| populate table | 10 entries, 0 null | `0x00731D00` |
| theme table / pattern table | 8 × {active, fn}; 4 × {points, count} | `0x00731E68`; `0x00731EB4` |
| gore limit | 2 (byte) | `0x00731BBC` |
| health gate | visited · 128 / rooms > 96 | `0x00547330` |
| shrine / well caps | 10 / 4, and ≤ rooms / 8 | `0x00547360`, `0x00547300` |
| spacing | 50 (shrines), 100 (wells), either axis | `0x00547430`, `0x005473A0` |
| cluster lists | §5.1 table | `0x00731D50`–`0x00731D98` |
| directions | DX, DY | `0x00731B7C`, `0x00731B9C` |
| fly spawn | class 103 when `lo' % 100` > 70 | `0x00551150` |
| collision masks | 0x3F11, 0xC01 | §5 |
| barrel classes | 7, 11 (1 in 3 first, 1 in 4 after) | `0x00551850` |
| trap monsters | 5 / 96; 0, 170, 274, 379, 383, 387; default 234 | `0x005474C0` |

## Randomness

Seeds: **R** = active-room seed (room +0x6C), **C** = object-control
seed. Per populated room, in order:

1. Guard: themed rooms only: `lo' % 100` C, then (if r < p + 12)
   `roll(k)` C.
2. Each slot i: `lo' % 100` R; if the slot passes: `lo' % 100` R for
   the member, then the populate function's draws.
3. Populate 1, 3, 4, 5, 6, 7: C (gate first). Populate 2: one R step,
   spot Q on R, the discarded shrine pick on C; populate 8: one R step,
   spot W on R; populate 9: one R step, spot P on C.
4. Every allocation runs the object's init draws and the PreOperate roll
   on C and the animation draw on the unit seed (`objects.md` §3, §4)
   at the point of the allocation, between the population draws.
5. Trap monster: none.

## Edge cases & original bugs

Reproduced by default.

1. **Dead themes:** the theme pick compares the bit index + 1 with the
   number of set bits; with the live `Themes` values no theme runs, but
   each themed room still steps the control seed once or twice.
2. **Spacing on either axis:** a shrine (well) is refused when it lines
   up within 50 (100) subtiles of a stored one on x **or** y, not only
   when it is near.
3. **Group id used as an objects row:** the health override reads
   `SubClass` of the `objects` row whose index equals the `objgroup`
   id, not of any object of the group.
4. **Health override needs `ObjPrb` 100:** r := 100 only passes a slot
   whose `ObjPrb` is 100; for other slots it blocks the group.
5. **Shrine conversion keeps the record:** a shrine turned into a health
   shrine changes its unit class and shrine id but keeps the object
   data's original `objects` record and the init draws already made.
6. **Corpses on sticks never placed** by population in 1.14d (§5.7),
   after drawing.
7. **Populate 5 picks twice** (the first pick is overwritten) and does
   not count the first crate of a cluster toward its cap of 8.
8. **Selectable fix uses the current mode** (after PreOperate), unlike
   `objects.md` §3 rule 7.
9. A missing `objgroup` row ends the whole room's population, skipping
   the later slots.

## Test vectors

Synthetic (CI), RNG per `rng.md` §3:

| Input | Expected | Source |
|---|---|---|
| Count, w = h = 40, density 128 / 255 / 0 | 6 / 11 / 0 | §5 |
| Barrels: C = {1, 666}, room (x0, y0, 40, 40) | gate 51; class step `lo'` 791599131, % 3 = 0 → class 11; x = x0 + 12, y = y0 + 33 | §5.4 |
| health needed: rooms 8, visited 7 / 6, health 0 | true (112) / false (96) | §1 |
| shrine spacing: stored (100, 100); candidate (149, 300) / (150, 150) | false (\|dx\| 49) / true | §1 |
| theme step: rooms 20, visited 11 / 16 / 5 | p = 5 / 10 / 0 | §3 |
| theme mask 60, j = 0 / 1 | v = 3 < 4 but entry 3 inactive / v = 4 ≥ 4: none | §3 |
| trap monster, Act I region classes {0x2C, 7} | 5 (7 in 5…9) | §6 |
| trap monster, Act II region {98} / {171} / {12} | 96 / 170 / 234 | §6 |
| no more shrines: rooms 40, shrines 5 / 6 / 10 | false / true / true | §1 |

Game-file (`#[ignore]`, `D2_GAME_DIR`): the tables `0x00731D00`,
`0x00731D50`–`0x00731D98`, `0x00731E68`, `0x00731EB4`,
`0x00731B7C`/`0x00731B9C` and the byte `0x00731BBC` read from the 1.14d
`Game.exe` equal the values above.

## Provenance

- 1.14d `Game.exe` (Ghidra export and `all.asm`): `0x00552610`,
  `0x00552560`, `0x00552400`; region tests `0x00547300`, `0x00547330`,
  `0x00547360`, `0x00547390`, `0x005473A0`, `0x00547400`, `0x00547430`,
  `0x00547490`; trap monster `0x005474C0`; populate functions
  `0x00550C20`, `0x00552B50`, `0x00551470`, `0x00551850`, `0x00551C00`,
  `0x00551690`/`0x00551150`, `0x00551200`, `0x005516C0`, `0x00551580`;
  placement `0x00550380`, `0x00550220`, `0x005502F0`, `0x00550540`,
  `0x00550A30`, `0x00552AC0`; room helpers `0x0061A210`/`0x0066BB60`,
  `0x0061ABB0`/`0x0066BA90`/`0x0067D7A0`, `0x00619730`. Tables dumped
  from the image file (`.data` raw bytes): `0x00731B7C`, `0x00731B9C`,
  `0x00731BBC`, `0x00731D00`, `0x00731D50`–`0x00731D98`, `0x00731D9C`–
  `0x00731E64`, `0x00731E68`, `0x00731EB4`.
- Live tables: `patch_d2` `levels.txt` (`Themes` non-zero on 39 rows:
  values 8, 48, 60), `objects.txt` (`PopulateFn` users, sizes, spacing,
  `Gore`), `shrines.txt`, `monstats.txt` (names of the trap ids).
- D2MOO 1.10f `Objects.cpp` (`OBJECTS_PopulateRoom`, the populate
  functions), `ObjRgn.cpp`: names and structure. 1.14d matches it in
  the parts compared (theme pick, health override, region tests, trap
  monster lists); D2MOO keeps the corpse-on-stick patterns in a local
  table with real counts, 1.14d reads a global whose counts are 0.

## Open questions

1. No recording of a populated room: record a new game's first rooms of
   the Den of Evil and the Cold Plains with RNG traces (control seed and
   active-room seeds) to confirm the draw order of §4–§5 and the object
   positions.
2. Whether the corpse-on-stick pattern counts (`0x00731EB8` + 8p) stay 0
   at run time (read the four dwords in a running 1.14d process); if any
   is non-zero §5.7 places corpses.
