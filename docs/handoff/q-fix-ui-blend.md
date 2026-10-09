# q-fix-ui-blend

## Done
- Cause (from q-fix-realdata-baseline): UI rectangles of draw mode 2 need the
  act's blend tables (`render/blend-modes.md` §1, §8 r2); only the play
  preview's tile feed pushed them, and `preview.rs` then overwrote
  `assets.shades` with `None` when its tiles held no PL2.
- Fix: `app/ui.rs` `push_text_colors` pushes `ShadeTables` of the model's
  palette act once per act (`TextColorMaps.shade_by_act`) into
  `assets.shades`; `world_view/preview.rs` only replaces `assets.shades`
  when its tiles hold tables.
- Proof (D2_GAME_DIR=/root/game): smoke_frontend x5, app_input_pass x4,
  app_play_640, app_play_e2e, app_play_visibility, app_levelup_ui now pass.
  `cargo nextest run -p d2-client`: 2532 passed.

## Open (not blend-related; the cause differs from the baseline note)
- `app_frame_loop frame_loop_ticks_the_server_and_feeds_the_world_view`:
  message log is (108, 18) handled/queued, the test's comment chain expects
  (76, 6): the expectation chain has drifted from the join sequence.
- `app_hud_e2e the_play_hud_draws_model_values_and_takes_clicks`: no
  `d2rs\hudfill` stamina bar (frame 1, 273,573,18) in the UI draws; the
  rest of the HUD draws. Owner: client UI/HUD.
- No side-by-side UI scene exists for the front end, so no pixel-compare run.

## Repro
```
D2_GAME_DIR=/root/game cargo nextest run -p d2-client --run-ignored only \
  -E 'binary(/^(smoke_frontend|app_input_pass|app_play_640|app_play_e2e|app_play_visibility|app_hud_e2e|app_levelup_ui|app_frame_loop)$/)'
python3 tools/coord/realdata.py
```
