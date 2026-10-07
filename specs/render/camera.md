# Spec: Render — Camera (world position → screen draw position)

- **Status:** draft (2026-10-06, RE on 1.14d `Game.exe`). §1–§3 match
  every frame of the first two 1.14d capture runs (15,934 frames, open
  modes 0–3, no shake; `capture.md` Test vectors); the pixel cases of §Test
  vectors have not run.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::world_view` (`ViewRules::tiles`,
  `unit_params`, `place`), `d2-client::composite` (`UnitParams.clip`)
- **Related specs:** `render/sprite-placement.md` (what a draw at (X, Y)
  covers), `render/composition.md` (frame size, frame cycle),
  `render/draw-order.md` (which tiles/units, in which order; to write),
  `render/unit-composite.md` (per-unit extra offsets; to write),
  `sim/tick.md` (the 25 Hz tick)

<!-- index -->
| Section | Lines |
|---|---|
| Summary | 41–51 |
| Inputs | 52–62 |
| Outputs / state changes | 63–68 |
| Rules | 69–70 |
|   1. Frame size and play area | 71–92 |
|   2. World coordinates → client pixels | 93–117 |
|   3. Camera origins (once per drawn frame) | 118–133 |
|   4. Units | 134–152 |
|   5. Panel shift for floors | 153–158 |
|   6. Tiles | 159–198 |
|   7. View culling | 199–248 |
|   8. Screen shake | 249–280 |
|   9. Time base: no interpolation | 281–323 |
|   10. What d2rs hooks get | 324–332 |
| Constants & data dependencies | 333–339 |
| Randomness | 340–345 |
| Edge cases & original bugs | 346–355 |
| Test vectors | 356–377 |
| Provenance | 378–401 |
| Open questions | 402–494 |
<!-- /index -->

## Summary

Each drawn frame the client recomputes two camera origins from the local
player's position: one for map tiles and one for units. A world point
becomes a screen draw position by subtracting the origin; the player ends
up at the middle of the play area. Positions are integers derived from
the units' 16.16 fixed-point subtile coordinates with floor shifts. There
is no interpolation between ticks: one frame is drawn per client tick and
shows the state after that tick. A screen shake adds a random offset to
both origins.

## Inputs

| Name | Type | Source |
|---|---|---|
| player position | 16.16 fixed subtile (x, y) (dynamic path) | sim via bridge |
| unit positions | 16.16 fixed (moving units) or integer subtile (static units) | sim via bridge |
| tile cells | absolute tile (tx, ty) | DRLG / map |
| frame size | W × H (800 × 600) | `composition.md` §1 |
| screen open mode | 0–3 (panels) | UI state (`ui/panels.md`) |
| shake | amplitude envelope + offsets | client effects (§8) |

## Outputs / state changes

Per frame: the tile origin `(cx_t, cy_t)`, the unit origin `(cx_u, cy_u)`,
the play-area left edge `left`, and from them the draw position (X, Y)
handed to `sprite-placement.md` for every tile and unit component.

## Rules

### 1. Frame size and play area

The frame is W × H with W × H = 800 × 600 (resolution mode 2) or
640 × 480 (resolution mode 0): `0x0044BA3C` / `0x0044BA5C` set
`GeneralDisplayWidth/Height` (`0x0071146C`/`0x00711470`, initial 640 × 480).
1.14d offers both; d2rs supports 800 × 600 only (EARLY_DECISIONS 9,
`ui.md` §A5) and every rule below is written with W, H.

The **play area** height is `H − 40` (`0x0044BA87`: `0x007A521C = H − 40`,
`0x007A5220 = W`). The view rectangle (`view +0x04..+0x10`, view object at
`[0x007A0640]`, set by `0x00476000`) is, per screen open mode `m`
(`D2Client_ScreenOpenMode 0x007A5210`, set by `0x0045AEA0`):

| m | left | top | right | bottom | `shiftX` (`0x007A5214`) |
|---|---|---|---|---|---|
| 0, 3 | 0 | 0 | W | H − 40 | 0 |
| 1 | −(W / 4) | 0 | W − W / 4 | H − 40 | −(W / 4) |
| 2 | W / 4 | 0 | W + W / 4 | H − 40 | W / 4 |

`W / 4` here is C division (all positive, so floor). Which panels set
which mode is `ui/panels.md`; mode 3 also calls `0x0044DA70`.

### 2. World coordinates → client pixels

Units: a moving unit (players, monsters, missiles) keeps a 16.16 fixed
subtile position `(x16, y16)` (unsigned). Its client pixel position is
(`0x00649EA9`–`0x00649ED3`, `0x0064FBD7`–`0x0064FBF6`, via `0x00643290`):

```
a = x16 >> 11            b = y16 >> 11            (logical shift: 1/32 subtile)
px = (a − b) >> 1        py = (a + b) >> 2        (arithmetic shift: floor)
```

Static units (objects, items, tiles; unit types 2, 4, 5) keep integer
subtiles `(sx, sy)` and `px = (sx − sy) × 16`, `py = (sx + sy) × 8`
(`0x00620B31` → `0x00643260`, stored at static path `+4/+8`). The client
reads them through `0x00620650` / `0x006206B0`.

Note: D2MOO's `DUNGEON_GameToClientCoords` (1.10f) divides with C `/`
(truncation); 1.14d shifts (floor). They differ for negative `a − b`.

Tiles: cell `(tx, ty)` (absolute tile coordinates) has
`sx = (tx − ty) × 80`, `sy = (tx + ty) × 40` (the `map-preview.md` cell
origin). 1.14d stores each tile entry as `e0 = sx − 80`, `e1 = sy + 80`
(`0x0066DC10`, `0x0066DDE0`: `0x00643310` on `(tx, ty + 1)`, then
`e1 += 40`).

### 3. Camera origins (once per drawn frame)

From the local player (`[0x007A6A70]`) client position `(P_x, P_y)`:

- **tile origin** (`0x0044C9CD`–`0x0044C9FA`, stored by `0x004760A0` in
  `view +0x24/+0x28`):
  `cx_t = P_x − (right − left) / 2 = P_x − W / 2`,
  `cy_t = P_y − (bottom − top) / 2 = P_y − (H − 40) / 2`,
  then plus the shake offsets `(dx, dy)` (`0x00476D40`, §8).
- **unit origin** (`0x0045B440`, globals `0x007A520C` / `0x007A5208`):
  `cx_u = P_x − W / 2 + dx`, `cy_u = P_y − H / 2 + 16 + dy`.

Divisions by 2 are C signed division (`cdq; sub; sar`) on positive sizes.
The tile origin is computed before `StartDraw`, the unit origin right after
it (`0x0044C990`), from the same player position.

### 4. Units

A unit at client `(px, py)` is drawn at (`0x00471EC0`, origin getters
`0x0045AFC0` = `cx_u − shiftX`, `0x0045AFD0` = `cy_u − 8`):

```
X = px + ox − cx_u + shiftX
Y = py + oy − cy_u + 8
```

`(ox, oy)` are per-unit extra offsets owned by `unit-composite.md`
(objects: `objects.txt` Xoffset/Yoffset and skipped when Draw = 0, record
`+0x148/+0x14C/+0x150`; missiles and the motion-record offsets of
`0x004DA0B0`–`0x004DA0F0`: `unit-composite.md` §8). Each component cel is then placed at
(X, Y) by `sprite-placement.md` §2.

So the local player (offsets 0, no shake) is drawn at
`(W / 2 + shiftX, H / 2 − 8)`: (400, 292) at 800 × 600, mode 0.

### 5. Panel shift for floors

The floor drawers subtract 80 from X and add −(W / 4) in open mode 1,
+(W / 4) in mode 2 (`0x005132DA`–`0x0051330A`, GDI `0x006C95E7`), i.e. the
same `left` the walls get from the view rectangle. Units get it as `shiftX`.

### 6. Tiles

With `left` from §1 and `top = 0`, tile cell `(tx, ty)` is drawn at:

| Tile list | Caller → drawer | (X, Y) handed to the drawer | block pixel `(bx, by)` lands at |
|---|---|---|---|
| floors | `0x004DE730` → `0x004DE410` → `DrawGroundTile 0x004F68E0` (driver slot `+0x7C`: floor drawer, DirectDraw `0x005132C0`, GDI `0x006C95D0`) | `(sx − cx_t, sy − cy_t)` | `(sx − 80 + bx − cx_t + left, sy + by − cy_t)` |
| walls (wall-layer list) | `0x004DF1C0` → `DrawWallTile 0x004F6920` (slot `+0x9C`: wall drawer, DirectDraw `0x005131B0`, GDI `0x006C94B0`) / translucent `0x004F6950` (slot `+0xA0`, DirectDraw `0x005130A0`, GDI `0x006C93A0`) | `(sx − 80 − cx_t + left, sy + 80 − cy_t + top)` | `(sx − 80 + bx − cx_t + left, sy + 80 + by − cy_t)` |
| shadow tiles (shadow pass kind 1) | `0x004DF510` → `0x004F6980` (slot `+0xA4`, GDI `0x006C9290`) | as walls: `(sx − 80 − cx_t + left, sy + 80 − cy_t + top)` | as walls |
| roofs (fading list) | `0x004DEA70` → `DrawGroundTile 0x004F68E0` (the floor drawer, as floors) | `(sx − cx_t, sy − roof_height − cy_t)` | `(sx − 80 + bx − cx_t + left, sy − roof_height + by − cy_t)` |

Shadow tiles take the wall position by the same expression: in GDI
(`0x004F51D0` = 0) `0x004DF5C6`–`0x004DF5D8` add the view offsets to the
tile record's +0x00 / +0x04 exactly as the wall walk does
(`0x004DF145`–`0x004DF156`); with a 3D renderer both pass the list
item's own +0x04 / +0x08. The GDI drawer places block `i` at (X + block
x, Y + block y) (s16 pair at block +0x00 / +0x02), as the wall drawer
`0x006C94B0`.

Roofs go through the floor drawer (`0x004DEA70` calls `0x004F68E0`
only), so they take its −80 and panel shift (§5) and its whole-tile
culling (§7), not the wall drawer's per-block culling.
`roof_height` is the DT1 tile header field at `0x04` (read at `0x004DEBA6`). Which
orientations are in which list, the order, shadows and the fade alpha are
`draw-order.md` / `blend-modes.md`. The floor/wall alignment equals
`map-preview.md` (walls 80 below floors, `WALL_BASE`); the absolute x is
80 left of its `sx`. Roofs differ from `map-preview.md` (no `+WALL_BASE`):
live roof blocks lie where floor blocks do (every block of the
orientation-15 tiles in the used DT1 files has y in 0 … 64, on the floor
diamond rows; the exact block count is *Pending*, Open question 1), so a roof is its cell's floor diamond raised by
`roof_height` rows; `map-preview.md`'s `sy + y0 + WALL_BASE − roof_height`
puts it `WALL_BASE` (80) rows lower than 1.14d. `roof_height` is read
unsigned (`0x004DEBA6`, 16-bit zero-extended); live values 0, 80, 100,
120, 156, 160, 190, 230, 240 and once 56,376 (`expansion\Siege\
temptile.dt1` tile 15: drawn 56,376 rows up, so always culled by §7).

Tile vs unit: a unit at the exact top vertex of cell `(tx, ty)`
(`px = sx`, `py = sy`) is drawn 12 rows below the floor's top vertex:
units use `H / 2 − 8`, tiles `(H − 40) / 2` (§3, §4).

### 7. View culling

- Floors and roofs: drawn only if the (X, Y) handed to the drawer lies in
  the view clip rectangle `view +0x14..+0x20` = `[−80, W + 80) ×
  [−80, H − 47)` (`0x00476000` sets it; `0x004DE410`, `0x004DEA70` test
  it). Perspective mode only (not the software renderer) widens it.
- Wall blocks (wall drawers only, lit `0x005131B0` and translucent
  `0x005130A0` alike): skipped when the block's screen x is
  outside `[−32, W)` in modes 0/3, `[−32, W − W / 2)` in mode 1,
  `[W / 2 − 32, W)` in mode 2, or its y outside `[−32, H + 32)`
  (`0x005131D3`–`0x0051324B`; the block's screen position is the X, Y
  handed plus the block's `(x, y)`). The test is per block: a kept block
  is drawn whole even where it reaches past the bound (up to 31 pixels).
  It equals one clip of the assembled tile image to the union of the kept
  blocks only when no culled block overlaps a kept one; with 32-wide
  blocks whose x and y lie on a 32 grid of the tile that always holds,
  and it does for the live data: every block of non-floor, non-shadow,
  non-roof tiles (orientation ∉ {0, 13, 15}) of the used DT1 files has
  x ≡ 0 and y ≡ 0 (mod 32) (both block formats are 32 wide; the count,
  and y, are *Pending* a recount, Open question 7). In modes 0/3 a culled block has no pixel in the frame, so
  culling changes no pixel there.
- Units: no view-rectangle test. The world unit draw `0x004DC7B0` skips a
  unit only (a) by the unit flags and states it checks (owner
  `draw-order.md`), (b) in perspective mode (not GDI) by `0x004F66E0`, and
  (c) when the visibility test `0x004DC710` fails: other players (not the
  local one) alive, monsters alive (mode ≠ 0, 12), missiles and items
  are hidden when `0x00622AA0(local player, unit, 2)` is non-zero, unless
  the player's level has `0x00642840` = 0 (that test is skipped); objects
  and dead units always pass. `0x00642840` is the level's `LOSDraw`
  (LevelDefs record of 0x9C bytes at `[0x0096C890]` via `0x0061E470`,
  field `+0x98`; D2MOO `dwLOSDraw`): in the live `levels.txt` 83 levels
  have 1 and 54 have 0 (all towns and outdoor areas, e.g. Act 1 –
  Wilderness 1–6, Act 5 – Siege 1), so outdoors nothing is hidden by
  sight. `0x00622AA0(a, b, mask)` (D2MOO `UNITS_TestCollisionWithUnit`)
  takes both units' positions (`0x0045ADF0`/`0x0045AE20`) and sizes
  (`0x00620510`, `sim/path-placement.md`), moves each end toward the other
  by its size (capped at 2, `0x00622920`; 0 = not blocked when |dx| + |dy|
  is below the two sizes' sum) and tests the line between them
  against the collision map of `a`'s room with `mask` (`0x0064E260`);
  mask 2 is D2MOO `COLLIDE_VISIBLE` (obstacles one cannot see or shoot
  over). The line walk itself belongs to the collision spec (to write;
  `monsters/ai.md` uses the same test with mask 4). Unit tiles (type 5) are never drawn
  (`0x00471EC0`). Pixels outside the frame are cut by the cel clip
  (`sprite-placement.md` §5), whose pre-test only rejects cels with no
  visible pixel.
- DT1 shadow tiles (shadow pass kind 1, `draw-order.md` §6 r3; slot
  `+0xA4`, GDI `0x006C9290`, `0x006C9318`–`0x006C932A`): the same
  per-block test as the wall blocks above, with the same bounds and the
  same mode argument (the drawer's last argument). No whole-tile test.

### 8. Screen shake

Started by `0x00476A80` (peak `A` in ECX, attack `t1` ms in EDX, sustain
`t2`, release `t3` on the stack; start time `GetTickCount`; ignored when
`t2 = 0`). Each drawn frame (`0x00476D40`), with `t = now − start` (ms,
wall clock):

| `t` | amplitude a |
|---|---|
| `t > t1 + t2 + t3` | 0; shake ends (`0x007B9534 = 0`, offsets 0) |
| `t < t1` | `(A × t mod 2^32) / t1` |
| `t1 ≤ t < t1 + t2` | `A` |
| otherwise | `(A × (t1 + t2 + t3 − t) mod 2^32) / t3` |

All values are unsigned 32-bit (comparisons unsigned, `t` a
`GetTickCount` difference); the product keeps its low 32 bits (`imul`) and
the division is unsigned. A zero divisor is never reached: the attack row
cannot apply with `t1 = 0`, and with `t3 = 0` the release row (only `t =
t1 + t2`) gives `a = 0` for that frame (`0x00476D9D`, `0x00476DBE`). When
`a = 0` and the shake has not ended, no random draw is made, the origins
get no offset, and `0x007B9538` / `0x007B8D20` keep their last values.

If `a ≠ 0`: `dx = −a + rnd(2a)`, then `dy = −a + rnd(2a)` (two draws of the
seeded RNG helper `0x00472280` on the local player unit's seed,
`unit +0x20`; `rnd(n)` per `sim/rng.md`), stored in `0x007B9538` /
`0x007B8D20`, added to the tile origin and, through `0x00476AC0`, to the
unit origin. `0x004769D0(a)` also drives a rumble sound (audio specs).
Callers of `0x00476A80` (skills/missiles) and their parameters belong to
their effect specs. The same seed is stepped by the mouse cursor in its
idle state and by the weather in the same frame (`capture.md` §3.3,
Randomness).

### 9. Time base: no interpolation

The client loop (`0x0044EFA0`) runs the in-game draw (`0x0044C990`) at
most once per client tick of 40 ms (`0x0070EF1C = 40`): in single player
the draw follows the server tick and the client update of the same loop
pass, and later passes without a tick do not draw (`0x007A0704`); when the
loop falls behind, draws are skipped, never interpolated. While a single
player game is paused the draw runs every pass with no tick. Every
position above is the integer state at draw time; no sub-tick time enters
any formula except the shake envelope (§8, wall clock).

Client path step: each client update (`0x0044C790`) runs the per-unit
update `0x00480810` once per client unit (`0x00465AA0` walks the client
unit tables with it). The path step is `0x00650840(unit, base)`
(`sim/pathing.md` §9.4); its only client callers are `0x004807C0`
(players and monsters) and the missile step `0x004D30C0`.

`base` is `[0x007A04C4]`, read through `0x0044DB10` by those two callers
only. The dword lies in the 0x80-byte block `0x007A0480`–`0x007A04FF`,
which `0x0044E200` (game start) and `0x0044C890` (game end) zero with
one memset. No instruction writes it, by absolute address or indexed.
(`0x007A0468` is the lock object passed to the imports at
`[0x006CC210]` / `[0x006CC218]` / `[0x006CC214]`, not a record that
holds it.) So `base` is always 0, and the step uses the server's 0x400
(`0x006502D0`: base ≤ 0 → 0x400).

Steps per client update:

| Unit | Steps | Condition (1.14d) |
|---|---|---|
| Player | 0 or 1 | `0x00463390` → `0x004807C0`. Steps when the mode class `[0x00711E00 + 12 × mode]` is 1, or when it is 2 and the used skill (`0x00620250`) has flag bit 0 (`0x006446A0`). The two calls are exclusive. |
| Monster | 0, 1 or 2 | `0x004B13A0`. First `0x004AF4C0` steps when the used skill (`0x00620250`, its skills row via `0x00643CE0`) has flag bit 0 (`0x006446A0`). Then the update steps again when the monster mode-table entry (`0x004AF400`) is 1. Nothing excludes both in one update. |
| Missile | 0, 1 or 2 | Each client missile function (table `0x0072A398`) calls `0x004D30C0` at most once. It steps once when the missile has a path (`0x006486C0`). It then loops once more (`0x004D3341` → `0x004D3141`) when the owner (`0x004639D0`) is the local player (`0x00463DD0`) in game types 0, 1, 6 or 8, its elapsed frames (`0x0064A3B0`) are 1, and it still has frames left. So a local player's missile moves two steps on its first client update, and its frames left drop by 2. |

d2rs follows this table, with each step one server step of 0x400.
Whether a monster meets both of its conditions in one update in practice
is a recording question (Open question 5). The rule is the table.

For d2rs: one frame per presented tick, positions from the snapshot of
that tick, no interpolation. The shake envelope uses `t = 40 × (ticks since
start)` (the original's frame spacing); pixel checks of shaking frames
take the recorded `(dx, dy)` as input (`capture.md` §4).

Dropped ticks (frame-cycle FC3): when several ticks run before one draw,
the original draws only the state after the last of them; which ticks
are dropped depends on wall-clock load, in 1.14d and in d2rs alike, so it
is not a fidelity rule. A d2rs loop that waits for the previous frame
(e.g. a GPU read-back of the index framebuffer) and then draws only the
latest tick matches this. What must hold: the framebuffer a drawn frame
starts from is the previous **drawn** frame's (`composition.md` §3), and
the draw-time writes (`draw-order.md` Outputs, `unit-composite.md` §3 r5)
happen once per drawn frame, never for a dropped tick. Checks select
frames by the recorded `seq` and tick (`capture.md` §4), never by count.

### 10. What d2rs hooks get

- `ViewRules::tiles`: (X, Y) per tile from §6, culled by §7;
  `sprite-placement.md` §8 turns it into `DrawItem (x, y)`.
- `ViewRules::place` / `ComponentResolver::place`: X, Y from §4, then
  `sprite-placement.md` §8.
- `UnitParams.clip`, `DrawItem.clip`: the frame `[0, W) × [0, H)`; the play
  area is not a clip (units and tiles may draw under the control panel).

## Constants & data dependencies

W, H; play-area height `H − 40`; tile cell 160 × 80 (`sx`, `sy`);
subtile 32 × 16 (`× 16`, `× 8`); unit origin constants `+16` and `+8`;
view clip margins 80 and 47; block cull margin 32; tick 40 ms
(`0x0070EF1C`).

## Randomness

Screen shake only: two draws of the local player unit's seed per drawn
frame while `a ≠ 0` (§8). This is a client-side copy of the unit; it does
not touch server RNG.

## Edge cases & original bugs

1. Tiles and units are anchored 12 rows apart (§6), from the play-area
  height used for tiles and the full height used for units. Reproduce.
2. The shake offset range `[−a, a − 1]` is not symmetric. Reproduce.
3. The shake envelope runs on wall-clock milliseconds, so the original's
  shake is not a function of ticks (§9).
4. Panel culling of wall blocks (§7) can leave wall pixels missing at the
  panel edge where the panel art does not cover them. Reproduce.

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| moving unit `x16 = 10 << 16`, `y16 = 10 << 16` | `a = b = 320`, `(px, py) = (0, 160)` | §2 |
| `a = 320`, `b = 321` | `px = −1 >> 1 = −1` (D2MOO's `/2` gives 0), `py = 160` | §2 |
| static unit subtile (12, 7) | `(80, 152)` | §2 |
| cell (3, 1) | `sx = 160`, `sy = 160`; entry `e0 = 80`, `e1 = 240` | §2 |
| 800 × 600, mode 0, player `(1000, 2000)`, no shake | `cx_t = 600`, `cy_t = 1720`, `cx_u = 600`, `cy_u = 1716`; player at (400, 292) | §3, §4 |
| same, mode 1 | `left = shiftX = −200`; player at (200, 292) | §1, §4 |
| same, floor cell with `sx = 640`, `sy = 1720`, block (0, 0) | X handed = 40, Y = 0; block at (−40, 0) | §6 |
| same cell as a wall tile, block (0, −64) | block at (−40, 16) | §6 |
| 640 × 480, mode 0, player `(0, 0)` | `cx_t = −320`, `cy_t = −220`, player at (320, 232) | §1, §3 |
| floor handed (X, Y) = (−81, 0) | culled; (−80, 0) drawn; Y = 553 at H = 600 culled | §7 |
| shake A = 10, t1 = 100, t2 = 200, t3 = 100, t = 50 | a = 5, offsets in [−5, 4] | §8 |
| shake A = 10, t1 = 100, t2 = 200, t3 = 0, t = 300 | a = 0: no draw, origins unchanged, shake not ended | §8 |
| shake A = 10, t1 = 100, t2 = 200, t3 = 0, t = 301 | ended: `0x007B9534 = 0`, offsets 0 | §8 |
| shake A = 0x10000, t1 = 0x20000, t2 = 1, t3 = 1, t = 0x10000 (attack) | product 2^32 keeps 0 → a = 0 | §8 |
| mode 1, W = 800: wall blocks at screen x 399 and 400 | 399 drawn (pixels 399–430), 400 skipped | §7 |
| 1.14d captures (`capture.md`): recorded path client `(px, py)` vs §2 from the fixed position; tile / unit origin, view rect and `shiftX` vs §1, §3 | equal on 15,934 of 15,934 frames | `frames-raw-1` runs 1, 2 |
| capture case `camera-0001`: walk 5 s in a cleared area, every frame captured with state (`capture.md`) | per frame, CPU reference from the recorded positions equals the capture; recorded origins equal §3 | capture, queued |

## Provenance

1.14d `Game.exe`: shadow tiles `0x004DF510`/`0x004F51D0`/`0x006C9290`;
draw frame `0x0044C990`, client loop `0x0044EFA0`,
view `0x00476000`/`0x00476070`/`0x004760A0`, open mode `0x0045AEA0`,
unit origin `0x0045B440`, unit draw `0x00471EC0`/`0x004DC7B0`, tile lists
`0x004DE730`/`0x004DE410`/`0x004DF1C0`/`0x004DEA70`, tile entries
`0x0066DC10`/`0x0066DDE0`, coordinate helpers `0x00643260`/`0x00643290`/
`0x00643310`, path updates `0x00649D00`/`0x0064FB90`, static path
`0x00620AE0`, shake `0x00476A80`/`0x00476D40`/`0x00476AC0`, floor drawers
`0x005132C0`/`0x006C95D0`, wall drawer `0x005131B0`. Community labels
(`refs/1.14d-notes`): `ScreenOpenMode`, `GeneralPlayAreaCameraShiftX`,
`GeneralDisplayWidth/Height`, `ResolutionMode`. D2MOO (1.10f)
`D2DynamicPathStrc` field names and `DUNGEON_*Coords` were hints; the
1.14d shifts differ as noted (§2). Driver slots read from the wrappers
`0x004F68E0` (`+0x7C`), `0x004F6920` (`+0x9C`), `0x004F6950` (`+0xA0`) and
the driver tables `0x0072F6D0` / `0x0074C4A8`; unit visibility
`0x004DC710`, `0x004DC7B0`; shake arithmetic `0x00476D40`. §1–§3 confirmed
by the `frames-raw-1` capture runs (`capture.md` Test vectors). Roof
block y's, roof heights and wall block grid counted 2026-10-06 over the
DT1 files `mpq-tool extract` wrote from `d2data.mpq` / `d2exp.mpq`
(`patch_d2.mpq` holds no listed DT1; the 6 known-unused files of
`formats/dt1.md` excluded); roof height read `0x004DEBA6`.

## Open questions

1. Roof Y: 1.14d hands `sy − roof_height − cy_t` to the floor drawer
   (§6); `map-preview.md` places roofs at `sy + 80 − roof_height`. Which
   y range do live roof (orientation 15) blocks use? A game-file read of
   roof block y's plus a capture under a roof settles it.
   *Partly answered* (game-file reads, d2data + d2exp DT1s, the 6 unused
   ones excluded): every orientation-15 block has block y in 0 … 64 on
   the floor diamond rows, and block x on a 16-pixel grid. §6 concludes
   from it that 1.14d's `sy − roof_height` stands and `map-preview.md`
   is 80 rows low. Both reads agree on that. They disagree on the
   numbers, and those are *Pending*:
   - the block count (one read: 13,432 blocks; the other: 15,432 blocks
     of 715 tiles in 250 used files);
   - whether y is a multiple of 8 always or only almost always.

   To settle: run `mpq-tool extract d2data.mpq "data\global\tiles\*.dt1"`
   and the same for `d2exp.mpq`. Keep the files that a `LvlTypes` File
   column names (the 6 unused excluded; `formats/dt1.md` counts 251).
   Over their orientation-15 tiles, count tiles and blocks, and list
   every block y not in {0, 8, …, 64}. §6's count follows the result.
   Still open: the capture under a roof that decides between the two y
   formulas (the pixel proof: a roof in view, e.g. the Rogue
   Encampment, player under a tent edge, roofs not faded).
2. ~~Unit culling~~: answered in §7 (no view test; visibility test
   `0x004DC710`). *Answered* (static): `0x00642840` is the level's
   `LOSDraw` gate (`render/draw-order-2.md` §15 r1) and
   `0x00622AA0(player, unit, 2)` the unit collision line with mask 2
   (`render/draw-order-2.md` §15.1; also §7). Open: the line walk of
   `0x0064E260` (owner: a collision spec, to write; Ghidra read).
3. *Answered* in `unit-composite.md` §8: the three getters read the
   unit's client motion record (gfx +0x30, `0x0046F060`; 0 without one),
   and missiles add `missiles` xoffset / yoffset + zoffset (+0xA2/+0xA4/
   +0xA6). §4 now links there.
4. *Answered* (static, asm of `0x004DF510`, `0x004DEF80`, `0x006C9290`):
   §6 shadow-tile row, the wall position and wall block placement.
   Shadow pass kind 0 / 2 units are placed as units (§4,
   `render/blend-modes.md` §5).
5. The client update between server tick and draw (`0x0044C790`): confirm
   that unit path positions advance exactly once per tick there (Ghidra
   read), so a capture's state equals the server state after the same
   tick plus the client's own path step.
   *Answered* (static, 1.14d asm), in §9's step table. It is not
   exactly once for every unit: players step 0 or 1 times, monsters 0,
   1 or 2 (skill-flag step in `0x004AF4C0`, then the mode-class step),
   and a local player's missile steps twice on its first client update.
   The base `[0x007A04C4]` is always 0, so every step is 0x400. The
   dword is in the block `0x007A0480`–`0x007A04FF` that the memsets at
   `0x0044E200` / `0x0044C890` zero, and nothing writes it.
   `0x007A0468` is a lock object, not its record. The update itself
   runs once per server tick (`client/model.md` §5 r1).
   Still open (recording): whether a monster ever meets both step
   conditions in one update, and whether client updates and server
   ticks are 1:1 in single player. The `frames-raw-2` `client_update`
   counter against the server tick count over one run settles it
   (OQ8).
6. ~~Client player seed init~~: `sim/rng.md` §5.3 (one step of the
   client room seed at the player's creation position, `0x00465FD0`).
   At a single-player join it is {0x6AC6935F, 0} (`client/model.md`
   Randomness rule 2); later draws on it are `client/model.md` open
   question 6.
   Outside that case, not reproducible without recordings: the room seed
   has already been stepped once per client unit created in that room
   before the player (S→C message order at join), and the cursor (state
   1, wall clock) and the weather step the same seed each frame
   (`capture.md` §3.3), so shake offsets also depend on them. Captures
   keep recording `seed_start` / `seed_end`; a join trace of the S→C
   unit-add messages plus the first frame's `seed_start` would check
   the init rule.
7. Wall blocks: is every live wall block 32 pixels wide with an x on a 32
   grid of its tile (§7 clip equivalence)? Game-file count with the C52
   DT1 counts.
   *Answered* (game-file reads, 2026-10-06/07, used DT1s of d2data +
   d2exp; `Patch_D2.mpq` unlisted): yes. Every block is 32 pixels wide
   by format (`formats/dt1.md`). Every block of orientations ∉ {0, 13,
   15} has x ≡ 0 (mod 32), in both reads. The six unused DT1s are the
   only files with off-grid wall blocks. The two reads disagree on the
   numbers, which are *Pending*:
   - the file set and count: one read gives 104,780 blocks in 251 files
     with x only checked; the other gives 104,767 blocks in 250 files
     with x and y checked;
   - y ≡ 0 (mod 32) comes from the 250-file read only.

   To settle: use OQ1's extract and file set (the files a `LvlTypes`
   File column names). Over the blocks of orientations ∉ {0, 13, 15},
   count the files and blocks, and list every block with x or y ≢ 0
   (mod 32). §7's count follows the result.
8. Draws with no server tick between them (118 frames of run 1 while not
   paused, `capture.md` §4, OQ8) against §9's "passes without a tick do
   not draw"; the `frames-raw-2` client-update counter settles it.
