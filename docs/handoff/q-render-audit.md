# q-render-audit — rule-by-rule audit of the render chain against its specs

Session `q-render-audit`, 2026-10-08, branch `claude/q-render-audit`. Cloud, no
game files: a code-vs-spec audit (M23 lesson of 2026-10-08, HANDOFF §8). Six
auditors, one spec area each: projection / camera / placement, draw order,
composition / palettes / blend / shading, frames / COF / anchors, lighting /
overlay, UI placement / fonts. Their full reports are the area sections below.

## Verdict

The rule modules are right. `rules/camera.rs`, `rules/placement.rs`,
`rules/draw_order`, `rules/blend.rs`, `rules/shading.rs`, the CPU / GPU
compositors, the DC6 / DCC / DT1 frame placement, `rules/lighting` and the UI
layout match their specs and every test vector the auditors checked (signs,
`>>` floor rounding, the 12-row tile / unit offset, `T[256·d + L[s]]`, the 47
uncleared rows, bottom-left DC6 origin, row-major cell order). **The broken
screen comes from the seams around them**: live-path code that bypasses a rule
(own keys, own cameras, opaque draws), state the next frame needs and nothing
keeps, and the local player's position taken from two sources.

## Findings, most visible first

| # | Area | Status | Spec rule | Where | Screen |
|---|---|---|---|---|---|
| 1 | draw order | queue `q-fix-render-edge-floors` | draw-order-2.md §14 | `rules/draw_order/mod.rs:1065` | Any `DrawEdges` level with no panel open: `order_grid` errors, the frame fails, the view freezes on its last image. Levels 74 / 120 the same through §12 backgrounds (`mod.rs:1046`) |
| 2 | composition | queue `q-fix-render-missile-blend` | blend-modes.md §4 | `world_view/missiles.rs:550-551` | Every missile and state overlay opaque: ~456 of 684 missile rows and 284 of 293 overlay rows should be additive; fire / lightning / cast overlays dark solid blocks |
| 3 | draw order | queue `q-fix-render-missile-key` | draw-order.md §3 r4, §10 | `world_view/missiles.rs:615` | Missiles and overlays keyed `(6, MAJOR_MAX)`: drawn over every wall and unit in front of them |
| 4 | lighting | queue `q-fix-render-wall-light-direction` | lighting.md §11 r2, OQ8 | `world_view/preview_blocks.rs:73` | Wall light points looked up by DT1 orientation, not light direction: orientations 4–9 wrong table, 10–14 and lower walls 16–19 flat: wall gradients wrong on most walls |
| 5 | projection | queue `q-fix-render-tile-block-mask` | shading.md §4 r4, lighting.md §11 r3 | `rules/view.rs:236-271` | Per-block lit floor / roof blocks drawn as the whole tile clipped to a 32×15 rect; neighbours overlap 16×7 px and repaint each other: sawtooth light seams on lit floors |
| 6 | projection | queue `q-fix-render-one-camera` | camera.md §3, §4, §8 | `corpse_click.rs:159`, `present.rs:401`, `automap_view.rs:143`, `ui/overhead_ui.rs:78`; `ground_items.rs:265`, `missiles.rs:639` | Hover, object labels, chat bubbles and the automap rebuild the camera from the server cell while the frame uses the predicted position: offset and jitter while walking; items and missiles build an unshaken camera |
| 7 | projection | queue `q-fix-render-overlay-local-pos` | camera.md §2 | `missiles.rs:353, 566, 592` | Local player's cast / state overlays and missile origins at the server cell: they trail the walking player |
| 8 | draw order | queue `q-fix-render-unit-shadow-flag` | draw-order.md §3 r4, §5 r3 | `world_view/near_rooms.rs:211-221` | Flag-ex 0x80 not kept between frames: no unit shadow ever on the model path; the preview forces it on (shadows of sight-hidden units) |
| 9 | draw order | queue `q-fix-render-ground-item-key` | draw-order.md §3 r4, §5 r3, §10 | `world_view/ground_items.rs:237-243` | Ground items after every pass-5 entry, never sight-tested, dropping items flat: items over shadows / corpses, visible through walls |
| 10 | UI | queue `q-fix-render-ui-blend` | inventory.md §2 r2–r3, §8 r4; panels.md §12 r4 | `ui/panels/inv_items_tint.rs` | Inventory tints solid opaque blocks (spec 25 % blend); cube animation, ethereal items opaque; item colour remaps not applied |
| 11 | UI | queue `q-fix-render-automap-blend` | automap.md §10 r4 | `world_view/automap_view.rs` | Automap lines solid instead of 25 / 50 / 75 % fades |
| 12 | draw order | queue `q-fix-render-automap-pass` | draw-order.md OQ14, §1 | `automap_view.rs:205` | Automap keyed `(9, 0, …)` like rain / snow: drops interleave with map lines |
| 13 | lighting | queue `q-fix-render-light-kinds` | lighting.md §6.4 r5, §8, §13 | `preview_light.rs`, `light_sources.rs:86` | Only the player's light is shadowed: torches and monster lights shine through walls |
| 14 | lighting | queue `q-fix-render-env-per-update` | lighting.md §9.2 r1, §10 r5 | `preview_light.rs` | Day / night steps per drawn frame (~2.4× fast at 60 fps); scripted overrides (Den of Evil fade, 107/108 glow, darkness) never step |
| 15 | frames | queue `q-fix-render-unit-frame` | unit-composite.md §3 r2, §10 | `world_view/unit_rules.rs:52-58` | Frame 0 read as "unset": idle objects / units at frame 0 cycle their animation; `% F` wrap not in the spec |
| 16 | draw order | queue `q-fix-render-preview-sight` | draw-order-2.md §15 | `world_view/preview.rs:99` | Preview answers "not hidden" for every unit: monsters behind walls drawn in the 83 `LOSDraw` levels |
| 17 | frames | queue `q-fix-render-cof-cull` | unit-composite.md §4 | `world_view/unit_rules.rs` | COF box pre-test never on the draw path: edge slivers of off-screen units |
| 18 | lighting | queue `q-fix-render-wall-light-fade` | lighting.md §11 r2 | `preview_blocks.rs:73` | Fading walls read the normal light-point table |
| 19 | composition | queue `q-fix-render-act-load-clear` | composition.md §3 step 4 | `world_view/present.rs` | The all-black first frame after an act load is not presented |
| 20 | UI | queue `q-fix-render-belt-popup` | control-panel.md §5 r8, r14 | `ui/original.rs` `TopUi` | Belt tip is a d2rs-own box, not the r14 pop-up |
| 21 | projection | queue `q-fix-render-fade-player-tile` | draw-order.md §9 | `world_view/near_rooms.rs:252` | Wall / roof fade follows the server cell: lags the drawn player |
| — | composition | **fixed** `7cf56b3f` | shading.md §6 r1.1 | `world_view/state_tint.rs` | State tint chosen by colorpri only (no live-table pixel change) |
| — | lighting | **fixed** `fa6258ce` | lighting.md §6.1, §9.2 r2–r4, §4 r2 | `world_view/preview_light.rs` | Player light pool trailed the player by up to a sub-tile; 0x53 / 0x5D never reached the ambient; light leaked past the loaded area |
| — | draw order | **fixed** `032cdf04` | draw-order.md §6 r7 | `rules/draw_order/mod.rs` | Automap reveal by leveldefs layer (not on the live path yet) |
| — | UI | **fixed** `7839b5c0` | panels.md §5 step 10, panels-3.md §23 r9; control-panel.md §5 r14 | `ui/original.rs`, `ui/hud_tips.rs`, `ui/text.rs` | Held item and tips drawn under the HUD; control-panel tips half a width right and a row low |
| — | frames | **fixed** `0c71004c` | unit-composite.md §5.1 r5; sprite-placement.md §3 | `composite/mod.rs`, `world_view/unit_assets.rs` | A COF with a duplicate layer record failed the whole world frame; DCC frames > 256 (gargoyle trap) drawn whole |
| — | UI | **fixed** `9721991f` (test) | panels.md §1 r3, §12.4 | `tests/game_panels.rs` | `q-fix-panel-horadric-offsets` was a test bug: the code applies horadric offsets; the game-file test now asserts the spec's vectors |
| — | projection | **fixed** (this commit series) | camera.md §2 | `bridge/hover.rs`, `world_view/object_label.rs` | Hover box and object label 8 rows below drawn objects / items (moving rule used for static units) |

