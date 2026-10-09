# Spec: Seam — World coordinates ↔ screen (draw and pick)

- **Status:** draft (2026-10-08, seam audit; contract written from the
  owner specs and both sides' code, no capture). Rules §2.1–§2.3 have
  passing contract tests (`crates/d2-client/tests/seam_world_screen.rs`);
  §2.4–§2.6 are broken in the play path (§Edge cases, findings F1–F3) and
  have no test yet.
- **Target version:** 1.14d
- **Crate/module:** `d2-client::rules::camera` (draw side),
  `d2-client::bridge::click::screen_to_world`, `bridge::hover`,
  `world_view::{feed, model_feed, corpse_click, object_label,
  ground_items, missiles, automap_view, near_rooms}`, `ui::overhead_ui`
- **Related specs:** `render/camera.md` (owner of every world → screen
  rule), `render/sprite-placement.md` (owner of what a draw at (X, Y)
  covers), `ui/controls.md` §6 r2 (owner of the click record and its
  world position, `0x0045AFF0`), `client/model.md` §3 r3 (owner of the
  unit positions the client holds), `client/bridge.md`

## Summary

This seam is every place a world position becomes a screen point (to
draw a tile, a unit, a label, a hover box) and every place a screen point
becomes a world position (a click, a hover pick). It owns no rule of its
own: it states which owner rule each side must call, in which units, and
which state both sides must read, so that one frame draws and picks with
one camera. A screen point and a world position are joined only through
`render/camera.md`; nothing on either side may re-derive the projection.

## Inputs

| Name | Type / units | Source (owner) |
|---|---|---|
| unit position, moving unit | 16.16 fixed subtile, unsigned | sim via bridge; the client model keeps a subtile (u16, u16) and reads it as the subtile centre (`client/model.md` §3 r3) |
| unit position, static unit (types 2, 4, 5) | integer subtile | `camera.md` §2 |
| local player position | 16.16 fixed subtile; in the `play` preview the walk prediction's (decision D2) | `camera.md` §3; `world_view::walk` |
| tile cell | absolute tile (tx, ty), 1 tile = 5 subtiles | DRLG; `camera.md` §2 |
| screen point (mouse) | integer frame pixel in `[0, W) × [0, H)` | UI input (`ui/controls.md` §6 r1) |
| open mode, shake | 0–3; `(dx, dy)` | `camera.md` §1, §8 |

## Outputs / state changes

None: the seam is a contract between two sides. Each side's outputs are
its owner's (draw positions: `camera.md` §10; click record:
`ui/controls.md` §6 r2).

## Rules

### 1. Units and spaces

| Space | Unit | Scale | Origin | Rounding into the next space | Owner |
|---|---|---|---|---|---|
| precise | 1/65536 subtile | — | world (0, 0) | `>> 11` logical → 1/32 subtile | `camera.md` §2 |
| subtile | 1 subtile = 32 × 16 client px diamond | `× 16`, `× 8` | world (0, 0) | moving: `(a − b) >> 1`, `(a + b) >> 2` (floor); static: exact | `camera.md` §2 |
| tile | 1 tile = 5 subtiles = 160 × 80 client px | `× 80`, `× 40` | world (0, 0) | exact | `camera.md` §2 |
| client px | 1 pixel | 1 | world (0, 0) | subtract the frame's origin | `camera.md` §3 |
| screen px | 1 pixel | 1 | frame top-left | — | `camera.md` §4, §6 |

