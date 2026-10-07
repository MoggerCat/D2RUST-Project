# Spec: DRLG — Outdoor levels (DrlgType 3) and act-wide outdoor placement

- **Status:** draft: every rule below was read from the 1.14d disassembly
  (addresses given) and the Act 1 build (Blood Moor, Cold Plains) matches
  the recorded draw order and per-site counts of
  `20261005-232125-rng.jsonl`; Acts 2–5 and the Act 3 jungle placer are
  read from the code but not yet recorded.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::drlg::outdoor`
- **Related specs:** `drlg/outdoor-act3-act5.md` (Act III jungle
  placer and stamping; Act III / Act V geometry, rooms, draw order,
  vectors); `drlg/outdoor-tilesub.md` (lvlsub tile substitution:
  border substitution called from here, room sub-themes and per-room
  substitution); `drlg/levels.md` (DRLG, act creation order, level
  allocation and seeds, level generation dispatch, vis/warp records,
  containment); `drlg/rooms.md` (room allocation and seeds, room seed
  reset, tile fill); `drlg/preset.md` (lvlprest file choice, DS1 → room,
  `BuildArea`); `drlg/maze.md`; `sim/rng.md` (generator, `roll`);
  `data/fields.tsv` (levels/leveldefs, lvlprest, lvlsub layouts).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 48–63 |
| Inputs | 64–75 |
| Outputs / state changes | 76–85 |
| Rules | 86–91 |
|   1. Structures (1.14d) | 92–151 |
|   2. Act-wide placement (`0x00678AD0`, D2MOO `DRLGOUTPLACE_CreateLevelConnections`) | 152–300 |
|   3. Level generation (`0x00675360`, D2MOO `DRLGOUTDOORS_GenerateLevel`) | 301–314 |
|   4. Vertex polygon (`0x0067D050`, `0x0067CE20`; D2MOO `DRLGVER_CreateVertices`) | 315–339 |
|   5. Preset primitives on the grids | 340–397 |
|   6. Borders (`0x00675850`, D2MOO `PlaceAct1245OutdoorBorders`) | 398–452 |
|   7. Act I (`0x006807F0`, D2MOO `OutWild`) | 453–620 |
|   8. Act II (`0x0067F980`, D2MOO `OutDesr`) | 621–665 |
|   9. Act III | 666–733 |
|   10. Act IV (`0x0067E890`) | 734–745 |
|   11. Act V (`0x0067E600`) | 746–837 |
|   12. Rooms | 838–873 |
| Constants & data dependencies | 874–896 |
| Randomness | 897–918 |
| Edge cases & original bugs | 919–940 |
| Test vectors | 941–1066 |
| Provenance | 1067–1107 |
| Open questions | 1108–1162 |
<!-- /index -->

## Summary

Outdoor levels (leveldefs `DrlgType` 3: the Act 1 wilderness, the Act 2
desert, the Act 3 jungles and Kurast, the Act 4 mesas and Chaos Sanctum,
the Act 5 highlands) are built on a coarse grid of 8×8-tile cells. When an
act is created, an act-wide placer positions the act's outdoor chain
(Blood Moor next to Cold Plains next to Stony Field…) from a **copy** of the
DRLG seed, sets warp and neighbour data and per-level border flags. When a
level is generated, its outer rectangle and the openings toward its
neighbours become a vertex polygon; borders, cliffs, rivers, paths and
fixed presets are stamped into the cell grids from the **level seed**;
lvlsub DS1 files substitute border cells (`outdoor-tilesub.md`); finally
every cell becomes one DRLG room: a preset (DS1) room for cells holding a
preset, or an outdoor-grid room otherwise, whose sub-themes are drawn from
the **room seed**.

## Inputs

| Name | Type | Source |
|---|---|---|
| DRLG seed (copied per placer call; advanced only where §2.3 says) | seed | `levels.md` §3 |
| level seed | seed, level +0x1C4 | `levels.md` §4.3, §5.1 (re-set before generation) |
| level id, level type, position/size | level +0x1D0, +0x1C0, +0x1C..+0x28 | `levels.md` §1 |
| difficulty | drlg +0x450 | `levels.md` §1 |
| leveldefs | `SizeX/Y[diff]`, `OffsetX/Y`, `SubType`, `SubTheme`, `SubWaypoint`, `SubShrine`, `Vis0..7` | `fields.tsv` |
| lvlprest | `Def`, `SizeX` (+0x28), `SizeY` (+0x2C), `Files` (+0x40) | `fields.tsv` |
| lvlsub | via `outdoor-tilesub.md` | `fields.tsv` |

## Outputs / state changes

- Act creation: outdoor level positions and sizes (level +0x1C..+0x28),
  Rogue Encampment / Lut Gholein / Outer Cloister preset direction, warp
  slots, per-level outdoor flags (outdoor +0x00), neighbour ("orth")
  entries.
- Level generation: four cell grids, vertex polygon, path data, the
  build list (+0x1CC), and the level's rooms (preset rooms via
  `preset.md`, outdoor rooms §12).

## Rules

Integer division truncates toward zero (all `/8`, `/2` here are signed).
`roll(n)` is `rng.md` §3 (n ≤ 0: no step, result 0; n = 1 steps). All
inline rolls in this spec use those semantics; `lo'&m` steps always.

### 1. Structures (1.14d)

**Outdoor info** (level +0x14, 0x268 bytes, zeroed at allocation
`0x00675320`):

| Offset | Field |
|---|---|
| +0x00 | flags (§1.3) |
| +0x04, +0x18, +0x2C, +0x40 | grids 0..3 (20 bytes each: cells, row offsets, width, height, flag) |
| +0x54, +0x58 | "width, height": set to 0 at generation; with +0x5C/+0x60 they form the rect (0, 0, gw, gh) |
| +0x5C, +0x60 | grid width gw = level width / 8, grid height gh = level height / 8 |
| +0x64 | vertex polygon (circular list) |
| +0x68 | path vertex lists [6] |
| +0x80 | path end-points: 24 vertices (20 bytes): [0..5] start, [6..11] start-adjusted, [12..17] join-adjusted, [18..23] join |
| +0x260 | path count |
| +0x264 | neighbour (orth) list |

Grid cells (one u32 per 8×8 cell):