Lower / open (details in the area sections): roof alpha path (`q-fix-render-roof-opaque`,
unreachable today), monster direction counts (`q-fix-render-monster-dircount`),
direction write-back, persistent light list (radius walks), object label not
centred (d2rs-own, F7 of UI), the `inventory.md` §8 r4 one-row wording question,
`map-preview.md` roofs 80 rows low vs camera.md §6 (Phase 1b viewer only),
ghostly / fade draw modes not reached, unit extra offsets (camera §4) not wired.

## Boundary (for q-seam-audit)

- **The local player's position has two sources**: `bridge::predict` 16.16
  (camera, player draw, light, weather, clicks) and `ClientUnit::position`
  (server cell: hover, labels, automap, overhead text, overlays, fades,
  near-room adjacency). Findings 6, 7, 21 share this cause.
- Flag-ex 0x80 / 0x10000000 is client-unit state the draw writes and the next
  frame reads; the model never keeps it (finding 8).
- No act-load (S→C 0x03 handled) signal in the model for the post-draw clear
  (finding 19).
- The environment and lighting overrides need a per-client-update pass in the
  bridge (finding 14).
- The DT1 light direction is parsed in `d2-formats` and dropped by the sim
  `TileInfo` (finding 4).
- `ClientUnit.frame` reset to 0 by the bridge's object code collides with the
  view's "0 = unset" (finding 15).
- Missile / ground-item / automap layers build their own camera with zero shake.
- `d2-sim drlg/tiles.rs:113` comment says shadows and roofs share an array; the
  code files type 15 in the wall array (correct per draw-order.md OQ8): stale
  comment only.

## Checks

Each fix carries a unit test from its spec rule or test vectors; none weakened
an expectation (changed expectations: `duplicate_layer_records_are_an_error` →
`duplicate_layer_records_use_the_first_match` per unit-composite.md §5.1 r5;
three `original_tests` open-panel lists end with `TOP_PANEL`; the object-label
test's mouse point is the static feet; the horadric game-file assertion follows
panels.md §1 r3). All fixes are unverified against 1.14d (no capture): the
local run queue should re-run `play` on the install and the `game_panels` test.


---

## Area report: projection

The core projection is correct. `rules/camera.rs` and `rules/placement.rs` match every rule and test vector of camera.md §1–§8 and sprite-placement.md §2–§8. The live frame (`present.rs` → `feed::build_frame` → `OriginalView`) does call them. Tiles, units, ground items and missiles all go through the same `Camera` and the same §2 formulas. No ad-hoc `*16`, `*32`, `/5` or `>>1` projection was found on the draw path.

The visible problems are seams:
- the local player's position source: the predicted position vs the server cell;
- the static vs moving formula;
- the per-block tile clips.

### Findings (most visible first)

- **F1 [QUEUE]** shading.md §4 r4 and lighting.md §11 r3 (per-block light of iso floor and roof blocks), with camera.md §6 and sprite-placement §7.
  - Where: `rules/view.rs:236-271` (`OriginalView::tile_draws`).
  - Code does: when the source gives per-block shades (the lit preview always does for floors and roofs), each block becomes a `TileDraw` of the WHOLE assembled tile image, clipped to the block's bounding rectangle (32×15 for iso blocks). The draws share one DrawKey, so they are drawn in block order.
  - Spec says: iso floor blocks are diamonds. Each pixel `s_r ≤ x < s_r + w_r` of row r belongs to one block, and only that block's corners light it. Neighbouring block rectangles overlap by 16×7 px (blocks sit on a 16/8 grid). Inside the overlap, a later block's draw repaints the earlier neighbour's diamond pixels with its own light map and gradient.
  - Screen shows: a sawtooth light seam along every block diamond edge of lit floors and roofs. It is visible wherever neighbouring blocks have different light (torch radius edges, flat-vs-gradient neighbours). Walls are not affected: RLE 32×32 blocks on a 32 grid do not overlap.
  - Queue row: `q-fix-render-tile-block-mask`. Draw each block from its own block image (a frame per block, or the assembled image with a per-block diamond mask), not the whole tile image under a rectangle clip.
- **F2 [QUEUE]** camera.md §3/§4 (one camera per drawn frame, from the same player position). The drawn frame's camera uses the predicted local position when the preview is on (`model_feed.rs:180-189` `position_of`). These cameras instead rebuild from the server cell `(cell<<16)|0x8000`:
  - `world_view/corpse_click.rs:159-166` `camera_for`: used by `present.rs:716` for the hover pick, the object labels and the corpse clicks;
  - `world_view/present.rs:401-407` `automap_facts`;
  - `world_view/automap_view.rs:143-148`;
  - `ui/overhead_ui.rs:78-82` (chat bubbles).

  Screen shows: while walking, hover boxes, object name labels, overhead bubbles and the automap's unit origin are offset from the drawn world by the prediction lead, and jitter as the server cell catches up. `bridge/click.rs:118-125` already does it right: it uses `local_at` (or `WorldViewState.camera` = `frame.camera`).
  - Queue row: `q-fix-render-one-camera`. Every consumer reads the frame's camera (`state.camera` / `frame.camera`) instead of rebuilding it.
- **F3 [QUEUE]** camera.md §2 (local player position source) — `world_view/missiles.rs:566-570` (overlays with `follow`), `:592-599` (state overlays) and `:353` (cast origin).
  - Code does: overlays that follow a unit, state overlays (auras etc.) and cast origins take `world.units[k].position` → `cell_centre`.
  - Problem: the local player itself is drawn at the predicted 16.16 position (`model_feed.rs:183-186`).
  - Screen shows: the local player's cast and state overlays detach from the walking player and trail behind it, snapping to the server cell. Missiles start from the lagging cell.
  - Queue row: `q-fix-render-overlay-local-pos`. Take the local player's position from `feed.unit_position` (or `local_at`), as the unit draw does.
- **F4 [QUEUE]** camera.md §2 (static units: objects and items, types 2, 4, 5: `px = (sx−sy)·16`, `py = (sx+sy)·8`) — `bridge/hover.rs:27-33` `feet`.
  - Code does: `feet` uses `moving_to_client(cell<<16 | 0x8000)` for every unit, objects and items included. That is `py = 8(sx+sy) + 8`.
  - Problem: objects and items are drawn through `static_to_client` (`model_feed.rs:47-52`, `ground_items.rs:223`).
  - Screen shows: the hover box and the object name label (`object_label.rs:62`) are 8 px lower than the drawn object or item. Same in `corpse_click.rs:71`: corpses are monsters, so that one is correct.
  - Queue row: `q-fix-render-hover-static-feet`. Use `unit_position(unit).client()`.
