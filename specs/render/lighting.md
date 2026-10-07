# Spec: Render — Lighting (light map, light sources, ambient, day and night)

- **Status:** draft (2026-10-06, RE on 1.14d `Game.exe`, measurements on
  `levels.txt`, `monstats2.txt`, `missiles.txt` of `patch_d2.mpq` and on the
  `frames-raw-1` run 1b captures; no capture compared against a computed
  light map yet). Every rule names its 1.14d address.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::lighting` (`LightMap`, `LightSources`,
  `Environment`), consumed by `d2-client::world_view` (light values per
  draw) and `d2-client::scene` (`ShadeChain` through `render/shading.md`)
- **Related specs:** `render/shading.md` §3–§4 (light value → light map
  `v >> 3`, tile gradients), `render/draw-order.md` §3, §6, §8 (passes,
  wall records, fade state), `render/capture.md` §3.4, §7 (recorded light
  state; its Open question 5 is answered here), `render/unit-composite.md`
  §1 (the unit draw that receives the light), `sim/path-placement.md` §2,
  §4 (unit positions, the collision point test), `drlg/rooms.md` §10.6
  (collision bits), `sim/rng.md` (seed steps), `sim/tick.md` §3 (server
  environment step, message 0x53), `sim/stat-lists.md` §7 (value-change
  callback), `data/fields.tsv` (leveldefs, monstats2, missiles, objects,
  overlay columns), `client/render-pipeline.md` §B8;
  tables `render/wall-light-points.tsv`, `render/env-periods.tsv`

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 51–67 |
| Inputs | 68–80 |
| Outputs / state changes | 81–86 |
| Rules | 87–88 |
|   1. The light map | 89–102 |
|   2. Build order (`0x00475800`) | 103–111 |
|   3. Ambient fill (`0x00474610`) | 112–143 |
|   4. Blocks-light flags (`0x004756D0`) | 144–152 |
|   5. Light quality and the draw rate | 153–178 |
|   6. Light records | 179–262 |
|   7. Contribution of one record | 263–342 |
|   8. Light sources | 343–407 |
|   9. Environment (day and night) | 408–540 |
|   10. Scripted ambient overrides (`0x0046BDD0`) | 541–594 |
|   11. Light values handed to the draws | 595–625 |
|   12. Captures (answers `capture.md` Open question 5) | 626–663 |
|   13. d2rs answers | 664–674 |
| Constants & data dependencies | 675–686 |
| Randomness | 687–693 |
| Edge cases & original bugs | 694–710 |
| Test vectors | 711–744 |
| Provenance | 745–788 |
| Open questions | 789–886 |
<!-- /index -->

## Summary

1.14d lights the world from one **light map** of 48 × 48 sub-tile cells
centred on the local player, rebuilt every drawn frame: each cell starts at
its room's **ambient** (a level's fixed `Intensity` from `levels.txt`, or
the act's day/night **environment** for outdoor levels, or a scripted
override), then every **light source** in the client's light list adds a
cone that falls off linearly with an octagonal distance, optionally
shadowed by light-blocking sub-tiles. Sources are players (radius 13 plus
the `item_lightradius` stat), monsters (`monstats2` `Light`), objects
(`objects` `Lit<mode>`), overlays and missiles. A **light quality** level
chosen from the measured draw rate switches shadows on and off. Units,
walls, floors and roofs read their light values from the map; the intensity
byte is the value `v` that `render/shading.md` turns into a light map
`v >> 3`. This spec owns the light map, the sources, the ambient and the
environment cycle, and what value each draw receives.

## Inputs

| Name | Type | Source |
|---|---|---|
| local player sub-tile position | 2 × int | `sim/path-placement.md` §2.1 getters |
| rooms near the player, their sub-tile rectangles and levels | room list | `drlg/rooms.md`; near list room `+0x00`, count `+0x24` |
| leveldefs `Intensity`, `Red`, `Green`, `Blue` | u8 × 4 per level | `data/fields.tsv` leveldefs +0x88..+0x8B |
| act environment | 0x38-byte record per act | §9; S→C 0x53 |
| collision of each sub-tile | u16 | `sim/path-placement.md` §4 point test |
| light records | list | §6, created by the sources of §8 |
| lighting-quality option, measured draw rate | ints | §5 |
| scripted overrides | client globals | §10 |

## Outputs / state changes

The light map (`0x007B0E68`, §1) and, per draw, the light values of §11.
Light records and the environment record are client state; nothing here
reaches the server.

## Rules

### 1. The light map

1. 48 × 48 cells of 8 bytes at `0x007B0E68`, row-major (row = y): `+0`
   u32 blocks-light flag, `+4` intensity `I`, `+5` R, `+6` G, `+7` B.
   Cell `(gx, gy)` is sub-tile `(ox + gx, oy + gy)`.
2. Origin `(ox, oy)` = the local player's sub-tile position − 24 on each
   axis (`[0x007B0A54]`, `[0x007B0A58]`); `[0x007B0A5C]`, `[0x007B0A60]`
   = position + 24 is the inclusive upper bound of the source window test
   (§7.1 r2). Writes outside 0…47 are dropped (§7.1 r6); reads clamp (§11).
3. The map is rebuilt by `0x00475800` from the world draw `0x00476BC0` on
   every drawn frame, before any world pass, never per tick. Everything
   that changes per drawn frame in §6 (radius steps) therefore follows the
   draw rate, not the tick.

### 2. Build order (`0x00475800`)

1. Origin (§1 r2).
2. Ambient fill (§3, `0x00474610` with argument 0).
3. Blocks-light flags (§4, `0x004756D0`).
4. Quality `q` (§5, `0x00475780`).
5. Every light record from the list head to its tail (newest first, §6
   r3): update and contribution (`0x004755A0`, §6.4, §7).

### 3. Ambient fill (`0x00474610`)

1. No local player, or the player has no room: every cell := I, R, G, B =
   0 and flag 0.
2. Every cell := the ambient of the player's room (§3.1), flag 0.
3. Then each room of the player's room near list, in list order, except
   the player's room itself (pointer compare): its ambient (§3.1) fills a
   rectangle. With the room's sub-tile rectangle `(x, y, w, h)` (room
   `+0x4C..+0x58`, `0x00619730`) and `x' = x − ox`, `y' = y − oy`: skip
   the room when `x' > 48`, `x' + w < 0`, `y' > 48` or `y' + h < 0`;
   else fill columns `max(x', 0) … min(x' + w, 47)` and rows
   `max(y', 0) … min(y' + h, 47)`, both **inclusive**: the fill covers one
   column and one row beyond the room (`x + w`, `y + h`). Later rooms
   overwrite earlier ones; reproduce.

#### 3.1 Ambient of a room (`0x00474550`, event flag 1)

Returns `(I, R, G, B)`; the first rule that yields any of R, G, B ≠ 0 wins:

1. The scripted override of the room's level (`0x0046BDD0`, §10).
2. The level's leveldefs `Intensity`, `Red`, `Green`, `Blue` (`0x00619D70`
   → `0x0066BFD0`: room `+0x10` → `+0x58` → level id `+0x1D0` →
   leveldefs record of 0x9C bytes, `0x0061E470`).
3. The act environment (§9): `I` = env `+0x0C`, R, G, B = env
   `+0x18..+0x1A` (`0x0061C0E0`, `0x0061C0B0`; act `[0x007A0634]`).

Measured on `levels.txt` (137 rows): 37 rows have Red = Green = Blue = 0
(the towns of acts 1, 2, 3 and 5, the outdoor levels and row 0) and take
the environment; the other 100 have 255, 255, 255 and their own
`Intensity` (0 for 75 of them; 8, 30, 40, 80, 120, 128 and 140 for the
rest; Act 4's town is 140).

### 4. Blocks-light flags (`0x004756D0`)

Only when the player and its room exist. For each cell, rows then
columns: the room holding the sub-tile is searched from the previous
cell's room and its near list (`0x00463740`; the player's room when none);
the cell's flag := 1 when the collision point test (`0x0064CB30`,
`sim/path-placement.md` §4) with mask 0x22 (bits 0x02 and 0x20,
`drlg/rooms.md` §10.6) is non-zero. Otherwise the flag keeps the 0 of §3.

### 5. Light quality and the draw rate

`q` ∈ {0, 1, 2} (`[0x007B567C]`, 0 at process start; last change time
`[0x007B5678]`, 0 at start). Option globals: low-quality flag
`[0x0072DA50]` (static 0; the settings `+0x08` of `render/shading.md` §4)
and missile-lights flag `[0x0072A348]` (static 1). The options-menu
"lighting quality" item (`0x0047CFE0`) sets them: low → 1, 0; medium →
0, 0; high → 0, 1.

1. Candidate: `[0x0072DA50]` ≠ 0 and `[0x0072A348]` = 0 → 0 (no rate test).
   Otherwise start from 2 (1 when both flags are set) and apply the
   measured draw rate `D`: `D` ≤ 9 → 0; 10–12 → 1; 13–15 → return the
   current `q` at once when it is > 0 (no timer update), else 1; `D` ≥ 16
   → keep the start value.
2. When the candidate ≠ `q`: take it only when `GetTickCount() − last` >
   2,000 ms, and set `last` := now; otherwise keep `q`.

Measured draw rate `D` (`[0x007A04A8]`): the client loop counts the
in-game draws that ran with a player room (`[0x007A04AC]` += 1 after the
draw call, `0x0044F29C`); `0x0044CCE0` runs each loop pass and, when the
loop's wall-clock time (`[0x007A048C]`) is more than 3,000 ms past the
window start, sets `D` := count / 3 (integer), restarts the window and
zeroes the count. `D` = 25 at game start (`0x0044F100`). So `q` depends
on wall-clock time and on how many frames the client drew; a debugger that
slows drawing can lower it (`capture.md` §8).

### 6. Light records

#### 6.1 Layout (0x34 bytes, client memory)

| Offset | Field |
|---|---|
| +0x00 | owner unit type (6 = none) |
| +0x04 | owner GUID (−1 = none) |
| +0x08 | lookup flag (u8): owner unit flags `+0xC4` bit 21 |
| +0x0C | kind: 0 shadowed when `q` = 2, 1 never shadowed, 2 shadowed with a cached grid |
| +0x10, +0x14 | x, y in 1/8 sub-tile |
| +0x18 | radius × 8 (current) |
| +0x1C | radius × 8 (target) |
| +0x20 | dying flag |
| +0x24 | intensity `I` (u8) |
| +0x25..+0x27 | R, G, B |
| +0x28 | next record |
| +0x2C | cache valid (kind 2) |
| +0x30 | cache: (2m + 1)² ints, `m` = radius (kind 2) |

Position of a unit (creation and every update): `x = (P >> 13) + 4`, with
`P` = the unit's precise x (`0x006203B0`: dynamic path 16.16 position
`0x00648940`; static path sub-tile `<< 16`); `y` likewise (`0x00620410`).
So the light's own cell `x >> 3` is the sub-tile rounded to nearest, not
the unit's sub-tile.

#### 6.2 Operations

1. **Create** (`0x00474160`, ECX unit or none, EDX kind, then radius `r`
   in sub-tiles, `I`, R, G, B): `r` < 1 → no record; `r` ≥ 18 → 18. Owner
   type, GUID, lookup flag and position from the unit; radius = target =
   `8r`; dying 0; cache invalid (kind 2 allocates the cache); inserted at
   the list head (`[0x007B5668]`).
2. **Set radius** (`0x004742D0`): `r` ≤ 0 → nothing; `r` ≥ 18 → 18; radius
   = target = `8r` at once; kind 2: cache invalid and freed.
3. **Set target** (`0x00474290`): `r` ≤ 0 → nothing; `r` ≥ 18 → 18;
   target = `8r` (the radius walks there, §6.4 r2).
4. **Set color** (`0x00474390`); **get radius** (`0x00474350`) = radius
   `>> 3`.
5. **Remove** (`0x004743D0`): unlink and free at once (fatal if not in the
   list). A unit's light (unit `+0x64`) is removed through `0x00464930`.
6. **Die** (`0x00474470`): dying := 1, target := 0 (fatal for kind 2); the
   record shrinks per drawn frame and is removed when the radius is < 1.

#### 6.3 The list

One list per client (`[0x007B5668]`), new records at the head. The
intensity sum of §7.1 is order-free; R, G, B (§7.1 r5) depend on the
order.

#### 6.4 Per drawn frame (`0x004755A0`, argument `q`)

1. Not dying and owner type ≠ 6: look the owner up by type and GUID
   (`0x004639B0` when the lookup flag is set, else `0x00463990`); found →
   x, y := its position (§6.1). The lookups are the client unit hash
   sets of `client/model.md` §2: `0x00463990` searches set S (server
   units, `0x007A5E70`), `0x004639B0` set C (client-only units,
   `0x007A5270`); each is 128 buckets per unit type (`type << 9`),
   bucket `GUID & 0x7F`, chain `+0xE4`, matched on GUID `+0x0C` (a match
   of another type is fatal 0x5A). Unit flag `+0xC4` bit 21 (0x200000)
   is the client-only mark, so the record looks in the set its owner
   lives in.
2. Radius ≠ target: kind 2 → cache invalid and freed; radius += 8 toward
   the target.
3. Radius > 255 → 248.
4. Dying and radius < 1 → remove (§6.2 r5) and stop.
5. Kind 0 and `q` > 1 → shadowed contribution (§7.3); else kind 2 →
   cached contribution (§7.4, building the cache first when invalid);
   else the plain contribution with `q` (§7.2).

A new client active room (`0x00475930`, ECX = the room just created;
registered by `0x00475B40` through `0x0061AF60` as the act callback and
called only by the active-room creation, `drlg/rooms.md` §5 rules 8–9)
walks the list; for each kind-2 record: owner type 6 or owner not found
(§6.4 r1 lookup) is fatal 0x591. With `R` = the owner's room
(`0x00620BB0`): `R` = the new room → nothing. Else, with `(ux, uy)` the owner's sub-tile (objects,
items and tiles, types 2, 4, 5: static path `+0x0C`, `+0x10`; types 0,
1, 3: dynamic path `0x006488C0` / `0x00648900`, 0 without a path) and
`m` = radius (`+0x18`) `>> 3`, the cell lookup `0x00463740(R, x, y)`
(`client/model.md` §2) is run on `(ux + m, uy)`, `(ux − m, uy)`,
`(ux, uy + m)`, `(ux, uy − m)` in this order; the first that returns the
new room sets cache valid (`+0x2C`) := 0 (the cache memory is kept)
and ends the record's tests.

### 7. Contribution of one record

#### 7.1 Shared rules

With `r` = radius × 8 (record `+0x18`; a multiple of 8, so `m = r / 8`
sub-tiles), `(x, y)` the record position:

1. Nothing when `r` ∉ 1…255.
2. Window: `x0 = (x − x mod 8) − r`, `y0 = (y − y mod 8) − r`, `n = 2r >> 3
   = 2m`. Nothing when `x0 >> 3 > ox + 48`, `(x0 >> 3) + n + 1 < ox`,
   `y0 >> 3 > oy + 48` or `(y0 >> 3) + n + 1 < oy`.
3. `k = (I << 16) / r` (signed, truncating).
4. For rows `j` = 0…n, columns `i` = 0…n: the sample point is the cell's
   **top-left corner** `(cx, cy) = (x0 + 8i, y0 + 8j)` (sub-tile
   `x0 / 8 + i`); `d = oct(|x − cx|, |y − cy|)` with
   `oct(a, b) = (983·max(a, b) + 407·min(a, b)) >> 10` (`0x004740D0`,
   `0x00474080`); base value `b = ((r − d) · k) >> 16` (arithmetic shift).
5. Add a value `v` > 0 to cell `(cx >> 3 − ox, cy >> 3 − oy)`
   (`0x004747C0`): `I' = min(255, I + v)`.
