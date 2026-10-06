# Spec: DRLG — Maze levels (DrlgType 1)

- **Status:** draft: every rule read from the 1.14d `Game.exe` code
  (0x670810–0x673FE0) and its data tables (dumped from the binary into
  `maze-specials.tsv`); the Act 1 recording builds no maze level (level
  seeds 8–12 are initialized, no maze site draws), so the draw order is
  not yet trace-confirmed.
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::drlg::maze`
- **Related specs:** `sim/rng.md` (generator, `roll`, seeds §5.4),
  `drlg/levels.md` (level alloc, level seed, dispatch by DrlgType, level
  position/size, staff/boss tomb choice), `drlg/rooms.md` (DRLG room
  alloc + room seed, room links ("orths"), room free, room list),
  `drlg/preset.md` (DS1 map alloc and its file roll, building a room from
  a DS1), `data/fields.tsv` (`lvlmaze`, `lvlprest` layouts),
  `drlg/maze-specials.tsv` (special-room tables, this spec's sibling).

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 42–56 |
| Inputs | 57–65 |
| Outputs / state changes | 66–74 |
| Rules | 75–76 |
|   1. lvlmaze row and level init | 77–91 |
|   2. Cells, sides and links | 92–122 |
|   3. Cell primitives | 123–242 |
|   4. Generation sequence (`0x00673B30`, D2MOO `DRLGMAZE_GenerateLevel`) | 243–285 |
|   5. Layout builders | 286–363 |
|   6. Special cells by level | 364–446 |
|   7. Placement against a neighbouring preset level | 447–495 |
|   8. Theme cells (`0x006735F0`, D2MOO `RollAct_1_2_3_BasicPresets`) | 496–513 |
|   9. Building cells and file choice (`0x00673A60`, `0x006738C0`) | 514–546 |
| Constants & data dependencies | 547–560 |
| Randomness | 561–596 |
| Edge cases & original bugs | 597–622 |
| Test vectors | 623–637 |
| Provenance | 638–690 |
| Open questions | 691–704 |
<!-- /index -->

## Summary

A maze level is built from equal-sized cells on a grid. Each cell is a
DRLG room of preset type whose DS1 ("lvlprest" row) is chosen from the
set of sides it connects to (W/E/S/N, 15 shapes per level type).
Generation runs once per level when `drlg/levels.md` dispatches a
DrlgType 1 level: put one cell in the middle of the level rectangle, grow
a ring and/or a random tree of cells (optionally linking touching cells,
"merge"), stamp special cells (entrance from the previous level, exit to
the next, waypoint, quest/boss rooms) onto cells of the right shape, move
the cells into the level rectangle, turn some plain cells into "theme"
variants, then hand every cell to the preset builder with a DS1 file
index. All randomness is on the level seed, except the per-cell direction
draw and the merge draw, which use DRLG room seeds.

## Inputs

| Name | Type | Source |
|---|---|---|
| level | DRLG level | `drlg/levels.md`: id, LevelType (`levels.txt`), position/size, level seed `{x,666}`, act DRLG (difficulty, staff tomb level, boss tomb level, act number) |
| lvlmaze row | 7 × i32 | first `lvlmaze.bin` record whose `Level` equals the level id (§1) |
| lvlprest rows | table | `Def`, `Files` (offset 64) of each cell's def (§9) |
| other levels | DRLG levels | Outer Cloister (27) for Barracks, Chaos Sanctum (108) for River of Flame (§7) |

## Outputs / state changes

- The level's room list: one preset room per cell (later replaced by the
  rooms `drlg/preset.md` builds from each DS1), with links between them.
- For Barracks (28) and River of Flame (107) only: the level's position
  and size are recomputed from the cells (§7).
- The level's "file rotation" list (level +0x1CC, §9) and the level seed
  advanced by every draw listed in Randomness.

## Rules

### 1. lvlmaze row and level init

1. `0x0061F490` scans the loaded lvlmaze records in file order and returns
   the first whose `Level` equals the level id. No match is fatal.
   `0x00673B10` stores it in the level (+0x14) and then calls the
   position/size routine `0x00642D10` (`drlg/levels.md`, no draws).
   `0x00673FE0` clears the pointer when the level is freed without
   keeping it.
2. Fields used: `Rooms[d]` (offset 4 + 4·difficulty; d = DRLG +0x450),
   `SizeX`, `SizeY` (cell size in tiles), `Merge` (per-mille chance).
3. 1.14d `lvlmaze.bin` holds 81 records (the `Expansion` separator line
   of the .txt is not one). Rows whose level is not DrlgType 1 are never
   used by maze code: Level 0 ("Cave 1 Treasure"), 13–16, 25, 37, 90,
   91, 93, 132 (all DrlgType 2 in `levels.txt`).

### 2. Cells, sides and links

1. Directions: 0 = W, 1 = N, 2 = E, 3 = S, 4 = NW, 5 = NE, 6 = SE,
   7 = SW. The cell adjacent to cell P in direction d sits at
   P.x − P.w (W, NW, SW) or P.x + P.w (E, NE, SE) or P.x (N, S), and
   P.y − P.h (N, NW, NE) or P.y + P.h (S, SE, SW) or P.y (W, E). The new
   cell's own size is the lvlmaze `SizeX × SizeY`; offsets use the
   parent's size.
2. Each cell carries: tile rect (x, y, w, h), a def (lvlprest Def; 0 at
   allocation), a file index (0 at allocation), a **lock** flag (clear at
   allocation; 1.14d bit 1 of the preset-room flags, D2MOO
   `HAS_MAP_DS1`) and its link list (`drlg/rooms.md` "orths": neighbour
   room, direction, init flag).
3. The level keeps its rooms in a singly linked list, **newest first**;
   adding a room prepends it and increments the level's room count;
   freeing an added room unlinks it and decrements the count
   (`drlg/rooms.md`). "List order" below always means newest first.
4. Linking P to N in direction d (`0x0066B5E0`, `drlg/rooms.md`) gives P
   a link (N, d) and N a link (P, (d+2) mod 4), each only if that room
   has no link to the other yet. Lock is not touched. Each new link is
   **prepended** to the room's link list (`0x0066B560`; P's first, then
   N's), so a link list is newest first; record layout `drlg/rooms.md`
   §1 (room +0x00).
5. **Overlap test** (`0x0066B800`, margin m): for rects A, B, the gap dx
   = B.x − A.w − A.x if A.x < B.x, else A.x − B.w − B.x; dy likewise on
   y/h. A and B "collide at margin m" when dx < m and dy < m.
6. **Direction from A to B** (`0x00642240`): if B.x < A.x and
   A.x = B.x + B.w → 0; else if B.x ≥ A.x and B.x = A.x + A.w → 2; else
   on y: B.y < A.y and A.y = B.y + B.h → 1; B.y ≥ A.y and
   B.y = A.y + A.h → 3; otherwise −1.

### 3. Cell primitives

**3.1 Allocate.** `0x0066B3E0` (owner `drlg/rooms.md`): one level-seed
step (child room seed `{lo',666}`, then one room-seed step stored as the
room's init seed). Maze code then sets w, h = `SizeX`, `SizeY`. Every
allocation below costs these two steps, whether or not the cell is kept.

**3.2 Place test** (`0x00670880`, D2MOO `LinkMazeRooms`): put new cell N
next to parent P in direction d (§2.1). Reject if N collides at margin 0
with the box of any of P's links, or with any room of the level other
than N and P (`0x0066B900`). A rejected cell is freed (`0x0066C100`).

**3.3 Pick shape** (`0x006709B0`, D2MOO `PickRoomPreset`; every 1.14d
call passes "unlock"). Mask = OR over the cell's links with direction
0 → 1 (W), 2 → 2 (E), 3 → 4 (S), 1 → 8 (N); other directions ignored.
The def is computed by level type:

| Type (lvltypes Id) | Def |
|---|---|
| 3 Act 1 Cave | 52 + mask |
| 4 Crypt | 108 + mask |
| 7 Barracks | 167 + mask |
| 8 Jail | 205 + mask |
| 10 Catacombs | 257 + mask |
| 13 Act 2 Sewer | 301 + mask |
| 14 Harem / 15 Basement / 23 Spider | remap table 0x006EF758 (below) |
| 17 Tomb | 413 + mask |
| 18 Lair | 481 + mask |
| 19 Arcane | 509 + mask |
| 22 Kurast (Durance) | 753 + mask |
| 24 Dungeon | 664 + mask |
| 25 Act 3 Sewer | 704 + mask |
| 28 Act 4 Lava | 836 + mask |
| 32 Act 5 Temple / 35 Act 5 Lava | remap table 0x006EF6D8 (below) |
| 33 Ice Caves | 1002 + mask, but the raw mask if lvlmaze `Rooms[d]` = 1 (`0x00670810`) |
| 34 Baal | 1058 + mask |
| any other | fatal ("Some really bad voodoo") |

Remap tables (0 = no shape for that mask):

| Mask | Harem (14) | Basement (15) | Spider (23) | Temple (32) | Act 5 Lava (35) |
|---|---|---|---|---|---|
| 1 W | 0 | 0 | 0 | 0 | 1056 |
| 2 E | 0 | 0 | 0 | 0 | 1055 |
| 3 EW | 0 | 0 | 0 | 0 | 1057 |
| 4 S | 0 | 0 | 0 | 0 | 1054 |
| 5 SW | 356 | 360 | 659 | 1045 | 0 |
| 6 SE | 355 | 359 | 660 | 1044 | 0 |
| 8 N | 0 | 0 | 0 | 0 | 1053 |
| 9 NW | 357 | 361 | 661 | 1043 | 0 |
| 10 NE | 354 | 358 | 662 | 1042 | 0 |
| 12 NS | 0 | 0 | 0 | 0 | 1058 |

Overrides after the lookup: Palace Cellar 1 (52): def 361 → file 2.
Palace Cellar 3 (54): def 361 or 359 → file 3. Spider Cave (84): def 662
→ 664 (Spider Chest NE). Spider Cavern (85): def 661 → 663 (Spider Chest
NW). If the final def is 0 the cell is left untouched. Otherwise: def
:= result, file := override or −1, lock cleared.

**3.4 Merge** (`0x00670C70`, D2MOO `MergeMazeRooms`), for a new cell N
(already linked to its parent, not yet in the list). Skip entirely if N
is locked. For each room R in list order with R ≠ N and R unlocked:
compute dx, dy (§2.5, A = N, B = R, margin 1); continue only if they
collide at margin 1 and dx ≠ dy (touching along an edge, not only at a
corner) and N has no link to R. Then **draw one step of R's room seed**;
if `lo' mod 1000 < Merge` (signed compare), take dir = direction from R
to N (§2.6); if dir ≠ −1, link R→N with dir and pick shape for R.
The parent is never drawn for (already linked).

**3.5 Attach variants.** All start with Allocate and Place test; on
rejection the cell is freed and nothing else happens.

| Variant (1.14d) | After a successful place test |
|---|---|
| grow (inline in `0x00670F60`, `0x00671210`, `0x006714D0`, `0x006718C0`, `0x006719F0`, `0x00672810`) | link P→N, merge N, add N, pick P, pick N |
| fixed (`0x00670DE0`, D2MOO `InitRoomFixedPreset`) | link, add, pick P if asked (all 1.14d callers ask), then N: def := given, file := given, lock set |
| special fallback (`0x00670EB0`, D2MOO `AddSpecialPreset`) | for each room in list order that is unlocked: try "fixed" with pick-P; stop at the first success. If none succeeds nothing is placed |
| blank (`0x00671320`) | link, add, def := given, file −1, lock set (no pick) |
| probe (`0x00672340`) | see 3.7 |

**3.6 Special-room stamp** (`0x006724E0`, D2MOO
`ScanReplaceSpecialPreset`) with a table row (find def F, special def S,
file f, fallback dir d) and an optional counter r:
1. The first room in list order that is unlocked and has def F becomes
   S (file f, lock set). No allocation.
2. Otherwise run "special fallback" with (d, S, f): a new cell placed in
   direction d of some unlocked room; its only opening faces the parent,
   which is the side named in S.
3. In both cases, if a counter was passed, r := (r + 1) mod 4.

Table rows are indexed by r ∈ {0, 1, 2, 3} = special opening N, E, S, W
(find defs "<type> N/E/S/W", fallback dir 3, 0, 1, 2). All rows used by
1.14d are in `maze-specials.tsv` (kind, row, find, special, file,
fallback dir, VA). A "stamp K" below means: stamp row r of table K, then
advance r.

**3.7 Probe and extreme cells.** Probe(P, d) (`0x00672340`, D2MOO
`CheckIfMayPlaceAdjacentPresetRoom`): false at once if P is locked or P
already has a link with direction d. Otherwise Allocate, Place test; on
success link, add, pick the new cell, then free it (net: P unchanged,
room count unchanged: freeing a room removes, for each of its links with
the init flag, the matching link from the neighbour's list,
`drlg/rooms.md` §2.1, so P loses the link it just gained). Returns whether the place test passed. Cost: one
allocation (2 steps) whenever it gets past the first checks.

Extreme-cell finders walk the list in order, keeping a best cell; a
room is probed only when there is no best yet or it strictly improves
on the best:

| 1.14d | Keeps | Probe dir | D2MOO name |
|---|---|---|---|
| `0x006723E0` | smallest y | 1 N | `GetFreeLocationForRoomSouth` |
| `0x00672460` | largest x | 2 E | `…West` |
| `0x006724A0` | largest y | 3 S | `…North` |
| `0x00672420` (no catalogued callers; reached through table 0x006F0468) | smallest x | 0 W | `…East` |

**3.8 Random cell** (`0x006711A0`): k = `roll(level room count)` on the
level seed (rng.md §3: no step if the count < 1); walk k rooms from the
list head.

### 4. Generation sequence (`0x00673B30`, D2MOO `DRLGMAZE_GenerateLevel`)

1. Allocate the first cell F (§3.1); x = level.x + (level.w − SizeX)/2,
   y = level.y + (level.h − SizeY)/2 (signed division truncating to
   zero); add F.
2. Run the builder for the level type (table below; §5, §6).
3. Barracks (28): §7.1. River of Flame (107): §7.2. Every other level:
   normalize (`0x00642590`): compute the cells' bounding box (min x,
   min y, max x+w, max y+h over the list, `0x00642520`); fatal if its
   width exceeds level.w or its height level.h; shift every cell by
   (level.x − minx, level.y − miny).
