# Spec: World — Object population (objgroup slots, PopulateFn 1–9, per-level shrine and well limits)

- **Status:** draft: every rule below was read from the 1.14d `Game.exe`
  disassembly (addresses inline) and the live 1.14d tables (`patch_d2`
  `levels.txt`, `objects.txt`, `d2exp` `objgroup.txt` 133 records); D2MOO
  1.10f gave names and structure only (differences in Provenance). No
  recording of a room's object population exists yet (open question 1).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::world::objects::populate`
- **Related specs:** `world/objects.md` (§2 object control and its
  per-level regions, §3 creation and init, `object-functions.tsv` kind
  `populate`), `sim/tick.md` §4 (when a room is populated, rules 2 and 5),
  `monsters/population.md` §1 (order: presets, restore, objects,
  monsters), `sim/rng.md` (§3 `roll`, §5.2 object-control seed, §5.4
  active room seed), `sim/path-placement.md` (§4 collision queries, rule
  2 point and rule 4 box), `drlg/rooms.md` (room flags +0x28, room type
  +0x48), `drlg/outdoor.md` (§1.2 grid-2 bits, §12.2 outdoor room data),
  `drlg/levels.md` (town levels), `data/fields.tsv` (`levels` `ObjGrp`,
  `ObjPrb`, `Themes`; `objgroup`; `objects` `SizeX`, `SizeY`, `Xspace`,
  `Yspace`, `Orientation`, `Selectable`, `Gore`, `PopulateFn`, `Parm1`),
  `data/loading.md` (objgroup from `d2exp`), `sim/units.md` §1
  (allocation `0x00555230`).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 47–61 |
| Inputs | 62–73 |
| Outputs / state changes | 74–81 |
| Rules | 82–83 |
|   1. Entry (`0x00552610`) | 84–94 |
|   2. Level regions | 95–126 |
|   3. Pre-check (`0x00552560(game, levels record)`, room in EBX) | 127–144 |
|   4. Theme gate (`0x00552400`, region in EAX) | 145–171 |
|   5. Group slots | 172–200 |
|   6. Placement helpers | 201–245 |
|   7. Populate functions | 246–383 |
|   8. Live data (1.14d) | 384–397 |
| Constants & data dependencies | 398–417 |
| Randomness | 418–445 |
| Edge cases & original bugs | 446–476 |
| Test vectors | 477–498 |
| Provenance | 499–523 |
| Open questions | 524–560 |
<!-- /index -->

## Summary

When a room is populated for the first time (`sim/tick.md` §4 rule 2),
after its preset units and the inactive-unit restore and before monster
population, `0x00552610(game, room)` places the random objects of the
room. A pre-check skips rooms without population (towns, waypoint rooms,
rooms next to a town, dirt-path outdoor rooms) and counts the room in its
level's object region; a themed-room gate draws but never runs a theme
with the live data. Then each of the level's eight `ObjGrp` slots rolls
its `ObjPrb` on the room seed, picks one member of the objgroup record by
`PROB`, and calls that object's `PopulateFn` (1–9) with the member's
`DENSITY`. The populate functions place clusters of caskets, urns,
barrels and crates, single shrines and wells (with per-level limits and
spacing), corpses with flies, and trapped souls.

## Inputs

| Name | Type | Source |
|---|---|---|
| game, active room | pointers | `sim/tick.md` §4 |
| room sub-tile rect {x0, y0, w, h} | room +0x4C, +0x50, +0x54, +0x58 (`0x00619730` copies 8 dwords from +0x4C) | `sim/path-placement.md` §4 |
| active room seed | room +0x6C | `sim/rng.md` §5.4 |
| object-control seed and level regions | game +0x10F0 | `world/objects.md` §2 |
| `levels.txt` `ObjGrp0`–`7` (+0xE5, u8), `ObjPrb0`–`7` (+0xED, u8), `Themes` (+0x210, u32) | record (0x220 bytes, `0x0061DB70`) | `data/fields.tsv` |
| `objgroup.txt` `ID0`–`7` (+0x00, u32), `DENSITY0`–`7` (+0x20, u8), `PROB0`–`7` (+0x28, u8) | record (0x34 bytes, `0x006412F0`; id ≥ count → none) | `data/fields.tsv` |
| `objects.txt` `SizeX` (+0xD0), `SizeY` (+0xD4), `Selectable0`–`7` (+0xC4), `Orientation` (+0x13C), `Xspace` (+0x162), `Yspace` (+0x163), `SubClass` (+0x167), `Gore` (+0x172), `Parm1` (+0x17C), `PopulateFn` (+0x1B2) | record (0x1C0 bytes, `0x00640E90`) | `data/fields.tsv` |

