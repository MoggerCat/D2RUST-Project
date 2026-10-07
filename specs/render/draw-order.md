# Spec: Render — Draw order (passes, the draw-cell grid, tile and unit lists)

- **Status:** draft (2026-10-06, RE on 1.14d `Game.exe`; no capture yet).
  Every rule names its 1.14d address; the pixel proof is the capture
  cases of §Test vectors (unverified until they run).
- **Target version:** 1.14d
- **Crate/module:** `d2-client::scene::order` (pass numbers), `d2-client::
  world_view` (`ViewRules::tiles`, `unit_params`, UI pass),
  `d2-client::rules` (`ViewSource::map_tiles`, `MapTile`)
- **Related specs:** `render/camera.md` (§6 tile positions, §7 culling),
  `render/sprite-placement.md` (§7 DT1 drawers), `render/unit-composite.md`
  (what one unit draws), `drlg/rooms.md` §9 (tile records, variant choice,
  hidden flag, animation), §3/§6 (near-room arrays), `render/map-preview.md`
  (Phase 1b simplification this replaces), `client/render-pipeline.md` §A6,
  §B6, §B10; continued in `render/draw-order-2.md` (§11 weather, §12
  level backgrounds, §13 pass 8, §14 edge floors, §15–§16 sight test)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 43–56 |
| Inputs | 57–65 |
| Outputs / state changes | 66–72 |
| Rules | 73–74 |
|   1. Frame passes | 75–100 |
|   2. The draw-cell grid (`0x004DCE60`, `0x004DDB70`) | 101–130 |
|   3. Filling the grid (`0x004DD7C0` per room) | 131–175 |
|   4. List insertion | 176–186 |
|   5. Which units draw | 187–209 |
|   6. The passes | 210–288 |
|   7. Tile records that never draw | 289–302 |
|   8. Wall fade targets (`0x004DD180`, `0x004DD060`) | 303–359 |
|   9. Map-tile feed | 360–380 |
|   10. d2rs mapping | 381–430 |
| Constants & data dependencies | 431–439 |
| Randomness | 440–445 |
| Edge cases & original bugs | 446–463 |
| Test vectors | 464–492 |
| Provenance | 493–529 |
| Open questions | 530–625 |
<!-- /index -->

## Summary

Each drawn frame the client rebuilds a square grid of **draw cells** (one
per map tile around the view), files every visible wall-array tile record,
shadow-array record and unit of the player's near rooms into per-cell
lists, then draws the world in fixed passes: lower walls, floors (straight
from the rooms, not the grid), the shadow pass (tile shadows, flat units,
unit shadows), walls and standing units cell by cell, roofs four times by
layer bit, then screen effects and the UI. Order inside a pass is grid
order (rows of tile y, then tile x) and list order inside a cell. This spec
owns the passes, the grid, which records and units enter which list and in
which order, the tile-record skip tests and the map-tile feed. Positions
are `camera.md`; blend, shading and lighting are hooks named here.

## Inputs

| Name | Source |
|---|---|
| local player, its active room and that room's near-room array | client DRLG / bridge (`drlg/rooms.md` §3, §6 own the array and its order) |
| per room: tile origin, wall / floor / shadow record arrays (0x30-byte records) | client DRLG copy (`drlg/rooms.md` §9) |
| per room: unit list (room +0x74, next unit +0xE8) | client units |
| view rectangle, tile origin `(cx_t, cy_t)`, open mode | `camera.md` §1, §3 |

## Outputs / state changes

The draw order of every world item of a frame. Writes during the frame:
record flags 0x400 / 0x8 and fade bytes (§8), record flag 0x20000 (§6
r6), unit flag 0x10000000 when a unit draws, unit flag-ex
0x80 (§5 r3).

## Rules

### 1. Frame passes

The in-game draw `0x0044C990` builds the grid (`0x00476140` → `0x004DDB70`
with "skip units" = 0, view = `[0x007A0640]`; not in open mode 3), then
draws the world `0x00476BC0` (not in open mode 3), then `0x00456EE0`,
`0x004F98E0`, `0x00468820`, `0x004684C0` (when its argument is 0),
`0x00477980` and a timed screen fill (`0x004F63B0`). The world draw, in
order:

| # | Pass | Function | Condition |
|---|---|---|---|
| 1 | level background | `0x00476290` (level 74) / `0x00476460` (level 120) | player's level (`draw-order-2.md` §12) |
| 2 | lower walls | `0x004DF480` → `0x004DEDF0` per cell | grid flag lower walls (`[0x006CE268]`) |
| 3 | floors | `0x004DED10` → `0x004DE730` | always |
| 4 | environment pools: rain splashes, mud bubbles | `0x00473C00` | live pools (`draw-order-2.md` §11.6) |
| 5 | shadow pass | `0x004DF510` | grid flag shadows (`[0x006CE274]`) |
| 6 | walls and units | `0x004DF1C0` | always |
| 7 | roofs | `0x004DEA70` | grid flag roofs (`[0x006CE26C]`) |
| 8 | light-map debug view (never runs) | `0x00475B20` | `[0x007B9564]` ≠ 0 (`draw-order-2.md` §13) |
| 9 | lightning flash, rain / snow particles | `0x00473910` | always (`draw-order-2.md` §11.7) |
| 10 | screen fade | `0x004DC000` | `[0x007C89CC]` running: fill of the play area `[0,W) × [0,H−47)` (half width with a panel open), alpha `255 × remaining / 500` ms |