4. Theme pass (§8).
5. For every cell in list order (next pointer read first): build it
   (§9), which frees the cell.

| Type | Builder (order) |
|---|---|
| 3 Cave | grow tree (§5.2); cave stamps (§6) |
| 4 Crypt | grow tree; crypt stamps |
| 7 Barracks | ring(2) (§5.1); grow tree |
| 8 Jail | ring(2); grow tree; jail stamps |
| 10 Catacombs | catacomb start (§5.3); grow tree; catacomb stamps |
| 13 Act 2 Sewer | ring(2); grow tree; sewer specials (§6.2) |
| 14 Harem, 15 Basement, 23 Spider | ring(2) only |
| 17 Tomb | Claw Viper Temple 2 (61): F := def 480 (Tomb Tainted Sun X), file −1, locked; nothing else. Others: hub (§5.4); grow tree; tomb stamps (§6.3) |
| 18 Lair | ring(2); grow tree; lair stamps |
| 19 Arcane | spiral (§5.5); summoner stamp |
| 22 Kurast | ring(2); grow tree; Durance stamps |
| 24 Dungeon | ring(2); grow tree; dungeon stamps |
| 25 Act 3 Sewer | Sewers 1 (92): ring(5), then the four corner swaps (below); others ring(2). Then grow tree; sewer stamps |
| 28 Act 4 Lava | grow tree |
| 32 Temple | ring(2); temple stamps |
| 33 Ice | Cellar of Pity (114): `roll(2)` on the level seed; F := 1038 (Ice River A) if non-zero, else 1039 (Ice River B); file −1, locked. Echo Chamber (116): F := 1040 (Ice Pool A). Glacial Caves 2 (119): F := 1041 (Ice Pool B). Others: ring(2); grow tree; ice stamps |
| 34 Baal | hub; grow tree; Baal stamps |
| 35 Act 5 Lava | lava cross (§5.6) |
| other | fatal |

