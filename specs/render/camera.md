# Spec: Render — Camera (world position → screen draw position)

- **Status:** draft (2026-10-06, RE on 1.14d `Game.exe`; no capture yet).
  Every rule names its 1.14d address; unverified until the capture cases
  of §Test vectors run.
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
| Summary | 40–50 |
| Inputs | 51–61 |
| Outputs / state changes | 62–67 |
| Rules | 68–69 |
|   1. Frame size and play area | 70–91 |
|   2. World coordinates → client pixels | 92–116 |
|   3. Camera origins (once per drawn frame) | 117–132 |
|   4. Units | 133–151 |
|   5. Panel shift for floors | 152–157 |
|   6. Tiles | 158–178 |
|   7. View culling | 179–189 |
|   8. Screen shake | 190–211 |
|   9. Time base: no interpolation | 212–227 |
|   10. What d2rs hooks get | 228–236 |
| Constants & data dependencies | 237–243 |
| Randomness | 244–249 |
| Edge cases & original bugs | 250–259 |
| Test vectors | 260–276 |
| Provenance | 277–291 |
| Open questions | 292–313 |
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
`+0x148/+0x14C/+0x150`; missiles and the offsets of `0x004DA0B0`–
`0x004DA0F0`; Open question 3). Each component cel is then placed at
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
| floors | `0x004DE730` → `0x004DE410` → `DrawGroundTile 0x004F68E0` | `(sx − cx_t, sy − cy_t)` | `(sx − 80 + bx − cx_t + left, sy + by − cy_t)` |
| walls (wall-layer list) | `0x004DF1C0` → `DrawWallTile 0x004F6920` / translucent `0x004F6950` | `(sx − 80 − cx_t + left, sy + 80 − cy_t + top)` | `(sx − 80 + bx − cx_t + left, sy + 80 + by − cy_t)` |
| roofs (fading list) | `0x004DEA70` → `DrawGroundTile` | `(sx − cx_t, sy − roof_height − cy_t)` | `(sx − 80 + bx − cx_t + left, sy − roof_height + by − cy_t)` |

`roof_height` is the DT1 tile header field at `0x04` (read at `0x004DEBA6`). Which
orientations are in which list, the order, shadows and the fade alpha are
`draw-order.md` / `blend-modes.md`. The floor/wall alignment equals
`map-preview.md` (walls 80 below floors, `WALL_BASE`); the absolute x is
80 left of its `sx`. Roofs differ from `map-preview.md` (no `+WALL_BASE`):
Open question 1.

Tile vs unit: a unit at the exact top vertex of cell `(tx, ty)`
(`px = sx`, `py = sy`) is drawn 12 rows below the floor's top vertex:
units use `H / 2 − 8`, tiles `(H − 40) / 2` (§3, §4).

### 7. View culling

- Floors and roofs: drawn only if the (X, Y) handed to the drawer lies in
  the view clip rectangle `view +0x14..+0x20` = `[−80, W + 80) ×
  [−80, H − 47)` (`0x00476000` sets it; `0x004DE410`, `0x004DEA70` test
  it). Perspective mode only (not the software renderer) widens it.
- Wall blocks: skipped when the block's screen x is outside `[−32, W)` in
  modes 0/3, `[−32, W − W / 2)` in mode 1, `[W / 2 − 32, W)` in mode 2, or
  its y outside `[−32, H + 32)` (`0x005131D3`–`0x0051324B`).
- Units: Open question 2.

### 8. Screen shake

Started by `0x00476A80` (peak `A` in ECX, attack `t1` ms in EDX, sustain
`t2`, release `t3` on the stack; start time `GetTickCount`; ignored when
`t2 = 0`). Each drawn frame (`0x00476D40`), with `t = now − start` (ms,
wall clock):

