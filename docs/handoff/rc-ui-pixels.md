# rc-ui-pixels hand-back (causes: ui-draws pixels, skill pick, help/msglog/automap)

Base: integ-r23 (merged at fa1715ff9). Suite `--filter 'ui-draws-*' --orig-cache`: draws MATCH 6 of 25
(char-skill, inv-char, inv-char-l5, inv-char-tip, quest-inv, quest-skill; before: 0). The other 19 stop
first at the rain `DrawLine` rows 195-199 (footstep / client-seed cause, not mine).

## Fixed

- Player light (also coordinator's `q-fix-pc1today-cast-light`): 1.14d keeps the unit's own light
  (`+0x64`, the player light) apart from the cast light in the stat list (`0x00643A00`); the player
  machine `0x00461250`, the client skill end and the 0x15 placement remove only the cast light
  (`client/model.md` §8 r4). d2rs removed any light of the unit, so the first mode request dropped the
  player light. Now `ClientWorld::cast_lights`; `bridge/modes.rs::remove_unit_light`. Belt tick 72:
  111005 -> 1637 differing pixels (rest: rain lines + ~1.3k px on torches / units, lighting the
  comparator can't see while d2rs writes `?` light cells). Spec: `render/lighting.md` §13 row.
- Light quality `q` replayed per drawn tick from the frame schedule's `quality` column (rc-ui-div's
  format); 1.14d drops `q` 2 -> 0 at about tick 16 under the debugger. `PreviewLight::set_quality`.
  Spec: `tools/scenario-diff.md` §3 r7 step 5, `render/lighting.md` §13.
- Key S (command 13, `0x00468B00`): SetUIState(3, toggle, 0) then `0x004A8CE0(0)`; d2rs ignored the
  action. `ui/original.rs`, test `s_toggles_the_right_skill_pick`.

## Next causes (seen past the rain rows with a diagnostic row diff, not the check)

- Skill pick list (skillbar, right/left-skill-pick, row 232): 1.14d draws the list before `level.dc6`,
  3 icons (frames 2, 6, 4) at y 514 from x 672 step -48; d2rs draws it after `level.dc6`, 6+ icons
  (frames 2, 6, 0, 14, 14, 18) at y 552 from x 635. Contents (which skills / scroll entries), row
  position and draw order of the skill-select list; spec `ui/control-panel.md` §7.
- Help screen (H, ui 0x21, row 217 DrawBox then ~580 more rows) and message log (M, ui 0x18, row 244
  DrawBox): command 6 is not handled in `ui/original.rs` (`hotkey_state` lacks 0x21), and neither
  screen has a spec or a drawer (listed under "Owners still to write" in `ui/panels.md` Open questions). Needs RE first.
- Automap (Tab, row 217): 1.14d `CelDrawClipped` maximap cels at (344, 228)...; d2rs `CelDraw` at
  (360, 260). `ui/automap.md`.

Ledger: no part written; the 6 MATCH checks above (fresh run, this branch) can settle their ui rows.
Tests: d2-client lib 2326+1 pass, d2-sim lib 4686 ok, clippy clean. Integration test binaries not built (disk).