Act 3 Sewers 1 corner swaps (after ring(5)): for each pair in this order
(709 → 735, 710 → 736, 713 → 737, 714 → 738; Sewer SW/SE/NW/NE → Sewer
Prev SW/SE/NW/NE) the first unlocked room in list order with the old
def gets the new def, file −1, lock set; a missing match is fatal
(1.14d asserts; D2MOO only warns).

### 5. Layout builders

**5.1 Ring(n)** (`0x00670F60`, D2MOO `InitBasicMazeLayout`): starting
at F, grow n−1 cells N, then n−1 cells W, then n−1 cells S, then n−2
cells E, each new cell being the parent of the next; finally link the
last cell to F with direction 2 (E) and pick last, then pick F. This is
the border of an n × n square with F at its south-east corner. If any
grow is rejected the next step uses a null parent and crashes (never
happens with the shipped data). ring(2) = 3 grows; ring(5) = 15 grows.

**5.2 Grow tree** (`0x00671210`, D2MOO `BuildBasicMaze`). Target =
`Rooms[d]`, ×3 if the level is the act's staff tomb level (DRLG +0x94),
×2 if it is the boss tomb level (DRLG +0x484) (both chosen by
`drlg/levels.md`). While the level room count < target:
1. R = random cell (§3.8; one level-seed step if count ≥ 1).
2. dir = (one step of **R's room seed**) & 3.
3. If R is unlocked: grow from R in dir (§3.5; rejected tries are
   simply lost). If R is locked nothing happens but both draws stay
   consumed.

**5.3 Catacomb start** (`0x006714D0`). Catacombs 1 (34): grow N, E, S,
W from F (in that order), then F := 290 (Catacombs Prev NSEW), file −1,
locked. Other catacomb levels: one level-seed step; if `lo'` is even
grow N then S from F and set F := 289 (Prev NS); if odd grow W then E
and set F := 288 (Prev EW); file −1, locked.

**5.4 Hub** (`0x006718C0`, D2MOO `PlaceAct2TombPrev_Act5BaalPrev`):
r = level-seed step & 3; grow from F in directions r, r+1, r+2 (mod 4).
Then with q = (r+3) mod 4 (the side left open), F := def from table
0x006EF888, file −1, locked:

| q (open side) | Tomb (17) | Baal (34) |
|---|---|---|
| 0 W | 447 Tomb Prev NSE | 1075 |
| 1 N | 444 Tomb Prev SEW | 1077 |
| 2 E | 446 Tomb Prev NSW | 1076 |
| 3 S | 445 Tomb Prev NEW | 1074 |

**5.5 Arcane spiral** (`0x006719F0`, D2MOO `PlaceArcaneSanctuary`):
r = level-seed step & 3. For each branch b = 0, 1, 2, 3, starting from
parent F, place cells k = 0 … 14 by "grow" (merge on) in direction
(b + o_k) mod 4, with o = 0 for k ∈ {0, 1, 3, 4, 5, 6, 8}, +3 for
k ∈ {2, 12}, +1 for k ∈ {7, 9}, +2 for k ∈ {10, 11, 13, 14}. Each new
cell becomes the parent of the next, except cells k = 8 and k = 12, which
are dead ends: the next cell grows from the previous parent (9 from 7,
13 from 11). The kept cells are recorded in a 60-slot array (slot
15·b + k; rejected cells and k = 8, 12 leave 0). **After all four
branches** a separate pass gives every recorded cell file = (r + slot /
15) mod 4 = (r + b) mod 4, then F gets file 4. A file reset to −1 by a
later branch's merge is therefore overwritten for every recorded cell. Cells 8 and
12 keep file −1. A rejected grow leaves a null parent (crash; never
happens). Total 1 + 60 cells = lvlmaze `Rooms` 61.

**5.6 Act 5 lava cross** (`0x00671430`, Hell 1–3 125–127 and
Pandemonium Run 3 135): s = (one step of **F's room seed**) & 3 (its
second step; the first was the init seed). Two "fixed" placements from
F (pick F on), rows 2s and 2s+1 of table 0x006EF828:

| Row | Def | Dir | File |
|---|---|---|---|
| 0 | 1054 Lava S | 1 | 0 |
| 1 | 1053 Lava N | 3 | 1 |
| 2 | 1054 Lava S | 1 | 1 |
| 3 | 1053 Lava N | 3 | 0 |
| 4 | 1055 Lava E | 0 | 1 |
| 5 | 1056 Lava W | 2 | 0 |
| 6 | 1055 Lava E | 0 | 0 |
| 7 | 1056 Lava W | 2 | 1 |

Then fill blanks with def 836 (Act 4 - Lava X; the Act 4 def, as in
D2MOO) and no excluded cell.

**Fill blanks** (`0x00671320`, D2MOO `FillBlankMazeSpaces`, def D,
excluded cell X): copy the current list (count taken first). For each
copied cell C ≠ X, for j = 0 … 7: "blank"-attach a cell in direction j
(§3.5). Eight allocations per copied cell, kept or not. Cells added
here are not visited.

### 6. Special cells by level

Unless stated, each builder draws r = level-seed step & 3 first and
then stamps (§3.6) in the order listed, the counter advancing after
each stamp. Tables are the `kind` values of `maze-specials.tsv`.

| Levels (id) | Stamps in order |
|---|---|
| Cave (type 3, `0x00672550`): Den of Evil 8 | cave_prev, cave_doe |
| Cave Level 1 (9) | cave_prev, cave_down, cave_coldcrow |
| Underground Passage 1 (10) | cave_prev, cave_down, cave_next |
| Hole 1 (11), Pit 1 (12) | cave_prev, cave_down |
| Crypt (type 4, `0x00672610`): Crypt 18 | crypt_prev, crypt_bonebreak |
| Mausoleum 19, Matron's Den 133 | crypt_prev, crypt_chest |
| Tower Cellar 1–4 (21–24) | crypt_prev, crypt_next |
| Jail (type 8, `0x006726D0`): Jail 1 (29) | jail_prev, jail_waypoint, jail_next |
| Jail 2 (30) | jail_prev, jail_pitspawn, jail_next |
| Jail 3 (31) | jail_prev, jail_cath |
| Catacombs (type 10, `0x006727A0`): 34, 36 | catacombs_next |
| Catacombs 2 (35) | catacombs_next, catacombs_waypoint |
| Arcane Sanctuary (74, `0x00672E50`) | arcane_summoner |
| Flayer/Swampy dungeons 86–89 (`0x00672EA0`) | dungeon_prev, dungeon_next |
| Act 3 Sewer 92 (`0x00672F00`) | a3sewer_drain, a3sewer_chest |
| Durance 1 (100, `0x00672F60`) | meph_prev, meph_next |
| Durance 2 (101) | meph_prev, meph_waypoint, meph_next |
| Worldstone Keep 1, 3 (128, 130, `0x006730B0`) | baal_next |
| Worldstone Keep 2 (129) | baal_next, baal_waypoint |
| Ice (type 33, `0x00673530`): Crystalized Cavern 1 (113), Glacial Caves 1 (118) | ice_prev, ice_next, ice_down, ice_waypoint |
| Crystalized Cavern 2 (115) | ice_prev, ice_next, ice_down, ice_theme, ice_waypoint |

The crypt special table has 8 rows in the binary (Bonebreak N/E/S/W
then Portal N/E/S/W); r ≤ 3 so the Portal rows are never used.

**6.1 Lair** (`0x00672DC0`, Maggot Lair 62–64): r = level-seed step
& 3. Maggot Lair 3 (64): stamp the single row lair_tightspot (Lair S →
509 Tight Spot S, fallback 1), set r = 3, stamp lair_treasure row 3
(Lair W → 505 Treasure W, fallback 2; r becomes 0), stamp lair_prev[r].
Levels 62, 63: stamp lair_next[r], then lair_prev[r]. (D2MOO writes this
differently with the same effect.)

**6.2 Act 2 sewers** (`0x00672810`): two level-seed steps first: a = lo₁,
r = lo₂ & 3; e = (a & 1)·2 + 1 (1 = N or 3 = S).
- Sewers 1 (47): A = smallest-y cell probing N (§3.7). Grow N from A
  (cell B), grow N from B (cell C); "fixed" from C west (dir 0): def 333
  (Sewer Prev E), file 0. D = largest-x cell probing E; grow E from D
  (E1), grow E from E1 (E2); "fixed" from E2 in direction e: def 336
  (Sewer Prev NS), file 0, giving G; then allocate a cell H next to G in
  direction e (place test; on success link G→H, add H, pick H only, no
  merge, G stays locked). Finally stamp a2sewer_next[r]. A rejected grow
  or a null G crashes (never happens).
- Sewers 2 (48): a2sewer_prev, a2sewer_waypoint, a2sewer_next.
- Sewers 3 (49): a2sewer_prev, a2sewer_radament.
- Ancient Tunnels (65): a2sewer_prev, a2sewer_chest.
- Any other type-13 level: nothing after the two draws.

**6.3 Tombs** (`0x00672BE0`, levels 55–60, 66–72; 61 is §4): no draw.
Find the first room in list order whose def is > 428 (the hub cell;
the scan stops there or at the list end). Its def sets r: 446 → 0,
445 → 1, 447 → 2, 444 → 3; anything else is fatal. Then, in this order,
each line stamped only for the listed levels (counter advancing after
each stamp):

| Stamp | Levels |
|---|---|
| tomb_next | 55–58 |
| tomb_waypoint | 57 (Halls of the Dead 2) |
| tomb_chest | 59, 61 |
| tomb_chest | 66–72 except the staff tomb level |
| tomb_leatherarm | 59 (Stony Tomb 2) |
| tomb_cube | 60 (Halls of the Dead 3) |
| tomb_treasure | 59, 61 |
| tomb_talrasha | the staff tomb level |
| tomb_kaa | the boss tomb level |

(61 never reaches this function.)

**6.4 Temple** (`0x00673000`, Halls of Anguish 122, Halls of Pain 123):
r = level-seed `lo' mod 3` (unsigned). Unless the level is 124 (never
true for a maze level), stamp temple_down[r] (rows 0–2: NE/NW/SW Down,
fallback 1/0/3). For 123 only: stamp temple_waypoint[r] with the
advanced r ∈ {1, 2, 3} (rows NW/SW/SE Waypoint; row 0, NE Waypoint, is
unreachable). 1.14d asserts r changed (always true).

### 7. Placement against a neighbouring preset level

**7.1 Barracks** (28, `0x00673120`, after the builder). Let L = level 27
(Outer Cloister, looked up through the act, `0x00642BB0`) and q = its
preset direction (L's preset data +4, `drlg/preset.md` owner).

| q | Finder (§3.7) | Fixed cell from it | File | Link to L | Shift (sx, sy) |
|---|---|---|---|---|---|
| 0 | largest x, probe E | dir 2, def 167 Barracks Court Connect | 0 | dir 2 | (L.x − (C.x + SizeX), L.y + L.h/2 − C.y) |
| 1 | largest y, probe S | dir 3, def 167 | 1 | dir 3 | (L.x + L.w/2 − C.x − 6, L.y − (C.y + SizeY)) |
| 2 | smallest x, probe W | dir 0, def 167 | 2 | dir 0 | (L.x + L.w − C.x, L.y + L.h/2 − C.y + 1) |

C is the new court cell; its link to L is a cross-level link without
the init flag (`0x0066B790`): a link record whose target field is the
**level** L itself (not a room) and whose box is L's rect (level +0x1C:
x, y, w, h), init flag 0. It is not prepended but inserted in order
(`0x0066B720` with comparator `0x0066B6A0`): before the first record,
from the second on, that it precedes, where a precedes b when a.dir <
b.dir, or the directions are equal and by box: dir 0 a.y < b.y, dir 1
a.x > b.x, dir 2 a.y > b.y, dir 3 a.x < b.x; an empty list takes it as
head; with one record it goes before or after the head by the
comparator; with more, the head is never displaced. Being init 0 it is
never copied to a built room (§9 step 4) nor removed from a neighbour
on free; halves truncate toward
zero; SizeX/SizeY are the Barracks lvlmaze sizes (10 × 14). A null C
crashes (D2MOO notes the same). Then one level-seed step: if `lo'` is
odd stamp barracks_next[q] then barracks_forge[q+1]; if even stamp
barracks_forge[q] then barracks_next[q+1]. Then add (sx, sy) to every
cell and set the level rect to the cells' bounding box (no
normalization, no size check).

