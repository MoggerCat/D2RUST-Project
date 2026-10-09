# Handoff: 1.14d rendering facts under Wine, scenes 2-6 — `claude/q-facts-scenes`

Cloud session, 2026-10-09. Ground truth only (1.14d side). No `crates/` change.
Tools: `tools/cloud-game/scenes.py` + `scene_defs.py` (the exact scripts), `prepare_scene_chars.sh`
(fixed characters), `tools/trace-recorder/{autostart,facts_render}.py`, `tools/d2s-tool`
(`--left-skill` / `--right-skill`). Facts: `facts/render/scenes/<scene>/{draws,frame}.tsv`,
`facts/render/sprites.tsv`, open requests in `facts/requests.tsv`.

## How a scene is recorded

`python3 tools/cloud-game/scenes.py GROUP --runs 2 --install` runs the group's input script twice
(`record_frames.py --every 1 --draws-every 1`, character from `prepare_scene_chars.sh`, `-seed 1234`),
cuts each scene at the first frame with a draw log at or after the tick of its `mark` (Esc menu: the
last frame of the paused tick), converts both runs with `facts_render.py --tile-light unknown` and
compares `draws.tsv` / `frame.tsv` row by row. The first run is installed.
Input is **tick-based** (`waitticks N`, `mark NAME`): under Wine the game runs about 5x slower than real
time (about 5 ticks/s, a frame every second tick), so wall-clock waits land on other ticks.

## Findings (measured, not yet in specs)

1. **Floor light read is not reproducible.** `FloorTileDraw`'s 768-byte light read differs between two runs of
   the same frame in 1 to many bytes (offsets 304-320, 400, 580, 760 ...), while the presented frame
   (`index_sha256`) is equal: the read covers bytes the game does not set. The tile rows' `light` cell
   is therefore `?` (`facts_render.py --tile-light unknown`); `capture.md` 3.5 should not call it a digest of
   the light. Settle: which bytes of the grid the rasterizer reads.
2. **The Esc menu stops the tick counter in single player.** `mark esc` after the key stays at the same
   tick for seconds and frames keep being drawn at that tick (menu drawn in later frames of the same
   tick). Step 4 item 2 of `pc1-data.md` (pause) is answered on the observation side: yes, ticks stop;
   the call is still PC 1's.
3. **Walking and running in town use the same cel set per mode**: walk is player mode 6 (`TW`), run mode 3
   (`RN`) even in town; 8 screen directions give `dir` 32, 39/40, 47, 55/56, 63/0, 7/8, 15, 23.
4. The game reads the **real pointer** (hover, cursor draws); posted `WM_MOUSEMOVE` alone leaves the cursor
   at the window centre. `autostart.py` now also `SetCursorPos`es (Wine moves the X pointer).
5. The Act I waypoint panel lists no Blood Moor (Cold Plains is the first entry); the Blood Moor is entered
   on foot from the east side of the camp (level rects `dumpdrlg`: Rogue Encampment tiles x 944..1000,
   y 832..872; Blood Moor x 1000..1056, y 824..920; Cold Plains x 1016..1096, y 920..1000).
6. Weather: `rain = 1` in the Act I town frames (`a1-town-idle-sor`), Act III (rain 1, snow 1) and Act V (rain 1);
   Act II and IV 0 (frame.tsv `rain` / `snow`).

## Scenes recorded and how stable each is (2 runs of the same script, same ticks)

"Weather" = the rain `DrawLine` / `DrawBox` rows (their positions and number differ between two runs of the
same tick, so `index_sha256` differs too): 1.14d's rain is not reproducible, a compare must skip those rows.
All other columns, the `frame.tsv` inputs and the `palette_sha256` are equal unless stated.

| Scenes | Character / script (`scene_defs.py`) | Two runs |
|---|---|---|
| `a1-panel-inventory`, `a1-panel-cube` | SceSor, group `panels` | identical (all rows, both hashes) |
| `a1-panel-character`, `-skilltree`, `-automap`, `-esc-menu` | SceSor, `panels` | equal except weather rows |
| `a1-panel-belt-open` | SceSor, `panels` (a Sash equipped, backtick) | one row differs: the cursor `ohand.dc6` frame 7 vs 6 (the cursor animation follows wall-clock time) |
| `a1-walk-{n,s,ne,sw,e,w,se,nw}`, `a1-run-*` | SceSor, `walk` (clicks 200 px around the player; first frame 10 ticks after the click; run after `R`) | `a1-walk-{n,s,ne,sw,e}` identical, the other 11 equal except weather rows |
| `a1-town-idle-sor` | SceSor, `a1town` | identical |
| `a2-town-lut-gholein`, `a3-town-kurast-docks`, `a4-town-pandemonium-fortress`, `a5-town-harrogath` | SceAct2..5 (`--act 1..4`), arrival | identical |
| `a1-cold-plains-monsters` | SceSor, `fight`: walk to the waypoint, Cold Plains, the Fallen group on the pad | identical |
| `a1-cold-plains-corpse` | same run, after Fire Bolt kills (2 dead Fallen drawn) | equal except weather rows and one cursor row |
| `a1-cold-plains-drop` | same run, an item on the ground (unit type 4) | **not stable**: the fight diverges between runs (first non-weather row 108, monster positions); kept as measured, do not compare against it |
| `a1-panel-stash`, `a1-npc-*`, `frontend-*`, `a1-blood-moor-*`, `a1-den-of-evil` | | not recorded: rows in `facts/requests.tsv` |

Characters (`prepare_scene_chars.sh`): SceSor (level 1 sorceress, Fire Bolt as left skill, 400 life, 500 gold,
Horadric Cube, Sash, cap, gloves, cube / stash contents, all Act I waypoints), SceAct2..5 (same, standing in
the town of Act II..V). The first three groups were re-recorded after the left skill was added.

Act I rain: `rain = 1` in every Act I town scene (SceSor and the earlier ScnAma ones), so the rain
scene is `a1-town-idle-sor` / the Cold Plains scenes; Act II and IV have `rain = 0`, Act III `rain = 1, snow = 1`.

## Not reachable under Wine (rows in `facts/requests.tsv`)

stash panel, NPC dialog / shop / gamble (clicks on objects and NPCs do not open anything), the four
front-end scenes (`record_frames.py` hooks only the in-game draw entry), Blood Moor (monsters, night),
Den of Evil. Why: scripted clicks on the Act I camp's east side never found the gate to the Blood Moor
(`dumpdrlg` rects in finding 5); clicks on the stash and on NPCs do not open anything under Wine.