Screen y grows down. One subtile step in +x moves the client point
(+16, +8) (down-right); one step in +y moves it (−16, +8) (down-left). A moving unit at the subtile
centre (`<< 16 | 0x8000`) and a static unit at the same subtile differ by
8 client rows (centre vs the diamond's top vertex).

### 2. Contract

#### 2.1 One projection

Every world → client conversion on either side calls `camera.md` §2
(`moving_to_client`, `static_to_client`, `cell_origin`) and every client
→ screen conversion calls the frame's `Camera` (`camera.md` §3–§6). The
pick inverts §2 and §4 four rows down: screen (X, Y) →
client `(X + cx_u − shiftX, Y + cy_u − 4)` → subtile
`(⌊(px + 2·py) / 32⌋, ⌊(2·py − px) / 32⌋)` (floor, not truncation, as §2
floors). Revision 2026-10-09 (q-scenes-compare, `ui/controls.md` §6 r2,
PROVISIONAL REC-514): measured on the `a1-walk-*` scenes; the plain
inverse (`Y + cy_u − 8`) sent a1-walk-n's click to a blocked subtile
one to the left of 1.14d's. So a unit drawn at subtile s picks back as s, moving or static,
in every open mode and with any shake (tests:
`moving_unit_drawn_feet_pick_back_to_its_subtile`,
`static_unit_draw_point_picks_back_to_its_subtile`).

#### 2.2 One camera per frame

A frame has exactly one `Camera`, built once by `camera.md` §3 from the
local player's position, the open mode and the shake. Every consumer of
that frame — the tile and unit draws, ground items, missiles, the hover
pick and its highlight, object labels, overhead text, corpse clicks, the
world click, the automap's unit origin, the near-room centre — reads that
camera, or builds one from the **same** local-player position, the same
open mode and the same shake. Without a walk prediction the pick camera
equals the draw camera (test
`without_prediction_the_pick_camera_is_the_draw_camera`); the local
player draws at `(W / 2 + shiftX, H / 2 − 8)` and picks its own subtile
(test `local_player_draws_mid_play_area_and_picks_its_own_subtile`).

#### 2.3 Pick resolution

The world click position is a subtile (`ui/controls.md` §6 r2,
C→S point codes `[x u16][y u16]`). Every pixel of one subtile diamond
(256 pixels, the 32 × 16 diamond whose top vertex is 4 rows above the
subtile's draw point, §2.1 revision) picks that subtile and no other (test
`every_pixel_of_a_subtile_diamond_picks_that_subtile`).

#### 2.4 The local player's position has one owner per frame

Which local-player position the frame uses (the model's subtile centre
on the strict path; the walk prediction's 16.16 position in `play`) is
decided once; the camera of §2.2 and every "where is the player" reader
(near-room centre `draw-order.md` §9, automap) take that one.

#### 2.5 A unit's pick anchor is its draw anchor

A hover box, a label or a click hit test placed "at a unit" starts from
the client point the unit is drawn from: static units (objects, items)
by `static_to_client`, moving units by `moving_to_client` of their
precise position (`camera.md` §2, §4). The box size is the pick owner's
(d2rs-own today, `bridge::hover`).

#### 2.6 Shake reaches every world draw and the pick

The shake offsets of `camera.md` §8 enter the frame's one camera, so
every world draw (tiles, units, ground items, missiles) moves together
and the pick inverts the shaken camera.

## Constants & data dependencies

Owned by `render/camera.md` (W × H, 40, 16, 8, 80, 40, 32 × 16). The seam
adds none.

## Randomness

None. The shake's draws are `camera.md` §8; §2.6 only requires the frame
to make them once and use the result everywhere.

## Edge cases & original bugs

1. **Tiles and units are anchored 12 rows apart** (`camera.md` §6, Edge
   case 1): the pick inverts the unit origin, so a click lands 12 client
   rows above the floor pixel under the cursor. Kept until
   `0x0045AFF0` is specified (Open question 1).
2. **d2rs findings (2026-10-08 seam audit), not original behavior:**
   - F1, breaks §2.2/§2.4: in `play` the frame camera is built from the
     walk prediction (`model_feed.rs` `position_of`), while the hover
     highlight, object labels, corpse clicks, overhead text and automap
     build theirs from the model subtile (`corpse_click::camera_for`,
     `overhead_ui::camera`, `automap_view`), and the near-room centre
     reads the model subtile. The world click (`ModelClick::camera`) uses
     the prediction, so the highlighted unit and the clicked unit can
     differ.
   - F2, breaks §2.5: `bridge::hover::feet` anchors every unit at its
     subtile centre; objects and items are drawn from the static vertex,
     8 rows higher.
   - F3, breaks §2.6 (latent: no shake starts today): ground items,
     missiles and every pick camera use shake (0, 0).

## Test vectors

| Input | Expected output | Source |
|---|---|---|
| camera at player client (1000, 2000), any mode, any shake; moving unit at subtile centre (sx, sy), sx, sy ∈ 90 … 109 | `screen_to_world(unit_draw(·))` = (sx, sy) | §2.1 |
| static unit (sx, sy) | same | §2.1 |
| player at subtile (5000, 4000), modes 0–3 | drawn at (400 + shiftX, 292), picks (5000, 4000) | §2.2 |
| static subtile (7, 3), the 32 × 16 diamond with its top vertex 4 rows above its draw point | 256 pixels, all pick (7, 3) | §2.3 |
| model subtile (100, 100), prediction (103, 100) centre | hover / label / corpse camera = frame camera (fails today, F1) | §2.2 |
| object at (7, 3), player at client (0, 0) | `feet` = (464, 372), the draw point (today (464, 380), F2) | §2.5 |

## Provenance

Authored from `render/camera.md`, `render/sprite-placement.md`,
`ui/controls.md` §6 and `client/model.md` §3, and from reading the d2rs
code on both sides (no `re/`, no `../refs/`). The 12-row anchor gap and
the floor rounding are `camera.md`'s RE results.

## Open questions

1. Answered (re-checked 2026-10-09 against `0x0045AFF0`):
   `render/camera.md` §4 "Screen → world": the unit origin, minus
   `shiftX`, with **no −8** on y, then `0x00643510` (floor shifts);
   perspective first when on. d2rs's `bridge::click::screen_to_world`
   subtracts 8 from y: q-fix-click-no-minus-8.
2. The hover model `0x00467A10` (the original hit-tests drawn sprites):
   when specified, §2.5 becomes "the pick reads the drawn cel's
   rectangle", and the d2rs-own box goes.
