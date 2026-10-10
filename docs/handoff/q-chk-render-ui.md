# q-chk-render-ui hand-back (T4: UI panels against 1.14d)

Scope: branch `claude/q-chk-render-ui`, 2026-10-09. REC-1435..1438 used (REC-1439 reserved for the skill tree captions, below).
Oracle: 1.14d under Wine through `tools/sidebyside` (private page `reports/side-by-side/2026-10-09-3/` in the private repo).

## Per panel (scene % -> first difference left)

| Panel scene | Before | After | What was left (first difference) |
|---|---|---|---|
| inventory | 99.5 | 99.35 | UI rows equal; cursor start position (1.14d pointer 120,237 vs d2rs 400,300: harness) and NPC rows (world draw, q-scenes-compare) |
| character | 98.0 | 98.92 | Fire Bolt damage block (`Damage`, `3-6`) missing; Defense 2 vs 1.14d 8 (sim stat 31 of the equipped SceSor: items / stats owner) |
| skill tree | 97.0 | 97.83 | 10 text lines missing: captions + points number (see below) |
| automap | 93.2 | 94.03 | 843 UI rows in 1.14d vs 129: automap cells (automap owner) |
| Esc menu | 98.2 | 98.25 | pentagram frame phase (1.14d wall-clock ms; not tick-deterministic) |
| cube | 99.9 | 99.96 | cursor animation frame |
| belt rows | 96.8 | 98.78 | cursor animation frame |
| quest log | 71.9 | 99.43 | cursor; the title / text pane is unmeasured (nothing selected) |
| waypoint, stash | 16 / 11 | 16 / 11 | walk position drift (q-scenes-compare, q-fix-client-crash); panels not reached |
| NPC menu | - | - | owned by q-fix-npc-menus (scene group needs frame-anchored clicks) |
| front end | - | - | NO-CHECK: traces are state traces, no frames (needs a capture, L) |

## Fixed (all in `d2-client`, tests green: 2270 lib tests, clippy -D warnings)

1. **Harness**: 1.14d writes `Mini Panel` into the Wine registry at exit (control-panel.md §9 r9), so every run after the first started with the mini panel closed and showed a false "mini panel extra in d2rs". `build.py` removes `Mini Panel` / `Help Menu` before each 1.14d recording (`reset_game_settings`). New scene groups `panels2` (quest log, waypoint) and `panels3` (stash); the panel marks now leave 2 ticks before the next key (the automap scene was caught closing).
2. **Belt key** (`controls.md` §3 cmd 22): was mapped but never handled. `ToggleBelt` with a worn belt toggles state 0x1F; the belt draw clears the pop-up flag with 0x1F closed and the 0x1F draw sets it (`hud_belt.rs`, `original.rs`). PROVISIONAL REC-1435: `[0x007BEFA0]` reads 0 when the rows fold (1.14d draws the mini panel at layout 2 after the key closes the rows; before, d2rs kept layout 1).
3. **Esc menu draw order**: `Panel::draw_first`, the menu is UI pass step 1 (`panels.md` §5 r1), before the control panel (it was last).
4. **Attack Rating label** (`panels-2.md` §17 r5): "ATTACK / Rating" (4065, upper-cased name) for the Attack skill, not "Attack / Attack Rating" (4063). PROVISIONAL REC-1436: the skills record is taken as null for skill 0.
5. **Quest log layout**: the d2rs-own layout is replaced by the measured one (positions in 640 x 480 space + (sx, H+sy-480): 4 background frames at (0,256) (256,256) (0,432) (256,432), tab strip (5,33), icon (26+97c, 121+97r), socket (-4,+5), close buysellbtn 10 at (278,422), questlast at (226,422); icon before socket; `open_tab` selects at open). PROVISIONAL REC-1437: tab strip frame = act (1.14d shows frame 0 for Act I).
6. **Character panel order**: art, labels, skill blocks, close, class and name, values (`draw_staged`). PROVISIONAL REC-1438: the stat-points box and add buttons stay with the values (no recorded scene with points).

## Open (routed, not fixed here)

- Skill tree captions (REC-1439): in `a1-panel-skilltree` 1.14d draws (screen y, x about 675 = centre of W-sx-90 .. W-sx, Font16): `Skill` 85, `Choices` 97, `Remaining` 109 (strings 4227-4229), the number `0` at 140 (drawn with 0 points; the spec says only from 1), then `Cold`/`Spells` 216/228, `Lightning`/`Spells` 324/336, `Fire`/`Spells` 430/442 (strings 4249-4251, 4231). The Sorceress only; the other six classes need their string pairs (4232-4248, d2exp ids for Druid / Assassin): a spec revision (`panels-2.md` §19 r6, Open questions 2), then code. Owner: `claude/q-fix-pc1-client-ui`.
- Automap cells (843 vs 129 rows at the first TAB): automap owner (`specs/ui/automap.md` §3-§5, §10).
- Character: Fire Bolt `descdam` value, Defense 8 vs 2 (sim stat).
- Cursor: start position of the 1.14d pointer is not reproduced by d2rs (`mouse X Y` step missing in the shared input form) and the cursor animation phase differs (`panels-3.md` §23 r8): hides behind every scene; q-scenes-compare.
- Waypoint / stash scenes: re-run `--groups panels2,panels3` once walks match.
- Front end: no pixel check exists; needs `record_frames.py` front-end frames + a d2rs menu dump.
- Sounds: every scene still differs at call 0 (1.14d unit-0 calls first; d2rs 2599): known from run 2.

## Repro

```sh
export D2_GAME_DIR=$HOME/game
tools/cloud-game/prepare_scene_chars.sh   # SceSor etc. (once)
python3 tools/sidebyside/build.py --out /root/sbs/panels --groups panels,panels2,panels3 [--reuse]
```
Ledger rows: `docs/handoff/ledger/q-chk-render-ui.tsv` (12 `ui.scene.*` rows, validated with the integrator's `ledger.py --parts`).
Disk: the debug target alone is 9 GB; `rm -rf target/debug` before a new test build when the container's allowance is spent.
