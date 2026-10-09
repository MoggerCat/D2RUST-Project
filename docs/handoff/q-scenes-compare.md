# Handoff: d2rs vs 1.14d scene compares — `claude/q-scenes-compare`

Cloud session, 2026-10-09. REC block 510–519 (used: 510–517).
Facts: `facts/render/scenes/*` (q-facts-scenes, Wine). d2rs: dev build,
`d2-client play --save S.d2s --seed 1234 --dump-draws DIR --at-tick T [--input SCRIPT]`
under Xvfb + lavapipe; compare: `d2-client facts-compare SCENE DIR --ignore tick,index_sha256 --skip-weather`.

## How to run a scene
- Saves: `prepare_scene_chars.sh`'s `d2s-tool new` lines (SceSor, SceAct2..5) into any folder (no Wine needed).
- Tick T = the 1.14d `frame.tsv` `tick`. Input: the group script of `tools/cloud-game/scene_defs.py` turned into absolute ticks; `key`/`click` steps at the 1.14d mark tick act one tick later in 1.14d, so a scene gets only the steps before T − 1.
- Walk schedule: click k (n, s, ne, sw, e, w, se, nw of `_DIRS`) at tick 22 + 14k, dump at 32 + 14k; run: the eight walks, `key R` at 134, run clicks at 140 + 14k, dump 150 + 14k.
- Panel schedule (absolute ticks): 22 I, 52 click 678 330, 62 click 563 250, 77 I, 97 0xC0, 127 0xC0, 143 C, 173 C, 193 T, 223 T, 243 TAB, 273 TAB, 293 I, 303 rclick 446 345, 333 ESC, 353 ESC (Esc scene: ESC at 354, dump 354).

## Tools added
- `facts-compare --skip-weather` (`facts-render.md` §6 r5, REC-510): drops pass-9 rows (1.14d `at` in 0x00473470..0x00473F50, d2rs `at` = `pass9`).
- `play --input` `key K` (§5 r11 revision).
- `facts_render.py --merge-sprites` (§4 r3): joins two branches' sprites.tsv.

## Fixes
1. Unit shadows have their own pre-test on the sheared shadow box, not the body's (`blend-modes.md` §5 r3 revision, REC-511): inventory / character / skilltree past rows 97–103.
2. Player TN frame at tick T = ((T − 1)·s >> 8) mod F (`unit-composite.md` §3 r2 revision; REC-512 for monsters / missiles): idle-sor, a2, skilltree past the player rows.
3. A save in Act III–V: the act slot is built at the join (`intents-events.md` §8.2 step 4; `d2-server` `enter_game`): a3–a5 draw their towns (were UI only).
4. Two tile records of one cell are two calls in the export (`facts-render.md` §5 r1 revision): a4 past row 13.
5. itemtypes `class` 0xFF (empty cell, real data) is "none" (`items/inventory.md` §4.2 step 7 revision, REC-513): classless items equip; the Sash goes to the belt and the cube opens.
6. Click → subtile four rows below the plain inverse (`ui/controls.md` §6 r2, REC-514): walk-n reaches 1.14d's position.
7. A predicted walk in town is drawn in mode 6 (`pathing.md` §1.5 r2; `Predict::mode`).
8. Inventory family draw order (`ui/panels.md` §9 r1 revision, REC-515 for stash / shop; §9 r6; §12 r3): the mode panel (cube) draws before the inventory (`Panel::draw_before`, routing unchanged); per item its cell tints (one box per cell, row by row) then the item; gold line, gold button, close button after the items; cube buttons after its grid. UI rectangles export as `DrawBox` rows (`facts-render.md` §5 r16). Cube: equal through row 24.
9. The local player's walk frame counts from the walk request at speed 213 (`sim/units.md` §4.7 step 7 revision, REC-516; run not yet); a point walk keeps the facing from its start to the clicked subtile (`render/unit-composite.md` §3 r1, REC-517).
10. The export's unit `dir` is keyed by draw slot, not GUID (a torch with GUID 1 overwrote the player's; `facts-render.md` §5 r14). a1-walk-n now equal through the player rows.
11. A listed unit whose body fails the pre-test still writes its `unit` row (`facts-render.md` §5 r17; the unit draw calls the pre-test inside): a1-panel-inventory past row 128.

## First difference per scene (after fixes 1–11, staging with q-fix-real-unit-seed-order merged)
| Scene | First difference |
|---|---|
| a1-town-idle-sor | r110 NPC `wa` WL dir 47 (1.14d) vs 0 (NPC facing) |
| a1-walk-n | r110 `wa` WL dir 47 vs 0 (same) |
| a1-panel-inventory | r137–138 an extra `unit` row: d2rs's `wa` (1:7) stands in this cell (NPC position) |
| a1-panel-character | r109 `wa` NU dir 47 vs WL (NPC pose) |
| a1-panel-skilltree | r111 `wa` vs `rc` (NPC pose) |
| a1-panel-automap | r108 torch vs `rc` (NPC position) |
| a1-panel-esc-menu-wine | r108 `rc` NU at 814,484 vs a torch (NPC position) |
| a1-panel-cube | r25 the hovered cube's tint: d2rs 118 (hover, §3 r2), 1.14d 234 (usable): 1.14d's hover state may follow mouse moves only (settled by a recording that moves onto the item) |
| a2-town-lut-gholein | r115 critter `bg` shadow missing (q-fix-real-town-critters) |
| a3-town-kurast-docks | r97 pass-9 cel (`at` 0x473bd0, file ?) vs d2rs `rain3.dc6` f1 |
| a4-town-pandemonium-fortress | r100 object shadow `98` vs `99` (object order) |
| a5-town-harrogath | r76 monster `6z` NU frame 3 vs 5 (6 frames, rate 32: the monster's animation start, REC-512) |
| a1-walk-s .. nw | frame input tile_origin_x 1–33 px off (walk-s 9933 vs 9936): the chained walks drift |
| a1-run-* | tile_origin_x ~90 px off (inherits the walk drift) |
| a1-cold-plains-monsters | level 3 vs 1: the waypoint walk does not reach the waypoint (walk drift) |
| a1-town-arrival-ama | skipped (q-fix-real-unit-seed-order) |

## Notes
- `app_single_player::the_session_flow_creates_the_game_then_loads_the_character_at_the_join` (ignored, real data) fails with and without fix 3 (0x23 count 3 vs 2): not caused by this branch.
- Disk: one target dir only; two (worktree gate) overflow the session allowance.