6. Cells outside 0…47 on either axis are skipped. With the colored-light
   flag `[0x00712B8C]` (static 1; only `0x00474140` toggles it):
   `R' = min(255, ((R · I + R_src · v) · T[I']) >> 16)` with the old `I`,
   G and B alike, `T[i] = 65536 / i` for `i` = 1…255 and `T[0] = 0`
   (`0x00475B40`, table `0x007B0A68`); flag 0: R, G, B := 0. The GDI
   reference draws only `I` (§11).

#### 7.2 Plain contribution (`0x004748D0`, argument `q`)

`q` = 0 first caps by owner type (`+0x00`): a player that is not the local
player (`0x00463DE0`) with `r` ≥ 16 → `r` := 16; a monster → `r` := 8
(even when smaller); a missile → no contribution. Then §7.1 with
`v = b`, using the capped `r` for the window, `k` and `b`.

#### 7.3 Shadowed contribution (`0x00474D70`, kind 0, `q` = 2)

A 64 × 64 grid of cells `(row, col)`, centre `(32, 32)` = the light's cell
`(lx, ly) = (x >> 3, y >> 3)`; `(row, col)` is sub-tile
`(lx + col − 32, ly + row − 32)`. Two int planes: blockers `B` and shade
`S` (`0x007ACA50`, `0x007A8A50`; `v(c)` = `B[c]` when non-zero, else
`S[c]`).