| Grid | Content |
|---|---|
| 0 | lvlprest id stamped at the top-left cell of a placed preset (0 elsewhere) |
| 1 | room flags given to the cell's room: link flags `0x10 << i` (vis slot i), shrine bits 0x1000–0x8000, waypoint 0x10000 / 0x20000 |
| 2 | cell flags (§1.2) |
| 3 | outdoor "flags ex" handed to the room (never written by this spec's code paths seen; 0) |

**1.2 Grid-2 cell bits**

| Bit | Meaning |
|---|---|
| 0x1 | border cell (on the polygon edge, or a border preset) |
| 0x2 | the border has a direction (cliff side) |
| 0x80 | dirt path passes here |
| 0x100 | blank: no room is created (§12) |
| 0x200 | holds part of a placed preset |
| 0x400 | level link (opening to a neighbour) |
| 0x800 | waypoint |
| 0x1000 | shrine |
| 0xF0000 | picked file (lvlprest file index) |

"Spawn valid" (`0x00674200`): `(cell & 0x1B81) == 0`. "Not a link"
(`0x006741E0`): `(cell & 0x400) == 0`.

Grid operations (`0x0067C4F0` op codes): 0 OR, 1 AND, 2 XOR, 3 overwrite,
4 overwrite-if-zero, 5 AND-NOT.

**1.3 Outdoor flags** (+0x00): 0x4 bridge, 0x8 river (other side), 0x10
river, 0x20 cliffs, 0x40 cave/entrance placed, 0x80/0x100/0x200/0x400 Act
1 town-transition variants, 0x400000/0x800000 Act 4 fortress transition
rows.

**1.4 Vertex** (20 bytes, `0x0067CDD0` alloc): x +0, y +4, direction u8 +8,
flags +0xC (bit 0 link, bit 1 link to a preset level), next +0x10.
**Neighbour (orth) entry**: level +0x00, direction +0x04 (0 W, 1 N, 2 E,
3 S), init flag +0x0C, box (rect) +0x10, next +0x14; built by
`0x0066B790` (`levels.md`/`rooms.md` owner of the list type).

**1.5 Build list** (level +0x1CC; 16-byte nodes: preset id, file count,
current file, next), §5.1.

### 2. Act-wide placement (`0x00678AD0`, D2MOO `DRLGOUTPLACE_CreateLevelConnections`)

Called once per act from DRLG creation (`levels.md` §3.7).

#### 2.1 Per act

| Act (index) | Function | Steps in order |
|---|---|---|
| I (0) | `0x00677750` | link driver (§2.3) on table A1W with check `0x00676DD0` and flags fn `0x00677180`; driver on A1M with check `0x00676EB0` and flags fn `0x00677180`; neighbour entries (§2.7) for levels 1..17 |
| II (1) | `0x00677790` | driver on A2 (check `0x00676F50`, no flags fn); driver on A2C (check `0x00676FC0`); neighbour entries for levels 40..46 |
| III (2) | `0x006789B0` | Kurast Docks (75) position := leveldefs offset, size := leveldefs size; jungle placer (§9.1, `0x00677880`); Kurast chain (`0x00678910`, §9.2); level-adjacency warps (§2.7) for 75..83; neighbour entries for 75..83 |
| IV (3) | `0x00678A20` | driver on A4 (check `0x006770A0`); driver on A4C (check `0x00677110`); Outer Steppes (104) outdoor flags |= the transition flag left by its linker (global `0x0096D66C`); neighbour entries 103..106 |
| V (4) | `0x00678A70` | driver on A5 (no check, no flags fn); driver on A5T; adjacency warps 111..112; neighbour entries 111..112; adjacency warps 110..111; adjacency warps 109..110; driver on A5U (check `0x00677030`) |

#### 2.2 Link tables

Rows `(linker, level, link)`: the level is placed relative to row
`link` (−1: absolute). Tables are 15 rows of 16 bytes; a row with level 0
ends the table. Linker names: see §2.4.

| Table (address) | Rows |
|---|---|
| A1W `0x006F0750` | (Def, 4 Stony Field, −1), (R4, 3 Cold Plains, 0), (BM, 2 Blood Moor, 1), (RE, 1 Rogue Encampment, 2), (R4, 17 Burial Grounds, 1) |
| A1M `0x006F0840` | (Def, 39 Moo Moo Farm, −1), (Def, 26 Monastery Gate, −1), (Fix, 7 Tamoe Highland, 1), (R4, 6 Black Marsh, 2), (R4, 5 Dark Wood, 3) |
| A2 `0x006F0930` | (Def, 40, −1), (RW, 41, 0), (R8, 42, 1), (R8, 43, 2), (R8, 44, 3), (VS, 45, 4) |
| A2C `0x006F0A20` | (Def, 46, −1) |
| A5U `0x006F0B10` | (Def, 134, −1), (Def, 136, −1) |
| A4 `0x006F0C00` | (Def, 103, −1), (OS, 104, 0), (R4, 105, 1), (R4, 106, 2) |
| A4C `0x006F0CF0` | (Def, 108, −1) |
| A5 `0x006F0DE0` | (Def, 109, −1), (Def, 110, 0), (B1, 111, 1), (B2, 112, 2) |
| A5T `0x006F0ED0` | (BD, 117, −1) |

#### 2.3 Link driver (`0x006772C0`)

State: rects C[15] (x, y, w, h), four arrays R0..R3[15] (all −1), iteration i.

1. **Seed copy**: copy the DRLG seed {lo, hi} into the driver's own seed.
   Linkers draw from the copy; the DRLG seed itself is not advanced.
   Each driver call starts from the current DRLG seed again.
2. For every row: C[row].w, C[row].h := leveldefs `SizeX/SizeY[difficulty]`.
3. i := 0. While row i has a level: call its linker. If it returns false:
   R0..R3[i] := −1 and i −= 1 (the previous row's linker runs again and
   tries its next alternative). If it returns true: if there is no check
   function or the check passes, i += 1; else the same linker runs again
   (next alternative).
4. For every row in order:
   - `level` := get-or-allocate (`levels.md` §4.2; allocation order is
     observable through level seeds); level rect := C[row].
   - If the level is a preset level (DrlgType 2): level 1 → its preset
     direction (preset info +0x04) := R0[row]; level 40 → := R0[row + 1].
   - If the level is 6 (Black Marsh): get-or-allocate level 27 (Outer
     Cloister); if R0[row] = 1: **step the DRLG seed itself** (site
     `0x006774DB`), Outer Cloister direction := 2 − (lo' & 1); if R0[row] =
     3: step (site `0x006774C0`), direction := 1 − (lo' & 1). Other values:
     no draw.
   - Flags function (Act I only, §2.6) with (level, row, R0).
   - Unless act V: if the row has a link L, set the warp slots both ways
     between this level and row L's level via `0x00642920`
     (`levels.md` §7.3; requested slot −1, warp −1); same for the second
     link column (always −1 in 1.14d tables).

#### 2.4 Linkers

Each linker is called with the driver state; "first call" means R1[i] =
−1. "Place A/B/C(parent, child, case, variant)" are §2.5. Parent = C[link].
All draws are on the driver's seed copy.

| Name | 1.14d | First call (draws in order) | Retry | Placement |
|---|---|---|---|---|
| Def | `0x006760F0` | none; R0 := −1 | same, always true | x, y := leveldefs `OffsetX/OffsetY` |
| R4 | `0x00676150` | R1 := R0 := `lo' & 3` (site `0x00676165`) | if (R0+1) mod 4 = R1: false; else R0 := (R0+1) mod 4 | Place A(R0, 1) |
| R8 | `0x00676280` | R1 := R0 := `lo' & 7` | if (R0+1) mod 8 = R1: false; else R0 := (R0+1) mod 8 | Place C(R0, 1) |
| BM | `0x00676650` | R1 := R0 := `lo' & 3` (`0x00676669`), then R3 := `lo' & 1` (`0x0067669F`); t := R3 | t := (R2+1) mod 2, r := (R2+R0) mod 4; if r = R1 and t = R3: false; else R0 := r | R2 := t; w, h := (96, 56) if R0 odd else (56, 96); R2 = 1: Place A(R0, 1), else Place B(R0, 1) |
| RE | `0x00676450` | as BM (sites `0x00676469`, `0x0067649F`) | as BM | R2 := t; no size change; R2 = 1: Place A(R0, 2), else Place B(R0, 2) |
| Fix | `0x006768C0` | R1 := R0 := 0 | same, always true | Place A(0, 0) |
| RW | `0x006769A0` | R1 := R0 := (`lo' & 1`) + 1 | r := 2 if R0 = 1 else 1; if r = R1: false; else R0 := r | R0 = 1: Place A(1, 0); else Place B(R0, 0) |
| VS | `0x00676AE0` | R1 := R0 := `lo' & 7` | as R8 | Place C(R0, 0) |
| OS | `0x00676C00` | R1 := R0 := 3; one step: `lo' & 1` = 0 → Place B(3, 3) and transition flag 0x400000, else Place A(3, 3) and flag 0x800000 | same (draws again) | — |
| B1 | `0x0067D830` | R0 := R1 := `lo' & 1`; w, h := (64, 160) if 0, (160, 64) if 1 | draws again | x := parent.x − w; y := parent.y + parent.h − h − 16 |
| BD | `0x0067D8E0` | as B1's draw and size | draws again | x, y := leveldefs offset |
| B2 | `0x0067D980` | R1 := R0 := `lo' & 1` | r := (R0 = 0); if r = R1: false; else R0 := r | size as B1 from R0; offset table [(0, −160), (−96, −64), (−64, −96), (−160, 0)] at index R0 + 2·R0[link]: x, y := parent x, y + offset |

The bodies of the linkers at `0x00676150`–`0x00676C00` are outside the
function list of the export (no Ghidra function); the table above is from
D2MOO, confirmed by the table pointers (`0x006F0750`…), the recorded draw
sites and values (§Test vectors), and the recorded grid size of Blood Moor
(area 50 = 10×5, i.e. 96×56 or 56×96).

#### 2.5 Placement cases (child c next to parent p)

| Case | Place A (`0x00675DE0`) | Place B (`0x00675EB0`) |
|---|---|---|
| 0 | (p.x, p.y + p.h); v1: x −= 16 | (p.x + p.w − c.w, p.y + p.h); v1: x += 16 |
| 1 | (p.x − c.w, p.y); v1: y −= 16; v2: y += 8 | (p.x − c.w, p.y + p.h − c.h); v1: y += 16; v2: y −= 8 |
| 2 | (p.x + p.w − c.w, p.y − c.h); v1: x += 16 | (p.x, p.y − c.h); v1: x −= 16 |
| 3 | (p.x + p.w, p.y + p.h − c.h); v1: y += 16; v2: y −= 8; v3: y += 8 | (p.x + p.w, p.y); v1: y −= 16; v2: y += 8; v3: y −= 8 |

Place C (`0x00675F80`, Act II, 8 cases; "v1" applies only with variant 1;
half = size/2 + 8): 0 (p.x, p.y + p.h), x −= c.w/2 + 8; 1 same origin,
x += c.w/2 + 8; 2 (p.x − c.w, p.y), y −= c.h/2 + 8; 3 same, y += …; 4
(p.x, p.y − c.h), x −= …; 5 same, x += …; 6 (p.x + p.w, p.y), y −= …; 7
same, y += ….

#### 2.6 Checks and Act I border flags

Gap test (`0x0066B800` via `0x0066B860`, margin 0): gx := b.x − a.w − a.x
if a.x < b.x else a.x − b.w − b.x; gy likewise; "not overlapping" ⇔ gx ≥ 0
or gy ≥ 0 (touching is allowed).

| Check | 1.14d | Rule |
|---|---|---|
| A1W | `0x00676DD0` | every earlier row j ≠ link must not overlap. Rogue Encampment: additionally `T[R0[i] + 4·(R2[i] + 2·(R0[l] + 4·R2[l]))]` must be 1 (l = link, T = 64 entries at `0x006F1158`, ones at 0, 1, 9, 19, 21, 24, 30, 31, 33, 39, 41, 53, 54, 56, 62, 63). Burial Grounds: no other of the 15 table rows with the same link may have the same R0 |
| A1M | `0x00676EB0` | rows j < i, j ≠ link, must not overlap; then for i > 0 the row-0 rect extended 200 upward (y −= 200, h += 200) must not overlap row i |
| A2, A2C, A4, A4C, A5U | `0x00676F50`, `0x00676FC0`, `0x006770A0`, `0x00677110`, `0x00677030` | rows j < i, j ≠ link, must not overlap |

**Flags function** (`0x00677180`, Act I, outdoor levels only): for each of
15 rows `(level filter, excl1, excl2, r, rNext, flags)` at `0x006F1258`
(filter 0 = any level): if the level matches the filter, is neither excl1
nor excl2, R0[row] = r and R0[row + 1] = rNext: outdoor flags |= flags.

| Filter | Excl | r, rNext → flags |
|---|---|---|
| any | 2, 3 | (1, 0) → 0x4; (2, 3) → 0x4 |
| any | 3, 17 | (2, 1) → 0x8; (3, 0) → 0x8; (1, 1) → 0x10; (3, 3) → 0x10 |
| 2 | — | (0, 0), (2, 2), (3, 0), (3, 2) → 0x8; (0, 1), (1, 1) → 0x400; (2, 1) → 0x200; (2, 2) → 0x80; (3, 2) → 0x100 |

#### 2.7 Warps and neighbour entries

- **Adjacency warps** (`0x006775C0`, ids a..b): for i = a..b (outer,
  ascending): get-or-allocate level i (`levels.md` §4.2) and get or
  create its warp record (`0x00642860`, `levels.md` §7.2); then for j =
  a..b (inner, ascending), j ≠ i: get-or-allocate level j; if the rects
  share an edge (`0x0066B880`, margin −1: one gap is 0 and the other
  ≤ −1), set warp in level i's record toward j (`0x00642920`, slot −1,
  warp −1). So an unallocated id in a..b is allocated during i = a, in
  ascending order (level seeds are observable, `levels.md` §4.3).
- **Neighbour entries** (`0x00677680`, ids a..b): for each id in a..b
  (ascending): **get-or-allocate** the level first (`0x00642BB0`), then
  test its DrlgType (level +0x00) = 3 (outdoor); other types: nothing
  more. For an outdoor level, for vis slot j = 0..7 with vis ≠ 0 and
  warp id = −1 (from the DRLG's vis/warp arrays, `levels.md` §7.2):
  **get-or-allocate** the neighbour level vis[j] (through the level's
  DRLG, level +0x1B4) and add a neighbour entry to this level's outdoor
  info (outdoor +0x264, `0x0066B790`) with direction := direction from
  this level's rect to the neighbour's (`0x00642240`, `maze.md` §2 rule 6),
  preset flag := the neighbour's DrlgType is 2. These entries drive the
  vertex polygon (§4). Calls per act: §2.1 (Act I 1..17, Act II
  40..46, Act III 75..83, Act IV 103..106, Act V 111..112).

### 3. Level generation (`0x00675360`, D2MOO `DRLGOUTDOORS_GenerateLevel`)

Called by `levels.md` §5 after the level seed is re-set.

1. Outdoor +0x54, +0x58 := 0; gw := width/8, gh := height/8; allocate
   grids 0..3 (gw × gh, zeroed, `0x0067CB80`).
2. Vertex polygon (§4), then divide every vertex coordinate by 8 and
   merge consecutive equal vertices (`0x00675080`: the earlier keeps its
   place, ORs the later's flags, takes the later's direction).
3. Act dispatch (act of the level id, level 134 counts as Act II):
   I `0x006807F0` (§7), II `0x0067F980` (§8), III `0x0067F450` (§9), IV
   `0x0067E890` (§10), V `0x0067E600` (§11).
4. Rooms (`0x006750F0`, §12).

### 4. Vertex polygon (`0x0067D050`, `0x0067CE20`; D2MOO `DRLGVER_CreateVertices`)

Temporarily shrink the level rect and every neighbour box by 1 in w and
h. Corners V0 (x, y+h), V1 (x, y), V2 (x+w, y), V3 (x+w, y+h) form the
circular list V0→V1→V2→V3→V0 (edges W, N, E, S). For each neighbour entry
in list order (box = its rect, or the level rect if it has none), by its
direction:

| Dir | corner c | axis | s | p (first) | q (second) | a (edge start) | b (edge end) |
|---|---|---|---|---|---|---|---|
| 0 W | V0 | y | −1 | box.y + box.h | box.y | V0.y | V1.y |
| 1 N | V1 | x | +1 | box.x | box.x + box.w | V1.x | V2.x |
| 2 E | V2 | y | +1 | box.y | box.y + box.h | V2.y | V3.y |
| 3 S | V3 | x | −1 | box.x + box.w | box.x | V3.x | V0.x |

If s·p > s·a: if s·p ≤ s·b: insert vertex u at coordinate p (other
coordinate from c) right after c, flag u link (and preset-link if the
neighbour is a preset level); then if s·q < s·b insert a vertex at q
right after u. Else (s·p ≤ s·a): if s·q ≥ s·a: flag c link (and preset)
and, if s·q < s·b, insert a vertex at q right after c. Other cases add
nothing. Insertions always follow the corner (or the just-inserted u), so
two neighbours on one edge end up in reverse processing order. An
unknown direction is fatal. Then subtract the level origin from every
vertex and restore the sizes. All vertices get direction 0.

### 5. Preset primitives on the grids

#### 5.1 Stamp a preset (`0x006743C0`, D2MOO `…SpawnOutdoorLevelPresetEx`)

Arguments: cell (x, y), lvlprest id P, file F (−1 = from build list),
border flag. Size in cells: lvlprest SizeX/8 × SizeY/8.

1. If F = −1: build list (`0x00674320`): find the node for P; if absent,
   append a node {P, n := lvlprest `Files`, r := **`roll(n)` on the level
   seed** (site `0x0067438F`)} at the head. Then r := (r + 1) mod n
   (signed; n = 0 divides by zero — never reached by 1.14d data paths);
   F := r. So the first stamp of P uses file (roll + 1) mod n and later
   stamps cycle.
2. For every covered cell (rows y.., columns x..): grid 2 AND-NOT 0xF0000,
   OR (0x200 | F << 16); if the border flag is set and P is in 4..15
   (Act 1 wild borders) or 364..375 (Act 2 desert borders), OR 0x1;
   grid 0 := 0.
3. Grid 0 at (x, y) := P.

#### 5.2 Tests

Preset fits (`0x00674230`; P = 0 means 1×1): start (x, y), size
(SizeX/8, SizeY/8); with margin m ≠ 0 and flag bits: 1: y −= m, h += m;
2: w += m; 4: h += m; 8: x −= m, w += m. Every cell of the rect must be
inside grid 2 and spawn valid (§1.2).

#### 5.3 Shuffled cell list

Many placers use the same list: W := gw − 2, H := gh − 2, A := W·H. If A
= 0: nothing (no draws). Entry k := (k mod W, k div W). Then for k in
0..A−1: a := **roll(A)**, b := **roll(A)** (level seed, in this order),
swap entries a and b. Candidate cell of entry k is (x + 1, y + 1). The
first roll has two sites per function (power-of-two and modulo branch),
the second one site (§Test vectors).

#### 5.4 Placers

| Placer | 1.14d | Draws (level seed, in order) | Rule |
|---|---|---|---|
| SpawnOutdoorLevelPreset(P, F, m, flags) | `0x00674730` | §5.3 shuffle | first candidate passing §5.2 → stamp (§5.1); returns found |
| SpawnRandomDS1(P, F) | `0x00674920` | §5.3 shuffle | first candidate whose grid-2 has 0x80 (path): try its 8 neighbours in order dx = [−1, 0, 0, 1, −1, 1, 1, −1], dy = [0, −1, 1, 0, −1, 1, −1, 1] with §5.2 (m 0, flags 15); first fit → stamp. None: SpawnOutdoorLevelPreset(P, F, 0, 15) (a second shuffle) |
| FarAway(rect, P, F, m, flags) | `0x006744F0` | rx := roll(gw − 2), ry := roll(gh − 2) | W, H as §5.3; centre (rect.x + rect.w/2, rect.y + rect.h/2); for i in 0..H (inclusive), j in 0..W (inclusive): cell ((j + rx) mod W + 1, (i + ry) mod H + 1); if it fits: ax := \|8x − cx + level.x + 4\|, ay likewise; d := (ax > ay ? ay + 2ax : ax + 2ay) / 2; keep the first strictly larger d. Stamp the best (no draw if none) |
| Waypoint | `0x00674B70` | Cold Plains link found: none; else §5.3 shuffle | level 3: i := first vis slot of level 3 holding 2 (none → i = 8), mask := 1 << (i + 4); scan rows y, then columns x, for grid-1 & mask and grid-2 & 0x400; clamp x to 1..gw−2, y to 1..gh−2; grid 1 |= 0x20000, grid 2 |= 0x800; done (no shuffle); not found → general case. General: first spawn-valid candidate: grid 1 |= 0x10000, grid 2 |= 0x800 |
| Shrines(n) | `0x00674E40` | k := `lo' & 3` (site `0x00674E59`, always), then §5.3 shuffle | for candidates in order while n > 0: if spawn valid: grid 1 |= [0x1000, 0x2000, 0x4000, 0x8000][k], grid 2 |= 0x1000, k := (k+1) mod 4, n −= 1 |

#### 5.5 Link flags (`0x00675770`) and link vis flag (`0x00674040`)

For each polygon vertex with flag link, mark the polygon edge from it to
the next vertex, both ends included (`0x0067C760`): grid 1 OR the link vis
flag, grid 2 OR (0x1 | 0x2 if the vertex direction ≠ 0). Link vis flag of
a vertex at cell (vx, vy): side index s := vx = 0 → (vy = 0 ? 1 : 0); vy
= 0 → (vx = gw−1 ? 2 : 1); vx = gw−1 → (vy = gh−1 ? 3 : 2); vy = gh−1 →
3; else flag 0. Probe point := (level.x + 8vx + dx[s], level.y + 8vy +
dy[s]) with (dx, dy) = (−4, 4), (4, −4), (12, 4), (4, 12). The first
neighbour entry with direction s whose box contains the probe
(`levels.md` §8.2): if its init flag is 0, flag := `1 << (j + 4)` for the
vis slot j of this level that holds the neighbour's id; else 0.

### 6. Borders (`0x00675850`, D2MOO `PlaceAct1245OutdoorBorders`)

Per polygon edge (vertex v → next n; dir (dx, dy) = sign of n − v; the
next edge's dir likewise):

1. Style s by level type: 2 (Act 1 wild) → 1 if v.direction = 0 else 0;
   16 (desert) → 2; 27 (mesa) → 3; 31 (barricade) → 4 + (level = 117);
   other → −1. Straight piece := Border(dx, dy, s).
2. If v is not a preset link (flag bit 1 clear): walk from v toward n one
   cell at a time (excluding v, including n): stamp the straight piece
   (§5.1, F = −1) and OR grid 2 with (0x1 | 0x2 if v.direction ≠ 0).
3. If v is a link and not a preset link, at the midpoint cell (min x +
   |dx|·L/2, min y + |dy|·L/2), L = edge length in cells:
   - Act I and V: grid 2 AND-NOT 0xF0000, OR 0x30400 (link, file 3); level
     17 uses 0x40400 (file 4).
   - Act IV: 0x30400 (no level-17 case).
   - Act II: stamp desert pair at the midpoint and the next cell along the
     edge: index dx + 2dy + 2 into [(373, 372), (372, 375), (0, 0),
     (373, 374), (374, 375)] (desert borders 10/9/12/11).
   - Act III: nothing.
4. Corner at n: d := v.direction, or n.direction if v's is 0 (grid bit 0x2
   := d ≠ 0); style s' as step 1 but from d. Corner piece := Corner(a, b,
   c, e, s') with a = dx, b = dy, c = next dx, e = next dy. The corner
   index is computed inline (`0x00675BB7`–`0x00675CDD`; same table
   reads as `0x00675600`): first (a, b) are doubled unless v is a
   preset link and (c, e) doubled unless n is one; **then** a and c
   (not b, e) grow by 2 in magnitude when non-zero; k := N[a + b +
   9(c + e) + 50]. Piece 19 (cliff 6A) becomes 20
   (6B) if v.direction = 1 and n.direction ≠ 1, 21 (6C) if v.direction ≠
   1; any nonzero piece is stamped at n (F = −1) and grid 2 ORed as in
   step 2.
5. After all edges: blank corners (`0x00675670`): from each grid corner
   ((0,0) step (+1,+1), (gw−1,0) (−1,+1), (0,gh−1) (+1,−1), (gw−1,gh−1)
   (−1,−1)) fill rows then columns with grid 2 |= 0x100 until a cell with
   bit 0x1 is met (the row loop stops at the first row whose start cell
   has bit 0x1).

Lookup (`0x006755C0`, `0x00675600`; table N = 91 ints at `0x006F0FC0`):
Border(dx, dy, s): k := N[dx + 3dy + 4]; s < 4: P[k + 1][s]; s ≥ 4:
Q[k][s − 4]. Corner(a, b, c, e, s): each argument with |value| > 0 grows by
2 in magnitude (a, c); k := N[b + a + 9(e + c) + 50]; k = −1 → 0; s < 4:
P[k][s], else Q[k − 1][s − 4].

N (index: value, −1 omitted): 1:1, 3:0, 5:2, 7:3, 9:0, 10:1, 11:9, 12:9,
15:1, 16:8, 19:12, 24:12, 25:4, 28:5, 29:2, 30:2, 31:10, 36:10, 37:1,
38:9, 39:9, 61:11, 62:11, 63:3, 64:12, 69:12, 70:4, 71:4, 72:7, 75:2,
76:10, 81:10, 84:6, 85:3, 88:11, 89:11, 90:3.

P (`0x006F0620`, rows 1..12 × [cliff, wild, desert, mesa]; row 0 is not
reachable): 1 (0, 4, 364, 799), 2 (16, 5, 365, 800), 3 (17, 6, 366, 801),
4 (0, 7, 367, 802), 5 (18, 8, 368, 803), 6 (19, 9, 369, 804), 7 (22, 10,
370, 805), 8 (0, 11, 371, 806), 9 (0, 12, 372, 807), 10 (23, 13, 373,
808), 11 (0, 14, 374, 809), 12 (0, 15, 375, 810).
Q (`0x006F06F0`, rows 0..11 × [barricade, snow]): (881+k, 957+k).

### 7. Act I (`0x006807F0`, D2MOO `OutWild`)

Level-id ranges: 2..7 = Blood Moor..Tamoe Highland; 39 Moo Moo Farm.

1. If the level is not 2, 3 or 17: cliff marking (`0x00680070`, §7.1).
2. Link flags (§5.5); borders (§6).
3. Levels 2..7: border substitution (`outdoor-tilesub.md` §2) with lvlsub
   type 0, base preset 4; river/cliff block (§7.2); substitution types
   1 and 2; transitions and caves (§7.3); substitution type 3; dirt paths
   (§7.5).
4. Level 39: substitution types 0, 1, 2, 3.
5. Levels 3..6: waypoint (§5.4). Levels 2..7: shrines(5) (§5.4).
6. Special presets (§7.4).

#### 7.1 Cliff marking (`0x00680070`)

Outer walk: v := polygon head, p := its predecessor. v starts a run if
neither v nor p is a link and (v.x < n.x and p.y > v.y) or (v.y > n.y and
p.x > v.x) (n = v.next). A run walks w from v: if w is the head, note
"head passed"; if w is a stop vertex, end the run at w; if w is a turn
vertex, remember it as u; w := w.next; the run also ends on returning to
v. Stop vertex: w.y < w.next.y, or w.x > w.next.x, or w or w.next is a
link. Turn vertex: neither w nor w.next is a link and (w.x < w.next.x and
w.next.y < w.next.next.y, or w.y > w.next.y and w.next.x <
w.next.next.x). If u was found: direction := 1 for every vertex from v
to u inclusive; outdoor flags |= 0x20. Then p := the vertex where the
run ended (or v if none started), v := p.next; the outer walk ends when
"head passed" is set or v is the head again.

#### 7.2 River, cliff caves, side cave (`0x00680200`; not level 39)

1. If flags & 0xC and no row y has grid-2 bit 0x2 at (gw−2, y) or (gw−1,
   y) (`0x0067FC70`): river at x = gw−2 (§7.6).
2. If flags & 0x20 and not 0x40: one step, bit := `lo' & 1` (site
   `0x00680251`). bit 0: scan rows y (outer) and columns x (inner); bit 1:
   outer index over gh, inner over gw, used as (x = outer, y = inner).
   First cell whose grid 0 is 16 → stamp 25 (cliff cave left), 17 → 24
   (cliff cave right) (F = −1), flags |= 0x40, stop. None: warning only.
3. If flags & 0x1C and not 0x40: y := gh − 4, x := gw − 4 if flags & 0x10
   else gw − 5; r := `lo' & 3` (site `0x0068034F`); r odd → x := 3; r ≥ 2 →
   y := 3; stamp 52 (DOE entrance) if level 2 else 51 (cave entrance) at
   (x, y), F = −1; flags |= 0x40.

#### 7.3 Transitions and caves (`0x006803D0`; not level 39)

1. Flags & 0x10: x := gw/2 − 1; if no row y in 0..gh−1 has a direction
   bit at (x, y) or (x+1, y), river at x (§7.6). (gh ≤ 0: river.)
2. Flags 0x80: stamp 3 (town transition S) at (0, 0), F 1; 0x100: 3 at
   (gw−7, 0), F 2; 0x200: 2 (transition E) at (0, 1), F 1; 0x400: 2 at
   (0, gh−6), F 1.
3. If not 0x40: level 2 → FarAway(rect of level 1, 52, −1, 1, 15); else
   SpawnOutdoorLevelPreset(51, −1, 1, 15). Not found: warning. Flags |= 0x40.

#### 7.4 Special presets (`0x00680580`)

"Cottage(P, extra)" (`0x006804E0`): one step `lo' & 3` (site
`0x006804ED`); nonzero: RandomDS1(P); if extra, one step `lo' & 1` (site
`0x0068052C`), nonzero → RandomDS1(49 Cottages 3). Zero: RandomDS1(P)
twice. "R" = RandomDS1(…, −1); "S" = SpawnOutdoorLevelPreset(…, −1, 0, 15).

| Level | Sequence (lvlprest ids) |
|---|---|
| 2 Blood Moor | R 46; Cottage(47, no); S 29; S 30 |
| 3 Cold Plains | Cottage(48, yes); S 44; S 29; S 30 |
| 4 Stony Field | R 160; R 45; S 162; Cottage(47, yes); Cottage(42, no); S 31 |
| 5 Dark Wood | S 161; S 41; S 40; Cottage(48, yes); Cottage(43, no); S 29; S 30 |
| 6 Black Marsh | S 163; S 38; S 39; Cottage(47, yes); Cottage(42, no); S 29; S 30 |
| 7 Tamoe Highland | Cottage(48, yes); Cottage(43, no); S 31 |
| 17 Burial Grounds | stamp 108 at (1, 1), F −1 |
| 39 Moo Moo Farm | S 50; S 46; S 31; S 38; S 39; S 29; S 30 |

"Cottage(42/43, no)" reproduces D2MOO's "one more fallen camp with
probability 1/4, then one": zero → two, nonzero → one.

#### 7.5 Dirt paths (`0x00681420`)

1. **Starts** (`0x00680D70`): count := 0. For each neighbour entry: level
   1 (Rogue Encampment, at rect R): start := by entry direction 0 (R.x+59,
   R.y+19), 1 (R.x+29, R.y+35), 2 (R.x+4, R.y+22), 3 (R.x+29, R.y+3),
   direction := entry direction; level 26: (R.x+27, R.y+13), direction 1.
   Then for x in 0..gw−1 (outer), y in 0..gh−1: start (level.x + 8x + 3,
   level.y + 8y + 3), direction by grid 0 = P and picked file f: P 4/5/6/7
   with f = 3 → 3/0/1/2; 24 → 1; 25 → 0; 28 with f = 1 and x = gw−2 → 2;
   51, 52 → (f ≠ 0); else no start.
2. Start-adjusted point [6+i] (`0x00680CC0`): relative q := start −
   level origin; direction 0: q.x := 8(q.x/8) + 11; 1: q.y := 8(q.y/8) +
   11; 2: q.x := 8(q.x/8) − 5; 3: q.y := 8(q.y/8) − 5; 4: unchanged; add
   the origin back.
3. **Join point** (`0x00681000`): if flags & 0x10 and a bridge cell exists
   (`0x0067FB90` at x := gw/2 − 1: first y in 1..gw−2 with grid 0 = 28
   and file 1): bx := level.x + 8x + 3, by likewise; join[i] := (bx, by),
   direction 2 if start[i].x ≤ bx else (bx + 8, by), direction 0. Else:
   centre := (gw/2, gh/2) if count = 1, else (Σ(start.x − level.x) /
   (8·count), same for y); for r in 0..7, for k in 0..3: cell (cx +
   r·dxk, cy + r·dyk), (dx, dy) = (−1,0), (0,1), (0,−1), (1,0); stop at the
   first in-grid spawn-valid cell (if none, the last tried cell is used);
   join[0] := (level.x + 8x + 3, level.y + 8y + 3), direction 4, and every
   other join[i] copies it. Join-adjusted [12+i] as step 2.
4. For each i: grid path (§7.5.1); if found: OR 0x80 into grid 2 along
   its cells (in-grid only, `0x0067C890`), then jitter (§7.5.2).

**7.5.1 Grid path** (`0x006817D0`, search `0x00681630`; no draws).
A := (start-adjusted[i] − origin)/8, B := (join-adjusted[i] − origin)/8
(cells). If |Ax−Bx| + |Ay−By| < 2: path list := [A, B]; done. Else a
depth-first search. h(p) := min(|px−Bx|, |py−By|) + 2·max(…). Order rows
(`0x006F2840`): r0 = [0,1,2,3], r1 = [0,1,1,1], r2 = [3,2,1,2], r3 =
[0,3,2,1]; steps X = [1, 0, −1, 0], Y = [0, 1, 0, −1]. A node holds cell,
g, try count, an order row with a position in it, and a facing.
Root: cell A, g 0, tries −1, row r0 at position 0, facing (Dir(A, B)/2)
& 3. Budget := h(A) + h(A)/2. Rounds: run the search from a fresh root;
on failure budget += 5 and retry while budget < h(A) + h(A)/2 + 35; a
round that allocated 900 nodes ends the whole search with failure.
Search step at the current node (until its cell is B): c := cell +
step[facing]. c is acceptable if c = B, or c lies in (0, 0, gw, gh), has
no grid-2 bit 0x200, and is not on the chain from the current node back
to the root. If acceptable: g' := g + 2; f := g' + h(c); if f ≤ budget:
the child (reusing the node's previous child slot, else a new node) gets
cell c, g', tries 0, row r[(facing_parent − Dir(c, B)/2) & 3] at position
0, facing (Dir(c, B)/2 + row[0]) & 3, and becomes current. Otherwise
("advance"): if tries < 4, move to the next row position and facing :=
(facing + row[pos]) & 3; tries += 1; when tries reaches 3, climb to the
parent and advance it the same way (repeating while its tries reach 3);
reaching the root this way fails the round.
Output: walk from the node at B through parents to the root, appending a
vertex per node: path list i = B … A (join side first).
Dir(p, q) (`0x00678CF0`, via `0x00678B80`): d := q − p; if |dx| ≥ 2|dy|:
dy := −1 if dy < 0 else dy & 1; else if |dy| ≥ 2|dx|: dx := −1 if dx < 0
else dx & 1; clamp both to −2..2; Dir := T[5dx + dy + 12], T (25 entries,
12-byte stride at `0x006F1518`) = 5, 4, 4, 4, 3, 6, 5, 4, 3, 2, 6, 6, 6,
2, 2, 6, 7, 0, 1, 2, 7, 0, 0, 0, 1. (The `& 1` for a non-negative minor
axis is the original's parity quirk; reproduce.)
**7.5.2 Jitter** (`0x00681240`): k := `lo' & 3` (site `0x0068126D`,
always, even without a path). If join[i].direction ≠ 4, prepend a vertex
at join[i]. The first vertex of the grid path := join-adjusted[i]. For each following
vertex that has a successor: ox := ((`lo' & 1`) + 2)·X'[k] (site
`0x00681310`), oy := ((`lo' & 1`) + 2)·Y'[k] (site `0x0068134E`), k := (k
+ 1) mod 4, X' = [1, 0, −1, 0], Y' = [0, 1, 0, −1]; vertex := (8x +
level.x + ox + 3, 8y + level.y + oy + 3). The last vertex := start-
adjusted[i], and a vertex at start[i] is appended.

**7.5.3 Per room** (`0x00680C80` → `0x00680A70`, `0x00680B10`; no draws):
when an Act I outdoor room's grids are built (§12.2), a (w+3)×(h+3) path
grid over the room's tiles (origin room − 1) gets every path segment
drawn 2 cells thick (Bresenham-like, `0x0067C8E0`); then (`0x00680B10`)
for path-grid columns X = 1..w+1 (outer) and rows Y = h+1 down to 1
(inner), a cell G(X, Y) ≠ 0 gets the 8-neighbour mask, most significant
bit first: b7 G(X+1, Y−1), b6 G(X+1, Y), b5 G(X+1, Y+1), b4 G(X, Y−1),
b3 G(X, Y+1), b2 G(X−1, Y−1), b1 G(X−1, Y), b0 G(X−1, Y+1) (each bit =
cell ≠ 0). Mask 0 → nothing; else s := byte `0x006F2700`[mask]
(`drlg/outdoor-path-floor.tsv`, 256 rows, values 0..46, 240 non-zero);
s ≠ 0 → floor cell (X − 1, Y − 1) of the room := (s << 8) | 0x82
(overwrite, grid op 3). Reads come from the path grid only, so the visit
order does not change the result. (Earlier text named `0x006F2860`,
which holds ASCII text.)

#### 7.6 River (`0x0067FE90`) and bridge (`0x0067FD20`)

For y in 0..gh−1: upper half at (x, y): grid 0 = P, file f: P ≠ 0: file
:= 3 if P = 7 and f = 3, else U[P]; P = 0: file := 0 if grid 2 has 0x100
else 3; stamp 26 (river upper). Lower half at (x+1, y) likewise with L[P]
(P = 7, f = 3 → 3); stamp 27. U/L for P = 4..15: (2,2), (0,3), (1,1), (3,0),
(0,2), (0,1), (1,0), (2,0), (2,3), (1,3), (3,1), (3,2) (`0x006F26A0`).
Then if flags & 0x14: R := gh − 2; r := roll(R) (sites `0x0067FD58` /
`0x0067FD84`); for i in 0..R−1: y := (r + i) mod R + 1; require spawn
valid at (x−1, y) and, unless flags & 0x4, at (x+2, y); and files 3 at
(x, y) and (x+1, y): stamp 28 (bridge) at (x, y) file 1 and at (x+1, y)
file 3 if flags & 0x4 else 2; stop.

### 8. Act II (`0x0067F980`, D2MOO `OutDesr`)

Link flags; borders. Then by level ("PB" = border substitution types 2, 1,
3 with base 364, `0x0067F630`; "Exit" = SpawnOutdoorLevelPreset(E, −1, 0,
15) with E = 388 for 41/42, 390 for 43, 412 for 44, 389 for 45, fatal if
not placed; "V(list, iter)" = §8.1; "WP" = waypoint; "SH" = shrines(5)):

| Level | Sequence |
|---|---|
| 41 Rocky Waste | town transition (§8.2); PB; Exit; SH; V([395, 411, 401, 402, 399, 398, 403], no) |
| 42 Dry Hills | cliffs (§8.3); PB; Exit; WP; SH; V([395, 411, 400, 398], no); V([404, 405, 406, 407], yes) |
| 43 Far Oasis | cliffs; PB; Exit; V([396, 397], no); WP; SH; V([411, 399, 398, 403], no); V([395], yes) twice |
| 44 Lost City | cliffs; PB; Exit; V([413, 408, 409, 410], no); WP; SH; V([395, 400, 398, 404, 405], no); V([411], yes) twice |
| 45 Valley of Snakes | Exit |
| 46 Canyon of the Magi | tomb row (§8.4); PB; SH; V([401, 402, 406, 407, 403], no); V([392, 393], yes) |
| 134 (Pandemonium 2) | stamp 394 at (4, 4), F −1; PB; V([401, 402, 406, 407, 403], no); V([392, 393], yes); SH |

8.1 **Variants** (`0x0067F470`): r := roll(n) (n = list length); for n
times: P := list[r]; iter: for f in 0..Files(P)−1 S(P, f); else S(P, −1);
r := (r + 1) mod n. (S = SpawnOutdoorLevelPreset with m 0, flags 15.)
8.2 **Town transition** (`0x0067F560`): first neighbour entry whose level
is 40: direction 3 → stamp 363 at (0, gh−1); else stamp 362 at (gw−1, 0);
F −1. None: nothing.
8.3 **Cliffs** (`0x0067F5C0`): r := `lo' & 7` (one level-seed step);
stamp the 5 entries of row r of `0x006F2390` in entry order, each (P, F,
x, y) (§5.1; F −1 takes the build-list file, so the first stamp of each
P in the level draws `roll(Files)`):

| r | entries in stamp order (P, F, x, y) |
|---|---|
| 0 | (376,1,0,4) (378,−1,2,4) (377,−1,4,4) (377,−1,6,4) (376,2,8,4) |
| 1 | (376,1,0,4) (377,−1,2,4) (378,−1,4,4) (377,−1,6,4) (376,2,8,4) |
| 2 | (376,1,0,4) (377,−1,2,4) (377,−1,4,4) (378,−1,6,4) (376,2,8,4) |
| 3 | (376,2,8,4) (377,−1,6,4) (382,−1,4,4) (381,−1,4,6) (379,2,4,8) |
| 4 | (376,2,8,4) (378,−1,6,4) (382,−1,4,4) (380,−1,4,6) (379,2,4,8) |
| 5 | (379,1,4,0) (381,−1,4,2) (380,−1,4,4) (380,−1,4,6) (379,2,4,8) |
| 6 | (379,1,4,0) (380,−1,4,2) (381,−1,4,4) (380,−1,4,6) (379,2,4,8) |
| 7 | (379,1,4,0) (380,−1,4,2) (380,−1,4,4) (381,−1,4,6) (379,2,4,8) |

(Read from the file image; arguments mapped at `0x0067F600`–`0x0067F612`:
entry +0 = P, +4 = F, +8 = x, +12 = y.)
8.4 **Tomb row** (`0x0067F8D0`): stamp (384,0,8,0), (383,2,6,0),
(383,1,4,0), (383,0,2,0), (387,0,0,0), (385,0,0,2), (385,1,0,4),
(385,2,0,6), (386,0,0,8); then 394 at (4, 4), F −1.

### 9. Act III

#### 9.1 Jungle placer (`0x00677880`; DRLG seed itself)

Owner: `drlg/outdoor-act3-act5.md` §2 (placement of levels 76..78,
river and attach-point block grid, block ids, every draw).

#### 9.2 Kurast chain (`0x00678910`)

Anchor: the level the jungle placer returns, **level 78** (Flayer
Jungle; `0x006789B0` passes it on). For ids 79..83 in order: y −=
SizeY(id); level x := 78.x + 78.w/2 − SizeX/2, y := 78.y + y, w, h :=
leveldefs size (allocation order 79..83). Then adjacency warps and
neighbour entries 75..83 (§2.7).

#### 9.3 Level build (`0x0067F450`)

Link flags (§5.5); jungle stamping (`0x0067E910`, levels 76..78; owner
`drlg/outdoor-act3-act5.md` §3); Kurast (`0x0067F190`, §9.4);
Travincal (`0x0067F3B0`, §9.4).

#### 9.4 Kurast and Travincal stamps

Read from the 1.14d calls (stamp arguments: x in EDX, then y, P, F,
border; all stamps here have border 0). gw := w/8, gh := h/8 (level
size in cells, truncated toward zero); J := the jungle-link bit (drlg
+0x474, `drlg/levels.md` §1). "Stamp P at (x, y), F" is §5.1; "S(P, F)"
is SpawnOutdoorLevelPreset(P, F, m 0, flags 15) (§5.4); "R(lo, hi,
max)" is the random preset placer below.

**Border rows** (levels 79–81 only, before anything else; all F −1):

| Level | Top row y = 0 | Bottom row y = gh−1 | Sides, for i = 1..gh−2 | Corners (0,0), (gw−1,0), (0,gh−1), (gw−1,gh−1) |
|---|---|---|---|---|
| 79 Lower Kurast (`0x0067EAD0`) | i = 1..gw−2: 605, or 613 at i = T (T = 1 if J else gw−2) | i from 1 while i < gw−1: 606, or 614 at i = (gw−1)/2, which also skips the next i | (gw−1, i) 607, then (0, i) 608 | 610, 609, 612, 611 |
| 80 Kurast Bazaar (`0x0067EC30`) | for i = 1..gw−2, first (i, 0) 619, or 627 at i = B; then (i, gh−1) 620, or 628 at i = A; A = gw−2, B = 1 if J = 0, A = 1, B = gw−2 if J ≠ 0 | (with the top row) | (gw−1, i) 621, then (0, i) 622 | 624, 623, 626, 625 |
| 81 Upper Kurast (`0x0067ED70`) | i from 1 while i < gw−1: 636, or 644 at i = (gw−1)/2, which also skips the next i | i = 1..gw−2: 637, or 645 at i = T (T = gw−2 if J else 1) | (gw−1, i) 638, then (0, i) 639 | 641, 640, 643, 642 |

Halves truncate toward zero. Each level stamps in the table's column
order: the whole top row, then the whole bottom row (level 80: top and
bottom alternate per i, top first), then the sides (for each i, east
(gw−1, i) before west (0, i)), then the corners in the order given
(confirmed call by call in `0x0067EAD0`, `0x0067EC30`, `0x0067ED70`;
every stamp F −1, border 0). The first stamp of each id draws its
build-list roll (§5.1), so this order fixes the roll order.

**Fixed and random presets** (`0x0067F190`, after the border rows;
X := gw − 4, Y := gh − 4):

| Level | Sequence |
|---|---|
| 79 | S(631, 0); R(618, 618, 4); R(616, 617, none); R(615, 615, none) |
| 80 | stamp 629 at (3, 3) F 0; stamp 629 at (X, 3) F 1; S(630, 0); S(630, 1); S(631, 0); R(635, 635, 4); R(633, 634, none); R(632, 632, none) |
| 81 | stamp 646 at (3, Y) F 0; stamp 646 at (X, Y) F 1; S(647, 0); S(647, 1); S(631, 0); R(651, 651, 4); R(649, 650, none); R(648, 648, none) |
| 82 Kurast Causeway | stamp 652 at (0, 0) F 0 |
| 83 Travincal (`0x0067F3B0`) | stamp, all F −1: 653 at (0, 0), 654 at (2, 0), 655 at (6, 0), 656 at (0, 4), 657 at (2, 4), 658 at (6, 4) |

**Random preset placer** R(lo, hi, max) (`0x0067EED0`, level seed): n
:= hi − lo + 1; A := gw·gh (the full grid, unlike §5.3); A = 0 →
nothing. Entry k := (k mod gw, k div gw); A swaps of entries `roll(A)`,
`roll(A)` (in this order, as §5.3). Then for each entry in order: P :=
lo + `roll(n)` (drawn for every tried entry; n = 1 still steps; n ≤ 0
would give lo without a step); if the preset fits at the entry's cell
(§5.2, m 0, flags 15): stamp P at that same cell (no margin, no offset;
`0x0067F121`–`0x0067F14B` pass the entry's (x, y) to both the test and
the stamp), F −1, border 0; count += 1; stop when max > 0 and count ≥
max.

### 10. Act IV (`0x0067E890`)

Level 108: link flags; 25 stamps at (3(i mod 5), 3(i div 5)) from
`0x006F22C0` (836 lava, 857 entry, 858 arm W, 859 arm E, 860 arm S, 861
arm N, 862 heart at i = 22, 11, 13, 17, 7, 12). Levels 104..106: link
flags; borders; transitions (`0x0067E660`: flag 0x400000 → stamp 798 at
(0, 1); 0x800000 → 798 at (0, 4); F −1); border substitution types 1, 2,
3 with base 799; then (`0x0067E6A0`) S(811) if 106; with mesa base M =
812/817/823 and pit base T = 828/832/832 for 104/105/106: S(M), S(M+1)×2,
S(M+2)×2, S(M+3)×2, S(822) if 105, S(M+4)×4, S(T), S(T+1)×2, S(T+2)×2,
S(T+3)×4. (All S with F −1, m 0, flags 15.)

### 11. Act V (`0x0067E600`)

Level 110: siege strip (`0x0067E560`): s := SizeX(865)/8; for i in
0..14: x := gw − s·(i + 1); x < 0 is a fatal error; stamp 865 + i ("Siege
To Town", "Siege Strip 1..13", "Siege To Barricade") at (x, 0), F 0.

Other Act V outdoor levels, in this order (`0x0067E600`; gw, gh = grid
size, outdoor data +0x5C, +0x60; every stamp has border 0; "S(P, F)" =
SpawnOutdoorLevelPreset(P, F, m 0, flags 15)):

1. Link flags (§5.5).
2. **Barricade border walk** (`0x0067DCF0`), s := 4 + (level = 117),
   for each polygon edge v → n (the list from its head, circular):
   (dx, dy) := sign of n − v (`0x0067D280`), (dx', dy') likewise for n →
   n.next. (vx, vy) := v's cell with bit 0 cleared, (nx, ny) likewise.
   If v is not a preset link (flag 2): step (x, y) from (vx, vy) by
   (2dx, 2dy) until it equals (nx, ny), stamping Border(dx, dy, s)
   (§6) at each new (x, y), F −1, and grid 2 |= 0x1 there. If v has flag
   1 (link): x := (max(v.x, n.x) − 4|dx|) with bit 0 cleared, y :=
   (max(v.y, n.y) − 4|dy|) with bit 0 cleared; grid 2 |= 0x400 at (x, y)
   and at (x + 2|dx|, y + 2|dy|). Corner: `0x00675600` with a = 2dx, b
   = 2dy, c = 2dx', e = 2dy' (a and c then grow by 2 in magnitude, §6
   lookup); a non-zero piece is stamped at (nx, ny), F −1, grid 2 |= 0x1.
   After the walk, level 111 only: grid 2 |= 0x400 at (gw−2, gh−4) and
   (gw−2, gh−3).
   The straight walk has no step count: it tests (x, y) = (nx, ny)
   before every step (`0x0067DD92`–`0x0067DDCD`). It always ends: §4
   edges are axis-aligned, so only one coordinate moves; both ends have
   bit 0 cleared, so their difference is even and of the sign of n − v
   (or 0), and the walk reaches (nx, ny) after |Δ|/2 stamps (0 when the
   cleared ends are equal).
3. **Ravine walk** (`0x0067DEF0`): B := 881, B' := 893, ends 906 / 905
   (level 117: 957, 969, 982 / 981). (x, y) := (gw−2, 0); while (x, y)
   ≠ (0, gh−2): k := grid 0 (x, y) − B; stamp B' + k at (x, y), F −1;
   (x, y) += 2·D[k] with D (`0x006F1FD8`, k = 0..11) = (−1,0), (0,−1),
   (1,0), (0,1), (0,−1), (1,0), (0,1), (−1,0), (−1,0), (0,−1), (1,0),
   (0,1). Then stamp 906 at (gw−2, 0) and 905 at (0, gh−2), F −1.
   No check in 1.14d: k is not range-checked (D is read at `0x006F1FD8`
   + 8k for any k) and nothing stops a walk that never reaches
   (0, gh−2). Neither happens with 1.14d data: every cell the walk visits
   holds a step-2 barricade piece B + k with k = 0..11 (§6 table Q:
   barricade column 881 + k, snow 957 + k), and D[k] moves along that
   closed outline from the NE piece to the SW piece. An out-of-range k
   would stamp a non-row id (null lvlprest row, crash in §5.1) or loop
   forever; d2rs may report either as a fatal error.
4. **Entrances** (`0x0067DB50`; F −1, the code's F 1 branch is for
   level 110 only): the first x = 0..gw−1 with grid 2 (x, 0) & 0x400 →
   stamp 909 at (x, 0); the first x = 0..gw−1 with grid 2 (x, gh−2) &
   0x400 → 908 at (x, gh−2); the first y = 0..gh−1 with grid 2 (0, y) &
   0x400 → 910 at (0, y); the first y = 0..gh−1 with grid 2 (gw−2, y) &
   0x400 → 907 at (gw−2, y).
5. **Caves** (`0x0067DA70`, table `0x006F1F9C`, rows (level, F, side,
   P tall, P wide): (112, 0, 0, 913, 914), (117, 0, 1, 983, 984), (117,
   0, 0, 985, 986)): for each row of this level: if level w > h: stamp P
   wide at (side ? gw−2 : 0, 2); else P tall at (2, side ? gh−2 : 0); F
   from the row. w, h are the level's tile rect (level +0x24, +0x28,
   `0x0067DA8C`), not the grid; w = h takes the tall id here (never
   reached: the linkers give 64×160 or 160×64).
6. Level 111: **connect to siege** (`0x0067E4B0`): x := gw −
   SizeX(880)/8, y := gh − SizeY(880)/8 (lvlprest sizes); stamp 880 at
   (x, y) and 896 at (x, y − 2), F −1 (fatal if the level, its outdoor
   data or lvlprest 880 is missing).
7. Border substitution with lvlsub type 12 and the barricade callbacks
   (`0x0067E0E0`, `outdoor-tilesub.md` §2.3).
8. **Prisons** (`0x0067E240`, level 111 only): cell value V(x, y) :=
   grid 0 (x, y) when grid 2 (x, y) has 0x200, else 0 (`0x00674120`).
   Up to 90 tries while placed < 3: x := 2·roll(gw/2), y :=
   2·roll(gh/2) (level seed +0x1C4, helper `0x0045C390` = `roll` of
   `sim/rng.md` §3, read in full: n < 1 → 0 without a step, else one
   step, `lo' & (n−1)` for a power of two, else `lo' mod n`; x first); if
   915 ≤ V ≤ 922: stamp V + 16 at (x, y), F −1, placed += 1. Then sx :=
   roll(gw/2), sy := roll(gh/2) (inline, level seed, always drawn); for
   i = 0..gh−1 (outer), j = 0..gw−1 (inner), while placed < 3: x := (j
   + 2sx) mod gw, y := (i + 2sy) mod gh, same test and stamp. Fewer than
   3 placed: fatal (error 0x259).
9. **Special presets** (`0x0067E160`, 15 rows of 7 dwords at
   `0x006F2100`: level, P tall (used when level w < h), P wide, F, a
   dword this code does not read, count, fatal): for each row of this
   level, count times S(P, F); if the last call placed nothing and the
   row is fatal: fatal error 0x219. Tall/wide is chosen once from the level's
   tile rect (level +0x24 < +0x28 → tall, `0x0067E166`–`0x0067E172`; w =
   h takes the wide id, never reached). Rows: (111, 955, 956, F 0, ×1,
   fatal), (112, 955, 956, 0, ×1, fatal), (117, 955, 956, 1, ×1, fatal),
   (112, 953, 953, −1, ×1, fatal), (117, 954, 954, −1, ×1, fatal), (111,
   944, 947, −1, ×1), (111, 942, 945, −1, ×4), (111, 943, 946, −1, ×4),
   (112, 941, 941, −1, ×1), (112, 939, 939, −1, ×1), (112, 940, 940,
   −1, ×5), (117, 948, 948, −1, ×4), (117, 949, 949, −1, ×4), (117, 950,
   950, −1, ×4), (117, 951, 951, −1, ×3).

All of the above read from the 1.14d functions named (stamp arguments
mapped from the disassembly); D2MOO `OutSiege` was not needed.

### 12. Rooms

#### 12.1 Cell to room (`0x006750F0`)

DT1 mask by level type: 2 → 0x44103; 16, 22, 27, 28 → 0x1; 21 → 0x4; 30,
31 → 0x11; else 0. For y in 0..gh−1 (outer), x in 0..gw−1, tile origin
(level.x + 8x, level.y + 8y): f1 := grid 1, f2 := grid 2.
- f2 & 0x200: P := grid 0; if P ≠ 0: preset map (`0x00666ED0`,
  `preset.md`) with rect (origin, 0, 0) and **the level seed**: it draws
  roll(Files(P)) (site `0x00666F33`) even though the next step overrides
  the file; set the map's file := (f2 >> 16) & 0xF (`0x00666EC0`); build
  the area with room flags f1 (`0x00667ED0`, `preset.md`). P = 0: nothing.
- else if not f2 & 0x100: outdoor room (§12.2) with (origin, 8, 8, room
  flags f1, outdoor flags f2, flags ex = grid 3, DT1 mask).

#### 12.2 Outdoor room (`0x0067D540`) and its grids (`0x0067D2D0`)

Creation: allocate a type-1 room (`rooms.md` §2: one level-seed step and
room seed init); tile rect := (x, y, 8, 8); add to the level; room flags
|= f1 | 0x80000; room DT1 mask (+0x50) := mask; outdoor room data
(+0x20): flags (+0x54) := f2, flags ex (+0x58) := grid 3; sub type
(+0x64) := leveldefs `SubType`, sub theme (+0x68) := `SubTheme`, picked
themes (+0x6C) := sub-theme pick (`outdoor-tilesub.md` §3; room seed
draws).

Grids (called from the room seed reset `0x0066EE40`, `rooms.md` §9.2):
tile-type, wall and floor grids (w+1)×(h+1); floor cells (0..7, 0..7) :=
0x40002; Act I: path floor (§7.5.3); tile grid allocation; substitutions
in order: waypoint rows (leveldefs `SubWaypoint`, theme 0, picked := (room
flags >> 16) & 3) if nonzero; shrine rows (`SubShrine`, theme 0, picked :=
(room flags >> 12) & 0xF) if nonzero; the room's own (sub type, sub
theme, picked) — all `outdoor-tilesub.md` §4. Then floor flags by level
type OR into every floor cell without bits 0x3F0FF80: 16 → 0x100; 21 →
0x120000; 22 → 0x100000; 27 → 0xA00000; 28 → 0x1600000; 31 → 0x600000 if
level 117 else 0. Finally wall and floor grid edges |= 0x4.

## Constants & data dependencies

| Constant | Value | Where |
|---|---|---|
| cell size | 8 tiles | §3 |
| link tables | 9 tables, 15×16 bytes | `0x006F0750`–`0x006F0ED0` (§2.2) |
| Rogue check table | 64 BOOLs | `0x006F1158` |
| Act I flag rows | 15×6 ints | `0x006F1258` |
| border tables N, P, Q | 91, 13×4, 14×2 | `0x006F0FC0`, `0x006F0620`, `0x006F06F0` |
| desert link pairs | 5×2 | `0x006F112C` |
| link-probe offsets | 4×2 | `0x006F05D8` |
| neighbour offsets (RandomDS1) | dx, dy bytes | `0x006F0614`, `0x006F060C` |
| shrine bits | 0x1000…0x8000 | `0x006F061C` |
| river U/L files | 12×2 | `0x006F26A0` |
| desert cliffs, tomb row | 8×5×4, 9×4 | `0x006F2390`, `0x006F2610` |
| path order, steps, jitter | | `0x006F2840`, `0x006F2850/54`, `0x006F2820/30`, join dirs `0x006F2800/10` |
| Act IV mesa/pit/lava | | `0x006F22A4`, `0x006F22B0`, `0x006F22C0` |

Data: leveldefs `SizeX/Y[diff]`, `OffsetX/Y`, `SubType`, `SubTheme`,
`SubWaypoint`, `SubShrine`, `Vis0..7`; lvlprest `SizeX`, `SizeY`, `Files`
(file count drives every build-list roll); lvltypes ids as used by
levels.txt `LevelType` (the "Expansion" separator row is not an id).

## Randomness

**Act creation** (`levels.md` §3.7): per driver call a copy of the DRLG
seed; linker draws in table order including retries' extra draws (OS, B1,
BD draw on every call); the real DRLG seed steps once for Black Marsh
with R0 ∈ {1, 3} (Act I) and throughout the jungle placer (Act III).