**7.2 River of Flame** (107, `0x00673320`). The whole step is skipped
when the DRLG's act index (+0x480) is 4 (Act V); level 107 belongs to
Act IV (index 3), so it always runs. Let S = level 108 (Chaos Sanctum).
1. Largest-y cell probing S; "fixed" from it, dir 3, def 852 (Lava Warp
   N), file −1.
2. Smallest-y cell probing N; "fixed" from it, dir 1, def 855 (Bridge 1)
   → B1.
3. Next to B1, dir 1: allocate + place test; on success link, add, def
   856 (Bridge 2), file −1, locked (no pick) → B2. Same from B2 → B3
   (def 856). A rejected B2/B3 leaves a null parent (crash).
4. Cross-level link B3 → S, dir 1.
5. Shift = (S.x + 2·B3.w − B3.x, S.y + S.h − B3.y), taken now.
6. One level-seed step; stamp a4lava_forge row (`lo'` & 1) (row 0: Lava
   W → 853 Forge W, fallback 2; row 1: Lava E → 854 Forge E, fallback
   0), no counter.
7. Fill blanks with def 836, excluding B3.
8. Add the shift to every cell; level rect := bounding box.

### 8. Theme cells (`0x006735F0`, D2MOO `RollAct_1_2_3_BasicPresets`)

