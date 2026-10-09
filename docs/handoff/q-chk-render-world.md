# q-chk-render-world — world rendering against 1.14d (T4)

Cloud session, 2026-10-09, branch `claude/q-chk-render-world`. REC-1430, REC-1431 used (no provisional choices).

## Done
- **Scene groups** (`tools/cloud-game/scene_defs.py`): towns of every act already existed (`a1town`, `act2town`..`act5town`); added one outdoor level per act by the town waypoint: `act1out` (Cold Plains), `act2out` (Dry Hills), `act3out` (Spider Forest), `act4out` (City of the Damned), `act5out` (Frigid Highlands). The click paths were generated, not hand-written: d2rs's autoplay host (`d2-client autoplay-host`, `map` read = collision grids) walked a grid route to each town waypoint (`WP` positions read from the existing `tools/autoplay/routes/act*-probe.route` replays: A2 5069,5084; A3 5159,5049; A4 5044,5019; A5 5114,5069; A1 4899,4209), clicked it and a menu row (row 1, A2 row 2), one click per 13 ticks.
- `tools/sidebyside/build.py`: the 1.14d recording limit scales with the group's ticks (was a fixed 420 s: a3 stopped at tick 481 of 483, fight at 477).
- Ledger rows (10, all DIVERGED): `docs/handoff/ledger/q-chk-render-world.tsv` (valid with `ledger.py`: 10 rows, 0 format errors).
- Pages: private repo `reports/side-by-side/2026-10-09-q-chk-render-world/` (README with commands).

## Results (scene, pixels over compared ticks, first difference)
| Scene | Result |
|---|---|
| a1-town-idle-sor | 99.22%, tick 3, row 111 critter `ck` shadow missing |
| a2-town-lut-gholein | 98.57%, tick 3, row 115 critter `bg` shadow missing |
| a3-town-kurast-docks | 98.72%, tick 3, row 99 NPC `m3` shadow dir 14 vs 23 |
| a4-town-pandemonium-fortress | 95.20%, row 278 the cursor only (wall-clock) |
| a5-town-harrogath | 99.41%, row 76 monster `6z` start frame 3 vs 5 |
| a1/a2/a3 outdoor | 1.14d does not reach the waypoint (its NPCs / walk differ from d2rs's): level 1/40/75 vs 3/42/76 |
| a4-outdoor-city-of-the-damned | both reach 106; 65%; rows before 114 (floor, walls, torch) equal |
| a5-outdoor-frigid-highlands | both reach 111; 78%; first diff row 121 (player pose) |

## Findings, all outside what this session may edit (route; none fixed here)
1. **Client keeps the walk mode after a waypoint travel** (a4, a5 outdoor, first difference): after the menu row click the client model's local player stays mode 2 (WL) in the new level (autoplay host `state`: `client.player.mode` 2 while server `snap` mode 1) and the world view draws the walk cel (`soralitwlhth` frame 37 / 3) where 1.14d draws `nuhth` frame 0. Repro: `python3 ~/scratch`-style host run of `scene_defs.GROUPS["act4out"]` clicks, or the sidebyside group `act4out`. Owner by `owners.tsv`: client action state / prediction (`crates/d2-client/src/bridge/predict.rs`, `claude/q-fix-input-lock`; `movement-prediction.md`, `claude/q-fix-client-crash`).
2. **Waypoint travel lands one tick later in d2rs** (1.14d level changes at the click frame: a4 117, a5 182; d2rs 118, 183): d2rs's `frame F; click` is press at F and release at F+1 (`facts-render.md` §5 r11); the 1.14d recorder posts press and release together. The row selects on mouse up. Owner: scenario-diff / input (`claude/q-tool-state-diff`) to say which side's click timing is the original's; then the scene steps follow it.
3. **City of the Damned: an extra object draw in d2rs.** d2rs draws a `data/global/objects/uy/tr/uytrlitnuhth.dcc` body + shadow (frame 1) at screen (448,68) before the player; 1.14d draws only one `uy` (frame 4). Likely a client object visible in d2rs that 1.14d does not draw (object visibility or population). Owner: render (`claude/q-scenes-compare`) to confirm which unit it is (`state` units near 5456,4473: class 380).
4. **Frigid Highlands UI rows**: 1.14d draws a 'Quest Log' hint (`levelsocket` + `level` button at 40,495, text at 18,457, `0x00501BC0`) that d2rs does not; d2rs draws the area-name banner (`fontingamechat` at 212,60) 79 ticks after arrival where 1.14d has none at the scene tick. Owner: client UI (`claude/q-fix-pc1-client-ui`).
5. Towns: unchanged from `q-scenes-compare.md` (NPC / critter position and facing rows: `claude/q-fix-real-unit-seed-order`).
6. The outdoor groups for a1–a3 need the NPC / walk fixes before they compare past the walk: re-run them then.

## Open
- No fix of mine: every first difference found is owned elsewhere (rule: route, don't fix).
- The generated click paths are fixed strings; if the walk / click geometry changes in d2rs they may stop reaching the waypoint in d2rs (check `summary.json` `feel` and the level row), then regenerate with the host as above.

## Repro
```sh
export D2_GAME_DIR=$HOME/game
tools/cloud-game/prepare_scene_chars.sh
python3 tools/sidebyside/build.py --out ~/sbs/outdoor --groups act1out,act2out,act3out,act4out,act5out,a1town,act2town,act3town,act4town,act5town --jobs 1
```
Checks run: `python3 tools/coverage.py --check`, `python3 tools/spec_index.py --check` (green); no Rust changed.
