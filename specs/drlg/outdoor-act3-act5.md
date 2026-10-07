# Spec: DRLG — Act III jungles and Act V outdoor levels

- **Status:** draft: every rule below was read from the 1.14d disassembly
  (addresses given); D2MOO 1.10f `DrlgOutPlace.cpp` / `DrlgOutJung.cpp`
  were a map only, and §2.7 differs from it. No recording covers Act III
  or Act V yet (`traces/raw/` holds only Act I DRLG draws); the vectors
  are derived from these rules.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::drlg::outdoor` (`place.rs` jungle placer,
  `acts.rs` Act III / Act V builds)
- **Related specs:** `drlg/outdoor.md` (owner of the outdoor grids, stamp
  and placer primitives §5, link flags §5.5, act-wide placement §2,
  Kurast and Travincal stamps §9.4, the Act V build steps §11, rooms
  §12); `drlg/outdoor-tilesub.md` (Act V border substitution);
  `drlg/levels.md` (DRLG seed, jungle-link bit, level allocation and
  seeds); `drlg/preset.md` (preset rooms); `sim/rng.md` (`roll`);
  `data/fields.tsv` (leveldefs, lvlprest layouts).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 39–55 |
| Inputs | 56–66 |
| Outputs / state changes | 67–75 |
| Rules | 76–81 |
|   1. Level map | 82–102 |
|   2. Jungle placer (`0x00677880`, D2MOO `DRLG_GenerateJungles`) | 103–295 |
|   3. Jungle stamping (`0x0067E910`, levels 76..78) | 296–325 |
|   4. Act III rooms and links | 326–347 |
|   5. Act V outdoor levels | 348–369 |
| Constants & data dependencies | 370–391 |
| Randomness | 392–426 |
| Edge cases & original bugs | 427–450 |
| Test vectors | 451–524 |
| Provenance | 525–550 |
| Open questions | 551–586 |
<!-- /index -->

## Summary

Act III's three jungle levels (Spider Forest 76, Great Marsh 77, Flayer
Jungle 78) are placed during act creation by a jungle placer that draws
from the **DRLG seed itself**: it lays three 64×192-tile jungles next to
each other above the Kurast Docks, traces a river through each one on a
grid of 32-tile blocks, picks clearing ("attach") blocks beside the
river, and turns every block into a lvlprest id (river piece, river piece
with a side exit, clearing, or nothing). Each jungle level stores its
list of block ids; when the level is generated the ids are stamped as
32×32 presets from the **level seed**, with the Spider Forest entry and
the Flayer Jungle exit rows replaced by fixed transition presets. The
Kurast chain (79..83) hangs above Flayer Jungle. Act V's outdoor levels
(Bloody Foothills 110, Frigid Highlands 111, Arreat Plateau 112, Frozen
Tundra 117) are built by `outdoor.md` §11; this spec adds their geometry,
draw order, rooms and vectors.

## Inputs

| Name | Type | Source |
|---|---|---|
| DRLG seed (after the jungle-link draw) | seed, drlg +0x00 | `levels.md` §3 step 4 |
| jungle-link bit J | drlg +0x474 | `levels.md` §3 step 4 (read by `outdoor.md` §9.4) |
| Kurast Docks rect | level 75 +0x1C..+0x28 | `outdoor.md` §2.1 (leveldefs offset and size) |
| jungle size SX, SY | leveldefs `SizeX`, `SizeY` [difficulty] of level **76** (all three jungles) | `fields.tsv` |
| level seed | level +0x1C4 | `levels.md` §5 step 1 |
| lvlprest `Files`, `SizeX`, `SizeY` | per stamped id | `fields.tsv`, `outdoor.md` §5.1 |

## Outputs / state changes

- Act creation: rects of levels 76..78 (allocated in that order),
  their jungle block ids (level +0x1BC, an array of (SX/32)·(SY/32)
  ints, row-major from the top-left block) and clearing counts (level
  +0x1B8); rects of 79..83 (`outdoor.md` §9.2); DRLG seed advanced.
- Level generation: grid stamps (`outdoor.md` §5.1) and then rooms
  (`outdoor.md` §12).

## Rules

Integer division truncates toward zero. `roll(n)` is `rng.md` §3; "`lo'`"
is the low word after one step. Blocks are 32×32 tiles (4×4 outdoor
cells). Block rows grow southward (larger y).

### 1. Level map

