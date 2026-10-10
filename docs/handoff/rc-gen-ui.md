# rc-gen-ui hand-back

Generator for the 174 `system.ui.*` ledger rows (NO-CHECK before: 174, EQUAL 0).

## Result
- New family `ui` in `tools/check-gen/check_gen.py` (`UI_SCENARIOS`): 13 checks `gen-ui-*` in
  `traces/checks/gen/`. Draws channel: input script (keys I, C, T, TAB, `, clicks) + `draws-at`;
  packets channel: belt use (key 1), ground clicks. `ledger-areas.tsv` gained the system.ui areas.
- Run with suite.py (orig-cache filled, not committed): draws 11 DIVERGED, packets 2 MATCH.
- Ledger part `docs/handoff/ledger/rc-gen-ui.tsv` (all 174 rows): EQUAL 0 -> 2
  (controls.6 `gen-ui-walkclick`, controls.7 `gen-ui-beltuse`), DIVERGED 77, NO-CHECK 95.

## Open
- One shared cause (size S, game code, not touched): in every panel scenario the first difference is
  tick 57-198, draw row 172-175 (`CelDraw` on 1.14d vs a unit row on d2rs; the rows before it equal).
  Likely the tail of the frame (cursor); find it with `gen-ui-hud`. Others: `gen-ui-wp` row 1
  FloorTileDraw frame 38 vs 34; `gen-ui-stash` row 100 CelDrawShadow frame 12 vs 7 (animation phase).
- Dropped scenarios: ESC menu (1.14d recording hung: ESC pauses it, no tick after), set-item tip
  (poke item quality 5: 1.14d wrote no frames), inventory grid click packets (state-dump refuses
  key I headless). Rows stay NO-CHECK (frontend-options o1/o2/o4 etc.).
- NO-CHECK with reason in `note`: front-end screens (menus, credits, loading, options: the
  frontend channel only runs the dolls script, ~M to add a screens script with a verdict), server
  text messages (chat, alerts, hire offers), automap persistence/party, text wrap/clipping/edit box.
- Many scenario rows are DIVERGED only because the shared whole-frame draw list diverges; once the
  shared cause is fixed they may turn EQUAL without further work (rerun suite --filter 'gen-ui-*').