## Outputs / state changes

Object units allocated in the room (each through `0x00555230(type 2,
class, x, y, game, room, alloc flag 1, mode 0, GUID 0)`, which runs the
object's init, `world/objects.md` §3); region counters and coordinate
lists (§2); unit flag 0x2 set from `Selectable[mode]` by some populate
functions; draws on the room seed and the control seed (Randomness).

## Rules

### 1. Entry (`0x00552610`)

Callers: the tick room pass `0x0052D160` and the off-tick population
`0x0052D0F0` (`sim/tick.md` §4 rules 2 and 5), once per room.

1. L0 := level of the room's DRLG room (`0x0061A1B0`: DRLG room +0x58 →
   level +0x1D0). Its levels record (`0x0061DB70`: 0 < L0 < count) is
   missing → fatal.
2. Pre-check (§3) returns 0 → nothing more (no slot draws).
3. Slots i = 0 … 7 (§5).

### 2. Level regions

The 0x90-byte region of a level (`world/objects.md` §2 rule 4;
`0x00546F90(game, level)` returns it; D2MOO `D2ObjectRegionStrc`):

| Offset | Field | Written by |
|---|---|---|
| +0x00 | act (u8) | control build |
| +0x04 | rooms counted so far | §3 rule 4 |
| +0x08 | populated-room total, 0x7FFFFFFF until set | §3 rule 3 |
| +0x0C | theme count | no writer: always 0 (open question 4) |
| +0x10 | health shrines | §7.2 |
| +0x14 | shrines (≤ 10) | §7.2 (`0x00547490`) |
| +0x18 | wells (≤ 4) | §7.8 (`0x00547400`) |
| +0x1C | trap monster id (−1) | `world/objects.md` open question 3 |
| +0x20 | well points, 4 × {x, y} | §7.8 |
| +0x40 | shrine points, 10 × {x, y} | §7.2 |