The three grid flags live at view +0x38; the builder sets them when it
files an entry of that kind (§4) into a pool slot (not for a dropped
entry, §2).

### 2. The draw-cell grid (`0x004DCE60`, `0x004DDB70`)

With view width `Wv` = view +0xC − view +4 and height `Hv` = view +0x10 −
view +8 (`camera.md` §1: `W` and `H − 40`), C signed division:

- `a = Wv / 160 + 11`, `n = a + Hv / 80 + 11` (side); 34 at 800 × 600,
  31 at 640 × 480.
- `T(x, y)` = tile of a client pixel (`0x00643340`):
  `tx = q(x + 2y)`, `ty = q(2y − x)` with `q(v) = v / 160` (truncating)
  for `v ≥ 0` and `v / 160 − 1` for `v < 0` (so `q(−160) = −2`; the
  negative case never occurs for map tiles or units).
- Origin `(x0, y0) = T(cx_t, cy_t) − (3, a)`; cell `(c, r)` (0 ≤ c, r < n)
  is tile `(x0 + c, y0 + r)` and has index `r × n + c`. Every pass walks
  cells in **index order** (rows of tile y, tile x inside a row).
- A cell (0x24 bytes) holds a flag word and five list heads: shadow list,
  wall list, unit list, roof list, lower-wall list. All are cleared, and
  the entry pool emptied, every drawn frame.
- Entries come from one pool of **3,000** per frame (view +0x3C, count
  +0xEA9C). An entry past the 3,000th is dropped silently (the count still
  rises). Entry: kind (0 unit, 1 tile, 2 unit shadow), x, y (used only by
  the perspective renderer), pointer, next.
- **Pool overflow.** Every inserter (`0x004DD550`, `0x004DD460`,
  `0x004DD350`, `0x004DD180`, `0x004DCFA0`, `0x004DD600`, `0x004DD6E0`)
  first increments the count and stops when the old count was ≥ 3,000.
  A dropped entry therefore sets no grid flag (§1) and, for a wall-list
  record, gets no fade-target update (§8: `0x004DD180` returns before
  reading `GetTickCount`). Cell flag 4 (§3 r2) is set **before** the pool
  test (`0x004DD350`, `0x004DD180`), so a dropped lower wall or wall still
  sets it.

### 3. Filling the grid (`0x004DD7C0` per room)

Rooms: the near-room array of the local player's active room
(`0x004646A0`, array +0x00, count +0x24), in array order (`0x00619BD0`).
View rectangle in client pixels: `left = cx_t`, `top = cy_t`,
`right = cx_t + Wv`, `bottom = cy_t + Hv`. Per room:

1. **Room test.** Room corners `E(x, y) = ((x − y) × 80 − 80, (x + y) × 40
   + 80)` (`0x006433C0`) on the room's tile rectangle (+0x5C x, +0x60 y,
   +0x64 w, +0x68 h): room left = `E(x, y+h).x`, top = `E(x, y).y`,
   right = `E(x+w, y).x`, bottom = `E(x+w, y+h).y` (`0x00619C40`, fatal
   if left > right or top > bottom). The room is skipped unless room left
   ≤ right, room top ≤ bottom + 400, room right ≥ left − 200, room bottom
   ≥ top.