1. After §7.1 r1–r2: `B` := 16 for every cell of the window
   `32 − m … 32 + m` (both axes) whose light-map cell is outside the map
   or has the blocks-light flag (`0x00474A70`), else 0.
2. For rings `t` = 2…m (none when `m` < 2), for `s` = 0…t, eight cells
   in this order: `(lx − s, ly − t)`, `(lx + s, ly − t)`,
   `(lx − s, ly + t)`, `(lx + s, ly + t)`, `(lx + t, ly − s)`,
   `(lx + t, ly + s)`, `(lx − t, ly − s)`, `(lx − t, ly + s)`
   (`0x00474B50` + `0x00474C00` per cell; corners are computed twice, the
   second value stands).
3. One cell: its centre `(px, py) = (8·sx + 4, 8·sy + 4)`; `dx = x − px`,
   `dy = y − py`; `σx` = −1 when `dx` < 0 else +1, `σy` likewise.
   - `dx = dy = 0`: `S` unchanged.
   - `dx = 0`: `S = v(row + σy, col)`.
   - `dy = 0`: `S = v(row, col + σx)`.
   - `|dy| ≤ |dx|`: `f = (|dy| << 8) / |dx|`;
     `S = ((256 − f)·v(row, col + σx) + f·v(row + σy, col + σx)) >> 8`.
   - else `f = (|dx| << 8) / |dy|`;
     `S = ((256 − f)·v(row + σy, col) + f·v(row + σy, col + σx)) >> 8`.
4. Cells of rings 0 and 1 are never written and hold 0 (static memory;
   the per-light clear covers only rows 0–15 and every other cell read in
   a pass was written earlier in that pass).
5. §7.1 r3–r6 over the window with `S` = the cell's shade (row
   `32 − m + j`, column `32 − m + i`): `S` ≥ 16 → nothing; else
   `v = (b · (8 − (S >> 1))) >> 3`.

So `B` = 16 is a wall: cells behind it get `S` = 16 (dark), cells beside
its shadow get partial `S` (soft edge); the blocking cell itself is lit.

#### 7.4 Cached contribution (kind 2: `0x004750F0`, `0x00475420`)

1. Build when the cache is invalid (§7.1 r1 only; no window test): `B` for
   all 64 × 64 cells := 16 when the collision point test (§4 mask 0x22)
   from the owner's room at sub-tile `(ux + col − 32, uy + row − 32)` is
   non-zero, `(ux, uy)` = the owner's sub-tile (`0x0045ADF0`,
   `0x0045AE20`; it can differ by one from the light's cell, §6.1); then
   §7.3 r2–r4; copy the `(2m + 1)²` window of `S` row by row into the
   cache; valid := 1. No owner or no room: fatal.
2. Contribution at every `q`: §7.1 with `S` from the cache and §7.3 r5.

### 8. Light sources

`I` = 255 for all. Radius in sub-tiles, clamped by §6.2.

| Source | Site | Kind | Radius | R, G, B | Changes |
|---|---|---|---|---|---|
| Player (every player unit, client init `0x00460BF0`) | `0x00460CF0` | 0 for the local player (or when no local player exists yet), 1 for others | 13 (unit `+0x68`) | 255, 255, 255 | stat callback `0x004609F0` (`sim/stat-lists.md` §7): stat 89 `item_lightradius` → set radius (§6.2 r2) to 13 + new value (`0x00460930`); stat 90 `item_lightcolor` → R, G, B = bits 16–23, 8–15, 0–7 of the new value, 0 → white (`0x004609A0`) |
| Monster (`0x004AE210`; callers `0x00478C75`, `0x004AEB82`, `0x004AF058`, `0x004AFFAF`) | `0x004AE2EE` | 0 | `max(L_c, monstats2 Light)`, `L_c` = `0x0063EBD0` (§8 r1); in level 8 with client quest byte 1 set, the unit not client-only (flag 0x200000, §6.4 r1) and monstats `Align` (`+0x4C`) ∉ {1, 2}: 3; none when 0 | `light-r`, `light-g`, `light-b` | replaces the unit's previous light |
| Monster umod 3 `light` hook `0x004ACC70` (umod table `0x00724D78`, §8 r2) | `0x004ACCEF`, `0x004ACD2C` | 0 | 7 | §8 r2 | replaces the unit's light |
| Overlay (`0x00470390`) | `0x00470555` | 1 | `InitRadius`, then target `Radius` when different; none when `Radius` = 0 | overlay `Red`, `Green`, `Blue` | — |
| Missile (client create `0x004CD540`) | `0x004CDAC5` | 1 | `Light` (missiles +0x130) | missiles `Red`, `Green`, `Blue` | only when `[0x0072A348]` ≠ 0 (high quality), creation flags without 0x4000 and `Light` ≠ 0 or creation `+0x50` ≠ 0; flicker below; ends by dying (`0x004CD3EE`, `0x004D301D` → §6.2 r6) |
| Skill cast light (`0x004C5680`, §8 r3) | `0x004C56D7` | 1 | 1, then target `Light` of the skill's `cltmissile` | missiles colors | replaces the unit's light (`0x00643A00`) |
| Object (`0x004BC580` from `0x004BC5E0`, `0x004BC720`, `0x004BCBB0`, `0x004BCF60`) | `0x004BC5BC` | 2 | `Lit<mode>` / 2 (objects +0x110 + mode; `0x004BCBB0` uses `Lit2`) | objects `Red`, `Green`, `Blue` | `Lit` = 0 → light removed; a later mode with a light sets the target |
| Overlay 182 `horadric_light` event (`0x004D6D40`) | `0x004D6F85` | 2 | 60 → 18 | 255, 255, 255 | — |
| Missile 191 `cursecenter` (`0x004F3530`, §8 r4) | `0x004F3630` | 1 | 1, then target `max(2, aurarangecalc)` | 255, 0, 0 | missile's light (`+0x64`) |
| Den of Evil lights | §10 r1 | missile 287 `denofevillight`, `Light` 10 | | | |