Runs for types 3, 4, 7, 8, 10, 13, 17, 22, 24, 25 with base B = 52,
108, 167, 205, 257, 301, 413, 753, 664, 704 respectively; all other types
skip it with no draw. Den of Evil (8) skips it with no draw.
1. start = level-seed `lo' mod 15`.
2. Array a = 0 … 14. Fifteen times: i = `lo' mod 15`, j = `lo' mod 15`
   (two consecutive level-seed steps); swap a[i], a[j].
3. need = max(2, room count / 5 + 1); tries = 2 × room count.
4. While need > 0 and tries > 0: target def = B + a[start]; the first
   room in list order that is unlocked with that def gets def
   target + 15 (the "Theme" row), file −1, locked, and need −1.
   tries −1; start = (start + 1) mod 15.

31 level-seed steps always, even if no cell can change. a[k] = 0
targets the base def itself (B; no cell has it) and the 4-way shape
(mask 15) is never targeted, so 4-way cells never get a theme.

### 9. Building cells and file choice (`0x00673A60`, `0x006738C0`)

For each cell (list order):
1. Allocate a DS1 map for the cell's def and rect (`0x00666ED0`, owner
   `drlg/preset.md`). It always draws `roll(Files)` on the level seed as
   the default file.
2. File choice (`0x006738C0`, maze-owned): if the cell's file ≠ −1 it
   is used. Else, with base B' by type (3: 52, 4: 108, 7: 167, 8: 205,
   10: 257, 13: 301, 17: 413, 18: 481, 22: 753, 24: 664, 25: 704,
   28: 836, 33: 1002, 34: 1058, 35: 1052; other types: none), if
   B' < def < B' + 16: find the level's rotation record for this def
   (list at level +0x1CC); if none, create it with n = `Files` and
   value v = `roll(n)` on the level seed (`0x0045C3E0`), prepended to
   the list. Then v := (v + 1) mod n (signed) and the map's file := v.
   Otherwise the default from step 1 stays.