- **F5 [QUEUE]** draw-order.md §9 (`[0x007C8A08]/[0x007C8A10]` = the player's path sub-tile / 5) — `world_view/near_rooms.rs:252-257`.
  - Code does: `player_tile` and `player_logical` come from the server cell (`world.local().cell()`).
  - Problem: in the preview the camera and the player draw use the predicted path position.
  - Screen shows: the wall and roof fade around the player lags the drawn player by the prediction lead while walking.
  - Not fixed: `MapState::near_rooms` has no access to `local_at`. The fix needs a parameter from `model_feed.rs`, which is not my file.
  - Queue row: `q-fix-render-fade-player-tile`.
- **F6 [QUEUE, latent]** camera.md §3/§8 (the shake goes into both origins of the one frame camera). `ground_items.rs:265-268` and `missiles.rs:639-642` each build their own `Camera::new(..., (0,0))` without shake, not `frame.camera`. No shake is started today (`ModelFeed::shake` → `NoFeed`), so nothing shows yet. With a shake, ground items and missiles would not shake with the world.
  - Queue row: folded into `q-fix-render-one-camera`.
- **F7 [OPEN, preview-only]** map-preview.md §Screen position, roofs at `sy + y0 + WALL_BASE − roof_height` (`map/layout.rs:101-105`). The code matches map-preview.md. camera.md §6 states that this is 80 rows lower than 1.14d. Only the Phase 1b DS1 viewer (`map/`) uses it; the live play path uses camera.md §6 (correct).
  - Spec-owner item: map-preview.md should defer roofs to camera.md §6.

### Rules checked and agreeing
- **camera.md:**
  - §1: view rect for modes 0–3, `shiftX`, play height H−40.
  - §2: moving `(a−b)>>1`, `(a+b)>>2` with logical `>>11` and arithmetic floor; `static ×16/×8`; cell `×80/×40`; `tile_entry` (sx−80, sy+80).
  - §3: tile origin `P − W/2`, `P − (H−40)/2`; unit origin `P − W/2`, `P − H/2 + 16`; shake added to both.
  - §4: `X = px+ox−cx_u+shiftX`, `Y = py+oy−cy_u+8`, giving the player at (400, 292).
  - §5: floor/roof drawer −80 + left.
  - §6: floor, wall, shadow-tile (Wall list) and roof handed positions; roof_height unsigned u16. Tiles vs units 12 rows apart.
  - §7: floor/roof clip `[−80, W+80) × [−80, H−47)`; wall/shadow per-block cull, mode-dependent; units not view-culled.
  - §8: unsigned envelope, `dx` then `dy` draws.
  - §9: `t = 40·ticks`.
  - §10: unit clip = the frame.
  - Every §Test vector is in `rules/tests.rs`.
- **sprite-placement.md:**
  - §2/§8: DC6 bottom row `Y + yoff`, DrawItem `y + yoff − h + 1`.
  - §3: DCC box `y_min = y_offset − h + 1` (d2-formats `dcc.rs:306-311`); `IndexFrame (x_min, y_min)` Top anchor.
  - §4/§5: top-down clip as bottom-up.
  - §7: block pixel at `origin + b + p`; DT1 image at `(X + x0, Y + y0)`.
- **map-preview.md:** §Cells, §Screen position (floors and walls; WALL_BASE 80), §What is drawn, §Draw order, §Tile images (assemble, index 0 skipped: exact for live data per sprite-placement §6), §Sprites.
- **Tile cells:** absolute `room.tiles + rec` (`draw_order/mod.rs:492`); record x/y are room-relative (`d2-sim drlg/tiles.rs:121`).

### Not implemented / not reached from the live path
- Unit extra offsets (camera §4, unit-composite §8): the preview passes `(0, 0)` plus skill-motion offsets (`preview.rs:116`, `model_feed.rs:200-210`). `rules::unit_composite::unit_offset` exists but is not wired. Objects' `objects.txt` Xoffset/Yoffset and missile x/y/zoffset are not applied to model units. The `missiles.rs` effects layer applies its own `art.offset`.
- Screen shake: never started (no `ViewFeed::shake` source).
- The strict path draws no map (`ModelFeed::map` off). Only the preview reaches tiles.

### Boundary (for q-seam-audit)
- The local player's position has two sources: `bridge::predict` 16.16 (camera, player draw, light, weather, clicks) and `ClientUnit::position` (server cell: hover, labels, automap, overhead, overlays, fades, near-room adjacency via `local_room`). F2, F3 and F5 all come from this. The near-room set itself follows the server's room, not the predicted position.
- Moving units are at the cell centre `(c<<16)|0x8000` (`model_feed.rs:43`). This puts them 8 px lower on screen than a static unit on the same sub-tile. That is correct per camera §2 and path-placement §1 r2, but every hand-rolled "feet" must use the same per-type rule (F4).

### Files changed
None. The fixes for F1–F6 are in files outside my edit set (`rules/view.rs`, `world_view/{corpse_click,present,automap_view,missiles,ground_items,model_feed}.rs`, `bridge/hover.rs`, `ui/overhead_ui.rs`). `rules/camera.rs`, `rules/placement.rs`, `map/`, `tile_assets.rs` and `preview*.rs` needed no change. Test build: `cargo test -p d2-client --lib rules::tests` does not compile right now because of another auditor's in-progress edit: E0061, `frame(...)` now takes 4 arguments, in a test around line 1329. This is not mine; my area has no changes to verify.

---

## Area report: draw-order

### Findings (most visible first)

- F1 [QUEUE] draw-order.md §3 r4 / §5 r3 (flag-ex 0x80 persists between frames) — world_view/near_rooms.rs:211-221 + unit_facts.rs:176 — the order writes flag-ex 0x80 into `NearRooms` unit facts (rules/draw_order/mod.rs:697-700), but `MapState::rebuild` rebuilds the facts from the model every bridge frame and nothing writes 0x80 back (no writer of 0x80 on `ClientUnit::flag_ex` anywhere). Spec: 0x80 set by the sight test of frame N gates the kind-2 shadow entry of frame N+1. Screen: in the non-preview path (`model_facts`) **no unit shadow is ever drawn** (players, monsters, objects). In the preview the fill forces 0x80 for types 0–2 (preview.rs:100), so shadows draw without the one-frame lag and even for units hidden by sight. Queue: `q-fix-render-unit-shadow-flag` — persist flag-ex 0x80 (and 0x10000000) per unit across rebuilds, like the record `draw` state, or write it to the bridge unit (draw-order.md §5 r3, §3 r4).
- F2 [QUEUE] draw-order.md §3 r4 (missiles are room units filed in their cell's unit list, pass 6 major = cell index) — world_view/missiles.rs:615 — client missiles and state overlays are keyed `(6, MAJOR_MAX, n)`: after every wall and unit of the frame. Screen: missiles and state overlays always paint over walls and units in front of them (e.g. a bolt behind a palisade shows over it). Module doc labels it d2rs-own. Queue: `q-fix-render-missile-key` — key each missile by its cell index and its unit-list position (`OrderKey` of §10), overlays inline with their host (`unit-composite.md` §5 r1).
- F3 [QUEUE] draw-order.md §3 r4, §5 r3, §10 (ground items: flat units, shadow list of their cell, sight-tested) — world_view/ground_items.rs:237-243 — keyed `(5, MAJOR_MAX, n)` sorted by `(sx+sy, guid)`, drawn after every pass-5 entry and never sight-tested; also drawn in mode 5 (dropping), which is not flat (§3: items flat in mode 3 only → pass 6). Screen: items paint over unit shadows, shadow tiles and corpses of later cells; items behind walls in `LOSDraw` levels stay visible; a dropping item draws under walls/units. Queue: `q-fix-render-ground-item-key` — take the item's slot from the order (`OrderedSource::unit_slot`; NotDrawn = hidden).
- F4 [QUEUE] draw-order.md OQ14 / §1 (every UI pixel after pass 10) — world_view/automap_view.rs:205 — automap items keyed `(9, 0, minor)`, the same pass and major as the rain/snow/flash items of pass 9 (weather_view.rs:485 `sky_key`). Equal-pass items sort by minor, so automap cels and rain particles **interleave** item by item. Screen: with rain or snow and the automap open, drops appear partly over and partly under the map lines. (The pass-9 placement is labelled d2rs-own; the collision is not.) Queue: `q-fix-render-automap-pass` — key the automap in the UI pass (or at least a major distinct from 0).
- F5 [OPEN] draw-order-2.md §14 — rules/draw_order/mod.rs:1065 — any level with `DrawEdges` at open mode 0 makes `order_grid` return an Open error, so `build_frame` fails and the preview keeps the old image (present.rs:794). Spec's gate also needs resolution mode 2 (always true at d2rs 800×600). Screen: frozen view in every DrawEdges level with no panel open. Needs the act edge record wired (draw-order-2.md OQ2 answered: acts I–III have one) and `edges::edge_floors` called after the last room with `edges::edge_key`. Queue: `q-fix-render-edge-floors`.
- F6 [OPEN] draw-order-2.md §12 — mod.rs:1046 — levels 74 and 120 return an Open error (backgrounds not wired: recorded time seeds) → frame fails in Arcane Sanctuary / Arreat Summit. Known gap.
- F7 [QUEUE] draw-order-2.md §15 (sight test) — world_view/preview.rs:99 — the preview's unit facts answer `sight_hidden: Some(false)` even when `unit_tables` are loaded and `unit_facts::sight_gate` could run. Screen (preview): in the 83 `LOSDraw` levels monsters/missiles/items behind walls are drawn. Queue: `q-fix-render-preview-sight` — use `sight_gate` when tables exist.
- F8 [FIXED] draw-order.md §6 r7 (reveal walks rooms whose level has the same leveldefs `Layer`, not only the player's level) — rules/draw_order/mod.rs `AutomapReveal::frame` compared `room.level == level`. Now takes a `layer` map and compares layers. Not reached from the live path (the automap session has its own reveal), so no screen effect today. Test: `automap_reveal_takes_rooms_of_every_level_on_the_player_layer`; existing `automap_reveal_distance_and_records` kept its assertions (levels 1, 2 given distinct layers).
- F9 [OPEN, minor] blend-modes.md §5 / draw-order.md §6 r3 — rules/view.rs:357 — a unit's kind-2 shadow is emitted only inside the unit loop of `build`, after `unit_pose`; a unit that is NotDrawn this frame (sight-hidden this frame, or skipped by §5 r1) but carries a kind-2 entry loses its shadow, while the original draws the shadow entry independently (subject only to `0x00471620`'s own skips). Edge: one frame at a sight change, or invisible/attached units. Owner: the unit-shadow wiring.

### Rules checked and agreeing
§1 pass numbers and gating (2 lower walls on grid flag, 3 floors always, 5 on shadow flag, 6 always, 7 on roof flag; mode 3 draws no world); §2 `a`, `n`, `q`, `T`, origin, cell index row-major, pool 3,000 with count rising, dropped entry sets no grid flag, cell flag 4 before the pool test; §3 r1 room corners/rect/slack test, r2 record test with `rh`/`h`, cell `T(e0,e1)−(0,1)`, cell flag reset, routing by record type (no layer bits → shadow list; 15 roof; 16–19 lower; else wall with fade target); r3 shadow array; r4 unit cell, flat test, kind-2 entry gated by 0x80, in-place stable Y sort by client y (`unit-order.md` §5 r7) for rooms passing the test; §3 flat units (incl. DrawUnder `&2` / `&1 && mode 2`); §4 layer-sorted insertion (all 3 vectors); §5 r1 skips, r3 tested types; §6 r1–r5 (roof masks: ℓ3 in passes 1–3), r6 0x20000 sites, floors by room/ℓ1,ℓ2/record index, orientation-0 floors only; §7; §8 group/geometric near, retarget, advance (unsigned compare, wrapping product); §10 key table (pass/major/minor for every pass incl. roofs `(L−1)n²+cell`, units `wall count + pos`); render-pipeline §A6 key layout `4|28|24|8` and stable sort (`sort_by_key`), COF `sub` = draw-order position (composite slot_order); unit keys from the order via `OriginalView::unit_params`, NotDrawn units hidden; tile keys copied in `resolve_drawn`; pass 4 (splash major 0, bubble 1, slot order) and pass 9 keys; water floors in draw order; draw-order-2.md §15/§15.1/§16 (size cap, end pulling, error-term walk, both line vectors); model.md §13 r1, r5 (unit_visibility.rs), r2 COF box.

### Not implemented / not reached from the live path
- Pass 1 backgrounds (F6), edge floors (F5), pass 10 screen fade (no input), pass 8 (never runs; correct).
- `AutomapReveal` / `reveal_room` (rules) unused by the live automap.
- Sight test bypassed in the preview (F7); in the model path it runs (`sight_gate`).
- Flag-ex 0x80 / 0x10000000 write-back (F1).

### Boundary (for q-seam-audit)
- Unit flag-ex 0x80 is a client-unit field the draw writes and the next frame reads; the bridge model never receives it (F1).
- Missiles / ground items / automap layers build their own `Camera` with shake (0, 0) (missiles.rs:640, ground_items.rs:268), while the world frame uses the feed's shake: during a screen shake these layers slide against the map (camera owner).
- d2-sim `drlg/tiles.rs:113` doc says "Shadows and roofs share this array"; the code files type 15 in the wall array (correct per draw-order.md OQ8) — stale comment only.

### Files changed
- crates/d2-client/src/rules/draw_order/mod.rs (`AutomapReveal::frame`: layer filter)
- crates/d2-client/src/rules/draw_order/tests.rs (callers updated; new test)

---

## Area report: composition

Specs: render/composition.md, render/blend-modes.md, render/shading.md, client/render-pipeline.md §A4/A5/A8/A9, formats/palette.md.

### Findings (most visible first)

- F1 [QUEUE] blend-modes.md §4 (missile row: `missiles.Trans` 1 → mode 3 ADD, 2 → mode 4 MUL; overlay row: mode = `overlay.Trans`): world_view/missiles.rs:550-551 (missiles and state overlays) sets `ShadeChain::EMPTY` + `BlendOp::Opaque` ("the `Trans` columns are not applied"). `rules::blend::missile_mode` / `overlay_mode` / `cel_ops` exist but nothing calls them. On screen: 456 of 684 missile rows and 284 of 293 overlay rows should be additive. Drawn opaque, fire, lightning, cold and cast overlays look dark and solid, with their dim pixels painted over the scene where they should brighten it. Queue row: `q-fix-render-missile-blend`: build missile and overlay draws with `cel_ops(tables, missile_mode(trans, hovered) / overlay_mode(trans), remap, v)` (overlays: no remap, Edge case 5).
- F2 [FIXED] shading.md §6 r1.1 (state colour choice, `0x004D97F0`): world_view/state_tint.rs `palette_index`. The code only considered states with `colorshift` ≠ 0, and a `colorpri` of 0 could win. The spec says a state wins only when its `colorpri` is greater than the best so far (start 0), `colorshift` plays no part, and a best state with id 0 counts as none. The spec's vector "state 5 pri 0 shift 9; state 7 pri 3 shift 0 → p = 0" gave 9 before the fix. On screen: wrong tint only for those synthetic combinations; with live `states.txt`, all seven `colorpri` rows have a shift. Test: `world_view::state_tint::tests::the_choice_is_by_colorpri_only` (spec vectors poison+cold → 108, freeze+cold → 108, the synthetic one → 0). Module doc updated: the rule is no longer PROVISIONAL REC-245, since the spec now states it.
- F3 [QUEUE] composition.md §3 step 4 (`[0x0070F2C0]` post-draw clear: the first in-game frame after each S→C 0x03 act load presents all index 0). `FrameCycle::set_post_clear` exists and both compositors apply `clear_after`. The live path never calls it: it is called only in tests, and the model exposes no act-load event, only `palette_act`. On screen: one frame that should be black after each act load shows the drawn scene. Queue row: `q-fix-render-act-load-clear`: the bridge/model reports each 0x03 handled; present.rs then calls `state.cycle.set_post_clear(1)` before the next frame's plan.
- F4 [OPEN, not visible] blend-modes.md §6 "Roofs are never translucent" r1–r2: world_view/preview.rs `tile_ops` (about lines 132-140) sends `TileKind::Roof` with alpha < 0xFF to `IndexTableSrcRow`. The spec says roofs are drawn by the opaque floor drawer, whatever the alpha. This branch is unreachable today, because only wall records get a fade target (rules/draw_order `retarget` / `walk_wall`) and roof alpha stays 0xFF. Also `GradientKind::Wall`'s doc (scene/item.rs) says "Walls and roofs"; the spec gives roofs the floor gradient (§4). That is a doc-only error. Queue row (low): `q-fix-render-roof-opaque`: in `tile_ops`, treat Roof as `WallDraw::Lit` with floor light.
- F5 [QUEUE, lighting owner] blend-modes.md §6 / shading.md §4 r4: a translucent wall's `L` is always the per-pixel gradient map, with no flat or unlit branch. The live path (preview.rs `tile_art`) gives translucent walls `preview_light.tile_chain` (one flat byte) or the per-block shades of preview_blocks. `rules::blend::wall_block_ops` implements the spec, but the tile path does not call it. A fading wall would be lit flat instead of with the gradient. This is lighting-audit territory (files outside my area). Noted only.

### Rules checked and agreeing

- composition §2: pixel (x, y) = byte y·W + x, top row first (scene/frame.rs, cpu.rs).
- composition §3 step 2: `FrameCycle::plan` clears rows 0 … H−48 (553 rows of 600) and keeps the bottom 47 (`UNCLEARED_ROWS`). BlankScreen = 0 means no clear, and no player room means BlankScreen 0 (model_feed.rs `blank_screen`). The framebuffer persists between frames in both the CPU and GPU live paths (present.rs commits the GPU indices before the next pack). CPU `compose_frame`, `compose_binned_frame`, WGSL `compose` and `pack::emulate` all start from the base with `y < clear_rows → 0` and apply `clear_after` last.
- composition §4: `present_palette` reads the PL2 first 1,024 bytes as R, G, B with index 0 included. `Pl2::parse` uses R,G,B,x and `.dat` is B,G,R. `to_rgba` and the WGSL `to_rgba` are plain palette lookups with no transparency for index 0. The act palette path is `act<a+1>`, with n outside 1…5 → 1 (app/palette.rs). The palette and the shade tables both follow `world.palette_act` (palette.rs, preview.rs `prepare` → `ensure_shades`, ui.rs text colours).
- composition §5 / shading §10: `PixelTables::ops` gives no T → `L[P[s]]`; T without L → `T[256·d + P[s]]`; T with L → `T[256·d + L[s]]` (P dropped). Index 0 is tested before the chain, and a mapped 0 is written as opaque index 0 (shading §7).
- blend-modes §1: `mode_table` gives 0→A2 (0x23500), 1→A1 (0x13500), 2→A0 (0x3500), 3→ADD (0x33500), 4→MUL (0x43500), 6→MAX (0x5B500), and 5/7/other → none. `ShadeTables::push` pushes the Pl2 tables in file order, unchanged and untransposed. The Pl2 parser's row i is file bytes 256·i…, so for cels row = destination.
- blend-modes §2 orientation: CPU `BlendOp::apply`, WGSL and `emulate` index `IndexTable` as `map[base + d][s]` and `IndexTableSrcRow` as `map[base + s][d]`. Cels, unit shadows and shadow tiles use `IndexTable`; translucent walls use `IndexTableSrcRow` (wall_block_ops, tile_ops). GDI rectangle k=1 is `T[d]` = row 0, column d, via chain [Z] + SrcRow.
- blend-modes §3 decision (`component_mode`, `unit_override`), §5 r1 (chain [Z] + A0 when blended, else 0), r2 shear (`shadow_image`, C truncation of yoff/2; vector y0 = 47 holds), r3/r4 positions, §6 alpha thresholds (`wall_draw`), §8 r1 line (the vectors hold) and r2 rectangle.
- shading §3 (v >> 3, 0xFF unlit, mode 7 → H; `hover_light` clamp 0x40…0xFF), §4 G table (floor via arithmetic >> 5), wall r1–r4 and floor r1–r2 (including the c2 = e[g+17] bug), §5 H (⌊c·170/100⌋, nearest with the lowest j on ties, index 0 included), §6 r1 (p−1, p ≤ 128), r4 file selection, r6 shift index and map choice.
- render-pipeline §A4/A5/A8/A9: CPU and GPU agree (same gradient rows, same table orientation, same clear handling). The palette words are r,g,b,255, copied as bits.
- Unit draws in the live path go through `LitRules` → `component_ops` → `cel_ops` (rules/lighting/view.rs), so mode, H and table orientation are as specified.

### Not implemented / not reached from the live path

- §3 step 4 post-draw clear (F3).
- Missile, overlay and item hover draw modes (F1). Items are drawn mode 5, unshaded and never highlighted (ground_items.rs, d2rs-own D1).
- blend-modes §3 inputs `g` (ghostly) and `r` (fade/transparent/ethereal): preview_light.rs `PreviewLook::look` always passes `ghostly: false, override_input: None` (d2rs-own). Ghostly monsters and Fade / ethereal units are drawn opaque. The model does not hold the inputs.
- Blended Shadows setting: hard-wired on (`unit_shadow_ops(tables, true)`, `tile_ops`), which matches default 1. There is no setting.
- composition §7 DirectDraw: not the reference, not implemented (documented).
- render/palette.wgsl (`PaletteMaterial`, discards index 0) is the map-preview path in app.rs only. It is not used by play.

### Boundary (for q-seam-audit)

- The model has no act-load (S→C 0x03 handled) signal that the frame cycle could turn into `set_post_clear(1)` (composition §3 step 4). `palette_act` alone cannot tell a reload of the same act from no reload.

### Files changed

- crates/d2-client/src/world_view/state_tint.rs: `palette_index` follows shading.md §6 r1.1; module doc updated; new test `the_choice_is_by_colorpri_only`. The existing tests are unchanged and still valid.

---

## Area report: frames

### Findings (most visible first)

- F1 [FIXED] unit-composite §5.1 r5, §10 ("duplicate layer records: first match; not an error") —
  crates/d2-client/src/composite/mod.rs `check()` (was ~l.99) — code returned
  `CompositeError::DuplicateLayer` for a COF with two layer records of one component; that error
  propagates through `composite::build_with` → `world_view::build` (`?`) and fails the WHOLE world
  frame, not just the unit — screen: any live COF with a duplicate record (the parser accepts them)
  blanks the frame / errors the view. Fix: duplicate check removed (component < 16 check kept);
  `slot_order` already takes the first match. Existing test `duplicate_layer_records_are_an_error`
  contradicted the spec; changed to `duplicate_layer_records_use_the_first_match` (expectation
  now = spec text: slots [TR, TR], both layer 0).
- F2 [FIXED] sprite-placement §3 (PROVISIONAL: "d2rs draws nothing for a DCC frame with w > 256 or
  h > 256"), dcc.md §Frame size limit — crates/d2-client/src/world_view/unit_assets.rs `store_file`
  — component DCC frames were stored whole, so the gargoyle trap (`GTTRLITA1HTH`, `GTTRLITNUHTH`,
  345 × 324) drew a full-size image in the Jail / Catacombs. Fix: `sprite_cache_limit` turns such
  frames into empty frames (index kept, nothing drawn, shadow set derived from it is empty too).
  Test `world_view::unit_assets::tests::sprite_cache_drops_frames_over_256`.
- F3 [QUEUE] unit-composite §3 r2 / §10 (frame = unit +0x44 >> 8, used as is) —
  crates/d2-client/src/world_view/unit_rules.rs:52-58 — live path uses the model frame only when
  `unit.frame > 0`; a unit whose +0x44 is exactly 0 (every mode start; objects reset it to 0 in
  `bridge/objects/mod.rs:311,318`) is animated by `server_ticks × COF rate` instead, and every
  frame is taken `% F` (spec: no bound, frame ≥ F reads the next row). Screen: idle objects/units at
  frame 0 cycle through their animation instead of holding frame 0. (Marked d2rs-own D1 fill; not
  my file.) Row: `q-fix-render-unit-frame`: use `frame_index(unit.frame)` whenever the model holds
  the unit's frame (distinguish "unset" explicitly instead of `> 0`), drop the `% F` (§3 r6/§6 r3
  already define past-end reads).
- F4 [QUEUE] unit-composite §4 (COF box pre-test) — world_view/unit_rules.rs `unit_pose` (module doc
  says "no COF box pre-test"); `rules::unit_composite::unit_pose`/`cof_box_visible` are never called
  from the draw path (only from the model visibility predicate). Screen: a unit whose COF box is off
  screen but whose component cels reach into the frame is drawn where 1.14d draws nothing (edge
  slivers at the screen border). Row: `q-fix-render-cof-cull`: apply `cof_box_visible` at the final
  (X, Y) of camera §4 before building the composite (needs the place position in `unit_pose`).
- F5 [OPEN] unit-composite §3 r3 — world_view/unit_assets.rs `UnitArt::expected_directions`
  returns the COF `D` for monsters (monstats2 `d<mode>` not loaded), so the (8,4)/(16,8) snap never
  happens for monsters. Visible only for monsters whose `d<mode>` differs from the COF direction
  count (smoother turning than 1.14d). Marked d2rs-own, unverified. Row:
  `q-fix-render-monster-dircount`: load monstats2 `d<mode>` into `UnitLooks`, pass it as
  `DirectionSource::Monster`.
- F6 [OPEN] unit-composite §3 r5 (snap write-back, §10 "applied once per drawn frame") — 
  `UnitDirection::write_back` is computed but never applied anywhere in the live path. Not
  visible in the draw (the draw already uses the snapped dir64); affects later client logic on dead
  units. Row: `q-fix-render-dir-writeback`.

### Rules checked and agreeing
- unit-composite §3 r4 `cof_direction` formula + snap tables (8,4)/(16,8); dir64 ≥ 64 → 0, n = 1.
- §6 r3 `file_direction` = P_Df[cof_dir] (interleave P_1, P_2, P_4, P_8, P_2D recursion), cel
  `Ff × dcc_dir + frame`, frame = Ff allowed (next direction's first cel), past end refused;
  component files use the snapped dir64 (unit_rules → `unit_direction(...).dir64`). TSV regenerated
  row by row by `directions_tsv_matches_code_row_by_row`.
- §3 r6 draw-order row offset `28 + 9L + F + (dir·F + frame)·L` incl. the padded `f9` COFs
  (`game_row` reads padding then order); frame ≥ F reads the next direction's rows.
- §5 r1 S7 has no own graphic (left out of `slot_order`, `inline_slot`); missing layer record →
  slot draws nothing (not an error); slot index = draw-key `sub` (back to front).
- §5.1 codes: empty mode token → `xxx `; armor classes lit/med/hvy, DT/DD → lit; act II tables;
  §6 r1 name/path; §6 r2 DC6 lists and `OYTRlitTNhth` case-insensitive; §6 r4 missing file = empty
  slot.
- sprite-placement §2/§3/§4/§8 via `rules::placement::draw_position`: DC6 flip 0 → Bottom anchor,
  top = Y + offset_y − h + 1; DC6 flip 1 → TopDown, top = Y + offset_y; DCC → `(x_min, y_min)` with
  `y_min = y_offset − h + 1` (d2-formats `dcc.rs` box, bottom_up 0), Top anchor; DT1 tile image
  `(x0, y0)` Top anchor. Live unit `place` (rules/view.rs) and missiles/ground items call
  `placement::place`, no ad-hoc formula.
- Decoder row order: DC6 flip 0 first encoded row = bottom (`height − 1 − row`), flip 1 top-down;
  DCC frames top row first; both `IndexFrame` top row first (render-pipeline §A2).
- DC6 version ≠ 6 / flags bit 2 refused; flip ∉ {0,1} refused; DCC odd `variable0` refused
  (sprite-placement Edge 4, 5).
- DT1 tile image (map-preview §Tile images): union box of blocks, block order, 0 does not
  overwrite; iso 32×15 / RLE 32×32 decode is d2-formats'.
- Atlas (render-pipeline §A2): deterministic shelf packer, 1-px gutter, empty frames → EMPTY slot,
  BTreeMap store; missile art uses `file_direction` for the missile's dir64 (§9, no snap).

### Not implemented / not reached from the live path
- §4 COF box pre-test (F4); §3 r5 write-back (F6); monster `d<mode>` direction count (F5).
- Live preview fills (unit_rules/unit_assets, D1): players have no items (all `lit`, `hth`),
  monster component choices 0 / counts 0 (every component `lit`), shade none, blend opaque, draw
  key pass 6 / 0 / 0 unless OriginalView sets it. These are the documented D1 fills, not
  bugs, but they mean players always look naked / bare-handed and monsters all-`lit`.
- §7 colormap source and §8 motion offsets are implemented as rules but not checked against the
  live path here (shading / camera areas).

### Boundary (for q-seam-audit)
- `ClientUnit.frame` (+0x44, 8.8): objects reset it to 0 (`bridge/objects/mod.rs:311,318`); the
  view treats 0 as "unset" (F3). Needs a clear "model has a frame" signal from the bridge.
- `dir64` comes from `UnitArt::dir64` (predicted / observed facing), not a client path record
  (§3 r1); provisional until REC-51.

### Files changed
- crates/d2-client/src/composite/mod.rs (F1: duplicate-layer error removed, enum variant removed)
- crates/d2-client/src/composite/tests.rs (F1: test changed to the spec's first-match rule)
- crates/d2-client/src/world_view/unit_assets.rs (F2: `SPRITE_CACHE_SIDE`, `sprite_cache_limit`,
  applied to component DCCs; test module added)

---

## Area report: lighting

The live play path lights the world through `world_view/preview_light.rs`
(`PreviewLight::refresh`, called from `model_feed.rs:423` once per drawn
frame), tiles through `preview_blocks.rs` / `preview.rs:296-313`, units
through `LitRules` (`feed.rs:478`). `LightMap::build` / `LightList::frame`
(the §2 / §6.4 path) is never called from the live path; preview_light
builds a throw-away `LightList` per frame.

### Findings (most visible first)

- F1 [QUEUE] §11 r2 + OQ8 — `world_view/preview_blocks.rs:73` — wall light
  points are looked up with `dt1.orientation` instead of the DT1 light
  direction (tile header +0x00). `Dt1Facts` (`rules/draw_order/mod.rs:178`)
  has no direction field; the sim `TileInfo` feeds only orientation
  (`near_rooms.rs:201`). Spec OQ8: direction is 0→3; 1,5,8,10→1;
  2,6,9,11→2; 3,4,12,13,14→3; 7→4; 15→5; 16–19→6–9. Code: orientations
  1–3 happen to match; 4–9 read the wrong point table (e.g. orientation 5
  reads direction 5's points instead of 1's); 10–14 and lower walls 16–19
  error → `block_shades` empty → whole wall falls back to one flat value
  from its centre sub-tile (`preview_light::tile_light_byte`). Screen:
  wall light gradients wrong or missing on most wall types (doors, corners,
  all lower walls) — walls look flat/patchy against the floor light.
  Row: `q-fix-render-wall-light-direction`: carry `light_direction`
  (d2-formats `dt1.rs:46`) through sim `TileInfo` → `Dt1Facts` and pass it
  to `wall_light_words` (§11 r2). Boundary: sim TileInfo lacks the field.
- F2 [QUEUE] §11 r2 (fade state) — `preview_blocks.rs:73` passes fade 0;
  the `TileRecord` already carries `fade` (`near_rooms.rs:210`). Walls near
  the player that fade (draw-order §8) read the normal instead of the faded
  point table. Row `q-fix-render-wall-light-fade`: pass the record's fade
  state bit 0 (§11 r2).
- F3 [FIXED] §6.1 — `preview_light.rs` refresh — the player's light was put
  at `8·sx + 4` of its truncated sub-tile; spec: `(P >> 13) + 4` of the
  precise 16.16 position (the light cell is the rounded sub-tile). Screen:
  the light pool lagged up to one sub-tile behind the walking player and
  stepped per sub-tile instead of moving in 1/8 steps. Fix: `light_pos_of`
  + `build_map_eighths`; local player uses the predicted 16.16 position.
  Test `the_player_light_sits_at_its_precise_position` (spec vector 804).
  Other units still at `8·s + 4` (model has no precise position: Boundary).
- F4 [FIXED] §9.2 r2–r4 — `preview_light.rs` refresh — `env` was copied
  from `world.environment` on the first frame only (`get_or_insert_with`),
  so every later S→C 0x53 / 0x5D eclipse never reached the rendered
  ambient; the copy then free-ran. Fix: re-take the model record whenever
  it changes (`env_seen`). Test `a_new_environment_record_reaches_the_ambient`.
- F5 [QUEUE] §9.2 r1, §10 r5 — the environment advances once per *drawn
  frame* in preview_light (spec: once per client update), and nothing calls
  `Environment::update` / `Overrides::update_counters` / `update_darkness`
  per client update in the bridge. Screen: day/night runs at frame-rate
  speed (≈2.4× at 60 fps), Den of Evil fade, red glow (107/108) and
  darkness events never step. Row `q-fix-render-env-per-update`: run
  §9.2 r1 and §10 r5 in the bridge's per-client-update pass on
  `world.environment` / `world.overrides`, and have preview_light only read
  them. Boundary (bridge tick pass).
- F6 [QUEUE] §6.4 r5, §8 Kind column, §13 "record kinds" — `preview_light.rs`
  `build_map_eighths` / `light_sources.rs:86` — only the local player's
  light is shadowed; monster lights (kind 0, shadowed at q = 2) and
  umod lights draw plain, objects (kind 2) draw plain instead of cached
  shadow (§7.4). Spec §13 says "only the player is shadowed is not §6.4
  r5". Screen: torches/braziers/monster lights shine through walls.
  Row `q-fix-render-light-kinds`: carry the §8 kind per source in
  `LightRows::lights` and dispatch §6.4 r5 (objects need the §7.4 cache
  from the owner's sub-tile).
- F7 [FIXED] §4 r2 — `preview_light.rs` blocks closure — a sub-tile in no
  loaded room (`collision_at` = None) gave flag 0; spec: flag 1 (0x27
  unmasked). Screen: the player's shadowed light leaked over the void edge
  of the loaded area instead of being cut. Fix `blocks_light`; test
  `a_cell_in_no_room_blocks_light`.
- F8 [OPEN] §6.4 / §6.2 — records are rebuilt per frame (preview_light,
  light_sources), so radius walks (§6.4 r2: overlay InitRadius→Radius,
  missile flicker, cast lights, dying missiles shrinking) never happen;
  missiles pop off instead of fading. Needs the persistent `LightList` in
  the client model (§13 `LightSources`). Part of F6's row or its own
  `q-fix-render-light-list`.
- F9 [OPEN] §8 player row — the local player's colour ignores the state
  colour call (`shading.md` §6 r1.1); `player_light_radius(..).max(1)`
  (`preview_light.rs`) gives radius 1 for a total ≤ 0, where §6.2 r2 says
  "r ≤ 0 → nothing" (keeps the previous radius). Edge only.
- F10 [OPEN] §3 r1 — no player room → spec: every cell 0, no flags; the
  preview still fills with the level-0/environment ambient. Only before
  the first room arrives.
- F11 [OPEN] §11 r4 / blend-modes §6 — `preview_blocks.rs` comment calls
  the roof path PROVISIONAL, but the spec now answers it (floor grid,
  floor gradient, "drawn opaque"); the roof blocks are given the tile's own
  blend. For the blend auditor to confirm.

### Rules checked and agreeing

§1 r1–r2 (48×48, row = y, origin = player − 24, `read` clamps, writes
dropped); §3 r2–r3 (rect inclusive +1 column/row, skip tests, list order,
player-room skip; ActiveRoom rect is sub-tiles, `bridge/drlg.rs:273`);
§4 mask 0x22; §6.1 position formula (`unit_light_pos`); §6.2 r1–r6; §6.4
r1–r5; §7.1 r1–r6 (window, `k`, top-left corner sample, oct, add, colour
mix, T table); §7.2 caps; §7.3 r1–r5 (B window, ring order, shade cell
cases, rings 0/1 unwritten, `S >= 16`); §7.4; §9.1, §9.3, §9.4, §9.2 r2
setter (env_tests vectors); §10 (overrides tests); §11 r1 (unit word at
sub-tile × 8), r2 (point read `X + 8·dx`, corners c0 = c3 = I_c,
c1 = c2 = I_{c+1}, consistent with shading.md §4 c0 TL, c1 TR, c2 BR, c3
BL), r3 (grid `(sx − 1 + i, sy − 1 + j)`, material 0x100), r4 (height ≠ 0 →
environment). blend-modes §5 r2 shadow shape (`shadow_image`: source row
2k from bottom, row k at (x0 − k, y0 − k), trunc yoff/2) and r3 position
(unit (X, Y) − 2 in x, with oz = 0). overlay.md §2 r1–r14, §3 r2–r9
(OverlayList), unit-composite §5 r4 skip/frame tests (`overlay_draws`).

### Not implemented / not reached from the live path

- Overlay engine (`world_view/overlay.rs`): no live caller creates,
  advances or draws overlays; the overlay draw position of
  unit-composite §5 r4 (`X = … + ox + (+0x20)`, `Y = … + oy + oz + (+0x24)`)
  is implemented nowhere; overlay lights (§8 overlay row) never created.
- `LightMap::build`, `LightList::frame`, `room_created`, quality `q` (§5):
  not used live; q is fixed at 2.
- Unit shadows: state 146 / flag bit 5 skips, `oz`, COF box pre-test and
  the single-cel shadow of types ≥ 3 (blend-modes §5 r4: no −2, motion
  `>> 11`) are not modelled (the module says so).
- Monster `L_c`, level-8 quest override, umod 3, cast lights, cursecenter,
  horadric_light, Den lights: not fed (light_sources header says so).

### Boundary (for q-seam-audit)

- Sim `TileInfo` (`d2-sim/src/drlg/tiles.rs:313`) does not carry the DT1
  light direction (header +0x00) that d2-formats parses (F1).
- `ClientUnit` holds only a u16 sub-tile position; §6.1 needs the precise
  16.16 position for every light owner (non-local units' lights snap to
  sub-tiles).
- No per-client-update hook advances `world.environment` (§9.2 r1) or
  `world.overrides` (§10 r5) (F5); quest byte 1 (last 0x5E A[1]) is not
  held (preview reads 0).

### Files changed

- crates/d2-client/src/world_view/preview_light.rs (F3, F4, F7 + 3 tests;
  `build_map_blocked` keeps its signature, now a wrapper of
  `build_map_eighths`).

---

## Area report: ui

### Findings (most visible first)

- F1 [FIXED] `ui/panels.md` §5 (step 7 vs step 10), `ui/panels-3.md` §23 r9, `control-panel.md` §5 r8/r14 — `crates/d2-client/src/ui/original.rs` (BorderUi::draw, was ~l.1320–1360). The cursor item, the belt item pop-up and the inventory item tip were drawn inside `BorderUi`, which `install` adds BEFORE `HudUi` (globes, belt, skill buttons, mini panel), hire list, NPC menu, gold dialog, messages, overhead text and Esc menu. The spec draws the cursor item after the whole UI pass and the pop-up text at step 10, after the control panel. On screen: a held item dragged over the control panel / belt went under the belt, globes and buttons; a belt tip was covered by the popped belt rows; item tips sat under the HUD. Fix: new `TopUi` panel (`TOP_PANEL` 0x113, open for good, empty rect) added last; it draws the belt tip, the item tip, then the cursor item. Test: `original_tests.rs::install_mirrors_the_flags_and_keeps_the_border_open` (and two others) now expect `TOP_PANEL` last in the open list.
- F2 [FIXED] `control-panel.md` §5 r14 (steps 2, 4, 6), §4 r1/r2, §6, §8 — `ui/hud_tips.rs` `draw_tips` / `draw_globe_text`. Every control-panel tip (run, menu, new stats / skills, experience, stamina) is a `0x00502280` pop-up: block W = max width + 8, box centred on x (x0 = x − (W >> 1)), bottom b = y + 2 (clamped to Sh − 5 / Ht − 5, x' clamped to [0, Sw − W]), text at b − trunc(3·height/10) in block W, centred. The code passed `DrawText(x, y, centered)`, whose block starts AT x. On screen: every tip shifted right by half its width plus 4 px and one row low. Fix: `ui::text::popup_text` (r14 placement) + `FontMeasure::popup`; tips go through `tip_request` when the font is loaded. Tests: `ui::tests::text::popup_text_is_centred_on_x_with_its_bottom_at_y_plus_2` (spec statement "box centred on W/2 − 76, bottom H − 50"), `hud_tips::tests::the_stamina_tip_is_a_popup_centred_on_its_point`. Still missing (r14): the colour-0 mode-2 backing box (no rectangle / blend in the UI draw list), the too-tall font swap (step 3), the one-slot rule (only the last call of the frame draws).
- F3 [QUEUE] `inventory.md` §2 r2–r3 — `ui/panels/inv_items_tint.rs` `draw_tints` (via `esc_menu::push_fill`). Tints are painted as OPAQUE `d2rs\hudfill` tiles (REC-271) instead of the A2 25 % blend of draw mode 0. On screen: every stored / equipped item sits on a solid dark-blue (usable), red, green or brown block. Queue row `q-fix-render-ui-blend`: give `ImageRequest` a draw mode (5 default), have `PanelArtRules::ui_image` map it to `BlendOp` through the `ShadeTables` alpha tables the world rules already build (`rules/blend.rs`), and use mode 0/2 rectangles for tints, the pop-up box (F2) and the Esc box. Same row covers: cube transmute animation mode 3 (`panels.md` §12 r4, drawn opaque now) and ethereal items mode 1 (`inventory.md` §8 r4), plus the item colour remap (`0x0062C100`, §8 r4: unique / set / `InvTrans` colours are not applied: items show default colours).
- F4 [QUEUE] `automap.md` §10 r4 — `world_view/automap_view.rs` (module doc, `add_to_frame`). Every automap cel draws `BlendOp::Opaque`; the spec picks mode 0/1/2 (25/50/75 % fades) for v = 1 near the centre, mode 1 for v = 2, etc. On screen: the full automap is solid lines over the world rather than the translucent overlay. Queue row `q-fix-render-automap-blend` (map mode → `BlendOp::IndexTable(alpha[..])` with the world's `ShadeTables`). Also not drawn: header and name texts (§11 r7, §13).
- F5 [QUEUE] `control-panel.md` §5 r8 / r14 — `ui/original.rs` `TopUi` belt tip uses `item_tip::draw_tip` (d2rs-own box at `hover_tip`'s point) instead of the r14 pop-up (T = stats then name, `0x00502280(T, x, y, 0, centre 1)`). Queue row `q-fix-render-belt-popup`.
- F6 [OPEN, test bug, not code] HANDOFF `q-fix-panel-horadric-offsets`: the code does NOT assume zero offsets. `panels.md` §1 r3 explicitly excepts `menu\horadric` (§12 r4: frames 1–29 carry offsets, frame 1 = (−205, 17)) and `panel_art::panel_sprite` applies frame offsets through `rules::placement::draw_position` (`sprite-placement.md` §2). The failing assertion is the game-file test `crates/d2-client/tests/game_panels.rs:99` (`panel_files_frame_counts_and_sizes`), which checks (0, 0) offsets for every listed file including `menu\horadric`; the spec's own test vector (§Test vectors l.888) says frames 0 and 30 are 2×2 at (0, 0) and 1–29 non-zero (frame 1 (−205, 17), frame 15 (−280, 82)). Fix (outside my file list): in that test skip the offset assert for `menu\horadric` and add those three vectors. The row should be renamed a test fix, not a render fix.
- F7 [OPEN, d2rs-own] `world_view/object_label.rs:66` — the object label is `TextOpts::centered()` at the feet x; a centred `DrawText` block starts at x, so the name sits right of the object by half its width + 4 (the module doc says "centred above the feet"). No spec owns the label (REC-239); if it is the hover pop-up, it should go through `popup_text` (F2) with centre 1.
- F8 [OPEN, spec] `inventory.md` §8 r4 says the draw at (x, top + h) puts "the frame's top-left at (x, top)", but by `sprite-placement.md` §2 a cel at Y = top + h covers rows top + 1 … top + h (one row lower). The code (`inv_grid.rs:502`, `inv_items.rs:290` cursor item) follows the formula. A spec owner should settle which (capture `inv-0001`).

### Rules checked and agreeing
- `panels.md` §1 r1 (sx, sy = (80, −60) at mode 2), r2 (expression grammar, `layout.rs` `Expr::eval`), r3 (cel draw with offsets via `draw_position`), r4 (quads 256/64 × 256/176 at H + sy − 224 / − 48, `Screen::quads`), r5 (left X0 = sx, right W − sx − 320), r6 (`centered_in`, spans a … a + w − 1).
- `panels.md` §6 r1 (border frames 0–4 / 5–9 by open mode, absolute rows), r2 (`800ctrlpnl7` 6 frames at 0, W/2 − 235, −107, +21, +149, W − 117, y = H; GDI).
- `panels.md` §12 r4 (horadric at (W/2, H/2 − 1), offsets applied; test vector frame 1 → cols 115–206 rows 136–256 at 640×480 holds by `draw_position`).
- `sprite-placement.md` §2 / `dc6.md`: DC6 flip 0 → `FrameAnchor::Bottom`, top = Y + yoff − h + 1; synthetic fills anchor top.
- `control-panel.md` §3 r2–r6 (globe windows at x 29 / W − 111, H − 13, covers (28, H − 5) / (W − 110, H − 9), toggles, numbers at 65 − w/2 / W − 80 − w/2, H − 95), §4 r1 (bar lines H − 38/37, tip hover rect), §4 r2 (stamina rect W/2 − 127, H − 27, tip point (W/2 − 76, H − 52)); window rows (`text.md` §9 r2: rows B − skip − lines + 1 … B − skip) in `hud.rs::globe_draw`.
- `text.md` §3 (lookup by position, >0xFF → record 0), §4 r2 (pen = bottom row of glyph cell, `draw_position`), §4 r3 (remap then index 0 written: scene tests transparency on the source index only), §4 r4 (maps at 439,847 + 256k; maps 1–12 pushed), §5 (ÿc + unit − 0x30, ≥ 13 → 0, negatives kept, ÿc at end → 0 and stop, ÿ + other = glyph 255), §6 (widths A/B/C, line width with ÿ skips and `m` advance, max width, text height trunc(16·h·lines/10), line step), §7 (centred block max width + 8, `>> 1`, LF up one step), §8 (framed variants), §9 horizontal window, `text-fonts.tsv` (all 14 rows equal `FONTS`).
- `font-tbl.md` header / 14-byte records, u16 frame.

### Not implemented / not reached from the live path
- UI draw modes and remaps: `ImageRequest` has no mode / palette (`panel_art.rs` module doc) — tints, pop-up boxes, cube animation (mode 3), ethereal (mode 1), item colour transforms, skill-icon remaps (F3).
- Automap fades, header and names (F4); automap drawn in pass 9 rather than UI step 3 (same relative order to panels).
- Pop-up one-slot rule, too-tall font swap, backing box (F2 remainder).
- `text.md` §9 vertical window (open question 1: `Unspecified` error).
- Empty-slot pictures (`panels.md` §9.4), shooter / quiver tints.

### Boundary (for q-seam-audit)
- `world_view/automap_view.rs` builds its own `Camera::new(FrameSize::D2RS, open_mode, at, (0, 0))` from `player.position` (`moving_to_client((p << 16) | 0x8000)`) instead of the frame's world camera; whether that unit origin equals the one the world view uses (`camera.md` §3) is a seam question.

### Files changed
- crates/d2-client/src/ui/original.rs (TOP_PANEL / TopUi; BorderUi draws border + control panel only; `FontMeasure::popup`)
- crates/d2-client/src/ui/original_tests.rs (open-panel lists end with TOP_PANEL; Covers line)
- crates/d2-client/src/ui/text.rs (`popup_text`)
- crates/d2-client/src/ui/tests.rs (popup test)
- crates/d2-client/src/ui/hud_tips.rs (`tip_request`, `fonts` fields, new test)
- crates/d2-client/src/ui/hud.rs (passes the fonts)
Verified: `cargo test -p d2-client --lib ui::` 621 passed; `cargo clippy -p d2-client --lib --tests` clean (needs `PKG_CONFIG_PATH=/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig` in this container).