| Level | Name | DrlgType / LevelType | Size (tiles, live leveldefs) | Built by |
|---|---|---|---|---|
| 75 | Kurast Docks | 2 / 20 | 64×48 at (1000, 1000) | `preset.md` |
| 76, 77, 78 | Spider Forest, Great Marsh, Flayer Jungle | 3 / 21 | 64×192, position from §2 | §2, §3, `outdoor.md` §9.3 |
| 79, 80, 81 | Lower Kurast, Kurast Bazaar, Upper Kurast | 3 / 22 | 80×64 | `outdoor.md` §9.2, §9.4 |
| 82 | Kurast Causeway | 3 / 22 | 48×16 | `outdoor.md` §9.4 |
| 83 | Travincal | 3 / 22 | 64×64 | `outdoor.md` §9.4 |
| 109 | Harrogath | 2 / 29 | 40×40 at (1000, 1000) | `preset.md` |
| 110 | Bloody Foothills | 3 / 30 | 240×48 at (760, 1000) | `outdoor.md` §11 (siege strip) |
| 111 | Frigid Highlands | 3 / 31 | leveldefs −1; 64×160 or 160×64 from linker B1 | `outdoor.md` §2.4, §11 |
| 112 | Arreat Plateau | 3 / 31 | leveldefs −1; 64×160 or 160×64 from linker B2 | `outdoor.md` §2.4, §11 |
| 117 | Frozen Tundra | 3 / 31 | leveldefs 128×80 is **overridden**: 64×160 or 160×64 from linker BD (`0x0067D8E0`), position (2000, 1896) | `outdoor.md` §2.4, §11 |
| 120 | Arreat Summit | 2 / 31 | 20×28 | `preset.md` (not outdoor) |

Sizes are equal in all three difficulties (live `leveldefs.bin`,
`traces/raw/20261006-115547-tables`). The ice caves 113–116, 118, 119
are mazes (`maze.md`). Level 134 (DrlgType 3, Act II rules) is not part
of this spec.

### 2. Jungle placer (`0x00677880`, D2MOO `DRLG_GenerateJungles`)

Called from the Act III placer (`0x006789B0`) right after level 75's
rect is set. All draws are on the **DRLG seed** (drlg +0x00), inline.
SXb := SX/32, SYb := SY/32 (2 and 6 in 1.14d data).

#### 2.1 Jungle records

Three 0x38-byte records, zeroed (stack, `0x00677880`):

| Offset | Field |
|---|---|
| +0x00, +0x04, +0x08, +0x0C | rect x, y, w, h (tiles) |
| +0x10 | case (0..4, §2.2) the jungle was placed with; record 0: 0 |
| +0x14 | branch count (jungles placed on this one) |
| +0x18 | base record (the jungle this one was placed on) |
| +0x1C, +0x20, +0x24 | branch records, in placement order |
| +0x28, +0x2C | block column bx, block row by (§2.3) |
| +0x30 | block id array (SXb·SYb ints) |
| +0x34 | clearing count (§2.8) |

#### 2.2 Placing the three jungles

1. Record 0 := (docks.x, docks.y − SY, SX, SY). minX := docks.x,
   maxX := docks.x + SX, minY := docks.y − SY, maxY := docks.y
   (maxY is never updated).
2. k := 1. While k < 3:
   1. base := roll(k) (site `0x00677966`; k = 1 steps and gives 0).
   2. case := `lo' mod 5` (one step, site `0x0067799A`).
   3. New rect := base rect moved by case (`0x006777D0`): 0 (0, −SY);
      1 (−SX, y1); 2 (+SX, y1); 3 (−SX, y3); 4 (+SX, y3); w, h := SX, SY;
      record case := case. y1 := ⌊(⌊SY·0x55555555 / 2³²⌋ − SY) / 2⌋
      (floor), plus 1 if negative; y3 := −2SY/3 (truncated). SY = 192:
      y1 = −64, y3 = −128.
   4. If the new rect overlaps any of records 0..k−1 (gap test
      `0x0066B860`, margin 0, `outdoor.md` §2.6; touching is allowed):
      discard it and repeat from step 2.1 with the same k (new draws).
   5. Else: append it to the base's branch list (branch count + 1; more
      than 3 is fatal 0x63E, unreachable with 3 jungles), set its base
      record, minX := min(minX, x), minY := min(minY, y), maxX :=
      max(maxX, x + w); k += 1.
3. If (maxX − minX) or (maxY − minY) is not a multiple of 32: fatal
   0x64D / 0x64E (unreachable: every offset is a multiple of 32 for SX =
   64, SY = 192).

#### 2.3 Block grids

W := (maxX − minX)/32 + 2, H := (maxY − minY)/32 + 2. Four W×H int
grids, indexed (column, row) → row·W + column: **A** owner (jungle index
+ 1), **B** river order, **C** marks (0 none, 1 link, 2 attach point),
**D** code. Jungle k covers columns bx..bx+SXb−1, rows by..by+SYb−1 with
bx := (x − minX)/32 + 1, by := (y − minY)/32 + 1. The outer ring of
blocks belongs to no jungle and stays 0, so neighbour reads in
§2.5–§2.6 never leave the grids.

Code bits in D: low nibble = river exits 8 N (row − 1), 4 S (row + 1),
2 E (column + 1), 1 W (column − 1); attach exits 0x80 N, 0x40 S, 0x20
E, 0x10 W.

#### 2.4 River and attach points (per jungle, k = 0, 1, 2 in placement order)

Restart point R (also used by §2.6): all four grids := 0, then for each
k:

1. Store bx, by; allocate the block id array (a new one at every
   restart; the old one is not freed).
2. Side s: k = 0: `lo' & 1` (site `0x00677C43`); k > 0: s := 1 if the
   record's case is odd (1, 3), else 0. s survives the passes below.
3. **Pass** (repeated while n < 2):
   1. Every block of the jungle: A := k + 1, B, C, D := 0.
   2. col := bx + s; last := by + SYb − 1. "Opposite" of a block in
      this column means column col + 1 if s = 0, col − 1 if s = 1.
   3. k > 0: C(col, last) := 1; C(opposite, last) := 2.
   4. For each branch of this jungle in branch order, by the branch's
      case: C := 1 at (bx, by) for case 0, (bx, by + 3) for 1, (bx + 1,
      by + 3) for 2, (bx, by + 1) for 3, (bx + 1, by + 1) for 4.
   5. n := 1 if k > 0 else 0; run := 0; v := 100·(k + 1); row := last.
   6. While row ≥ by:
      - B(col, row) := v; v += 1.
      - If run = 0 or row = by: advance. Else t := `lo' mod 3` (site
        `0x00677E4F`); t ≠ 0: advance; t = 0: run := 0 and switch
        column (s = 0: col += 1, s := 1; s = 1: col −= 1, s := 0); the
        row is not advanced, so the next iteration writes B in the new
        column at the same row and advances without a draw.
      - Advance: run += 1; if run ≥ 2 and row ≥ 2 (absolute grid row)
        and C(opposite, row) = 0: C(opposite, row) := 2, n += 1. Then
        row −= 1.
4. **Drop** while n > 3: i := roll(n) (site `0x00677F1C`: `lo' & (n−1)`
   for n a power of two, else `lo' mod n`). Scan the jungle's blocks
   row by row from row by, columns from bx; the i-th (0-based) block
   with C = 2 gets C := 0; n −= 1. The k > 0 mark of step 3.3 can be
   dropped. n decrements only on a hit (`0x00677F8B`–`0x00677F92`): if
   fewer than i + 1 blocks have C = 2, nothing is dropped, n stays and
   the loop draws `roll(n)` again (`0x00677FB1`). Not reachable: n always
   equals the jungle's count of C = 2 blocks (the step 3.3 mark sits on
   row `last`, which no branch mark of step 3.4 (rows by, by + 1, by + 3)
   or later advance overwrites, and an advance marks only blocks with
   C = 0).

#### 2.5 River connections (whole grid, rows 0..H−1, columns 0..W−1; no draws)

For each block c with neighbours N, S, E, W: f := 0.

1. If C(c) = 1 (link): among N, S, E, W in this order, take the one
   with B ≠ 0, A equal to A(c) and the strictly smallest B (the first
   wins ties): f := its bit (8, 4, 2, 1) and that neighbour's D gets
   the opposite bit (N → its D |= 4, S → 8, E → 1, W → 2). Then for
   each of N, S, E, W with C = 1 and A ≠ A(c): f |= its bit.
2. If B(c) ≠ 0: for each of N, S, E, W with |B(neighbour) − B(c)| = 1:
   f |= its bit (A is not compared).
3. D(c) |= f.

#### 2.6 Attach directions (whole grid, row-major)

For **every** block c: d := `lo' & 3` (site `0x006782AA`, drawn before
any test). u := D(c); empty := (u = 0). Direction order from a start d:
(d + i) & 3 for i = 0..3, with 0 N (bit 0x80), 1 S (0x40), 2 E (0x20),
3 W (0x10). If C(c) = 2:

1. First search: the first direction whose neighbour has A = A(c) and 0
   < D < 15 (an unattached river block): u |= its bit.
2. If u = 0: restart at R (§2.4) — draws already made stay made; the
   whole of §2.4–§2.6 runs again, including k = 0's side draw.
3. If empty:
   1. Second search (same d): the first direction whose neighbour has A
      ≠ A(c) and C = 2: u |= its bit.
   2. m := `lo' & 1` (site `0x006784AE`); more := m = 1 and the second
      search found nothing.
   3. d := `lo' & 3` (site `0x006784D9`).
   4. If more: the first direction (from the new d) whose neighbour has
      0 < D < 15 (A not compared) and whose bit is not yet in u: u |=
      its bit.
4. The neighbour in each direction set in u gets the opposite attach
   bit (N → its D |= 0x40, S → 0x80, E → 0x10, W → 0x20). C(c) := 0.

Then D(c) |= u (for blocks with C ≠ 2 this changes nothing). Blocks
processed earlier no longer have C = 2, so the second search only sees
attach points later in row-major order.

#### 2.7 Code → lvlprest id (whole grid, row-major; no draws)

With L := D & 15 (`0x00678670`):