2. **Wall array** (records in array order). Record entry `(e0, e1)`
   (`camera.md` §2), DT1 tile `t` with height `h` (+0x08) and roof height
   `rh` (+0x04): kept when `e0 ≤ right`, `e0 + 160 ≥ left`,
   `e1 − rh + h ≤ bottom`, `e1 − rh − h ≥ top`, and its cell
   `T(e0, e1) − (0, 1)` (= the record's own tile) is inside the grid.
   The cell's flag word is set to 0, then the record is filed by:
   - no layer bits (flags & 0x1C000 = 0) → shadow list, append;
   - type 15 → roof list, layer-sorted (§4);
   - type 16–19 → lower-wall list, layer-sorted;
   - any other type → wall list, layer-sorted, after the fade target
     update of §8.
   Lower walls and walls also set cell flag 4 when their type is not 0 or
   13 and record flag 0x4 is clear (Open question 5).
3. **Shadow array**: same test without `rh`; every kept record → shadow
   list, append, kind 1.
4. **Units** (room unit list, in list order), unless "skip units": cell =
   `T(px, py)` of the unit's client position (`camera.md` §2,
   `0x00620650` / `0x006206B0`) minus the origin; outside the grid → not
   filed. Then (`0x00464860` true or unit flag 0x100000) → shadow list,
   append, kind 0 (a **flat** unit); else unit list, append, kind 0, and,
   when flag-ex 0x80 is set, a kind-2 entry (the unit's shadow) appended
   to the shadow list. Units have no rectangle test here. The room's
   list is first sorted by y in place (`0x00619EA0`, `sim/unit-order.md`
   §5 rule 7); which client code links units into room lists:
   `sim/unit-order.md` §5 rule 6.

Flat units (`0x00464860`): players in mode 17 (dead); monsters in mode 12
(dead) without monstats2 `unflatDead` (bit 20); objects with `DrawUnder`
bit 1, or bit 0 in mode 2; missiles with unit flag 0x10000; items in mode
3 (on the ground).

### 4. List insertion

- **Append** (shadow list, unit list): at the tail.
- **Layer-sorted** (wall, roof, lower-wall lists; `0x004DD180`,
  `0x004DD460`, `0x004DD350`): with `ℓ` = record flags bits 14–16 (layer
  + 1, `drlg/rooms.md` §9.5.1), walk the list from the head; at each
  element **that has a successor**, if the new `ℓ` < that element's `ℓ`,
  insert before it and stop; when the walk reaches the last element,
  append. The last element is never compared, so a one-element list
  always appends. Reproduce.

### 5. Which units draw

The unit draw entry `0x004DC7B0` (from the shadow pass and the wall pass,
"inline" = 0):

1. Skipped: flag-ex bit 18; a dead player (mode 17) without state 7
   `playerbody`; a monster or object with state 143 `attached` or 146
   `invis` (those draw inline, `unit-composite.md` §5 r1).
2. Perspective pre-test (`0x004F66E0`): not in the reference renderer.
3. **Sight test** (`0x004DC710`): other players not in mode 0 or 17,
   monsters not in mode 0 or 12, missiles and items are tested; objects
   and the local player always pass. If the level of the local player's
   room fails `0x00642840`, pass; else the unit is hidden when
   `0x00622AA0(local player, unit, 2)` ≠ 0. Hidden: flag-ex 0x80 cleared,
   not drawn; else flag-ex 0x80 set. (Flag-ex 0x80 also gates the shadow
   entry of §3 r4, so a unit's shadow follows the previous frame's
   result.) The level gate is leveldefs `LOSDraw` and `0x00622AA0` the
   size-shrunk line test with mask 2: `draw-order-2.md` §15, §16.
4. Draw at the unit's client position through `0x00471EC0`
   (`unit-composite.md` §1, missile offsets §8); players between
   `0x004D8520` / `0x004D85C0`, monsters with flag a = `0x004AE340` ≠ 0.
   A drawn unit gets flag 0x10000000.

### 6. The passes

1. **Lower walls** (`0x004DEDF0`, cells in order, lower-wall list): each
   record with layer bits and neither flag 0x8 (hidden) nor 0x400 (faded
   out) draws with the wall drawer at the wall position (`camera.md` §6
   walls row): opaque `0x004F6920` when its alpha byte (+0x28) is 0xFF,
   else translucent `0x004F6950` with that alpha. Light: one value per
   block from `0x00475AA0` (count by DT1 light direction, table
   `0x006DB9D8`): `render/lighting.md`.
2. **Floors** (`0x004DE730`): rooms in near-array order; per room floor
   layer value `ℓ` = 1 then 2; per layer the floor array in order, records
   whose `ℓ` matches (layer values 3 and 4 never draw). A record draws
   (`0x004DE410`) when flags & 0x408 = 0, its handed position (`camera.md`
   §6 floors) passes `camera.md` §7 and its DT1 orientation is 0; drawer
   `0x004F68E0` with a filter flag set only for `ℓ` = 1 in levels whose
   `FloorFilter` (+0x214) ≠ 0 (`render/shading.md`), light grid per
   `render/lighting.md` §11 (`0x00477730` only returns the render kind). After a draw: DT1 material bit
   0x2 tiles may start client water effects (`0x00472DA0` / `0x00472EC0`)
   with a draw `0x00472280(1000)` on the local player's seed, made for
   every such drawn floor (`draw-order-2.md` §11.5); `0x004DDE80` records
   the drawn extents. Levels with `DrawEdges` (+9) at open mode 0 and
   resolution mode 2 draw up to 3 extra edge floors per side near the
   drawn extents (`0x004DE6C0` / `0x004DE630`, `draw-order-2.md` §14).
3. **Shadow pass** (`0x004DF510`, cells in order, shadow list in list
   order): kind 0 → the unit (§5); kind 1 → DT1 shadow tile through
   `0x004F6980` (slot `+0xA4`) at the wall position; the caller passes
   draw mode 4, which the GDI drawer `0x006C9290` never reads (its blend
   is fixed: blended shadows or an opaque copy, `render/blend-modes.md`
   §5, branch `claude/spec-shading-blend`; Open question 4); kind 2 → the unit's shadow `0x00471620`
   (position and blend: `render/blend-modes.md` §5). After a cell's list, its wall list is walked once for
   records without layer bits (none exist, §3 r2; the walk still runs the
   §8 fade updates).
4. **Walls and units** (`0x004DF1C0`, cells in order): the cell's wall
   list (fade update §8, then draw as r1 when it has layer bits and not
   0x408; a type-15 record here is fatal 0xF9), then the cell's unit list
   (§5).
5. **Roofs** (`0x004DEA70`): for `L` = 1, 2, 3, 4 with mask `L << 14`,
   cells in order, roof list: records with `flags & mask = mask` (all
   mask bits set, so `ℓ` = 3 draws in passes 1, 2 and 3; `ℓ` = 5–7
   cannot occur) and not 0x408 draw with the **floor drawer** `0x004F68E0`
   at `camera.md` §6 roofs row, alpha byte as given, culled by `camera.md`
   §7; drawn records get flag 0x20000. This settles RC1: roofs use the
   floor drawer path (camera §6), not the wall drawer.
6. **Record flag 0x20000** ("drawn") is set after the draw call, so the
   culling of `camera.md` §7 comes first. Lower walls (`0x004DEDF0`),
   walls (`0x004DF1C0`) and the shadow pass's wall-list walk
   (`0x004DEF80`) set it when the wall drawer returns non-zero: the GDI
   wall drawers (`0x006C94B0` lit, `0x006C93A0` translucent) return 1
   when at least one block passed their block test, 0 when the tile does
   not load (`0x005FDEA0`) or every block was culled. Roofs
   (`0x004DEA70`) set it unconditionally once the record passed the
   whole-tile view test, whatever the floor drawer did. Floors set it
   too: `0x004DE410` calls `0x004DDE80` after every floor draw (the
   record passed the whole-tile test of `camera.md` §7), which sets the
   flag unconditionally and then widens the drawn extents. Shadow tiles
   and units never set it. No draw pass reads it; the reader is the
   automap reveal (r7).
7. **Automap reveal** (`0x00459020`, called by the frame `0x0044C7EB`):
   skipped while the countdown `[0x007A51A4]` is non-zero (it is
   decremented instead); otherwise, when the local player's position
   moved by `d ≥ 0x50` since the last reveal (`|Δx|`, `|Δy|` from the
   stored `[0x007A51FC]`/`[0x007A51F4]`: `d = (2·max + min) / 2`, C
   division), it stores the new position and walks the player room's
   near-room array; for each room of the player's level it calls
   `0x00458F40(room, 0, cell)`. That function walks the room's floor
   array (`0x00619660`: tile data +0x08 / +0x0C, §9) and then its wall
   array (`0x006196A0`: +0x00 / +0x04, the getter the grid fill uses) in
   order and
   adds to the automap (`0x00457CF0`, owner: the automap spec) every
   record without flag 0x8 that has flag 0x20000, or any record when
   `[0x007A51A0]` ≠ 0 (no writer in `Game.exe`: 0) or the second
   argument is 1; then `0x00458DC0` (room objects). The DRLG room-init
   callback `0x00459150` (drlg `+0x454`, `client/model.md` S→C 0x03)
   calls it with 1 for every room of a preset level whose lvlprest
   `AutoMap` ≠ 0 (`drlg/preset.md` step 4), so those levels are revealed
   whole. So a wall or floor
   joins the automap only after one frame drew it (r6) and the player
   then moved 0x50 or more.

### 7. Tile records that never draw

A record of the wall, floor or shadow arrays is not drawn when: flag 0x8
(hidden: `drlg/rooms.md` §9.5.1, animation frames §9.7, linking §9.6);
flag 0x400 (faded to alpha 0, §8); no layer bits (walls / lower walls /
roofs; floors need `ℓ` 1 or 2); floor whose DT1 orientation ≠ 0; outside
the tests of §3 or `camera.md` §7; its cell beyond the grid. No test looks
at the orientation of a wall-array record (10/11 specials included), its
DT1 file, material, sub-tile flags or rarity. So the variant drawn is the
record's tile as the client DRLG chose it (`drlg/rooms.md` §9.4, rarity),
and `map-preview.md` OQ3 (River.dt1 #28 at 8 `townN1` cells) has no
draw-path skip: its records draw unless the DRLG hides them (Open question
7).

### 8. Wall fade targets (`0x004DD180`, `0x004DD060`)

For each wall-array record filed into a wall list whose record flag 0x4 is
clear, at `t` = `GetTickCount() + 500`, with the record's absolute subtile
`(lx, ly)` = room subtile origin + 5 × record tile position and the
player's tile `(px, py)` (`[0x007C8A08]`, `[0x007C8A10]`, set by
`0x004DDB70` as path subtile / 5):

- "near" (`0x004DD060`) in 1.14d is the **group mode**: `[0x0072A968]`
  is 1 in `.data` and has no writer (its one reference is this read), so
  the geometric branch below never runs. With `G` = the record's
  coordinate record (+0x10; `drlg/levels.md` §11.1): near when `G` ≠ 0,
  `G` index (+0x28) ≠ the player's index `[0x007C8A0C]`, and (`px` <
  `G` x0 (+0x00) or `py` < `G` y0 (+0x04)) — the wall belongs to another
  logical room lying in front of the player (larger x or y). The
  player's index is `0x0061B130(player room, path sub-tile x, y)`, set by
  `0x004DDB70` each frame (0 when the point is in no room, −1 for a null
  record). Record +0x10 is set only by the grid build of a preset room
  with lvlprest `Logicals` ≠ 0 (`0x0066C9C0` at the end of `0x0066D110`:
  the coordinate record at the wall's tile); every other wall record has
  0 (arrays are zeroed, `0x0066EEE0`), so walls of outdoor levels and of
  presets without `Logicals` (the towns included: `Act 1 - Town 1` has
  `Logicals` 0) never fade.
- Unused geometric branch (`[0x0072A968]` = 0): near when `5px < lx <
  5px + 20` and type ∈ {1, 4, 5, 7, 8, 10, 12}, or `5py < ly < 5py + 20`
  and type ∈ {2, 3, 6, 7, 9, 11, 12}.
- Near, fade state (+0x24) bit 0 clear: from 0xFF to 0x80, end time
  `t + 500 × (alpha − 0xFF) / 127` (`t` when alpha is 0xFF; C division),
  state |= 3. Not near, bit 0 set: from 0x80 to 0xFF, end time
  `t + 500 × (0x80 − alpha) / 127` (`t` when alpha is 0x80), state bit 0
  cleared, bit 1 set. Otherwise nothing changes.
- Each pass that walks a wall-array record (§6 r3–r5; the lower-wall
  pass r1 only applies state bit 2, its records never get a fade target)
  first advances a running fade (state bit 1): alpha := to when
  `0x00477730` ≤ 3 or the end time is reached (bit 1 cleared), else
  `from + (to − from) × (now − end + 500) / 500` in bytes; alpha 0 sets
  flag 0x400, other values clear it; state bit 2 sets flag 0x8 once its
  end time is reached. The translucent draw of the resulting alpha is
  `render/blend-modes.md` §6 (branch `claude/spec-shading-blend`).
- **Clock arithmetic** (`0x004DD180`, `0x004DEF80`, `0x004DF1C0`,
  `0x004DEA70`, `0x004DEDF0`): `now` is one `GetTickCount()` read per
  cell in the lower-wall, shadow and wall passes and once for all four
  roof passes, an unsigned 32-bit millisecond
  count. End times are stored as 32-bit values (`t` + the signed C
  quotient, wrapping mod 2^32). "End time reached" is the **unsigned**
  compare `end ≤ now`, so it misfires across the 49.7-day wrap of
  `GetTickCount` (reproduce: compare as `u32`). The ramp product
  `(to − from) × ((now − end) + 500)` is a 32-bit signed product
  (`now − end` wrapping), divided by 500 with C truncation; its low byte
  is added to `from` mod 256. The reference renderer (GDI, render kind
  `0x00477730` < 4, `capture.md` §3.4) completes every ramp at the first
  walk, so only the bit-2 compare depends on the clock there. d2rs: the
  `FadeClock` hook gives `now` as a `u32` millisecond count; a capture
  case without recorded `GetTickCount` values must not contain a record
  with state bit 2 set; no 1.14d code sets that bit (Open question 16,
  answered), so the bit-2 branch is unreachable from original data.

### 9. Map-tile feed

What the client draws per frame is fully determined by: the local
player's active room and its near-room array (order: `drlg/rooms.md` §3,
§6); per room, the tile origin (subtile +0x4C/+0x50, tile +0x5C/+0x60)
and the three record arrays of its tile data (+0x08: walls +0x00/+0x04,
floors +0x08/+0x0C, shadows +0x10/+0x14); per record (0x30 bytes):
entry `(e0, e1)` (+0x00, +0x04), tile position in the room (+0x08,
+0x0C), flags (+0x14), DT1 tile (+0x18: orientation +0x14, main +0x18,
sub +0x1C, roof height +0x04, height +0x08, light direction +0x00),
type (+0x1C), fade state (+0x24), alpha (+0x28), fade from / to (+0x29,
+0x2A), fade end (+0x2C). Records, their order, variants and flags come
from the client DRLG copy (`drlg/rooms.md` §9; `drlg/preset.md`,
`drlg/outdoor.md` for the grids); this spec only reads them.