Region tests (the region of the room's populated level L, §3 rule 1):

- **Want health** (`0x00547330`): total > 0 and (counted · 128) / total
  > 96 (signed) and health shrines = 0.
- **Shrine cap** (`0x00547360`): shrines = 10 or shrines > total / 8
  (signed, toward 0).
- **Well cap** (`0x00547300`): wells = 4 or wells > total / 8.
- **Shrine spacing** (`0x00547430(game, L, x, y)`): fails when any
  recorded shrine point has |x − sx| < 50 **or** |y − sy| < 50.
- **Well spacing** (`0x005473A0`): the same with 100 and the well points.
- Record (`0x00547490` shrines, `0x00547400` wells): when the count is
  below its cap (10, 4), store {x, y} at the count and increment it;
  otherwise nothing.

### 3. Pre-check (`0x00552560(game, levels record)`, room in EBX)

Returns 0 (no population) at the first rule that fails:

1. Waypoint room (`0x0061A210`: DRLG room flags & 0x30000) → 0.
2. L := populated level (`0x0061A1F0`: 0 when DRLG room flag 0x800000,
   the `Populate` = 0 / next-to-town flag of `drlg/preset.md`, else the
   level). L = 0 → 0. Outdoor dirt-path room (`0x0061ABB0`: room type 1
   and outdoor room data (+0x20) flags (+0x54) bit 0x80, the grid-2
   dirt-path bit of `drlg/outdoor.md` §1.2) → 0. L ≥ levels count → 0.
   Town level (`0x0061AB00` → `0x006426A0`: 1, 40, 75, 103, 109) → 0.
3. Region total = 0x7FFFFFFF → total := number of DRLG rooms of level L
   in the act of the region's `act` byte (game +0xBC + 4·act) without
   flag 0x800000 (`0x0061ABF0` → `0x00642BE0`).
4. Rooms counted += 1.
5. Levels `Themes` ≠ 0 and the theme gate (§4) returns 1 → 0.
6. Return 1.

### 4. Theme gate (`0x00552400`, region in EAX)

1. Total < theme count · 10 → return 0 (no draw).
2. r := C step, lo' mod 100 (C = control seed).
3. p := 5 if counted > total / 2, + 5 if counted > total − total / 4
   (signed divisions toward 0).
4. r ≥ p + 12 → return 0.
5. List the theme numbers k + 1 for each set bit k = 0 … 6 of `Themes`, in
   bit order; n := their count. j := `roll(n)` on C (n = 0: no draw, j =
   0). t := list[j].
6. Run theme t only when j < n, **t < n**, the table `0x00731E68`
   entry t {active, function} has active ≠ 0 and a function; its result
   is the gate's result. Otherwise return 0.

Theme table: 0 and 3 inactive; 1 `0x00551FF0` (returns 1, places
nothing), 2 `0x00552000`, 4 `0x00552140`, 5 `0x005523C0` and 6
`0x005523E0` (both `0x00552200`), all returning 1. Entry 7 overlaps the
next data (never read: t ≤ n − 1 ≤ 6).

The t < n test compares the theme number with the bit count, so a theme
runs only when its own number is smaller than the number of set bits.
Live `Themes`: 0 (98 levels), 8 (24), 60 (14), 48 (1); for 8 ({4}, n =
1), 60 ({3, 4, 5, 6}, n = 4) and 48 ({5, 6}, n = 2) no pick passes, so
**no theme runs in 1.14d**: themed levels only spend rules 2 and 5's
draws (one or two C steps per room) and populate normally. The theme
bodies are not specified (open question 5).

### 5. Group slots

For each slot i = 0 … 7 of the levels record (L0's record of §1):

1. g := `ObjGrp[i]`.
2. r := room-seed step, lo' mod 100. **Always drawn**, also for g = 0.
3. If want health (§2, region of the populated level L) and the
   **objects** row g (`0x00640E90(g)`: the group id used as an object
   class) has `SubClass` ≠ 0: r := 100.
4. g = 0 or r > `ObjPrb[i]` → next slot.
5. Group record of g; none → **return** (the remaining slots are
   skipped).
6. r2 := room-seed step, lo' mod 100; acc := 0. For k = 0 … 7 while
   `ID[k]` ≠ 0: acc += `PROB[k]`; if r2 < acc and objects row `ID[k]`
   has `Gore` ≤ 2 (byte `0x00731BBC`): `PopulateFn` ≥ 10 → fatal; table
   `0x00731D00`[fn] null (fn 0) → nothing; else `DENSITY[k]` ≥ 128 →
   fatal; call fn(game, room, `DENSITY[k]`, `ID[k]`, probability 100);
   stop the member walk (a member with r2 < acc but `Gore` > 2 lets the
   walk go on).
7. No member chosen (r2 ≥ sum of `PROB`) → nothing.

Populate table `0x00731D00`: 0 null, 1 `0x00550C20`, 2 `0x00552B50`, 3
`0x00551470`, 4 `0x00551850`, 5 `0x00551C00`, 6 `0x00551690`, 7
`0x00551200`, 8 `0x005516C0`, 9 `0x00551580`.

The function's probability argument is always 100, so each function's
own "roll mod 100 > probability → nothing" test (below) never refuses;
its draw still happens.

### 6. Placement helpers

Room rect {x0, y0, w, h} (sub-tiles). Positions are compared as their
low 16 bits (u16) against signed bounds. Box queries (room, x, y, sx, sy,
mask) are `0x0064D800`: both sizes ≤ 1 → the point query, else the
centred box query (`sim/path-placement.md` §4 rules 2 and 4); "free" =
result 0. Masks: 0x3F11 placement, 0xC01 wall | object | door
(`drlg/rooms.md` §10.6).

| Helper | Accepts (x, y) when |
|---|---|
| **Fit A** (`0x00550220`, sizes sx, sy) | w ≥ sx + 2 and h ≥ sy + 2; x > x0 + 1, y > y0 + 1, x < x0 − sx − 2 + w, y < y0 − sy − 2 + h; box (sx + 7, sy + 7, 0xC01) free and box (sx, sy, 0x3F11) free |
| **Fit B** (`0x005502F0`) | w ≥ 2 and h ≥ 2; x > x0 + 2, y > y0 + 2, x < x0 − sx + w, y < y0 − sy + h; box (sx + 2, sy + 2, 0x3F11) free |
| **Fit C** (inside `0x00550380`, `0x00550A30`, `0x00550540`) | x ≠ 0, y ≠ 0, x ≥ x0 + 1, y ≥ y0 + 1, x < x0 + w − 1, y < y0 + h − 1; box (sx + 6, sy + 6, 0x3F11) free; then the filter, if any |

**Random spot** `0x00550380(game, class, sx, sy, room)`: w < 2 or h < 2
→ none (no draw). Up to 5 tries on the **control seed**: x := x0 +
`roll(w − sx − 1)`, y := y0 + `roll(h − sy − 1)`; Fit C → allocate
(class, x, y) and return it. 5 failures → none.

**Oriented spot** `0x00550A30(game, class, sx, sy, room, orientation,
level, filter)`: w < sx + 2 or h < sy + 2 → none (no draw). Up to 5 tries
on the **room seed**:

- orientation 1: x := x0 + w/4 + `roll(w/2)`, then y := y0 +
  range(1, 2) (`0x004BC500`: one step, result 1);
- orientation 2: x := x0 + range(1, 2) (one step, 1), then y := y0 + h/4
  + `roll(h/2)`;
- other: x := x0 + `roll(w − sx − 1)`, y := y0 + `roll(h − sy − 1)`.

(w/2, w/4, h/2, h/4: signed, toward 0.) Fit C with filter(game, level,
x, y) → allocate and return.

**Spread spot** `0x00550540(game, class, sx, sy, room, level, filter)`:
w < 2, h < 2, w ≤ sx or h ≤ sy → none. Up to 5 tries on the room seed
with the "other" formula and Fit C plus filter.

**Walk** (fn 1, 4, 5): direction d := C step lo' & 7; sign tables
`0x00731B7C` X = {−1, 0, 1, −1, 1, −1, 0, 1}, `0x00731B9C` Y = {−1, −1,
−1, 0, 0, 1, 1, 1}. The position moves on every step, accepted or not
(a random walk from the last spot).

**Selectable flag** ("sel" below): unit flag 0x2 := `Selectable[mode]`
of the passed class's record ≠ 0, with the object's current mode.

### 7. Populate functions

Common: d := density, P := 100. "count" := ((w · h) >> 7) · d >> 8
(signed shifts). Every allocation steps the game seed once and runs the
object's init and PreOperate draws (`world/objects.md` §3, Randomness).

#### 7.1 Fn 1, caskets, urns, coffins, baskets (`0x00550C20`)

1. Record := objects row of the argument class A (sizes sx, sy for every
   test, whatever class is placed).
2. C step mod 100 > P → none (never).
3. Class list, tries T, step base b, step range s, fit for the walk:

| A | List (`0x00731D50`…) | T | b, s | Walk fit |
|---|---|---|---|---|
| 3 | {3, 28} | 18 | 5, 5 | A |
| 1, 79 | {79, 53, 1} | 12 | 5, 5 | A |
| 4 | {4, 9, 52, 94, 95} | 12 | 1, 0 | B |
| 89 | {89, 284} | 12 | 5, 5 | A |
| 208, 209 | {208, 209} | 12 | 1, 0 | B |
| other | — | — | — | none: return |

4. j := count; j < 1 → none. Loop while T ≥ 1 (and j ≥ 1):
   1. class := list[`roll(len)`] (C); x := x0 + `roll(w − sx − 1)`, y :=
      y0 + `roll(h − sy − 1)` (C).
   2. Fit A fails → T −= 1, next try.
   3. Allocate (class, x, y). n := 1, found := 1.
   4. Cluster: if n >> 1 ≥ 1: `roll(n >> 1)` (C) ≠ 0 → end. found = 0 →
      end. Walk up to 3 · max(j, 4) steps: d (C step); x += 2 · (b +
      `roll(s)`) · X[d] (C, no draw when s = 0); y += 2 · (b + `roll(s)`)
      · Y[d]; walk fit → found. Found: class := list[`roll(len)`],
      allocate, n += 1, repeat rule 4. Not found within the steps: found
      := 0, repeat rule 4 (the `roll(n >> 1)` draw happens once more).
   5. End of cluster: j −= 1, T −= 1; j < 1 → return.

No cap on the number of objects.

#### 7.2 Fn 2, shrines and waypoint-shrines (`0x00552B50`)

1. L := populated level. r := room-seed step mod 100 (always drawn).
2. Want health (§2) → forced := 1, tries := 30, no probability test.
   Else tries := 3 and r > P → none (never).
3. Shrine cap → none.
4. Up to tries times: oriented spot (`Orientation` of A, filter shrine
   spacing). None → next try. A unit U:
   1. sel.
   2. U's `InteractType` (object data +0x04, `world/objects.md` §1) = 2
      (health shrine from init) → health shrines += 1.
   3. Else if forced: U's class id (unit +0x04) := `Parm1` of A's row
      when 0 < `Parm1` < 573, else 84 (`0x00552AC0`); shrine pick
      (`world/objects.md` §5.1 rule 3) for class 2 at **level 1**, result
      discarded (one C draw with the live table: shrine 2 has `LevelMin`
      1); `InteractType` := 2; shrine record := shrines row 2; health
      shrines += 1.
   4. Record U's position (static path +0x0C/+0x10) as a shrine point.
      Return U.

#### 7.3 Fn 3, common objects (`0x00551470`)

d > 128 → fatal. C step mod 100 > P → none. Then count times: random spot
(A, sizes of A); a unit → sel. Returns the last unit placed (or none if
the last try failed).

#### 7.4 Fn 4, barrels (`0x00551850`)

Record := objects row **7** (barrel), whatever A is; d > 128 → fatal.

1. C step mod 100 > P → none.
2. j := count, k := 2j, c := 0. While j > 0 and k > 0:
   1. class := 11 (exploding barrel) if C step lo' mod 3 = 0, else 7.
      x := x0 + `roll(w)`, y := y0 + `roll(h)` (C).
   2. Fit B (row 7 sizes) fails → k −= 1, next.
   3. Allocate; c += 1; c ≥ 8 → return. n := 1, found := 1.
   4. Cluster: `roll(n >> 1)` (C, if ≥ 1) ≠ 0 → end; found = 0 → end.
      Walk up to 15 steps: d; x += `Xspace` · X[d]; y += `Yspace` · Y[d]
      (row 7 spacing); Fit B. Found: class := 11 if C step lo' & 3 = 0,
      else 7; allocate; n += 1; c += 1; c ≥ 8 → return.
   5. End: j −= 1, k −= 1.

At most 8 objects per call.

#### 7.5 Fn 5, crates (`0x00551C00`)

Record := row of A (sizes and spacing); d > 128 → fatal.

1. C step mod 100 > P → none.
2. j := count; j < 1 → none. k := 2j, l := 4j, c := 0. While k ≥ 1:
   1. class := A if A = 46 (crate), else {4, 9, 52, 94, 95}[`roll(5)`]
      (C). x := x0 + `roll(w)`, y := y0 + `roll(h)`.
   2. Fit B fails → k −= 1; j < 1 → return; next.
   3. Class ≠ 46 → re-pick class from the urn list (one more `roll(5)`).
      Allocate (not counted). n := 1, found := 1.
   4. Cluster: `roll(n >> 1)` ≠ 0 → end; found = 0 → end. Walk up to
      max(l, 4) steps with `Xspace`/`Yspace` and Fit B. Found: allocate
      the same class; a unit → c += 1, c ≥ 8 → return; n += 1.
   5. End: j −= 1, l −= 4, k −= 1; j < 1 → return.

At most 9 objects per call (the first of each call is not counted).

#### 7.6 Fn 6, corpses with flies (`0x00551690`)

Fn 3 with the same arguments; if it returned a unit U: C step mod 100 >
70 → allocate class 103 (flies) at U's position in U's room
(`0x00551150`). One C draw whenever fn 3 returned a unit (only the last
unit of fn 3 can get flies).

#### 7.7 Fn 7, rogues on sticks (`0x00551200`)

1. C step mod 100 > P → none.
2. Pattern index q := `roll(4)` (C; count `0x00731E64` = 4).
3. Record := row of A. Up to 8 tries: x := x0 + `roll(w − sx − 1)`, y :=
   y0 + `roll(h − sy − 1)` (C); Fit A with sizes (5, 5) → step 4.
4. Pattern q: pointer at `0x00731EB4` + 8q, point count at `0x00731EB8`
   + 8q. For each point: Fit A (sizes of A) at base + offset → class 57 +
   (C step lo' & 1), allocate, flies (§7.6 helper) on it.

In the 1.14d image all four counts at `0x00731EB8` + 8q are **0** (the
real counts 6, 5, 5, 7 sit after each offset array, at `0x00731DCC`,
`0x00731DF8`, `0x00731E24`, `0x00731E60`, and are not read), so rule 4
places nothing: fn 7 only spends its draws (prob, q, 1–8 × 2 position
draws) and returns none. Offsets for reference: {(−4, 0), (0, 0), (4,
0), (8, 0), (0, 4), (0, −4)}, {(−8, 0) … (8, 0) step 4}, {(0, −8) … (0, 8)
step 4}, {(−7, −5), (−5, −3), (−3, −1), (0, 0), (3, −1), (5, −3), (7,
−5)}.

#### 7.8 Fn 8, wells (`0x005516C0`)

1. L := populated level; well cap → none (no draw).
2. d > 128 → fatal. Room-seed step mod 100 > P → none.
3. Spread spot with sizes (2 · sx + 1, 2 · sy + 1), filter well spacing.
4. A unit: sel; record its position as a well point. Return it.

#### 7.9 Fn 9, trapped souls and burning bodies (`0x00551580`)

d > 128 → fatal. Room-seed step mod 100 > P → none. Then up to count
times: random spot (control seed, sizes of A); the first unit → sel,
return it. At most one object.

### 8. Live data (1.14d)

- 123 of 137 levels have at least one `ObjGrp`; 92 distinct groups;
  largest `ObjPrb` 90; largest `DENSITY` 125.
- Group members by `PopulateFn`: 3: 157, 2: 109, 1: 24, 0: 15 (never
  placed), 6: 10, 8: 10, 4: 3, 9: 3, 7: 1. All `Gore` values are 0 or 1,
  so the `Gore` ≤ 2 test never refuses.
- `PROB` sums per group: 100 for 74 records, 0 for 34 (empty), others
  between 20 and 102.
- `SHRINES` / `WELLS` columns (objgroup +0x30, +0x31) are loaded but not
  read by any rule here.
- §5 rule 3 hits 109 (level, slot) pairs: the group ids 1, 4, 6, … equal
  object rows (casket, urn, chest) with `SubClass` 8.

## Constants & data dependencies

| Constant | Value | Where |
|---|---|---|
| slots / members | 8 / 8 | `0x00552610` |
| Gore limit | byte 2 | `0x00731BBC` |
| populate table | 10 entries | `0x00731D00` |
| density limit | fatal at ≥ 128 (slot), > 128 (fn 3–9) | `0x005527CE`, functions |
| theme gate | p + 12 of 100, p ∈ {0, 5, 10} | `0x00552400` |
| theme table | {active, fn} × 8 | `0x00731E68` |
| want health | counted · 128 / total > 96 | `0x00547330` |
| caps | shrines 10 and total/8, wells 4 and total/8 | `0x00547360`, `0x00547300` |
| spacing | shrines 50, wells 100 (either axis) | `0x00547430`, `0x005473A0` |
| fn 1 lists, counts | `0x00731D50` … `0x00731D98` | §7.1 |
| sign tables | `0x00731B7C`, `0x00731B9C` | §6 |
| fallback health class | 84 | `0x00552AC0` |
| flies | class 103, roll > 70 | `0x00551150` |
| barrel classes | 7, 11 (mod 3 first, & 3 cluster) | `0x00551850` |
| rogue-on-stick patterns | `0x00731E64` (4), `0x00731EB4` | §7.7 |

## Randomness

Seeds: **R** = the room's active seed (room +0x6C), **C** = object-control
seed, **G** = game seed (one step per allocation, `sim/units.md` §1).
Per room, in order:

1. Pre-check: [theme gate: C step (mod 100); if r < p + 12: `roll(n)` C].
2. Per slot i = 0 … 7: R step (mod 100); [if passed: R step (mod 100)
   for the member; then the populate function].
3. Fn 1: C prob; per try: `roll(len)` C, x C, y C; per cluster round:
   [`roll(n >> 1)` C], per walk step: C (direction), [x C, y C when s >
   0]; per extra object: `roll(len)` C.
4. Fn 2: R prob; per try (≤ 5 per call of the spot helper): x, y on R
   (orientation 1/2: one of them is the range(1, 2) step); forced
   health: the shrine pick's C draw after allocation.
5. Fn 3/6: C prob; per object: ≤ 5 × (x C, y C); fn 6: one C step.
6. Fn 4: C prob; per try: class C, x C, y C; per cluster round: [`roll(n
   >> 1)` C], walk steps C; per extra barrel: class C.
7. Fn 5: C prob; per try: [`roll(5)` C], x C, y C; [second `roll(5)` C];
   cluster as fn 4 without a class draw.
8. Fn 7: C prob, `roll(4)` C, ≤ 8 × (x C, y C).
9. Fn 8: [cap: no draw] R prob; ≤ 5 × (x R, y R).
10. Fn 9: R prob; then up to count × ≤ 5 × (x C, y C).

Each allocation, at its point in the list: G step, init draws (mostly
C), PreOperate `roll(14)` C, mode-change draws on the object's seed
(`world/objects.md` Randomness 1).

## Edge cases & original bugs

Reproduced by default.

1. **Object row of the group id:** §5 rule 3 reads `SubClass` from the
   objects row whose index is the objgroup id, not from the group. When a
   level wants a health shrine, the slots whose id matches a casket, urn
   or chest row are suppressed (r = 100 exceeds every `ObjPrb`); the
   draw of rule 2 still happens.
2. **Theme gate never runs a theme** (t < n compares a theme number with
   a bit count), but still draws.
3. **Rogue-on-stick counts are zero**: fn 7 places nothing (§7.7).
4. **Spacing is a cross, not a box:** a candidate within 50 (shrines) or
   100 (wells) of a recorded point on **either** axis is refused, so a
   shrine directly east of another at distance 500 is refused.
5. **Forced health shrine changes the class id only:** the unit's class
   (+0x04) becomes `Parm1` (or 84) while its objects record (data
   +0x00), init result and footprint stay those of the original row;
   clients see the new class.
6. **Fn 1 and 5 use one record for all list members:** sizes (and fn 5
   spacing) come from the argument class even when another list class
   is placed; fn 4 always uses row 7.
7. **Missing objgroup record ends the room:** §5 rule 5 returns from the
   whole function, so later slots get no draws.
8. **Only the last fn-3 unit can get flies** (fn 6), and fn 3 returns
   none when its last try failed even if earlier tries placed objects.
9. **Waypoint-shrine rows** (3 waypoint classes with `PopulateFn` 2) go
   through fn 2 and count as shrines for the caps and spacing.
10. **Cluster continuation needs both** a zero `roll(n >> 1)` and a found
    spot (D2MOO fn 5 has `||`).

## Test vectors

Synthetic (CI), RNG per `sim/rng.md` §3, seeds written {lo, hi}:

| Input | Expected | Source |
|---|---|---|
| slot, R = {1, 666}, `ObjPrb` 60 | r = 51 ≤ 60: member r2 = 31; members PROB {30, 70} → member 1 | §5 |
| slot, R = {1, 666}, `ObjPrb` 50 | r = 51 > 50: no member draw; R = {1791398751, 0} after | §5 |
| want health: total 20, counted 16, health 0 | 16 · 128 / 20 = 102 > 96 → true | §2 |
| want health: total 20, counted 15 | 96, not > 96 → false | §2 |
| caps: total 30, shrines 3 / 4 | 3 > 3 false → allowed; 4 > 3 → capped | §2 |
| shrine spacing, point (100, 100): (140, 300) / (160, 160) | refused (dx 40) / accepted | §2 |
| theme gate, `Themes` 60, total 20, counted 15, C = {1, 666} | r = 51 ≥ 5 + 12 → 0, one C draw | §4 |
| theme gate, `Themes` 60, r < p + 12 | n = 4, any j: t ∈ {3, 4, 5, 6}; t = 3 inactive, others ≥ 4 → 0 | §4 |
| count, room 40 × 40, density 32 | (1600 >> 7 = 12) · 32 >> 8 = 1 | §7 |
| fn 4, C = {1, 666} | prob 51; class draw lo' 791599131 mod 3 = 0 → class 11 | §7.4 |
| fn 7, any room | no object allocated | §7.7 |

Game-file (`#[ignore]`, `D2_GAME_DIR`): the §8 counts from the live
`levels.txt`, `objgroup.txt`, `objects.txt`; the `Game.exe` tables
`0x00731D00`, `0x00731D50`–`0x00731D98`, `0x00731E68`, `0x00731EB4`.

## Provenance

- 1.14d `Game.exe` (`tools/ghidra/disasm.py`, Ghidra export): entry
  `0x00552610`, pre-check `0x00552560`, theme gate `0x00552400` and
  themes `0x00551FF0`, `0x00552000`, `0x00552140`, `0x005523C0`,
  `0x005523E0`; region helpers `0x00546F90`, `0x00547300`, `0x00547330`,
  `0x00547360`, `0x00547390`, `0x005473A0`, `0x00547400`, `0x00547430`,
  `0x00547490`; room helpers `0x0061A1B0`, `0x0061A1F0`, `0x0061A210`,
  `0x0061ABB0`, `0x0061AB00`, `0x0061ABF0`/`0x00642BE0`, `0x00619730`;
  placement `0x00550220`, `0x005502F0`, `0x00550380`, `0x00550A30`,
  `0x00550540`, `0x0064D800`, `0x004BC500`; populate functions of §5's
  table, `0x00551150`, `0x00552AC0`. Data words dumped from the image:
  `0x00731B7C`, `0x00731B9C`, `0x00731BBC`, `0x00731D00`–`0x00731E64`,
  `0x00731E68`–`0x00731EB8`.
- Live tables: `game/extracted/patch_d2/data/global/excel/levels.txt`,
  `objects.txt`; `d2exp/data/global/excel/objgroup.txt` (loaded copy per
  `data/loading.md`).
- D2MOO 1.10f `Objects.cpp` (`OBJECTS_PopulateRoom` area, populate
  functions), `ObjRgn.cpp` (region helpers) for names. Confirmed in
  1.14d by reading each function; differences: 1.14d checks the room's
  waypoint flag before the level lookups (same effect); fn 5 continues a
  cluster only with a zero roll **and** a found spot (D2MOO: or); fn 7
  reads zero point counts (D2MOO has a counted table); the fn 1 class is
  drawn at the top of every try (as D2MOO).

## Open questions

1. **Needs recording**: no trace of a room population: record RNG + packets while entering a
   fresh Act I level and compare object classes, positions and draw
   counts with §5–§7.
2. Confirm in a live 1.14d process that `0x00731EB8` + 8q still reads 0
   (a debugger read of four dwords settles whether rogues on sticks are
   never placed by fn 7). **Answered** from the binary: no instruction
   writes them. Every reference to `0x00731EB4`–`0x00731ED4` in
   `all.asm` is a read inside `0x00551200` (`0x00551364`, `0x0055136C`,
   `0x00551383`, `0x00551393`, `0x00551424`); the image has no ASLR
   (DllCharacteristics 0) and its base relocations there cover only the
   pointer slots (`0x00731EB4` + 8q), so the counts are 0 at run time
   (§7.7).
3. Maze rooms: whether `0x0061ABB0`'s room type 1 also covers maze
   rooms (`drlg/rooms.md` +0x48 note) and what +0x20/+0x54 holds there.
   **Answered**: it never covers them. `0x0061ABB0` → `0x0066BA90`
   returns 0 unless DRLG room +0x48 = 1 and only then reads the type
   data (`0x0067D7A0`). Every maze room is allocated with type 2
   (`0x0066B3E0` with EDX = 2 at all maze sites, `0x00670D7F` …
   `0x0067210F`, and the maze builder `0x00673B30`); the only type-1
   allocation is the outdoor room `0x0067D540` (`drlg/outdoor.md`
   §12.2). So maze rooms skip the dirt-path test without reading
   +0x20 / +0x54.
4. Region +0x0C (theme count): find a writer (all.asm search of region
   offsets) or confirm it stays 0. **Answered**: it stays 0. The region
   is zeroed at the control build (`0x00546C60`: memset 0x90, then
   +0x00, +0x08, +0x1C); regions are reached only through game +0x10F0
   +0x48 + 4·level, i.e. `0x00546F90` (callers `0x00552560`,
   `0x00552610`) and the helpers `0x00547300`–`0x005474C0`, none of
   which stores to +0x0C; the only access is the theme gate's read
   (`0x00552409`). Rule 1 of §4 (total < 0 · 10) is therefore never
   true.
5. Theme bodies 2, 4, 5/6 (`0x00552000`, `0x00552140`, `0x00552200`)
   are unreachable with live data; specify them only if a mod enables
   themes.