| t | amplitude a |
|---|---|
| `t > t1 + t2 + t3` | 0; shake ends (`0x007B9534 = 0`, offsets 0) |
| `t < t1` | `A × t / t1` (unsigned, floor) |
| `t1 ≤ t < t1 + t2` | `A` |
| otherwise | `A × (t1 + t2 + t3 − t) / t3` |

If `a ≠ 0`: `dx = −a + rnd(2a)`, then `dy = −a + rnd(2a)` (two draws of the
seeded RNG helper `0x00472280` on the local player unit's seed,
`unit +0x20`; `rnd(n)` per `sim/rng.md`), stored in `0x007B9538` /
`0x007B8D20`, added to the tile origin and, through `0x00476AC0`, to the
unit origin. `0x004769D0(a)` also drives a rumble sound (audio specs).
Callers of `0x00476A80` (skills/missiles) and their parameters belong to
their effect specs.

### 9. Time base: no interpolation

The client loop (`0x0044EFA0`) runs the in-game draw (`0x0044C990`) at
most once per client tick of 40 ms (`0x0070EF1C = 40`): in single player
the draw follows the server tick and the client update of the same loop
pass, and later passes without a tick do not draw (`0x007A0704`); when the
loop falls behind, draws are skipped, never interpolated. While a single
player game is paused the draw runs every pass with no tick. Every
position above is the integer state at draw time; no sub-tick time enters
any formula except the shake envelope (§8, wall clock).

For d2rs: one frame per presented tick, positions from the snapshot of
that tick, no interpolation. The shake envelope uses `t = 40 × (ticks since
start)` (the original's frame spacing); pixel checks of shaking frames
take the recorded `(dx, dy)` as input (`capture.md` §4).

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

- Tiles and units are anchored 12 rows apart (§6), from the play-area
  height used for tiles and the full height used for units. Reproduce.
- The shake offset range `[−a, a − 1]` is not symmetric. Reproduce.
- The shake envelope runs on wall-clock milliseconds, so the original's
  shake is not a function of ticks (§9).
- Panel culling of wall blocks (§7) can leave wall pixels missing at the
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
| capture case `camera-0001`: walk 5 s in a cleared area, every frame captured with state (`capture.md`) | per frame, CPU reference from the recorded positions equals the capture; recorded origins equal §3 | capture, queued |

## Provenance

1.14d `Game.exe`: draw frame `0x0044C990`, client loop `0x0044EFA0`,
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
1.14d shifts differ as noted (§2). No capture yet.

## Open questions

1. Roof Y: 1.14d hands `sy − roof_height − cy_t` to the floor drawer
   (§6); `map-preview.md` places roofs at `sy + 80 − roof_height`. Which
   y range do live roof (orientation 15) blocks use? A game-file read of
   roof block y's plus a capture under a roof settles it.
2. Unit culling: which test skips units outside the view (the unit draw
   path `0x004DC7B0`/`0x00471EC0` beyond the cel pre-test)? A Ghidra read
   of `0x004DC710` and of `0x00471EC0` past `0x004720F6`.
3. The extra unit offsets of `0x004DA0B0`/`0x004DA0D0`/`0x004DA0F0`
   (record of `0x0046F060`, fields `+0x34/+0x38/+0x3C`) and the missile
   offsets (`0x0046ACE0` record `+0xA2/+0xA4/+0xA6`): what they are and
   when non-zero. Owner `unit-composite.md`; Ghidra read of `0x0046F060`.
4. Shadows (orientation 13 list, `0x004DF510`): their (X, Y). Owner
   `draw-order.md`; Ghidra read of `0x004DF510`/`0x004DEF80`.
5. The client update between server tick and draw (`0x0044C790`): confirm
   that unit path positions advance exactly once per tick there (Ghidra
   read), so a capture's state equals the server state after the same
   tick plus the client's own path step.
6. How the client's copy of the player unit seed (`unit +0x20`) is
   initialised, so d2rs can reproduce shake offsets without recordings.