A recorder (`capture.md` §3, RW1) can read the frame's lists directly:
view `[0x007A0640]`, grid base +0xEAA8, side +0xEAB0, origin +0xEAA0 /
+0xEAA4, pool +0x3C (20-byte entries), pool count +0xEA9C, flags +0x38;
per tile entry, the DT1 file is not stored in the record (library slot of
the room: Open question 12).

### 10. d2rs mapping

`DrawKey` (`render-pipeline.md` §A6) is filled so that a stable sort
reproduces §1–§6; items are built in the same order:

| Pass | `pass` | `major` | `minor` |
|---|---|---|---|
| level background | 1 | 0 | build order |
| lower walls | 2 | cell index | position in the cell's lower-wall list |
| floors | 3 | 2 × room position in the near array + (`ℓ` − 1) | record index in the floor array |
| environment pools (`0x00473C00`) | 4 | 0 for splashes, 1 for bubbles | slot index (`draw-order-2.md` §11.6) |
| shadow pass | 5 | cell index | position in the shadow list |
| walls and units | 6 | cell index | wall-list position; units: wall count + unit-list position |
| roofs | 7 | (`L` − 1) × n² + cell index | roof-list position |
| `0x00475B20` (never runs), `0x00473910`, screen fade | 8, 9, 10 | 0 | build order (pass 9: flash, else particles in slot order) |
| UI (everything after `0x00476BC0`) | 11 | `client/ui.md` | `client/ui.md` |