**Act I level build**, level seed, in order (Blood Moor and Cold Plains
recorded): build-list rolls of each new border preset while stamping
borders (§6) → substitution type 0 shuffles → river/cliff draws (§7.2) →
substitution types 1, 2 (`roll(groups)` for type 1, shuffles, variant
rolls) → FarAway or cave-entrance shuffle + its build-list roll →
substitution type 3 → path jitter per path (§7.5.2) → waypoint shuffle →
shrine `&3` + shuffle → special presets (each placer's shuffle, then its
build-list roll; cottage `&3`/`&1`) → per cell row-major: preset cell =
`roll(Files)` (`0x00666F33`) then `preset.md`'s room creation; outdoor
cell = room allocation (`rooms.md` §2: level-seed step, room seed) then
one room-seed step per sub-type lvlsub row (`outdoor-tilesub.md` §3).
Later, when a room's tiles are built: room-seed reset (`rooms.md`), then
the room's substitutions (`outdoor-tilesub.md` §4), then tile choice
(`rooms.md` §9).

## Edge cases & original bugs

1. Linkers' draws use a copy of the DRLG seed (§2.3): the Act I wild and
   monastery chains both start from the same state, so their first draws
   repeat (recorded: seq 2430/2431 = 2419/2420).
2. Check failures retry without drawing (alternatives cycle); backtracking
   resets a row and re-runs the previous linker. A Fix linker whose check
   fails would loop forever (not reached by 1.14d data).
