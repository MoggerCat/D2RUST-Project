# Handoff: d2rs vs 1.14d scene compares — `claude/q-scenes-compare`

Cloud session, 2026-10-09. REC block 510–519 (used: 510, 511, 512).
Facts: `facts/render/scenes/*` (q-facts-scenes, Wine). d2rs: dev build,
`d2-client play --save S.d2s --seed 1234 --dump-draws DIR --at-tick T [--input SCRIPT]`
under Xvfb + lavapipe; compare: `d2-client facts-compare SCENE DIR --ignore tick,index_sha256 --skip-weather`.

## How to run a scene
- Saves: `prepare_scene_chars.sh`'s `d2s-tool new` lines (SceSor, SceAct2..5) into any folder (no Wine needed).
- Tick T = the 1.14d `frame.tsv` `tick`. Input: the group script of `tools/cloud-game/scene_defs.py` turned into absolute ticks; `key`/`click` steps at the 1.14d mark tick act one tick later in 1.14d, so a scene gets only the steps before T − 1.
- Panel schedule (absolute ticks): 22 I, 52 click 678 330, 62 click 563 250, 77 I, 97 0xC0, 127 0xC0, 143 C, 173 C, 193 T, 223 T, 243 TAB, 273 TAB, 293 I, 303 rclick 446 345, 333 ESC, 353 ESC (Esc scene: ESC at 354, dump 354).

## Tools added
- `facts-compare --skip-weather` (`facts-render.md` §6 r5, REC-510): drops pass-9 rows (1.14d `at` in 0x00473470..0x00473F50, d2rs `at` = `pass9`).
- `play --input` `key K` (§5 r11 revision).
- `facts_render.py --merge-sprites` (§4 r3): joins two branches' sprites.tsv.

## Fixes
1. Unit shadows have their own pre-test on the sheared shadow box, not the body's (`blend-modes.md` §5 r3 revision, REC-511): inventory / character / skilltree past rows 97–103.
2. Player TN frame at tick T = ((T − 1)·s >> 8) mod F (`unit-composite.md` §3 r2 revision; REC-512 for monsters / missiles): idle-sor, a2, skilltree past the player rows.
3. A save in Act III–V: the act slot is built at the join (`intents-events.md` §8.2 step 4; `d2-server` `enter_game`): a3–a5 draw their towns (were UI only).

## First difference per scene (after fixes 1–3)
| Scene | First difference |
|---|---|
| a1-town-idle-sor | r108 NPC `rc` walk frame 7 vs 1 (NPC path / seed order: q-fix-real-unit-seed-order) |
| a1-panel-inventory | r111 `rc` walk frame 7 vs 3 (same) |
| a1-panel-character | r109 `wa` NU dir 47 vs WL dir 0 (NPC pose) |
| a1-panel-skilltree | r111 `wa` vs `rc` (NPC pose) |
| a1-panel-automap | r109 `wa` vs `rc` (NPC pose) |
| a1-panel-esc-menu | r108 `rc` NU at 814,484 vs a torch (NPC position) |
| a1-panel-cube | frame input open_mode 3 vs 1: rclick on the cube does not open it |
| a2-town-lut-gholein | r115 critter `bg` shadow missing (q-fix-real-town-critters) |
| a3-town-kurast-docks | r97 pass-9 cel (`at` 0x473bd0, file ?) vs d2rs `rain3.dc6` f1 |
| a4-town-pandemonium-fortress | r13 1.14d calls the lower wall at (320, 864) twice; d2rs once |
| a5-town-harrogath | r76 monster `6z` NU frame 3 vs 5 (monster phase, REC-512) |
| a1-walk-{n,s,ne,sw,e} | frame input tile_origin: the player walks another way from the same click (d2rs dx −13, 1.14d +8 after 10 ticks, walk-n) |
| a1-cold-plains-monsters | not run yet (waypoint travel script) |
| a1-town-arrival-ama | skipped (q-fix-real-unit-seed-order) |

## Notes
- `app_single_player::the_session_flow_creates_the_game_then_loads_the_character_at_the_join` (ignored, real data) fails with and without fix 3 (0x23 count 3 vs 2): not caused by this branch.
- Disk: one target dir only; two (worktree gate) overflow the session allowance.