`sub`: `unit-composite.md` §10. `ViewSource::map_tiles` returns the kept
records (§3, §6 tests) with their pass and list position; `TileList` needs
two more kinds placed as walls: lower wall and shadow tile. Cell index
ranges below n² ≤ 2^28 for any frame size the client supports.

Implementation questions (`impl-draw-order`, 2026-10-06): DO1 (vector
`T(−160, 0)`) is answered by the corrected Test vectors row (−2, 1); the
§2 rule was right. DO3 by §2 pool overflow (a dropped entry sets no grid
flag). DO4 by §6 r6: lower walls set 0x20000 like walls, after the
drawer returned non-zero, so the drawer's block culling (`camera.md` §7)
comes first; a record that is not drawn (hidden, faded out, no layer
bits) never gets it. DO5 by §8 clock arithmetic: `now` and end times are
`u32` millisecond counts compared unsigned; the render kind
`0x00477730` is the display type (`composition.md` §1: 1 GDI, 3
DirectDraw default, 4 / 6 the 3D drivers), so on the reference (1) and
the default renderer every ramp completes at its first walk and no
`FadeClock` value changes a pixel; the hook is needed only for the
unreachable bit-2 branch (OQ16).

A shadow tile item carries no draw mode: its blend is the shadow-tile
rule of `render/blend-modes.md` §5, whatever mode the caller names (§6
r3). A unit's items keep the order's `pass` / `major` / `minor` and take
`sub` from the composite; the unit's own position and offsets are
`camera.md` §4 and `unit-composite.md` §8. An item this order emits whose
drawing has no spec yet makes the frame an error, never a silent skip.
Since 2026-10-06 the former gaps are specified: water effects and passes
4 and 9 (`draw-order-2.md` §11; the particle floats are its Open
question 3, so a frame with live rain or snow particles stays an error
until it is answered), level backgrounds (§12; they need the recorded
background seed), pass 8 (§13: emits nothing), edge floors (§14; the act
edge record is its Open question 2), the sight test (§15, §16) and the
fade group mode (§8). Pass 10 has no d2rs input (screen fade timer).