3. The build list's file is (roll + 1) mod Files: the rolled file is never
   used first. `roll(1)` still steps. Files = 0 with F = −1 would divide
   by zero; 1.14d only stamps Files-0 presets with explicit files.
4. A preset cell draws `roll(Files)` in the room pass although the stamped
   file overrides it (§12.1).
5. Shrines draw `&3` before checking the grid area.
6. FarAway loops i, j inclusive of H and W (one wrap-around row and
   column tested twice).
7. Path jitter draws `&3` even when the path list is empty.
8. The cliff-cave scan's second order passes (outer < gh) as x and (inner
   < gw) as y.
9. Border lookup row 0 of P holds unrelated values (8192, 16384, 32768,
   0); unreachable for straight edges.

## Test vectors

Recorded (`20261005-232125-rng.jsonl`, server copy; the client copy at
seq ≥ 13367 repeats every value). Act I placement, DRLG seed after the
start-seed step {4014346869, 268778232}:

| Seq | Site | Seed | Value | Meaning |
|---|---|---|---|---|
| 2419 | `0x00676165` | copy 1 | lo' 1406222081 | Cold Plains R0 = 1 |
| 2420, 2421 | `0x00676669`, `0x0067669F` | copy 1 | 3154683627, 457460266 | Blood Moor R0 = 3, R3 = 0 |
| 2422, 2423 | `0x00676469`, `0x0067649F` | copy 1 | 1949180022, 1550108608 | Rogue R0 = 2, R3 = 0 |
| 2424 | `0x00676165` | copy 1 | 4175359377 | Burial Grounds R0 = 1 |
| 2430, 2431 | `0x00676165` ×2 | copy 2 (same start) | 1406222081, 3154683627 | Black Marsh R0 = 1, Dark Wood R0 = 3 |
| 2439 | `0x006774DB` | DRLG seed | 1406222081 (odd) | Outer Cloister direction 1; DRLG seed after {1406222081, 1674353446} |