**Missile flicker** (`0x004CD1C0`, from the client missile update
`0x004D2C70` when missiles `Flicker` (+0x131) ≠ 0; 35 rows): when the
missile's flags `+0x44` & 0x300 = 0, it has a light, and its radius ≥
`Light`: target := `Light` + rnd(`Flicker`) drawn from the missile unit's
seed `+0x20` with the range rule of `sim/rng.md` (`0x0045C3E0`). The radius
then walks there by 8 per drawn frame (§6.4 r2).

Rules behind the table:

1. `L_c` (`0x0063EBD0`): for components 0–15 of the monster, the
   `monstats2` choice code (`0x00664860`, choice byte monster data
   `+0x04 + i`); codes none, `lit`, `med`, `hvy` are skipped; else the
   item of that code (`0x00633640`) gives its `lightradius` (`+0x12F`);
   `L_c` = the largest (0 when none).
2. Umod hooks (`0x004AD020`, from the monster set-ups `0x0045E47A`,
   `0x004AEB96`, `0x004AF05F`): for a monster with a non-zero first umod
   byte (monster data `+0x1C`), the hooks of umods 1, 2, 3, 4 (list
   `0x006DA4C8`) and then of each of the 9 umod bytes are called through
   `0x00724D78 + 4·umod` with argument `u` = the unique type flag (0x8).
   Umod 3 `light` (`0x004ACC70`): `u` = 0 → nothing. Else the unit's
   light is removed, then: a champion (type flag 0x4) takes three D2 RNG
   steps of its seed (`+0x20`) and creates radius 7, `I` 255 with R, G, B
   = the low bytes of the third, second and first new low words; any
   other monster creates radius 7 with `monstats2` `light-r/g/b`. The
   light becomes unit `+0x64`. Since `u` requires the unique flag, the
   random colors need both flags (umod 3 on a unique that is also a
   champion).
3. `0x004C5680(unit, missile)`: called by the client skill start
   `0x004C6140` (callers `0x004C6660`, `0x004C6EB0`, `0x004C6F40`) after
   a successful start when `skills` `cltmissile` (`+0xE8`) > 0, with that
   missile; nothing when its `Light` is 0; the new light replaces the
   unit's (the old one is removed).
4. `0x004F3530` is `cltdofunc` 30 (table `0x00727BA8`; the curse skills
   and `MonCurseCast`): it creates client missile 191 `cursecenter`
   (`0x004CD540`) and gives it a red light whose target is the skill's
   `aurarangecalc` (`+0x64`, `0x00646CA0`) at least 2.
5. Missile create (`0x004CD540`): the radius is always `Light`; a
   creation record byte `+0x50` ≠ 0 with `Light` = 0 only makes the create
   call with radius 0, which creates nothing (§6.2 r1) after the old
   light was removed.
6. Flicker runs only when the missile's current frame (unit `+0x44`, 8.8
   fixed point, `sim/units.md`) has bits 0x300 clear: on frames whose
   number is a multiple of 4.

Measured: 100 `monstats2` rows have `Light` > 0 (e.g. `fallenshaman1`–`5`:
5, color 230, 168, 255; `andariel` 8); 288 `missiles` rows have `Light`
> 0 (e.g. `shamanexp` 8, `denofevillight` 10).

### 9. Environment (day and night)

#### 9.1 Record (act `+0x04`, 0x38 bytes, `0x0061BE40`)

| Offset | Field |
|---|---|
| +0x00 | period index 0–5 |
| +0x04 | period type (from the table) |
| +0x08 | ticks |
| +0x0C | intensity `I` |
| +0x10 | `GetTickCount()` at creation |
| +0x18..+0x1A | R, G, B |
| +0x1C, +0x20, +0x24 | floats: −cos θ, 0, the sine term `s` (§9.3) |
| +0x28 | speed (ticks per degree) = `[0x007443E4 + 4 · (+0x2C)]` (128, 4, 8); +0x2C: 0 (128) except during the Tainted Sun: its start `0x0061C450` writes 1 (speed `[0x007443E8]` = 4, `0x0061C465`), its end `0x0061C4D0` writes 0 (`world/quests-act2-2.md` §5.2) |
| +0x30 | eclipse flag |
| +0x34 | last reported hour (server, `0x0061C040`) |

Period tables `render/env-periods.tsv` (`0x007443F0` normal, `0x00744438`
act 4, `0x00744480` eclipse; 12-byte entries: start degree, type, color
`0x00BBGGRR`). Creation: index 2, type and ticks (= start × speed = 0)
from the normal entry 2, then §9.3 r4 and §9.4 with `A` = 0 and `L` = 0
(`0x0061BE40` zeroes EDI and ECX before the calls) and the eclipse flag
0: so ticks 0, `I` = 128 (`s` = 0) and R, G, B = entry 2's color
(255, 255, 255).

#### 9.2 Updates

1. Per client update (`0x0044C790` → `0x0061BFC0(act, player room)`), with
   `L` = the room's level id (0 when none) and `A` = its act index
   (`0x006427F0`: number of act start levels 40, 75, 103, 109 ≤ `L`):
   advance (§9.3 r1–r3), intensity (§9.3 r4), color (§9.4), then `L` =
   120 → R, G, B := 245, 240, 255.
2. S→C 0x53 (10 bytes: u32 @1 period index, u32 @5 ticks, u8 @9 eclipse;
   handler `0x0045E300`, only when the client act is the local player's
   act, r4) → `0x0061C240`: index > 5 or < 0, ticks < 0 → fatal; ticks >
   speed × 360 → 0; set index, ticks, type (normal or eclipse table by the
   flag); intensity (§9.3 r4, still with the **previous** eclipse flag);
   set the eclipse flag; when it is set, also the period reset
   `0x0061BDF0`, intensity again (now with the flag) and color (§9.4,
   `A`); `L` = 120 override. Period reset (`0x0061BDF0`, record in EAX):
   by the record's eclipse flag, normal or eclipse table entry of the
   current index → type := its type, ticks := its start × speed. So with
   the eclipse set the received ticks are discarded: index 5 (Tainted
   Sun, r3) → type 2, ticks 240 × 128 = 30,720. Without the flag the
   setter recomputes no color (the next per-update call of r1 does). The server
   sends it when its own cycle (same advance code, `sim/tick.md` §3 step
   1) changes period.
3. `0x0044C83B` and `0x0044E16C` call the same setter with index 5, ticks
   0, eclipse 1: S→C 0x5D (`0x0045E540` → `0x004A2CB0`) with quest byte
   @1 = 10 (Tainted Sun) and flag byte @2 bit 0 jumps to `0x0044C820`;
   with a client act it sets the eclipse at once, else it sets the
   pending flag `[0x007A060E]`, which the act load (S→C 0x03,
   `0x0044E142`) turns into the eclipse when the loaded act is act 2
   (byte 1).