## Constants & data dependencies

Grid margins 11, 11, 3 (§2); room slack 400 (bottom), 200 (left); tile
pitch 160 × 80; pool 3,000 entries; fade 500 ms, alphas 0x80 / 0xFF; pass
masks `[0x006CE268]`, `[0x006CE26C]`, `[0x006CE274]` (entries of the
bit table `0x006CE268`, also used by `0x004638A0`). Tables: `levels`
`DrawEdges`, `FloorFilter`; `monstats2` `unflatDead`; `objects`
`DrawUnder`; `states` 7, 143, 146.

## Randomness

None in the ordering. The floor pass draws `0x00472280(1000)` for each
drawn material-0x2 floor (§6 r2); the level-120 background uses its own
LCG at `[0x00712C50]` (multiplier 0x6AC690C5). Both: Open questions 1, 11.

## Edge cases & original bugs

- Layer-sorted insertion never compares the last element (§4).
- Roofs of layer value 3 draw three times (§6 r5); with alpha < 0xFF the
  three draws compound.
- The cell flag word is reset by every wall-array record filed in the
  cell, so it reflects the last one (§3 r2).
- Entries past 3,000 vanish without a message (§2).
- A unit's shadow appears one frame after it becomes visible (§5 r3).
- Floors are drawn per room, so overlapping floor tiles of two rooms
  (shared border cells) follow room order, not cell order.
- A pool-dropped wall still sets cell flag 4 but gets no fade target, and
  a dropped entry sets no grid flag (§2 pool overflow).
- Fade end times are compared unsigned, so a ramp running across the
  `GetTickCount` wrap overshoots (§8 clock arithmetic).
- The drawer's "shadow tile" draw mode 4 is passed but ignored by GDI
  (§6 r3).

## Test vectors

| Input | Expected | Source |
|---|---|---|
| 800 × 600 mode 0 (`Wv` 800, `Hv` 560) | `a` = 16, `n` = 34; 640 × 480: 15, 31 | §2 |
| `T(600, 1720)`; `T(−160, 0)` | (25, 17); (−2, 1): `tx = q(−160) = −2`, `ty = q(2·0 + 160) = q(160) = 1` (the earlier (−2, −1) had the sign of `ty` wrong; `0x00643340` confirms `ty = q(2y − x)`) | §2 |
| `T(0, −80)`; `T(−1, 0)` | (−2, −2): `q(−160)` both; (−1, 0): `q(−1) = −1`, `q(1) = 0` | §2 |
| tile origin (600, 1720) | grid origin (22, 1); player at client (1000, 2000) → cell (9, 17), index 587 | §2, §3 |
| wall record of tile (30, 20): `(e0, e1)` = (720, 2080) | `T` = (30, 21) → cell tile (30, 20) | §3 r2 |
| wall list `[ℓ2]`, insert `ℓ1` | `[ℓ2, ℓ1]` | §4 |
| `[ℓ2, ℓ3]`, insert `ℓ1` | `[ℓ1, ℓ2, ℓ3]` | §4 |
| `[ℓ1, ℓ3]`, insert `ℓ2` | `[ℓ1, ℓ3, ℓ2]` | §4 |
| roof `ℓ` 1 / 2 / 3 / 4 | drawn in roof passes {1} / {2} / {1, 2, 3} / {4} | §6 r5 |
| floor record `ℓ` = 3 | never drawn | §6 r2 |
| dead monster (mode 12), `unflatDead` 0 | shadow list, kind 0, drawn in pass 5 | §3 |
| item on the ground (mode 3) | shadow list, pass 5 | §3 |
| 3,001st entry | dropped | §2 |
| empty grid, pool full (3,000), file one roof (type 15) and one shadow-array record | count 3,002; roof and shadow grid flags stay clear; nothing filed | §2 pool overflow |
| pool full, file a wall (type 1, record flag 0x4 clear) | cell flag 4 set; record fade state, alpha and end time unchanged; not filed | §2 pool overflow |
| lit wall, every block culled by the drawer's block test | flag 0x20000 not set | §6 r6 |
| roof passing the whole-tile test, all blocks off screen | flag 0x20000 set | §6 r6 |
| running fade 0xFF → 0x80, `end` = 0xFFFFFF00, `now` = 0x00000010 (after the `GetTickCount` wrap), render kind ≥ 4 | unsigned `end ≤ now` is false → not reached; `now − end` = 0x110; `−127 × 772 = −98,044`, `/ 500` = −196, low byte 0x3C; alpha = 0xFF + 0x3C mod 256 = 0x3B | §8 clock (original bug) |
| same record, render kind < 4 | alpha := 0x80 at once, bit 1 cleared | §8 |
| capture `order-0001`: walk the Rogue Encampment along the palisade and behind a tent, every frame captured with the recorded lists (`capture.md`) | CPU reference with this order equals the capture | capture, queued |
| capture `order-0002`: walk under a Lut Gholein roof | roof passes and fade equal the capture | capture, queued |
| capture `order-0003`: stand next to a visible river-bank cell of the town variant (`townN1` (44, 32), (51, 32); run-2 seed `TownE1`: tile (950, 933)) | settles Open question 7 | capture, queued |
| floor record `ℓ` 1 passing the whole-tile test | flag 0x20000 set (`0x004DDE80`) | §6 r6 |
| last reveal at (1000, 2000), countdown 0; player at (1040, 2010) / (1060, 2000) / (1080, 2000) | `d` = 45 / 60 / 80: no reveal / no reveal / reveal (0x50 = 80) | §6 r7 |