Level seeds set in between (allocation order): 4, 3, 2, 1, 17 (after
copy-1 draws), 39, 26 (+ preset roll), 7, 6, 27 (+ preset roll), then the
Black Marsh draw, then 5. The list goes on with 8, 9, …, 16: the Act I
neighbour entries (§2.7, ids 1..17) get-or-allocate every id in order,
and 8..16 are the ones not yet allocated (`levels.md` Test vectors, seq
2425–2452).

Derived from the rules (simulation of §2, not yet recorded): wild chain
retries Blood Moor (3,0)→(3,1)→(0,0)→(0,1) and Rogue (2,0)→(2,1)→(3,0)→
(3,1); final rects (x, y, w, h): Stony Field (1000, 1000, 80, 80), Cold
Plains (920, 984, 80, 80), Blood Moor (904, 1064, 56, 96), Rogue
Encampment (960, 1112, 56, 40), direction 3, Burial Grounds (880, 968, 40,
48); monastery chain: Moo Moo Farm (5000, 1148), Monastery Gate (3000,
1000), Tamoe (3000, 1018), Black Marsh (2920, 1002), Dark Wood (2904,
1082) (Dark Wood retried 3→0). Act I flags: Blood Moor and Cold Plains 0
(consistent with no river/side-cave draws recorded).

