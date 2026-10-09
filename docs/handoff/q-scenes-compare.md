# Handoff: d2rs vs 1.14d scene compares — `claude/q-scenes-compare`

Cloud session, 2026-10-09. REC block 510–519 (all used: 510–519).
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
12. An object casts its composite shadow only where its mode's `BlocksLight` ≠ 0 (`blend-modes.md` §5 r3 revision, REC-518): a4 equal through the world (row 248).
13. A click while walking re-targets from the precise position (`sim/pathing.md` §1.5, already specified; `Predict::path_step`): walk-s, ne, sw, e match every frame input.
14. Control panel: UI cels export their 1.14d wrapper (`facts-render.md` §5 r18: orbs `CelDrawEx`, skill icons / glyphs `CelDrawColor`); the stamina bar is the rectangle (`control-panel.md` §4 r2); the new-stats / new-skills buttons after the skill icons (§1 r3); the mini panel open from the start (§9 revision, REC-519). a4 equal through row 257; next: the help button `0x004A64C0` ("Help (H)" + levelsocket, unspecified: `pc1-data.md` Step 4 item 41).
15. The client path sees the model's living monsters as footprints, mask 0x100 (`client/model.md` open question 2 revision, REC-706; `ClientPath::stamp_others`): walk-n, s, ne, e reach the critter / NPC rows; walk-se / nw match when d2rs's Warriv stands where 1.14d's does (NPC position: q-fix-real-unit-seed-order); runs drift ~50 px (was 90–240).
16. The grid hover tint and item tip follow the §5 hover state, not the raw mouse (`ui/inventory.md` §3 r2, already specified); a use press clears that state until the next move or press, and a release does not re-track it (§5 r4, PROVISIONAL REC-707). a1-panel-cube is now equal except the cursor row 73, which animates on wall-clock time (`facts-render.md` edge cases: known divergence).
17. `--skip-weather` also drops pass 4's environment-pool cels: 1.14d `CelDraw` rows from the pool draw [`0x00473A70`, `0x00473C00`), and d2rs rows exported with `at` = `pools` (`facts-render.md` §6 r5 revision, PROVISIONAL REC-708). Their spawns draw on the player seed, which the wall-clock cursor steps (`panels-3.md` §23 r8). a3 now reaches its NPC rows.

## First difference per scene (after fixes 1–17)
| Scene | First difference |
|---|---|
| a1-town-idle-sor, a1-walk-n, a1-walk-s | r110 NPC `wa` WL dir 47 vs 0: d2rs's `wa` already stands on its walk target (4866, 4235), 1.14d's still walks east past it (NPC path node: q-fix-real-unit-seed-order) |
| a1-walk-e | r100 `wa` shadow before the player's (NPC position) |
| a1-walk-ne | r112 critter `ck` shadow missing (q-fix-real-town-critters) |
| a1-walk-sw | r102 torch vs `rc` (NPC position) |
| a1-walk-w | tile_origin_y 72533 vs 72532 (1 px) |
| a1-walk-se, -nw | tile_origin 9992 vs 10012 / 9950 vs 9976: the walk bends round Warriv, who stands elsewhere in d2rs (NPC position) |
| a1-run-* | tile_origin ~50 px off. The run legs away from Warriv match 1.14d exactly (run-e Δ 91,17, run-w −77,0), so run speed and start are right; the error comes from walk-se / nw and run-n, which pass Warriv (NPC position) |
| a1-panel-inventory | r168 `rc` WL y (NPC position) |
| a1-panel-character, -skilltree | r109–111 `wa` NU / WL (NPC pose) |
| a1-panel-automap, -esc-menu-wine | r108 torch vs `rc` (NPC position) |
| a1-panel-cube | r73 the cursor only (wall-clock animation, known divergence) |
| a2-town-lut-gholein | r115 critter `bg` shadow missing (q-fix-real-town-critters) |
| a3-town-kurast-docks | r103 `m3` WL shadow dir 14 (NPC facing: q-fix-real-unit-seed-order) |
| a4-town-pandemonium-fortress | r258 the help button `0x004A64C0` ("Help (H)", `levelsocket`, `level` at 725, 440): not specified (`pc1-data.md` Step 4 item 41) |
| a5-town-harrogath | r76 monster `6z` NU frame 3 vs 5. Every a5 monster's start frame differs (7i 1/0, 7j 0/1): `msg-units.md` §1.2 r6.5 draws it on the unit seed, the room seed stepped once per unit add, so the Act V add order or room seeds differ (a1's NPCs match; unit seed order) |
| a1-cold-plains-monsters | level 3 vs 1: the waypoint walk does not reach the waypoint (walk / run drift) |
| a1-town-arrival-ama | skipped (q-fix-real-unit-seed-order) |

## Notes
- `app_single_player::the_session_flow_creates_the_game_then_loads_the_character_at_the_join` (ignored, real data) fails with and without fix 3 (0x23 count 3 vs 2): not caused by this branch.
- Disk: one target dir only; two (worktree gate) overflow the session allowance.