## Provenance

1.14d `Game.exe`: frame `0x0044C990`, world `0x00476BC0`, grid
`0x00476140`/`0x004DCE60`/`0x004DDB70`/`0x00643340`, filling
`0x004DD7C0`/`0x00619BD0`/`0x00619C40`/`0x006433C0`/`0x006196A0`/
`0x006196E0`/`0x00619660`/`0x004DCF70`, inserters `0x004DD550`/
`0x004DD460`/`0x004DD350`/`0x004DD180`/`0x004DCFA0`/`0x004DD600`/
`0x004DD6E0`, flat units `0x00464860`, fade `0x004DD060`, unit entry
`0x004DC7B0`/`0x004DC710`, passes `0x004DF480`/`0x004DEDF0`/`0x004DE730`/
`0x004DE410`/`0x004DF510`/`0x004DEF80`/`0x004DF1C0`/`0x004DEA70`, fade fill
`0x004DC000`. The view field offsets were read from the disassembly
(Ghidra shows the grid base relative to view +0x38). Record and room
layouts agree with `drlg/rooms.md` §1, §9; table offsets with
`specs/data/fields.tsv`. No D2MOO equivalent (D2Client is not in D2MOO);
no capture yet. Follow-ups from the implementation (DO1–DO7, 2026-10-06):
`T` re-read at `0x00643340` (`tx = x + 2y`, `ty = 2y − x`, negative
values truncate then −1); pool test and flag placement in the seven
inserters; flag 0x20000 sites `0x004DEDF0`/`0x004DF1C0`/`0x004DEF80`
(drawer result) and `0x004DEA70` (unconditional), drawer returns from GDI
`0x006C94B0`/`0x006C93A0` (1 when a block passed the block test); fade
compares `end ≤ now` unsigned in the four walkers; shadow-tile slot
`+0xA4` GDI `0x006C9290` reads only (tile, X, Y, half), not the mode
argument.
Ghidra backlog (2026-10-06, disassembly of 1.14d `Game.exe`): every
`0x20000` test in the export (18 sites) and the record-flag writers
(`0x004DDE80` floors, the four pass functions); automap reveal
`0x00459020`/`0x00458F40`/`0x00459150`; all writes of record `+0x24`
(none sets bit 2).
2026-10-06 (A7 answers): fade group mode `0x004DD060` (the flag
`[0x0072A968]` read with `disasm.py xref`: one reference, `.data` value
1), record +0x10 writer `0x0066C9C0` (record grid lookup `0x0067C570`),
array zeroing `0x0066EEE0`, player index `0x004DDDAD`; getters
`0x00619660` (floors, +0x08) / `0x006196A0` (walls, +0x00) /
`0x006196E0` (shadows, +0x10), as `0x004DD7C0` and `0x004DE730` use
them; render kind `[0x00712CCC]` = display type (`.data` 3, set by the
command-line handlers `0x004776E0`…`0x00477720`, `composition.md` §1).

## Open questions

1. ~~Level backgrounds (levels 74, 120)~~: answered in `draw-order-2.md`
   §12 (stars; mountains and clouds; both time-seeded). Open: a capture
   in the Arcane Sanctuary and on Arreat Summit with the seed recorded.
2. ~~What passes 4, 8 and 9 draw~~: answered in `draw-order-2.md` §11.6
   (splash and bubble pools), §13 (a light-map debug view that never
   runs) and §11.7 (lightning flash, rain and snow particles).
3. ~~Unit shadows `0x00471620`~~: answered in `render/blend-modes.md`
   §5 (position, layers, skip rules).
4. DT1 shadow tiles `0x004F6980`: driver slot, block placement and block
   culling (`camera.md` OQ4: the handed (X, Y) is the wall position).
   Partly read: slot `+0xA4`, GDI `0x006C9290`, uses the same per-block
   view test as the GDI wall drawers (`camera.md` §7, shadow tiles) and a
   fixed blend (`render/blend-modes.md` §5, branch
   `claude/spec-shading-blend`). Open: block placement (X, Y) of
   orientation-13 tiles (`camera.md` OQ4).
   *Answered* (static): `camera.md` §6 shadow-tile row (wall position,
   wall block placement; `camera.md` OQ4).