Per-site draw counts of one level build (one copy; both copies equal):

| Site | Blood Moor (L2, grid 7×12 or 12×7) | Cold Plains (L3, 10×10) | Rule |
|---|---|---|---|
| `0x0067438F` build list | 15 (n: 3,1,3,1,3,1,1,1,3,1; 2; 1; 6; 3; 3) | 13 (3,1,3,1,3,1,3,1; 2; 6; 1; 3; 3) | §5.1 |
| `0x0066F78D` / `0x0066F7B8` / `0x0066F7E9` | 1547 / 64 / 1611 | 1570 / 576 / 2146 | tilesub §2 |
| `0x0066F9C9` / `0x0066F8DB` / `0x0066F905` | 1 (n 9) / 1 / 2 | 1 (n 9) / 1 / 3 | tilesub §2 |
| `0x00674539`, `0x006745A4` | 1, 1 | 0 | FarAway |
| `0x006747A7` or `0x006747D2` / `0x00674803` | 100 / 100 | 256 / 256 | 2 resp. 4 shuffles |
| `0x0068126D` / `0x00681310` / `0x0068134E` | 3 / 11 / 11 | 4 / 14 / 14 | §7.5.2 |
| `0x00674E59` / first / `0x00674F3B` | 1 / 50 / 50 | 1 / 64 / 64 | shrines |
| `0x006749AD` or `0x006749D8` / `0x00674A09` | 100 / 100 | 64 / 64 | RandomDS1 |
| `0x006804ED` / `0x0068052C` | 1 / 0 | 1 / 1 | cottage |
| `0x00666F33` | 48 | 61 | preset cells |
| room allocations (`0x0066B42E`) | 81 | 98 | 48+33, 61+37 |
| `0x006706D7` (room seeds) | 198 = 33·6 | 222 = 37·6 | sub-theme pick |