3. Build the DS1 into room(s) (`0x00667ED0`, `drlg/preset.md`) with
   room flags F = 0 and the single-room flag set when the cell's w ≤ 12
   and h ≤ 12.
4. For every link of the cell with the init flag (link list order,
   newest first), link the **room BuildArea returned** (the last room
   built: in multi-room mode the room of the last row's last column) to
   that neighbour with the same direction (`0x0066B5E0`); then free the
   cell (`0x0066C100`, `drlg/rooms.md` §2.1), which removes its links and
   the neighbours' links back to it. A neighbour that was a cell already
   built is no longer in the list: its own build moved its link to its
   built room, so the link resolves to that room. A neighbour still to
   be built gets a link to the new room, which its own step 4 later
   copies to its built room.

Effect: plain cells of one shape cycle through that def's files, the
first one at (roll(Files) + 1) mod Files. Special, theme and fixed cells
(def outside the range or explicit file) do not rotate.

## Constants & data dependencies

| Item | Value / source |
|---|---|
| lvlmaze fields | `Level` +0, `Rooms[3]` +4, `SizeX` +16, `SizeY` +20, `Merge` +24 (`data/fields.tsv`) |
| lvlprest fields | `Def` +0, `Files` +64 |
| Merge test | `lo' mod 1000 < Merge`; Merge 1000 always links, 0 never |
| Shape bits | W 1, E 2, S 4, N 8 |
| Theme offset | +15 defs; theme pool size 15 |
| Special tables | `maze-specials.tsv` (210 rows, VAs 0x006EF8A8–0x006F05C8) |
| Other tables | shape remaps 0x006EF758 (12-byte rows), 0x006EF6D8 (8-byte rows); hub 0x006EF888; lava cross 0x006EF828; Barracks finders 0x006F0468 = {0x00672460, 0x006724A0, 0x00672420} |
| Level fields (1.14d) | room count +0x08, room list +0x10, lvlmaze row +0x14, x/y/w/h +0x1C/+0x20/+0x24/+0x28, LevelType +0x1C0, level seed +0x1C4, rotation list +0x1CC, id +0x1D0, DRLG +0x1B4 |
| DRLG fields | staff tomb level +0x94, difficulty +0x450 (byte), act index +0x480 (byte), boss tomb level +0x484 |

## Randomness

Seeds: **L** = level seed, **R(c)** = DRLG room seed of cell c. Each
Allocate = L step + R(new) step (rooms.md). In generation order:

1. Allocate F.
2. Builder (§4 table), with these draws:
   - ring(n): per grow: Allocate, then merge draws (one R(other) step
     per unlocked, edge-touching, unlinked room).
   - grow tree: per iteration: L `roll(count)`, R(picked) step `& 3`,
     then (if picked is unlocked) Allocate + merge draws on success.
   - catacomb start: level 34 none; else L `& 1`; then the grows.
   - hub: L `& 3`, three grows.
   - spiral: L `& 3`, 60 grows (with merges).
   - lava cross: R(F) `& 3`; two fixed placements (Allocate each); fill
     blanks (8 Allocates per copied cell).
   - Cellar of Pity: L `roll(2)`.
   - special builders (§6): L `& 3` first (Act 2 sewers: two L steps;
     temple: L `mod 3`; tombs: none); each stamp that falls back costs
     one Allocate per tried parent; Act 2 Sewers 1 adds its probes
     (one Allocate each), grows, fixed placements and the H cell.
3. Barracks: probes, one fixed placement, L `& 1`, two stamps. River of
   Flame: two probe passes, two fixed placements, two Allocates (B2,
   B3), L `& 1`, one stamp, fill blanks.
4. Theme pass: 31 L steps (`mod 15` each) for the listed types except
   level 8.
5. Per cell in list order: preset map alloc `roll(Files)` on L
   (preset.md), then possibly maze file-rotation `roll(Files)` on L,
   then the preset build's own draws (preset.md).

Recording status: the Act 1 recording (`traces/raw/20261005-232125-rng.jsonl`)
initializes the level seeds of 8–12 (seq 2441–2445 server, 13390–13394
client; Den of Evil = {4014346877, 666}) but contains no draw at any
maze site (0x670C70–0x673FE0): no maze level is generated on entering
the Rogue Encampment.

## Edge cases & original bugs

1. Every allocation draws, including probes, rejected places, failed
   fallback tries and the eight fill-blank tries per cell. Room seeds of
   discarded cells are lost with them.
2. Grow-tree draws its two values even when the picked cell is locked.
3. Several layouts dereference a null cell if a placement is rejected
   (ring, spiral, Act 2 Sewers 1, Barracks court cell, River bridges).
   Shipped data never triggers it; d2rs should treat it as a fatal
   generation error, not as a skip.
4. If a stamp's fallback finds no parent, nothing is placed but the
   counter still advances.
5. Pick shape with a mask that has no shape (remap 0) changes nothing;
   the cell keeps def 0 / file 0 if it was never set (allocation zeroes
   both). Ice levels with `Rooms` = 1 would get the raw mask as def (no
   1.14d path does this: those levels are single fixed cells).
6. The theme pass can target def B (a[k] = 0) and never targets the
   4-way shape; with few cells many tries are wasted.
7. Normalization (§4.3) is fatal in 1.14d when the cells do not fit the
   level rect (D2MOO only warns).
8. Act 5 lava levels fill with the Act 4 def 836, and their rotation
   base is 1052, so their shapes (1053–1058) rotate files.
9. The crypt Portal rows and temple NE Waypoint row are unreachable.
10. Merge only looks at cells already in the list; the new cell's parent
    and corner-touching cells never draw.

## Test vectors

Synthetic, computed by hand from the rules and rng.md §2 (CI-safe).
Level seed = {dwStartSeed + id, 666}, dwStartSeed 4014346869 (rng.md
§5.4, test character). Positions assume the level rect from
`drlg/levels.md`.

| Input | Expected | Source |
|---|---|---|
| Den of Evil (8), Normal, L = {4014346877, 666}, level rect (1500, 1000, 200, 200) | L1 = 2583727307: F at (1588, 1088), 24 × 24, F's init seed 3298855633. Grow tree target 1: no draw. L2 = 678241120 → r = 0. Stamp cave_prev[0]: no Cave N (F has def 0) → fallback dir 3 from F: L3 = 3697141424 allocates P at (1588, 1112); F → def 56 (Cave S); P = 86 Cave Prev N, locked; r = 1. Stamp cave_doe[1]: no Cave E → fallback dir 0: P locked is skipped, from F: L4 = 3271692635 allocates D at (1564, 1088); F → def 57 (Cave SW); D = 96 Cave Den Of Evil E, locked; r = 2. Normalize by (−64, −88): D (1500, 1000), F (1524, 1000), P (1524, 1024). No theme pass. Build order D, P, F; D's map alloc `roll(1)` = L5 (value 0) | rules |
| Spider Cavern (85), L = {4014346954, 666} | L1..L4 = 3082426380, 80731269, 2373915171, 3826282939 allocate F, A (N of F), B (W of A), C (S of B). Only merge draw: when C is placed, F's room seed second step = 500245367 → 367 < 500 → F–C linked (dir 0 from F). Final defs (list order C, B, A, F): C 662 Spider NE, B 660 Spider SE, A 659 Spider SW, F 663 Spider Chest NW (661 remapped for level 85); all file −1, unlocked. No stamps, no theme pass. L5 = 1615270581: C's map alloc `roll(3)` = 0 (no rotation for type 23) | rules |
| Grow-tree target | Tal Rasha tomb with lvlmaze Rooms 6: 18 if staff tomb, 12 if boss tomb, else 6 | §5.2 |
| ring(2) | 3 grows: N, W, S of F; F ends NW-shaped (mask 9), the three others SW/SE/NE; exactly one merge draw (on F, when the S cell is placed) | §5.1 |
| Theme pass need | room count 9 → need 2, tries 18; count 16 → need 4, tries 32 | §8 |

## Provenance

1.14d `Game.exe`, all addresses read in `re/exports/all.asm` (D2MOO
1.10f `DrlgMaze.cpp` used as a hint; every rule above was read from the
1.14d code). Tables were dumped from the binary by VA.

| 1.14d | D2MOO | Notes |
|---|---|---|
| 0x0061F490 | `DATATBLS_GetLvlMazeTxtRecordFromLevelId` | linear scan, fatal if absent |
| 0x00670810 | (inline) | lvlmaze `Rooms[d]` = 1 |
| 0x00670880 | `LinkMazeRooms` | dir in EAX, parent in ESI |
| 0x00670970, 0x00670C10, 0x00670D70, 0x00670E70 | `SetPickedFileAndPresetId`, `ReplaceRoomPreset`, `PlaceAdjacentPresetRoom`, `AddAdjacentMazeRoom` | present but unused or inlined in 1.14d |
| 0x006709B0 | `PickRoomPreset` | room in EBX; all 70 calls pass 1 |
| 0x00670C70 | `MergeMazeRooms` | |
| 0x00670DE0 | `InitRoomFixedPreset` | |
| 0x00670EB0 | `AddSpecialPreset` | |
| 0x00670F60 | `InitBasicMazeLayout` | |
| 0x006711A0 | `GetRandomRoomExFromLevel` | inline `roll` |
| 0x00671210 | `BuildBasicMaze` | also used for Cave/Crypt (D2MOO inlines an equivalent loop) |
| 0x00671320 | `FillBlankMazeSpaces` | string `.\DRLG\Maze.cpp` |
| 0x00671430 | `PlaceAct5LavaPresets` | |
| 0x006714D0 | catacomb case of `GenerateLevel` | |
| 0x006718C0 | `PlaceAct2TombPrev_Act5BaalPrev` | |
| 0x006719F0 | `PlaceArcaneSanctuary` | |
| 0x00672340 | `CheckIfMayPlaceAdjacentPresetRoom` | |
| 0x006723E0, 0x00672420, 0x00672460, 0x006724A0 | `GetFreeLocationForRoom*` | 0x00672420 decoded from raw bytes (not in `functions.tsv`) |
| 0x006724E0 | `ScanReplaceSpecialPreset` | fallback = 0x00670EB0 |
| 0x00672550, 0x00672610, 0x006726D0, 0x006727A0 | cave/crypt/jail/catacomb cases of `GenerateLevel` | |
| 0x00672810 | `ScanReplaceSpecialAct2SewersPresets` | |
| 0x00672BE0 | `PlaceAct2TombStuff` | |
| 0x00672DC0 | `PlaceAct2LairStuff` | restructured, same result |
| 0x00672E50 | arcane case of `GenerateLevel` | |
| 0x00672EA0, 0x00672F00, 0x00672F60 | `PlaceAct3DungeonStuff`, `…SewerStuff`, `…MephistoStuff` | |
| 0x00673000, 0x006730B0 | `PlaceAct5TempleStuff`, `PlaceAct5BaalStuff` | |
| 0x00673120 | `PlaceAct1Barracks` | |
| 0x00673320 | `PlaceAct4Lava` | |
| 0x00673530 | `PlaceAct5IceStuff` | |
| 0x006735F0 | `RollAct_1_2_3_BasicPresets` | |
| 0x006738C0, 0x00673A60 | `RollBasicPresets` | split in 1.14d; string `.\DRLG\Maze.cpp` at 0x006739A2 |
| 0x00673B10 | `InitLevelData` | |
| 0x00673B30 | `GenerateLevel` | D2MOO comment "1.14d: 0x00673B30" confirmed |
| 0x00673FE0 | `ResetMazeRecord` | |

1.14d differences from D2MOO found: normalization and the Act 3 Sewers
corner swaps are fatal instead of warnings; Matron's Den (133) gets the
crypt chest (D2MOO behind `D2_VERSION_HAS_UBERS`); stamps always use the
shared 0x006724E0 + 0x00670EB0 pair (D2MOO inlines several equivalent
variants); the Cave/Crypt grow loop is the shared 0x00671210.
Data: `lvlmaze.bin` and `lvlprest.txt` from `game/extracted/patch_d2`
(81 lvlmaze records); `levels.txt` DrlgType/LevelType columns.
Recording: `drlg_rng.py` over `20261005-232125-rng.jsonl` (no maze
sites; level seed writes at 0x642B62 for levels 8–12).

## Open questions

1. Trace confirmation: no recording yet generates a maze level; record
   entering Den of Evil (8) and Cave Level 1 (9) and compare with the
   vectors above.
2. Whether the Act 3 and Act 5 DRLGs use the same dwStartSeed as Act 1
   (assumed for the Spider Cavern vector; `drlg/levels.md` owns it).
3. `0x00673FF0` (just after the maze functions; returns a bit for level
   ids in an 8-entry list from `0x0066C040`) is not referenced by maze
   code; its owner is unknown.
4. The level rect used by normalization and by the first-cell centring
   (`0x00642D10`, `drlg/levels.md`) must give Den of Evil (1500, 1000,
   200, 200) for the first vector; confirm there.