| Case | Id |
|---|---|
| D = 0 | 0 (no preset) |
| L = 0, D ≥ 16 (clearing) | S[D >> 4] with S at `0x006F13BC`: 1..12 → 575, 576, 577, 578, 579, 580, 0, 581, 582, 583, 0, 584 (13..15 read 0); 0 → fatal 0x799 |
| L ≠ 0, D < 16 (river) | 529 + L (530 "Jungle W" … 544 "Jungle NSEW") |
| L ≠ 0, D ≥ 16 (river with a side exit) | r := L; for bit 0x10, 0x20, 0x40, 0x80 in that order, if set: r := T[r][slot] (slot 0..3); r = 0 at the end → fatal 0x78C |

T (`0x006F13F0` + 16·r, 4 ints per row; rows 1..14; slot 0 W, 1 E, 2
S, 3 N; 0 = none): 1 (0, 545, 546, 547), 2 (548, 0, 549, 550), 3 (0, 0,
551, 552), 4 (553, 554, 0, 555), 5 (0, 556, 0, 557), 6 (558, 0, 0, 559),
7 (0, 0, 0, 560), 8 (561, 562, 563, 0), 9 (0, 564, 565, 0), 10 (566, 0,
567, 0), 11 (0, 0, 568, 0), 12 (569, 570, 0, 0), 13 (0, 571, 0, 0), 14
(572, 0, 0, 0). Each lookup **replaces r**, so a second attach bit
indexes T with a preset id (edge case 1). D2MOO indexes every bit with
the original L instead.

Lookups outside rows 1..14 have no bound check: slot k of row r reads
the dword at `0x006F13F0` + 4k + 16r (32-bit wrap; `.rdata`, so the file
image is the run-time value):

| r | Slots 0..3 read | Result |
|---|---|---|
| 0 (an earlier lookup gave 0) | 0, 0, 0, 0 (`0x006F13F0`–`0x006F13FC`, i.e. S[13..16]) | stays 0 → fatal 0x78C |
| 15 (L = 15, "Jungle NSEW", with any attach bit: the first lookup) | 0, 0, 0, 0 (`0x006F14E0`–`0x006F14EC`, the 16 bytes after row 14; `.rdata` file image) | 0 → fatal 0x78C for every attach bit (a later bit then reads row 0) |
| 545..572 (an earlier lookup hit) | slots 0 and 2: 0; slots 1 and 3: a value V(r) in 1,071,743,488 .. 1,072,087,552 (not an lvlprest id) | 0 → fatal 0x78C; V is stored as the block id |
| V(r) (a third set bit after V) | the address `0x006F13F0` + 4k + 16V wraps into `0xFE8733F0`–`0xFEDB33FC`, outside the module image | reads memory outside the module (not reproducible) |

A stored V reaches §3 as a clearing (V > 574): with c < 3, P := V + the
family add and the stamp (`outdoor.md` §5.1) reads its size through the
lvlprest row lookup (`0x00666FD0` → `0x0061F0B0`), which returns none
for P ≥ the row count; 1.14d then reads +0x28 of a null row (access
violation). So every outcome of a block with two or more attach bits is
fatal 0x78C, a crash or an unreproducible read; d2rs reports all of them
as a fatal error (the crash cases have no original error id). Not reached (edge case 1).

#### 2.8 Hand-off to the levels

1. For each record in placement order: block ids[i] := id of block (bx
   + i mod SXb, by + i div SXb), i = 0..SXb·SYb−1; clearing count :=
   number of ids > 574.
2. Sort the three records by y descending: bubble passes over pairs
   (0, 1), (1, 2), swapping when the first y is smaller, until a pass
   swaps nothing (equal y keeps placement order). Record 0 has the
   largest y, so it always becomes level 76.
3. For n = 0, 1, 2: level 76 + n := get-or-allocate (`levels.md` §4.2;
   allocation order 76, 77, 78); level +0x1BC := block ids, +0x1B8 :=
   clearing count, rect := the record's rect.
4. Free the grids; return level 78. The Kurast chain (`outdoor.md`
   §9.2) is anchored on this level.

### 3. Jungle stamping (`0x0067E910`, levels 76..78)

Runs at level generation after the link flags (`outdoor.md` §9.3).
SXb, SYb from leveldefs 76 (by difficulty), as §2. The array of §2.8 is
never shorter than SXb·SYb: creation (`0x00677880`, `0x0067788C`–
`0x0067789E`) and build (`0x0067E92D`–`0x0067E964`) both read leveldefs
row 76 at the DRLG's difficulty byte (drlg +0x450), and the tables do not
change after load.

1. r := roll(2 + 4·(clearing count = 3)) on the level seed (helper
   `0x0045C390`). Drawn before the next check.