Order of the Blood Moor build (seq): 2561 level seed {4014346871, 666};
2562–2571 border build-list rolls (first: roll(3) = 1); 2572–3671
substitution type 0; 3672 roll(9) = 0; 3673–4461 types 1–2; 4462–4464
FarAway + roll(2); 4465–5800 type 3; 5801–5825 jitter; 5826–5926 shrines;
5927–6027 pond + roll(1); 6028–6129 cottage; 6130–6331 stone fills; from
6332 cells (first cell: `0x00666F33` then room allocation). Cold Plains:
6896 seed {4014346872, 666}, …, 9352–9480 cave entrance shuffle +
roll(2), …, 12010 cells.

**Cold Plains cells, kind and order** (recorded, seq 12010–12586, server
copy; 98 rooms is the 1.14d count). Each §12.1 cell that makes a room
leaves `0x00666F33` then the room allocation `0x0066B42E` (preset, P)
or the allocation alone (outdoor room, o); 6 sub-theme draws
(`0x006706D7`) follow every o. In draw order (row-major cell order,
§12.1):

```
PPPPPPPPPP PoPooooPPP PoooPPPPoP PoooPPPPoP PoPoooPPoP
PoooPoPPoP PPPooooooP PPPoPPoooP PoooPPoPPP PPPPPPPP
```