4. **Dispatch owner of 0x53** (`client/bridge-dispatch.tsv`). Client
   model state: the environment record (§9.1) of the client DRLG act
   (`[0x007A0634]` +0x04; d2rs: the act of `client/model.md` §1 `act`,
   built by §12 there) and the day-period cache `[0x007A6A74]`. Handler
   `0x0045E300`, in order:
   1. P := the local player (`0x00463DD0`); no local player → 1.14d
      reads P +0x1C through a null pointer (crash); bridge: handler error.
   2. The client act `[0x007A0634]` ≠ P's act pointer (unit +0x1C: set
      by creation at a point, `client/model.md` §2 rule 6, and by the
      0x15 placement, `client/msg-units.md` §3 rule 2) → nothing more.
      A player created at (0, 0) and not yet placed has none, so a 0x53
      then is ignored. Recorded joins send 0x53 in frame 2, after 0x15
      (frame 1): `53 02000000 00000000 00` (`20261006-022633` seq 228)
      applies index 2, ticks 0; `53 02000000 80080000 00` (seq 146616,
      frame 2177) index 2, ticks 0x880. With no client act and a P
      without act pointer (unplaced) both are null, so the check
      **passes**; step 3 then calls the setter with act null, whose
      first step (`0x0061C247` → `0x0061AA60`, the act's environment
      record) is fatal 0x547 on a null act, before the index and ticks
      checks and before any write. d2rs: fatal 0x547, no state change.
   3. The setter of r2 with (act, P's room (`0x004646A0` → `0x00620BB0`;
      none → null), index u32@1, ticks u32@5, eclipse u8@9).
   4. Day-period refresh `0x004646C0`: `p` := the act's day period
      (`0x0061C100(act, 0)`, 0–3, `sim/stats.md` §8); `p` = the
      cache → nothing; else cache := `p` and every object unit (type 2)
      of the client's sets S and C, bucket order, gets `0x004BC5E0(obj,
      0)` (object day/night refresh; owner: the client object spec,
      open question 11); a non-object found in those type-2 buckets →
      fatal 0x88C. **The day period** `p` is the environment record's
      +0x04 (period type); `0x0061C100` writes ticks / speed (0 when
      speed is 0) to its optional out argument (null here) and returns
      0 when the act has no record (a null act is fatal 0x547). Every
      writer of +0x04 (creation §9.1, §9.3 r3, the setter of r2, the
      period reset) takes the type column of the table chosen by the
      eclipse flag (normal when 0, else eclipse; never the act-4 table)
      at the current index, so `p` depends on (index, eclipse flag)
      only, not on ticks or act: index 0..5 → normal 3, 3, 0, 1, 1, 2;
      eclipse 3, 0, 1, 2, 2, 2 (`render/env-periods.tsv`; `0x0061C100`,
      `0x0061BEE0`, `0x0061C240`).
   5. P still present → the requirement refresh of S→C 0x47 on P
      (`0x004C1BC0` with a built `47 <P type> <P GUID>`,
      `client/msg-stats-items.md` §3 rule 3).
   No output (`client/bridge.md` §10): lighting and objects read the
   record each frame.

#### 9.3 Advance and intensity (`0x0061BEE0`, `0x0061BB80`)

1. ticks += 1; when the eclipse flag is 0: `A` = 3 → +15 more; else when
   the normal table's type of the current index is 2: +1 more, and in `A`
   = 2 another +8 (so +1, +2, +10 or +16 per update).
2. ticks ≥ speed × 360 → ticks := 0.
3. Next = index + 1 (6 → 0) in table `T` (normal when the eclipse flag is
   0, else act 4 when `A` = 3, else eclipse): when `T[next].start × speed`
   < ticks, index := next, and type and ticks := start × speed of the
   new index in the normal (flag 0) or eclipse table.
4. Intensity:
   - `A` = 3: target by `L`: 103 → 128, 104 → 64, 105 → 56, 106 → 48,
     else 16; `I` += 1 when below the target, else `I` −= 1 when above;
     then `I` = 0 → `I` := target.
   - else eclipse flag set: `I` > 32 → `I` −= 8; then `I` < 32 → 32.
   - else `L` = 120: `I` := 200.
   - else `θ = ((ticks / speed) / 180) · π_f` (doubles; `π_f` =
     3.1415927410125732, the float `π`), `s = sin θ` when ticks < speed ×
     180, else `s = 0.5 · sin θ`; `s` is stored as a float and reloaded;
     `I = trunc(s · 128 + 128 + 0.5)` (`0x00682FD0`), clamped to 0…`cap`,
     `cap` = 170 when `A` = 4, else 255.

#### 9.4 Color (`0x0061BCE0`, argument `A`)

Table: act 4 when `A` = 3, else eclipse when the flag is set, else normal.
`c` = entry of the index, `n` = the next entry (6 → 0); `t = (ticks −
c.start · speed) / ((n.start − c.start) · speed)` (floating);
each of R, G, B := `c` channel + `trunc((n − c channel) · t + 0.5)`, stored
as a byte (mod 256).

Consequences (reproduce): period 1 (start 340) lasts one update, because
its successor starts at 0, so the cycle runs 0–340 degrees; with speed 128
one normal day is 43,520 ticks plus the doubled night periods.

### 10. Scripted ambient overrides (`0x0046BDD0`)

By the room's level id:

1. **Level 8** (Den of Evil, `0x0046BD50`): when `[0x007A745C]` = 0 and
   client quest byte 1 (`[0x007C0EA5]`, `0x004B92E0`) ≠ 0: R, G, B = 255,
   64, 48 and `I` = 80 while the Den counter `[0x007129CC]` = −1, else
   `trunc(W[(a + 128) & 511] · 80.0)` with `a = trunc(counter · 128 / 30)`;
   else 0, 0, 0 (falls through, §3.1). `W[i]` = float(`sin(i · π_f /
   256)`) (`0x00707800`, 512 floats; this formula reproduces all 512).
   Counter: set to 0 by `0x0046B0C0` (event table `0x007129D8`), +1 per
   client update (`0x0046BEB0`) while ≥ 0 and the flag is 0; when it
   passes 29: flag := 1 and every loaded level-8 room gets Den lights
   (`0x0046B0D0`); rooms of level 8 loaded later get them too
   (`0x0046BE60`, from `0x0044C77D`). Game start resets flags to 0 and
   counters to −1 (`0x0046BF90`).
   Den lights per room (`0x0046AF70`): up to 25 tries until 3 placed: x =
   room x + rnd(w), y = room y + rnd(h) (two steps of the local player
   unit's seed `+0x20`, `sim/rng.md` range rule); placed when the point
   test with mask 5 is 0, as client missile 287 `denofevillight` (§8).
2. **Levels 107, 108**: when `[0x007A7460]` ≠ 0 and `[0x007129D0]` = −1:
   R, G, B = 255, 64, 48, `I` = 160; else 0, 0, 0. `0x0046B290` (event
   table `0x00712A08`) sets the flag; `0x0046B3A0` sets the counter 0;
   the counter then rises per update and at > 29 resets flag and counter
   (`0x0046BEB0`). Triggers: r4.
3. **Other levels** — darkness event: `I` = `[0x007A7430]`, R, G, B =
   `[0x007A7434..36]`, all 0 when no event runs. An event
   (`0x0046AE50(in, hold, out, level)`, callers `0x004D8893`,
   `0x0046AF5F`, r4; `out` = 0 → 25; total = in + hold + out) runs per client
   update (`0x0046AD10`): R, G, B, `I0` := the room ambient without
   override (§3.1 r2–r3); with counter `c` and angle `a` (both from 0):
   `c` < in → `I = trunc(W[(a + 128) & 511] · I0)`, then `a := c · 128 /
   in`; `c` < total − out → `I` = 0; else `I` as in the first case, then
   `a := 128 − (c − total + out) · 128 / out`; `c` += 1; `c` > total or a
   level other than the event's → event cleared.
4. **Triggers.** S→C 0x89 UniqueEvent (`0x0045EA30` → `0x0046B630`, id =
   u8@1): id ≥ 32 is fatal 0x1D9, id ≥ 20 fatal 0x1DA; else bit `id` of
   `[0x007A7458]` is set and the id's handler (`0x007129D8 + 4·id`) runs:
   0 → `0x0046B0C0` (Den counter := 0; the server sends it on the Den of
   Evil clear, `world/quests.md` §6.5); 1 → `0x0046AEE0` (near object
   17 `StoneAlpha` of the local player's room: client missile 288
   `cairnstones` and the darkness event of r3 from missile 288's
   fields); 12 → `0x0046B290` (levels 107/108 flag := 1; in level 108
   also client missile 372 at the local player and
   `0x0046F870(243, 1)`); 13 → `0x0046B3A0` (counter `[0x007129D0]` :=
   0, `[0x007A7464]` := a draw from `0x00410A80` + 90); 3, 6, 14, 16, 17,
   19 have other client effects (`0x0046B100`, `0x0046B1E0`,
   `0x0046B300`, `0x0046B440`, `0x0046B4A0`, `0x0046B520`); the rest
   none. Server senders of ids 12 / 13: `0x005B5230` / `0x005B52E0` (A4Q2
   Terror's End; `0x005B52E0` is its callback 8 in `world/quests.tsv`;
   owner `world/quests.md`). The second darkness caller `0x004D8893` is state
   setfunc 2 (table `0x0072A690`, `states` `setfunc` +0x1A):
   state 153 `cloak_of_shadows`.

### 11. Light values handed to the draws

`render/shading.md` turns each value into a light map; here only what
value each draw gets. A light-map read (`0x00475AA0`, x and y in 1/8
sub-tile) clamps the cell to 0…47 on each axis.

1. **Units** (all types, `render/draw-order.md` §6 r3–r4): the cell of the
   unit's sub-tile (draw entry `+4/+8` = sub-tile × 8, `0x004DD600`); the
   dword `B << 24 | G << 16 | R << 8 | I` goes to the unit draw
   `0x00471EC0` (EDX) and its cel draws (`render/unit-composite.md`); GDI
   uses the low byte as `v` (`render/shading.md` §3).
2. **Walls and lower walls** (`0x004DF1C0`, `0x004DEDF0`, `0x004DEF80`):
   `(X, Y)` = 8 × the tile's origin sub-tile (room sub-tile origin + 5 ×
   record tile position, `0x004DD180`); the DT1 direction (tile header
   `+0x00`) gives the point count (`0x006DB9D8`: 0 for direction 0, else
   6; 1.14d tiles use 1–9 only, Open question 8) and the points of `render/wall-light-points.tsv` (`normal` =
   `0x0072A9E8`; `faded` = `0x0072ABC8` when the record's fade state bit 0
   is set, `draw-order.md` §8); point `p` reads the cell at
   `(X + 8·dx_p, Y + 8·dy_p)` → dword as for units. Per 32-pixel block
   column `c` = block x `>> 5` (GDI `0x006C94B0`, `0x006C93A0`): `c0 = c3 =
   I_c`, `c1 = c2 = I_{c+1}` (low bytes), the corners of `shading.md` §4.
3. **Floors** (`0x004DE410` → `0x004DDEF0`, render kind `0x00477730` < 4):
   an 8 × 8 grid of 12-byte cells, row-major; cell `(i, j)` bytes 0–3 = I,
   R, G, B of sub-tile `(sx − 1 + i, sy − 1 + j)`, `(sx, sy)` = the tile's
   origin sub-tile; DT1 material flag 0x100 (header `+0x06`) → all four
   bytes 0xFF in every cell. This is the floor light grid of
   `shading.md` §4.
4. **Roofs** (`0x004DEA70`): the same grid at the roof record's sub-tile
   (draw entry `+4/+8 >> 3`); roof height (header `+0x04`) ≠ 0 → every
   cell := the act environment's `I`, R, G, B (§9), not the room ambient.

### 12. Captures (answers `capture.md` Open question 5)

1. The light map at frame end is a pure function of: the window origin
   (§1), the ambients of §3 (environment and overrides included), the
   blocks-light flags (§4), `q` (§5), and every light record's position,
   radius, target, intensity, color, kind and cache (§6). Wall-clock
   inputs: `q` (§5) and the event timers that run per client update
   (§9.2, §10); draw-rate inputs: radius steps (§6.4 r2) and `q`.
2. No random draw in the build (`0x00475800` and callees). Random draws
   that change lights happen elsewhere: missile flicker (§8, missile
   seed), Den lights (§10 r1, player seed), monster hook `0x004ACC70`.
3. A capture's state key must therefore hold the light map itself: SHA-256
   of the 18,432 bytes at `0x007B0E68` read at frame end (with `q`), not
   `q` alone; the draw log should list the light records (§6.1 fields).
4. Run 1b, first still segment (f 12,543–13,694, 1,148 frames, 7
   standing positions; the key of `capture.md` §7 without cursor and
   light): 614 same-key pairs (consecutive frames of a key), 547 differ.
   Every differing pixel is in a cursor blob (≤ 34 × 31, or two such
   overlapping where the cursor moved; 404 blobs), in the panel and globe
   area (y ≥ 494: life globe, life text, belt, the mini-panel bar) or in
   the first key's pair (an animated torch and a corpse, automap open),
   except 1,392 pixel changes in pairs starting at
   f 13,496–13,599 (85 pairs): all 0 ↔ 172 (index 172 = (4, 4, 4); act 1
   light maps 0 ↔ 1, i.e. `v` 0–7 ↔ 8–15), at x 0–114, y 139–337, both
   directions, 1–50 pixels per frame, gone from f 13,600 on. With the
   player at screen (400, 284) these pixels lie 9–18 sub-tiles to the left
   (octagonal distance up to ≈ 160 > 104 = the player's radius), so
   another light reaches them. Verdict: a light-map variation from a
   source outside the view that moved during those ~100 ticks; not the
   quality `q` (a change holds for at least 2 s, §5 r2, and 2 ↔ 1 would
   change the player's own shadows near the player, which stay identical;
   the observed pixels change from frame to frame in both directions) and
   not missile flicker of a visible missile. The ±1 steps "on and around
   the player" are the cursor's own shading (`capture.md` §7). The source's
   identity: Open question 9.
5. `capture.md` Open question 7's first region (0 ↔ 172 at x 0–125, y
   125–275 in f 13,486–14,636) is this effect.

### 13. d2rs answers

| Hook | Answer |
|---|---|
| `LightMap` | §1–§4, §7, rebuilt per presented frame in the client, from client state only |
| `LightSources` | §6 records created by the client's unit mirror (§8) |
| `Environment` | §9, advanced per client update, set by S→C 0x53 |
| `q` | §5; in verify cases taken from the recording, never measured |
| light value per draw | §11; `ShadeChain` from `render/shading.md` |
| floating point | §9.3, §9.4, §10 use doubles and `sin`; client-only (not `d2-sim`) |

## Constants & data dependencies

Light map 48 × 48 × 8 bytes; window ± 24; radius clamp 1–18 sub-tiles,
×8 inside; step 8 per drawn frame; `oct` weights 983 and 407 (/1024);
shade grid 64 × 64, wall value 16; quality thresholds 9 / 12 / 15, 2,000
ms hysteresis, 3,000 ms rate window, start rate 25; player radius 13;
`levels.txt` (leveldefs) `Intensity`, `Red`, `Green`, `Blue`; `monstats2`
`Light`, `light-r/g/b`; `missiles` `Light`, `Flicker`, `Red`, `Green`,
`Blue`; `objects` `Lit0–7`, `Red`, `Green`, `Blue`; `overlay`
`InitRadius`, `Radius`, `Red`, `Green`, `Blue`; `itemstatcost` 89, 90;
`render/env-periods.tsv` (speed 128), `render/wall-light-points.tsv`.

## Randomness

None in the light-map build. Seeded draws that create or change lights:
missile flicker (missile unit seed, one step per new target), Den lights
(local player unit seed, two steps per try, `capture.md` §7: the same
seed the shake and cursor use), the `0x004ACC70` monster hook (unit seed).

## Edge cases & original bugs

1. Near-room ambient fills cover one extra column and row (§3 r3).
2. Falloff samples the cell's top-left corner; shading rays use centres
   (§7.1 r4, §7.3 r3).
3. The light's cell is the rounded position (§6.1); the kind-2 blocker
   grid is centred on the owner's truncated sub-tile (§7.4 r1).
4. `q` = 0 forces monster lights to radius 8 even when smaller (§7.2).
5. Missile lights exist only with the high-quality option (§8).
6. Environment period 1 lasts one update (§9.4).
7. Wall direction 0 has no light points: the wall draw's six light words
   would be whatever its stack array last held (§11 r2). No 1.14d tile
   has light direction 0 (Open question 8, answered), so this never
   happens with the original data.
8. Ring-0/1 shade is never written (§7.3 r4).
9. The Den-light placement steps the local player's seed (§10 r1).

## Test vectors

Synthetic (CI-safe; §7 with ambient 0, `I` = 255, light at x = y = 804
(sub-tile 100, position fraction 0), radius 13 → `r` = 104, `k` =
160,689, map origin 76):

| Input | Expected output | Source |
|---|---|---|
| `oct(4, 4)`, `oct(36, 4)`, `oct(108, 4)` | 5, 36, 105 | §7.1 r4 |
| plain, cell (100, 100) and (101, 100) | 242, 242 | §7.1 |
| plain, cells (105, 100), (110, 100), (112, 100), (113, 100), (114, 100) | 166, 73, 36, 17, 0 | §7.1 |
| shadowed, one blocking cell (102, 100): cells (101, 100), (102, 100) | `S` 0, 0; `I` 242, 223 | §7.3 |
| same: (103, 100), (104, 100), (110, 100) | `S` 16; `I` 0 | §7.3 |
| same: (104, 101), (106, 101), (106, 102) | `S` 7, 9, 4; `I` 116, 74, 104 (plain 186, 149, 139) | §7.3 |
| `q` = 0, monster at the same point, radius 13 | radius 8: cells (100..101, 100..101) = 95 each, nothing else | §7.2 |
| two sources each adding 200 to a cell | `I` = 255 | §7.1 r5 |
| environment, normal, act 1, ticks = 128 × degree 0, 30, 45, 90, 135, 179, 180, 200, 270, 300, 339 | `I` = 128, 192, 219, 255, 219, 130, 128, 106, 64, 73, 105 | §9.3 r4 |
| same in act 5 (`A` = 4) | 128, 170, 170, 170, 170, 130, 128, 106, 64, 73, 105 | §9.3 r4 |
| color, normal, index 3, ticks 170 × 128; index 4, 190 × 128; index 0, 330 × 128 | (225, 204, 225), (160, 149, 218), (167, 164, 188) | §9.4 |
| 0x53 setter, index 5, ticks 0, eclipse 1, previous flag 0, act 2 (`A` = 1, `L` = 40) | first intensity (flag 0, θ = 0) `I` 128; period reset → type 2, ticks 30,720; second intensity (eclipse) `I` 120; color = eclipse entry 5 (0, 30, 243) | §9.2 r2 |
| Den counter 0, 15, 29 | `I` 80, 56, 4; past 29 the override is off (§3.1 r2 applies) | §10 r1 |
| `W[7]` | 0.08579731732606888 (float) | §10 r1 |

Real 1.14d values (game-file tests, `D2_GAME_DIR`):

| Input | Expected output | Source |
|---|---|---|
| leveldefs row 8 (Den of Evil) | `Intensity` 0, R, G, B 255: ambient `I` 0 | §3.1 r2 |
| leveldefs row 1 (Rogue Encampment) | R, G, B 0: ambient from the environment | §3.1 r3 |
| leveldefs row 103 | `I` 140 | §3.1 r2 |
| `wall-light-points.tsv` | 108 rows = `Game.exe` `0x0072A9E8` / `0x0072ABC8` for directions 1–9 | §11 r2 |
| `env-periods.tsv` | 18 rows = `0x007443F0`, `0x00744438`, `0x00744480` | §9.1 |
| `frames-raw-1` run 1b f 12,543–13,694 | §12 r4 counts | §12 |

## Provenance

1.14d `Game.exe`: build `0x00475800` (from `0x00476BC0`), ambient
`0x00474610`, `0x004744B0`, `0x00474550`, `0x0046BDD0`, `0x0046BD50`,
`0x00619D70`, `0x0066BFD0`, `0x0061E470`, `0x0061C0B0`, `0x0061C0E0`;
flags `0x004756D0`; quality `0x00475780`, `0x0044CCE0`, `0x0044F29C`,
`0x0044F100`, options `0x0047CFE0`, `0x004F5220`, `0x004CCF20`; records
`0x00474160`, `0x00474290`, `0x004742D0`, `0x00474350`, `0x00474390`,
`0x004743D0`, `0x00474470`, `0x004755A0`, `0x00475930`, `0x00475B40`;
contributions `0x004748D0`, `0x004740D0`, `0x00474080`, `0x004747C0`,
`0x00474D70`, `0x00474A70`, `0x00474B50`, `0x00474C00`, `0x004750F0`,
`0x00475420`; sources `0x00460BF0`, `0x004609F0`, `0x00460930`,
`0x004609A0`, `0x004AE210`, `0x004ACC70`, `0x00470390`, `0x004CD540`,
`0x004CD1C0`, `0x004D2C70`, `0x004C5680`, `0x004BC580`, `0x004D6D40`,
`0x004F3530`; environment `0x0061BE40`, `0x0061BFC0`, `0x0061BEE0`,
`0x0061BB80`, `0x0061BCE0`, `0x0061C240`, `0x0061BDF0` (EAX record;
call order in the `0x0061C240` disassembly), `0x0045E300`, `0x006427F0`, CRT
`sin` `0x00688590` / `cos` `0x006886C0` (x87 `fsin` / `fcos` paths),
`0x00682FD0` (truncating conversion); overrides `0x0046BEB0`,
`0x0046AD10`, `0x0046AE50`, `0x0046B0C0`, `0x0046B0D0`, `0x0046AF70`,
`0x0046BE60`, `0x0046BF90`, `0x0046B290`, `0x0046B3A0`, sine table
`0x00707800` (`0x0040B330`); draws `0x00475AA0`, `0x004DF1C0`,
`0x004DD180`, `0x004DD600`, `0x004DE410`, `0x004DDEF0`, `0x004DEA70`,
`0x006C94B0`, `0x006C93A0`. Tables read from the file:
`0x006DB9D8`, `0x0072A9E8`, `0x0072ABC8`, `0x007443E4`, `0x007443F0`,
`0x00744438`, `0x00744480`, `0x00707800`, act start levels `0x006EB2F0`
(1, 40, 75, 103, 109, 1024). Data: `levels.txt`, `monstats2.txt`,
`missiles.txt`, `objects` / `overlay` columns via `data/fields.tsv`.
Capture analysis: run 1b raw JSONL and PNGs, decoded with a stdlib PNG
reader, same-key pairs diffed and grouped into 8-connected blobs (gap 3),
palette and light maps from act 1 `pal.pl2`. D2MOO not used; riiablo not
needed.
Ghidra backlog (2026-10-06): lookups `0x00463990`/`0x004639B0`/
`0x00463940`; new-room callback `0x00475930` (called at `0x00619954`); monster light `0x004AE210`,
`0x0063EBD0`, umod hooks `0x004AD020`/`0x004ACC70` (tables `0x00724D78`,
`0x006DA4C8` read from the file); cast light `0x004C5680` (caller
`0x004C6140`), `cltdofunc` table `0x00727BA8` entry 30 = `0x004F3530`;
missile create `0x004CDA6F`–`0x004CDACD`; flicker `0x004CD1C0`; S→C 0x89
`0x0046B630` with handler table `0x007129D8`; S→C 0x5D `0x004A2CB0`
(jump table `0x004A2F74`/`0x004A2F94`); S→C 0x5E `0x004B92B0`; states
setfunc table `0x0072A690`; environment creation `0x0061BE40`. Live
data: `states` setfunc, `missiles` rows 191/288, `objects` row 17,
`skills` cltdofunc / cltstfunc, `monumod` rows 1–4.

## Open questions

1. ~~Which unit tables `0x00463990` / `0x004639B0` search~~: answered in
   §6.4 r1 (sets S and C).
2. ~~The room-unload test of `0x00475930`~~: answered in §6.4 (it runs
   for a new active room, not an unloaded one: `drlg/rooms.md` §5 rule 9).
3. ~~Monster light inputs~~: answered in §8 r1–r2 and the monster row
   (`Align`, client-only flag, umod 3 hook).
4. ~~Radius-1 missile light paths, `+0x50`, flags 0x300~~: answered in
   §8 r3–r6.
5. Triggers of the scripted events: answered in §9.2 r3 (eclipse) and
   §10 r4 (S→C 0x89 ids, Cloak of Shadows); client quest byte 1 is byte
   1 of the last S→C 0x5E (`0x0045E570` → `0x004B92B0` copies 37 bytes
   to `0x007C0EA4`: the not-intro byte of the Den of Evil quest,
   `world/quests.md` step 5). Open: the server conditions of 0x89 ids 12
   and 13 (`0x005B5230`, `0x005B52E0`) belong to `world/quests.md`; a
   trace of the Den of Evil clear confirms id 0.
6. Environment state before the first S→C 0x53: the creation values are
   answered in §9.1 (`A` = 0, `L` = 0: `I` 128, white). Open: whether
   0x53 always arrives before the first world draw (trace of a game
   join).
7. Whether `sin` / `cos` of the CRT (SSE2 path `0x00699E30` / `0x0069A000`
   or x87 `fsin`) give the correctly rounded value for every θ of §9.3, so
   `trunc` matches: compare a recorded env `+0x0C` per tick over one day
   with §9.3.
8. *Answered* (`impl-lighting-blend` "Not wired" 3), first half: no.
   Every tile header of the 250 version-7 DT1 files in `d2data.mpq` /
   `d2exp.mpq` (`mpq-tool extract "*.dt1"`; the 6 version-4 leftovers of
   `formats/dt1.md` are never loaded) has light direction 1–9, fixed by
   orientation: 0 → 3; 1, 5, 8, 10 → 1; 2, 6, 9, 11 → 2; 3, 4, 12, 13,
   14 → 3; 7 → 4; 15 → 5; 16–19 → 6–9. So the point-count table
   `0x006DB9D8` (10 entries: 0, then 6 × 9) is never indexed with 0 or
   past 9. If d2rs meets direction 0 (modded tiles): the wall pass keeps
   one eight-dword light array for the whole pass (`0x004DF1C0` loop),
   so the record reuses the light words of the previous record drawn in
   that pass; with none before it the values are undefined in the
   original (treat as fatal). Still open: what direction 5 (roofs,
   orientation 15; points all (0, 0)) reads, if a roof ever reaches a
   wall pass.
9. The light source behind run 1b f 13,496–13,599 (§12 r4): rerun with
   the light-map digest of §12 r3 in the key and the draw log, or read
   the light list (`[0x007B5668]`) at those frames.
10. Where the lighting-quality option is loaded at start (registry or
    settings) and its default.
    *Partly answered* (static): the registry. `0x0047CFE0`, the
    lighting-quality item's callback (pointer in the options item
    tables at `0x00718DD8` and `0x0071B858`), reads value "Light
    Quality" of key "Diablo II" (strings `0x006D73A8`, `0x006CC8B8`)
    through `0x00414F10` into item +0x124, then applies it as §5: 0 →
    low (`[0x0072DA50]` := 1, `[0x0072A348]` := 0), 1 → medium (0, 0),
    2 → high (0, 1), any other value → no change. With no value the
    statics stand: (0, 1), high. The other writers are the menu change
    handler (same three branches, `0x0047CFB2`…) and the toggle
    `0x004F54F0` (low flag := not low flag; callers `0x00405DAB`, a
    command-line/settings path, and `0x004776B0`). Still open: whether
    the item callback runs before the first in-game draw (it may run
    only when the options menu is built); a memory read of the two flags
    at the first draw settles it.
11. `0x004BC5E0(object, 0)` (the object refresh of §9.2 r4 step 4 when
    the day period changes): which object classes change (lights,
    torches, mode) and how; owner: the client object spec. A recording
    across a day-period change with objects in sight settles it.
    Also run every 500 ms for object class 39 through `ClientFn` 14
    (`0x004BDCC0`, `world/objects-client.md` §26.14).
    *Answered* (static, 1.14d asm of `0x004BC5E0`; ECX object, EDX
    flag): only objects whose objects `EnvEffect` (+0x139) ≠ 0 change;
    others return 0. `p` := the act's period type (`0x0061C100(act,
    0)` returns env +0x04, `env-periods.tsv` `type`: 0 day, 1–3 the
    others). `p` 1–3: object mode 0 → mode := 1, the graphics refresh
    `0x00470610(object)` (flag 0 only), the animation re-init
    `0x00624390(object)` and client unit flag 0x2 (+0xC4) := objects
    `Selectable1` ≠ 0; then, whatever the mode, the object light
    `0x004BC580` with radius `Lit<mode>` / 2 and `Red`/`Green`/`Blue`
    (§8 object row). `p` 0: mode 1 or 2 → mode := 0, the same refresh
    and re-init, the light with `Lit0` (0 → removed), flag 0x2 :=
    `Selectable0` ≠ 0; mode 0 → nothing (returns 0). `p` > 3 → fatal
    0x66. Live data (`patch_d2` `objects.txt`, 574 rows): 4 rows have
    `EnvEffect` 1 — `fire` (39: `Lit` 0, 19, 19, color 255, 236, 176,
    never selectable), `AmbientSound` (45) and two `Dummy` (71, 72),
    whose `Lit` and `Selectable` are all 0. So a day period puts fire 39
    in mode 0 with no light; dusk, night and dawn put it in mode 1 with
    light radius 9. Callers: the day refresh (§9.2 r4 step 4, flag 0),
    `0x004BC720` (flag 1) and `0x004BDCC0` (flag 0, when the wall-clock
    tick count exceeds object +0xD4); those two belong to the client
    object spec.
12. *Answered* (`impl-lighting-blend` "Not wired" 3): the eclipse branch
    of the 0x53 setter is the period reset `0x0061BDF0` (§9.2 r2): type
    and ticks from the eclipse table's entry of the index, discarding the
    received ticks; then intensity with the flag set and color.
13. *Answered (2026-10-08)* (`impl-client-msgs-3` Q8): with no client
    act and an unplaced P the act check of §9.2 r4.2 passes (null =
    null) and the setter is fatal 0x547 on the null act before any
    write (§9.2 r4.2; `0x0045E31C`, `0x0061C247`, `0x0061AA60`).
14. *Answered (2026-10-08)* (`impl-client-msgs-3` Q7): open question 11
    gives `p` = env +0x04 but not its value per state; that value is
    the period type of (index, eclipse flag) only, never ticks or the
    act-4 table (§9.2 r4.4, "The day period").