2. Block ids absent: fatal 0x27.
3. idx := 0, c := 0. For row i = 0..SYb−1 (top to bottom):
   - Level 76 and i = SYb − 1: stamp 573 "Jungle Head" at cell (0, 4i)
     with F := 1 if ids[SXb·SYb − 1] = 0 else 0; idx += 2.
   - Level 78 and i = 0: stamp 574 "Jungle Tail" at (0, 0) with F := 1
     if ids[1] = 0 else 0; idx += 2.
   - Otherwise for j = 0..SXb−1: P := ids[idx], idx += 1, F := −1. If P >
     574: c ≥ 3 is fatal 0x47; P += [0, 10, 20][level − 76] (`0x006F2370`:
     Webby 575–584, Boggy 585–594, Pygmy 595–604); F := G[3r + c]
     with G (`0x006F2328`) = 0,1,2, 1,0,2, 0,2,1, 1,2,0, 2,0,1, 2,1,0;
     c += 1. If P ≠ 0: stamp P at cell (4j, 4i), F, border 0
     (`outdoor.md` §5.1; F −1 draws the build-list roll on P's first
     stamp).

Clearing count ≤ 3 always (§2.4 keeps at most 3 attach points per
jungle and only attach points become clearings), so r picks one of 2
(count ≠ 3) or 6 (count = 3) file orders and the clearings of a level
never share a file when there are three.

### 4. Act III rooms and links

1. Act III level build (`0x0067F450`): link flags (`outdoor.md` §5.5),
   §3, Kurast (`outdoor.md` §9.4), Travincal. No borders, waypoint or
   shrine placers run; Act III has no blank cells (grid-2 0x100).
2. Rooms (`outdoor.md` §12.1): a stamped preset gives one preset room
   at its top-left cell, sized by the preset; the other covered cells
   give none. Every unstamped cell gives an 8×8 outdoor room: in the
   jungles, the 16 cells of each block with id 0 (DT1 mask 0x4, floor
   flags 0x120000, `outdoor.md` §12.2); in Kurast, cells the random
   placer left (mask 0x1, floor 0x100000).
3. Jungle 76's bottom row is always the head (it touches the docks);
   78's top row is always the tail (it touches Lower Kurast, §2.8 step
   4). The link marks of §2.4 steps 3.3 and 3.4 sit on facing blocks of
   a jungle and its base (case 0: N/S; cases 1–4: E/W), and §2.5 step 1
   gives each of them a river exit toward the other. Jungle levels have
   no table warps between them; level-to-level walking uses the
   adjacency warps and neighbour entries of `outdoor.md` §2.7 (vis slots
   whose warp is −1: 76's 84/85, 78's 86/88 and Kurast's sewer/temple
   slots carry lvlwarp ids and are not neighbours) and the link flags of
   `outdoor.md` §5.5.

### 5. Act V outdoor levels

The build steps are `outdoor.md` §11; the geometry is `outdoor.md` §2
(A5: Def 109, Def 110, B1 111, B2 112; A5T: BD 117; no checks; no
table warps in Act V).

1. **Neighbour entries** exist only for 111 and 112 (`outdoor.md` §2.1:
   built after the 111..112 adjacency warps and before the 110..111 and
   109..110 ones). Live vis/warp: 110 and 111 have no vis; 112 has 113
   with lvlwarp 71; 117 has 115 (warp 72) and 118 (warp 71). So 111's
   polygon opens only toward 112 and 112's only toward 111; 110 never
   runs the link flags; 117 has no openings and joins the world only
   through its cave presets (`outdoor.md` §11 step 5) and warps.
2. **Bloody Foothills (110)**: gw × gh = 30 × 6; the siege strip
   (`outdoor.md` §11) stamps 15 presets of 2×6 cells with F 0, covering
   every cell; no draw while stamping; 15 preset rooms, no outdoor
   rooms; no link flags, no border substitution.
3. **111, 112, 117**: gw × gh = 8 × 20 (tall, w < h) or 20 × 8 (wide).
   Rooms: `outdoor.md` §12.1 with DT1 mask 0x11 (type 31); floor flags
   0x600000 on 117 only. Every special row with a tall/wide pair uses
   the tall id when w < h, else the wide one (`outdoor.md` §11 step 9).

## Constants & data dependencies

| Constant | Value | Where |
|---|---|---|
| block size | 32 tiles | §2 |
| case offsets | 5 cases | `0x006777D0` |
| clearing ids S | 17 ints | `0x006F13BC` |
| river-with-exit ids T | rows 1..14 × 4 | `0x006F13F0` + 16·r |
| clearing family add | 0, 10, 20 | `0x006F2370` (level 76..78; read as `0x006F2240` + 4·level) |
| clearing file orders G | 6 × 3 | `0x006F2328` |
| fatal ids | 0x27, 0x47, 0x63E, 0x64D, 0x64E, 0x78C, 0x799 | §2, §3 |

Data (live 1.14d `lvlprest.txt` / `lvlprest.bin`): 529 "Act 3 - Town";
530–544 river pieces (Files 3, 541 "Jungle NS" Files 5); 545–572 river
pieces with a side exit (Files 1); 573 "Jungle Head" and 574 "Jungle
Tail", 64×32, **Files 0**, File1/File2 = TransL/TransU and TravL/TravU
(F 0 / 1 picks); 575–604 clearings, Files 1 but File1..File3 named
(the F of §3 picks among them; the room pass draws `roll(1)`); all
jungle pieces 32×32. Leveldefs: `SizeX`/`SizeY` of 75, 76, 79..83, 109,
110, 117 and `OffsetX/Y` of 75, 109, 110, 117 (`fields.tsv` offsets 12,
24, 36, 40). Act V ids are in `outdoor.md` §11.

