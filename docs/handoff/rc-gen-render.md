# rc-gen-render (REC-2560..2569 unused)

Generator `render` family in `tools/check-gen/check_gen.py`: 7 draws-channel scenes
(`gen-render-*`, traces/checks/gen/), each naming the system.render rows it reaches.

## Checks before/after
EQUAL 0 -> 0. 59 of the 89 rows now have a check; all 7 checks DIVERGED (draw list, whole frame):
| check | match | first difference |
|---|---|---|
| town-dawn / town-night | 72.4% | tick 38 row 175: 1.14d CelDraw vs d2rs unit draw |
| blood-moor | 56.8% | tick 57 row 98 CelDrawShadow file (player leg shadow) |
| firebolt | 61.4% | tick 22 row 98, same |
| frozen | 71.8% | tick 27 row 108 shadow dir 46 vs 0 |
| den-of-evil | 48.3% | tick 57 row 70 shadow frame 3 vs 1 |
| kurast-rain | 55.3% | tick 57 row 97 CelDraw frame 8 vs 0 |

## Changed
check_gen.py (family, area join), 7 check files, ledger part rc-gen-render.tsv (59 DIVERGED, 30 NO-CHECK with reason).

## Open (causes for others; no game code touched)
- Player leg shadow file/dir differs in outdoor/frozen scenes (gear/part selection or direction), render/unit-composite owner.
- Town row 175: d2rs draws a unit where 1.14d draws a CelDraw (object/NPC draw path).
- Kurast frame 8 vs 0 (animation start frame), den-of-evil shadow frame.
- Verdicts are whole-frame: a row is EQUAL only after the first difference is fixed; rows are not individually isolated.
- NO-CHECK (30): capture.*, map-preview.*, composition.1/2/7, camera.8/10, shading.5, d2rs-answers sections.
- Not run: fmt/clippy/nextest/coverage/spec_index (Python-only change besides ledger).
- Time-of-day periods and warp ids (8, 75) are unverified guesses; scenes ran but may not show the intended state.