5. Who reads cell flag 4 (§3 r2). Search the export for reads of the cell
   word.
   *Answered* (static, `all.asm`): nobody. The cell array (view
   +0xEAA8, side +0xEAB0) is loaded only by `0x004DCE60`, `0x004DCF10`,
   `0x004DCF30` (allocation, free), the builder `0x004DDB70` and the
   passes `0x004DEA70`, `0x004DF1C0`, `0x004DF480` (→ `0x004DEDF0`),
   `0x004DF510`; the passes read the list heads only (cell +0x10, +0x14,
   +0x1C, +0x20), and the only `& 4` tests in the draw module
   (`0x004DC000`–`0x004E0FFF`) are the record-flag tests of the setters
   `0x004DD180`, `0x004DD350` and the fade-state tests of record +0x24.
   Cell flag 4 is written and never read in 1.14d; d2rs may drop it.
6. ~~Fade group mode~~: answered in §8: the group mode is the only live
   mode in 1.14d (`[0x0072A968]` = 1, no writer); record +0x10 is the
   coordinate record of `drlg/levels.md` §11. Open: a capture walking
   past walls of a `Logicals` preset room (446 lvlprest rows, e.g. the
   `Act 1 - Crypt` rooms of the Crypt and Mausoleum, levels 18, 19)
   settles it on pixels.
7. The 8 `townN1` river-bank cells (`map-preview.md` OQ3): the draw path
   draws visible-flagged records (§7), so either the client DRLG hides
   them (`drlg/rooms.md` §9.6 linking or a different preset) or 1.14d
   shows them; capture `order-0003` or a client DRLG simulation of
   `townN1` with linking settles it (owner of a DRLG answer:
   `drlg/rooms.md`). Still open after the 2026-10-06 captures: every
   Act 1 town variant has such cells (wall layer 0, value `0x00500081`,
   no hidden bit; `TownN1` 8, `TownE1` 3 at (48, 36…38), `TownS1` 17,
   `TownW1` 17; survey of the four `lvlprest` files). Runs 1b and 2
   (`20261006-140102`, `20261006-141725`) use `TownE1` (the only variant
   with a bridge, floor rows 15–18; the player crosses it at tile y
   912.4–913.1, giving level origin (904, 896)), whose three cells are
   level tiles (952, 932…934), about 16 tiles south of the southernmost
   town position either run reached (y ≤ 916): never on screen. No
   sampled run-2 frame (every 25th) shows a uniform 16 × 16 area of one
   index in the play area other than near-black 172 in shadow. Capture
   `order-0003` for this seed: stand near tile (950, 933) on the west bank
   in daylight; a drawn tile #28 is a 160 × 128 wall of index 233 lit
   per block (light maps 0–1 give 0, 2–5 give 172, 6–11 give 173–174,
   12–22 give 136, 23–31 keep 233, act 1 `pal.pl2`).
8. Cross-spec (`drlg/rooms.md` §9.1 says roofs share the shadow list):
   the client reads roofs from the wall array (type 15, §3 r2); the DRLG
   owner should reconcile its wording. *Answered*: `drlg/rooms.md` §9.1
   and §10.4 now say roofs (type 15) are wall records from the wall
   layers (§9.5.1 step 6), and the shadow list holds type 13 only.
9. ~~The sight test~~: answered in `draw-order-2.md` §15 (leveldefs
   `LOSDraw`, size-shrunk ends) and §16 (the line test `0x0064E260`,
   which had no owner).
10. ~~Edge floors~~: answered in `draw-order-2.md` §14. Open there (its
    Open question 2): who fills the act's edge record.
11. ~~Water effects of material-0x2 floors~~: answered in
    `draw-order-2.md` §11.5: one `roll_range(0, 1000)` on the local
    player's seed per drawn water floor, then splash (`Rain3`/`Rain4`)
    and bubble (`bubble3`) spawns.
12. ~~A tile record holds no DT1 file~~: answered in `drlg/rooms.md`
    §9.3 "Entry identity" (`impl-client-drlg` §3 Q7): the record's DT1
    tile pointer (+0x18) is one header of one loaded DT1's tile array,
    so it is exactly (file, tile index); roof height (+0x04) and height
    (+0x08) are read from that same header, and d2rs carries them in
    the tile source's per-tile record. A recorder maps a pointer to
    (file, index) through the room's 32 library slots (room +0x68):
    the slot whose file's tile array (+0x110, count +0x10C) contains it.
13. Cross-spec (`render/sprite-placement.md` §7, not edited here): roofs
    go through the floor drawer (§6 r5), not with the wall drawer
    `0x005131B0` (RC1). *Answered*: `sprite-placement.md` §7 already
    says floors and roofs use the floor drawer (roof list `0x004DEA70`
    calls only `0x004F68E0`).
14. Whether any UI or cursor item is drawn between the world passes
    (`0x00456EE0` … `0x00477980` order): `client/ui.md` owner; a capture
    with a panel open.
15. ~~Who reads record flag 0x20000~~: the automap reveal
    `0x00458F40` (§6 r7); floors also set it (§6 r6). Search of every
    `0x20000` test in the export: no other reader of record `+0x14`.
16. ~~Who sets fade state bit 2~~: nobody. Every write of record
    `+0x24` in `Game.exe` (`0x004DD180`, `0x004DEA70`, `0x004DEDF0`,
    `0x004DEF80`, `0x004DF1C0`) sets bits 0–1 or clears bits 1–2, and no
    instruction ORs or stores an immediate with bit 2 into a `+0x24`
    field of a tile record; records start zeroed. §8 keeps the rule for
    completeness; d2rs needs no recorded `GetTickCount` for it.