## Randomness

**Act III creation, DRLG seed** (after `levels.md` §3's start and
jungle-link draws), in order:

1. Per new jungle (k = 1, 2), per try: base `roll(k)` (`0x00677966`),
   case `lo' mod 5` (`0x0067799A`).
2. Per run of §2.4 (the first and every restart): k = 0 side `lo' & 1`
   (`0x00677C43`); per jungle, per pass, `lo' mod 3` for every river
   step with run ≥ 1 and row > by (`0x00677E4F`); per drop `roll(n)`
   (`0x00677F1C`).
3. §2.6: `lo' & 3` per block in row-major order (`0x006782AA`); per
   attach point whose D was 0: `lo' & 1` (`0x006784AE`) then `lo' & 3`
   (`0x006784D9`). A failing attach point restarts at 2.

No other Act III creation step touches the DRLG seed; levels 75..83 are
allocated without draws (`levels.md` §4.3).

**Jungle level build, level seed**: §3 step 1 roll; build-list rolls of
each river piece's first stamp in stamp order (`outdoor.md` §5.1, site
`0x0067438F`); then the room pass (`outdoor.md` §12.1: per preset cell
`roll(Files)` at `0x00666F33`, which does not step for the head/tail
(Files 0); per outdoor cell a room allocation).

**Act V**: placement draws are on driver copies (`outdoor.md` §2.3: B1,
B2 on the A5 copy, BD on a fresh A5T copy; the DRLG seed is not
advanced). Level build, level seed, in `outdoor.md` §11 order:
build-list rolls of first stamps (border walk, ravine walk, entrances,
connect-to-siege; caves use F 0 and never draw); border substitution
type 12 (`outdoor-tilesub.md`); prisons (111: two `roll`s per try, then
`roll(gw/2)`, `roll(gh/2)` always); special presets (per call one
`outdoor.md` §5.3 shuffle of 2·(gw−2)(gh−2) draws, then the build-list
roll if F is −1 and the preset is new); then the room pass. Level 110
draws only in its room pass (15 × `roll(1)` plus `preset.md`).

## Edge cases & original bugs

1. A river block with two or more attach bits (§2.7 last row) looks the
   second bit up in T with the first result (545..572) as the row: it
   reads 0x006F13F0 + 16·P + 4·slot, outside T. In the 1.14d image that
   gives 0 for slots 0 and 2 (→ fatal 0x78C) and values near
   1.07·10⁹ for slots 1 and 3 (not lvlprest ids). A simulation of §2
   over 20,000 random DRLG seeds never produced such a block, a fatal,
   or a clearing count above 3 (5.2 % of the seeds restarted §2.6 at
   least once). Reproduce the lookup; treat a non-id result as fatal
   0x78C (OQ 2).
2. S[0] is 256 and unreachable (a clearing has D ≥ 16).
3. The head/tail rows advance idx by 2 whatever SXb is (SXb = 2 in all
   1.14d data).
4. §2.4 step 3.6's `row ≥ 2` test is on the absolute grid row, not the
   row inside the jungle.
5. §2.5 step 2 does not compare owners. It cannot join two jungles:
   a pass writes at most 2 B values per row (a column switch forces the
   next step), so jungle k's B stays in 100(k+1) .. 100(k+1) + 2·SYb − 1
   and ranges of different jungles never differ by 1.
6. Every restart of §2.4 allocates new block id arrays without freeing
   the old ones (memory only).
7. 117's leveldefs size (128×80) is never used: BD replaces it.

## Test vectors

**Derived from these rules** (a scratch simulation of §2, §3 and
`outdoor.md` §2/§9.2; CI-safe once the code exists). The creation rows
(draw counts, seed after, rects, block ids, clearings, Kurast chain)
are recorded below.
Input: init seed 644409375, difficulty 0 → DRLG seed after the start
and jungle-link draws {1406222081, 1674353446} (`levels.md` vectors);
docks (1000, 1000, 64, 48); SX 64, SY 192.

| Step | Expected |
|---|---|
| k = 1 | base `lo'` 3154683627 → 0; case `lo'` 457460266 mod 5 = 1: jungle 1 at (936, 744) |
| k = 2 tries | (base 0, case 3) overlaps; (base 1, case 2) overlaps; (base 0, case 0) → jungle 2 at (1000, 616) |
| grid | W 6, H 14; §2.6 passes without restart |
| draw counts | base 4, case 4, side 1, `mod 3` 12, drop 2, `&3` 84 (= W·H), `&1` 8, second `&3` 8: 123 steps; DRLG seed after {4015082244, 577631236} |
| level 76 | (1000, 808, 64, 192); ids 541, 533, 543, 565, 570, 582, 571, 575, 570, 575, 537, 0; clearings 3 |
| level 77 | (936, 744, 64, 192); ids 554, 577, 541, 0, 539, 534, 0, 541, 576, 569, 576, 566; clearings 3 |
| level 78 | (1000, 616, 64, 192); ids 0, 533, 535, 565, 570, 582, 539, 534, 558, 565, 541, 581; clearings 2 |
| Kurast chain | 79 (992, 552, 80, 64), 80 (992, 488, 80, 64), 81 (992, 424, 80, 64), 82 (1008, 408, 48, 16), 83 (1000, 344, 64, 64) |