61 P + 37 o = 98 of the 10 × 10 = 100 cells. Every preset cell with
P ≠ 0 makes exactly one room (no P without an allocation).

**Cold Plains grid** (grid 0 per cell after §7, rows y = 0..9 top to
bottom, x = 0..9; o = outdoor room, − = blank 0x100, no room):

```
 9  6  6  6  6  6  6  6  6 10
 5  o  A  o  o  o  o  A 44  7
 5  o  o  o 15  4  4 12  o  7
 5  o  o  o 14  6 10  5  o  7
 5  o 51  o  o  o  7  5  o  7
 5  o  o  o  A  o 14 13  o  7
 8  4 12  o  o  o  o  o  o  7
 9  6 13  o 15 12  o  o  o  7
 5  o  o  o  7  5  o 15  4 11
 8  4  4  4 11  8  4 11  −  −
```

A = 48, 29, 30 (Cottages 2, Stone Fill 1 and 2) in an order this
recording does not fix. Read in §12.1 order it gives the kind string
above, and each P's file-count class matches the recorded branch of
`0x00666F33` (`mul` site `0x00666F54` for a power-of-two `Files`: the
corner pieces 8–15, 44, 51; `0x00666F3A` otherwise: straight pieces
4–7, 48, 29, 30) for all 61 presets. How it is built (§6, §7; draws
recorded): ring of §6 pieces (polygon V0 (0, 9), link u2 (0, 3), V1,
V2, link u1 (9, 2), V3, link u3 (4, 9); link midpoints (0, 1), (9, 5),
(2, 9) get 0x400 and file 3); substitution type 1 (Border - Middle)
seq 8396 `roll(9)` = 0, seq 8447 `0x0066F8DB` lo' 1833932632 mod 10 = 2:
group 0 variant 2 at (3, 1); type 2 (Border - Corner) seq 8644
`0x0066F905` lo' 3559729267 & 1 = 1: group 1 (bottom-right, N 2)
variant 1 at (6, 6), whose pattern cells (2, 3) and (3, 3) are blank →
**the two roomless cells are (8, 9) and (9, 9)** (cell indexes 98, 99);
cave entrance 51 at (2, 4) (seq 9481 build roll; margin 1 rules out
(8, 1)); type 3 (Border - Border) seq 10204 and 10499 `0x0066F905`
(N 1): group 8 at (3, 6) and group 11 at (0, 5), neither with a blank;
then 48, 44, 29, 30 (§7.4, build rolls seq 11622, 11752, 11881, 12009).
Every "group g" above is the group index G of `outdoor-tilesub.md`
§2.2 step 3: the 0-based index into that lvlsub row's DS1 group list
(DS1 +0x50, 24-byte entries in file order; the group pointer is +0x50 +
24·G, `0x0066F9D5`–`0x0066F9E9`). Types 2 and 3 (`BordType` 1, 2) start
at g0 = 0 with no draw, so G = j there; type 1 (`BordType` 0) has g0 =
the roll (0 here). "Variant v" is §2.2's `roll(N)` result.
Room count rule: with every Cold Plains preset 8 × 8 (one cell), rooms
= 100 − blank cells; blanks come only from §6 step 5 (none here: every
corner holds a piece) and from blank pattern cells of a border
substitution (`outdoor-tilesub.md` §2.3). A build with 97 rooms has a
third blank cell: its grid differs from this one, and the first
differing cell (dump grid 0 / grid 2 per cell before §12.1) names the
substitution or step that differs.

Comparison (exact): for an act creation and each generated outdoor level,
the sequence of (site, seed state after) of every draw equals the
recording; level rects and outdoor flags equal a level-coordinate probe
(OQ 1).

## Provenance

- §2.7 allocation order read from the disassembly of `0x006775C0`
  (calls `0x00642BB0` for i, `0x00642860`, then `0x00642BB0` for each
  j ≠ i) and `0x00677680` (`0x00642BB0` for each id before the type
  test at `0x006776A3`; `0x00642BB0` on level +0x1B4 for vis[j] at
  `0x006776EB`); callers `0x00677750`, `0x00677790`, `0x006789B0`,
  `0x00678A20`, `0x00678A70`. Cold Plains cell kinds counted from
  `20261005-232125-rng.jsonl` seq 12010–12586 (sites `0x00666F33`,
  `0x0066B42E`, `0x006706D7`).
- **1.14d `Game.exe`**, `re/exports/all.asm`: `0x00678AD0`, `0x00677750`,
  `0x00677790`, `0x006789B0`, `0x00678A20`, `0x00678A70`, `0x006772C0`,
  `0x00677180`, `0x00676DD0`, `0x00676EB0`, `0x006775C0`, `0x00677680`,
  `0x00675DE0`, `0x00675EB0`, `0x00675360`, `0x00675080`, `0x006750F0`,
  `0x0067D050`, `0x006743C0`, `0x00674320`, `0x00674230`, `0x00674730`,
  `0x00674920`, `0x006744F0`, `0x00674B70`, `0x00674E40`, `0x00675770`,
  `0x00674040`, `0x00675850`, `0x006755C0`, `0x00675600`, `0x00675670`,
  `0x006807F0`, `0x00680070`, `0x00680200`, `0x006801A0`, `0x006803D0`,
  `0x00680580`, `0x006804E0`, `0x00681420`, `0x00680D70`, `0x00681000`,
  `0x006817D0`, `0x00681630`, `0x00681560`, `0x006815C0`, `0x00678B80`,
  `0x00678CF0`, `0x00681240`, `0x0067FE90`, `0x0067FD20`, `0x0067FB90`,
  `0x0067FC70`, `0x0067F980` and its helpers `0x0067F470`–`0x0067F920`,
  `0x0067E890`, `0x0067E660`, `0x0067E6A0`, `0x0067E600`, `0x00677880`
  (draw sites only), `0x0067D540`, `0x0067D2D0`, `0x0066B800`,
  `0x0066B880`. Tables read from the file image with
  `scratchpad/pe.py`. Linker bodies `0x00676150`–`0x00676C00` are not in
  the export (§2.4).
- **D2MOO** (1.10f) `DrlgOutdoors.cpp`, `DrlgOutPlace.cpp`,
  `DrlgOutWild.cpp`, `DrlgOutDesr.cpp`, `DrlgOutJung.cpp`,
  `DrlgOutSiege.cpp`, `DrlgOutRoom.cpp`, `DrlgDrlgGrid.cpp`,
  `DrlgDrlgVer.cpp` as a map. Confirmed equal on 1.14d where addresses are
  given. 1.14d differences: the per-act wild/monastery code order and
  tables match; the build-list step is a separate function; Act V's link
  table uses real linkers (D2MOO passes none for the driver checks);
  `GetBridgeCoords` scans y < gw − 1 (as D2MOO); Act IV mesa/pit placement
  and Act I special presets are compiled as shared tails (same order).
- **Recorded**: `20261005-232125-rng.jsonl` via `scratchpad/drlg_rng.py`
  (labels confirmed from the asm: `0x0066F690` and `0x00666F33` use the
  level seed passed or held at level +0x1C4; the linker draws use the
  driver's copy).

## Open questions

1. Level rects and directions after Act I creation (§Test vectors,
   derived): read level +0x1C..+0x28 of levels 1–7, 17, 26, 39 and
   preset direction of levels 1 and 27 after act creation.
2. Disassemble the linker bodies `0x00676150`–`0x00676DC0` (no function
   in the export) and compare with §2.4 (draw forms, B/A choice, BM size).
3. Record entering Act 2 (desert chain, `R8` / `RW` / `VS` draws, Lut
   Gholein direction from R0[i+1]) and Act 4 (Outer Steppes flag).
4. Record Act 3 entry: jungle placer draws on the DRLG seed
   (`drlg/outdoor-act3-act5.md` OQ 1).
5. Stony Field, Dark Wood, Black Marsh, Tamoe builds (river, bridge,
   cliff caves, side cave draws `0x00680251`, `0x0068034F`): record a run
   that generates them.
6. *Answered:* path floor table `0x006F2700` (not `0x006F2860`) is in
   `drlg/outdoor-path-floor.tsv`, the bit order in §7.5.3.
7. *Answered:* the jungle placer is read in full from 1.14d in
   `drlg/outdoor-act3-act5.md` §2 (its OQ 1 asks for the recording).
8. *Answered:* Kurast and Travincal lists and positions are §9.4, read
   from 1.14d; the jungle file table is §9.3. A recording of the Act 3
   levels would confirm them (as for OQ 4).
9. *Answered:* Act V beyond the siege strip is §11, read from 1.14d.
   A recording of levels 111, 112 and 117 would confirm the draws.
10. *Answered* (also `impl-room-population` §3, "Cold Plains live 97
    vs 98"): the roomless cells are (8, 9) and (9, 9), blanked by
    Border - Corner group 1 variant 1 at (6, 6) (Test vectors, "Cold
    Plains grid": the derived grid reproduces all 98 recorded cells and
    the file-count class of all 61 presets). 98 is the 1.14d count; a
    live 97 has one more blank cell, so its generation differs from
    this grid (most likely in a border substitution, the only source of
    blanks here); compare the implementation's per-cell grid with the
    table to find it.
11. *Answered* (`impl-drlg-act3-5` Q4): Kurast border order is §9.4
    (top row, bottom row, sides east before west, corners), read call by
    call from `0x0067EAD0`, `0x0067EC30`, `0x0067ED70`.
12. *Answered* (`impl-drlg-act3-5` Q5): the random placer R stamps at
    the tried cell itself, no margin or offset (§9.4, `0x0067F121`–
    `0x0067F14B`).
13. *Answered* (`impl-drlg-act3-5` Q6): the ravine walk has no range
    check and no termination guard in 1.14d; 1.14d data never leaves
    k = 0..11 or fails to end (§11 step 3).
14. *Answered* (`impl-drlg-act3-5` Q7): the barricade straight walk
    stops on equality, not a count, and always ends on §4's axis-aligned
    edges (§11 step 2).
15. *Answered* (`impl-drlg-act3-5` Q8): caves and special presets choose
    tall / wide from the level tile rect (level +0x24, +0x28), with
    opposite ties (§11 steps 5, 9).
16. *Answered* (`impl-drlg-act3-5` Q9): the prisons helper `0x0045C390`
    is `roll` (`sim/rng.md` §3) on the level seed (§11 step 8).
17. *Answered (2026-10-08)* (`fix-drlg-answers` Q3): yes, "group 8",
    "group 11" (type 3) and "group 1" (type 2) in the Cold Plains test
    vector are 0-based indexes into the lvlsub row's DS1 group list
    (Test vectors, sentence after the Cold Plains derivation;
    `0x0066F9D5`–`0x0066F9E9`).