Jungle stamping (§3) from level seed {dwStartSeed + id, 666},
dwStartSeed 4014346869; stamps as (cell x, cell y, P, F):

| Level | r | Stamps | Steps, seed after |
|---|---|---|---|
| 76 | 1 (`lo'` 4139712799) | (0,0,541,4) (4,0,533,2) (0,4,543,0) (4,4,565,0) (0,8,570,0) (4,8,582,1) (0,12,571,0) (4,12,575,0) (0,16,570,0) (4,16,575,2) (0,20,573,1) | 7, {2661750284, 470510827} |
| 77 | 0 | (0,0,554,0) (4,0,587,0) (0,4,541,4) (0,8,539,1) (4,8,534,1) (4,12,541,0) (0,16,586,1) (4,16,569,0) (0,20,586,2) (4,20,566,0) | 7, {2370309536, 189899886} |
| 78 | 1 | (0,0,574,0) (0,4,535,2) (4,4,565,0) (0,8,570,0) (4,8,602,1) (0,12,539,2) (4,12,534,2) (0,16,558,0) (4,16,565,0) (0,20,541,4) (4,20,601,0) | 8, {4128120053, 867080307} |

Rooms then: 76 and 78: 11 preset rooms, no outdoor room; 77: 10 preset
rooms and 32 outdoor rooms (blocks 3 and 6 have id 0).

**Recorded Act III creation** (`pc2rec-d1-rng`, TestSor, `-seed`
644409375, sha256 8ac70b456cea9732…, Act III via waypoint; block ids
from the same seed by `autostart.py --try TestSor --seed 644409375`,
`dumpdrlg` of the +0x1BC array, log D1b of `pc2-rec-lane.md`):

| Item | Recorded | Derived row |
|---|---|---|
| DRLG-seed draws | 246 at `0x00677966`–`0x006784D9` = 123 server + 123 client; DRLG seed after {4015082244, 577631236} | draw counts: equal |
| 76 | (1000, 808, 64, 192); +0x1B8 3; ids 541, 533, 543, 565, 570, 582, 571, 575, 570, 575, 537, 0 | equal |
| 77 | (936, 744, 64, 192); +0x1B8 3; ids 554, 577, 541, 0, 539, 534, 0, 541, 576, 569, 576, 566 | equal |
| 78 | (1000, 616, 64, 192); +0x1B8 2; ids 0, 533, 535, 565, 570, 582, 539, 534, 558, 565, 541, 581 | equal |
| Kurast chain | 79 (992, 552, 80, 64), 80 (992, 488), 81 (992, 424), 82 (1008, 408, 48, 16), 83 (1000, 344, 64, 64) | equal |
| level +0x08 of 76, 77, 78 (client copy, at the docks) | 192 each | not the 11 / 42 / 11 rooms above: what +0x08 counts for jungle levels is OQ 3 |

Mass check: `check_drlg_acts` on 12 more seeds × Acts I–III
(`pc2rec-d2-sweep.log`, 36 records, 271 level seeds) 0 errors; the
jungle bit is 0 for seed 555, 1 for the others.

Act V placement, same init seed (`outdoor.md` §2.4, derived): A5 copy
draws 1406222081 (B1: R0 1 → 160×64) and 3154683627 (B2: R0 1, offset
index 3 → (−160, 0)); A5T copy draws 1406222081 (BD: 160×64). Rects:
110 (760, 1000, 240, 48), 111 (600, 968, 160, 64), 112 (440, 968, 160,
64), 117 (2000, 1896, 160, 64); grids 30×6, 20×8, 20×8, 20×8.

**Real 1.14d values** (live tables, game-file tier):

| Input | Expected | Source |
|---|---|---|
| leveldefs 76..78, difficulty 0..2 | SizeX 64, SizeY 192, OffsetX/Y −1 | `20261006-115547-tables/leveldefs.bin` |
| leveldefs 111, 112 | SizeX/SizeY −1 (linker sizes) | same |
| lvlprest 573, 574 | 64×32, Files 0 | `lvlprest.txt` (patch_d2) |
| lvlprest 530..544, 545..572, 575..604 | 32×32; Files 3 (541: 5), 1, 1 | same |
| level 110 stamps | 865 + i at cell (28 − 2i, 0), F 0, i = 0..14 (879 "Siege To Barricade" at x 0) | `outdoor.md` §11, lvlprest 865 SizeX 16 |
| level 82 | one stamp 652 (48×16) at (0, 0), F 0: one preset room | `outdoor.md` §9.4 |
| level 83 | six stamps 653..658, F −1: six build-list `roll(1)`, all file 0; six preset rooms covering 8×8 cells | `outdoor.md` §9.4, lvlprest Files 1 |

Comparison (exact): for an Act III creation, the sequence of (site,
seed after) of every DRLG-seed draw and the rects and block ids of
levels 76..78 equal a recording / level probe; for each built level the
stamp list and draw sequence equal the recording.

## Provenance

- **1.14d `Game.exe`** (`re/exports/all.asm`, `tools/ghidra/disasm.py`):
  jungle placer `0x00677880` (placement loop `0x00677947`–`0x00677A7E`,
  restart point R at `0x00677B80` (the four clears), draw sites as
  §Randomness, code table `0x00678670`–`0x006786F3` with the chained
  `shl ecx, 4` lookups, hand-off with `0x00642BB0` per level 76..78 and
  the return of level 78; fatal ids pushed before `0x00408090`); case
  offsets `0x006777D0`; Act III placer `0x006789B0` (calls `0x00678910`
  with that return value); Kurast chain `0x00678910`; Act III build
  `0x0067F450`; jungle stamping `0x0067E910`; Act V linkers
  `0x0067D830`, `0x0067D8E0`, `0x0067D980`; Act V build `0x0067E600`,
  prisons `0x0067E240`. Tables read from the file image (`pefile`):
  `0x006F13BC`, `0x006F13F0`–`0x006F14EF`, `0x006F2328`, `0x006F2370`.
- **D2MOO** (1.10f) `DrlgOutPlace.cpp` (`DRLG_GenerateJungles` and
  helpers), `DrlgOutJung.cpp` (`DRLGOUTJUNG_BuildJungle`) as a map. 1.14d
  differences: the code table chains its lookups (§2.7; D2MOO uses a
  fixed row); clearing ids 13..15 and a missing id are fatal errors in
  1.14d (D2MOO warns); the Kurast chain anchors on the jungle placer's
  return value, level 78.
- **Live data**: `traces/raw/20261006-115547-tables/leveldefs.bin`,
  `game/extracted/patch_d2/.../lvlprest.txt`.
- **Recorded** (2026-10-08): Act III creation in `pc2rec-d1-rng`
  (PC 2 recording lane, kept on PC 2) and the D1b block-id log; no
  Act III level build and no Act V recording yet.

## Open questions

1. Record entering Act 3 (DRLG-seed draws at `0x00677966`…`0x006784D9`)
   and read levels 76..78 +0x1C..+0x28, +0x1B8, +0x1BC to confirm §2 and
   the derived vector (supersedes `outdoor.md` OQ 4 and 7).
   *Answered (2026-10-08, recorded: `pc2rec-d1-rng`)*: draws, DRLG seed
   after, rects, clearings, block ids and the Kurast chain equal the
   derived vector (Test vectors, "Recorded Act III creation"); `outdoor.md`
   OQ 4 and 7 are closed with it.
2. Can any seed give a river block two attach bits (edge case 1)?
   Settle by enumerating §2 over all jungle-link-time DRLG seeds reachable
   from 32-bit init seeds, or by a recording that hits fatal 0x78C.
3. Record a build of 76..78 (stamps, room count: are id-0 blocks 16
   outdoor rooms each?) and of 111, 112, 117 (`outdoor.md` OQ 9).
   Still open (2026-10-08): `pc2rec-d1-rng` reads level +0x08 = 192 on
   76, 77 and 78 at the docks (before any build), not 11 / 42 / 11, so
   +0x08 is not the room count there; a build of 76..78 is still
   needed. Act V is not reachable with the TestSor save.
4. *Answered* (`impl-drlg-act3-5` Q1): lookups outside rows 1..14 of T
   (row 0, rows 545..572, a third bit) are §2.7's table "Lookups outside
   rows 1..14": 0 → fatal 0x78C, V(r) stored then a crash in §3, a third
   bit an unreproducible read (`.rdata` read from the 1.14d file; `0x00678670`–
   `0x006786F3`, `0x0061F0B0`, `0x00666FD0`).
5. *Answered* (`impl-drlg-act3-5` Q2): a drop index past the C = 2
   blocks drops nothing, keeps n and draws again (§2.4 step 4,
   `0x00677F00`–`0x00677FB4`); unreachable since n equals the mark count.
6. *Answered* (`impl-drlg-act3-5` Q3): the jungle id array is never
   short at build (§3: same leveldefs row 76 and difficulty byte at
   creation and build).
7. *Answered (2026-10-08)* (`fix-drlg-answers` Q1): row 15 of T (L =
   15 with an attach bit) reads four zeros from the 1.14d file image
   (`0x006F14E0`–`0x006F14EF`), so every such block is fatal 0x78C
   (§2.7 table "Lookups outside rows 1..14", row 15). Open inside it:
   whether any seed gives an L = 15 block an attach bit; settle with
   the enumeration of OQ 2.
