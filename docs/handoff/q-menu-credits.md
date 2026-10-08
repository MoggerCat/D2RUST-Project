# q-menu-credits: credits screen

Branch `claude/q-menu-credits`. File `crates/d2-client/src/ui/front_end/screens/credits.rs` (+ `tests/front_end_credits.rs`). PROVISIONAL points: REC-185 (HANDOFF §7).

## Connected
- Decode / parse / scroll / layout (C3, C4) as pure functions; `CreditsScreen` builds the controls (background 41, EXIT 42 with Esc, text columns 43–45 / 47–49 as descriptors), steps the scroll each 40 ms tick, never returns by itself. EXIT / Esc → `Trigger::Exit` → main menu (existing flow row). Click on picture, Space, Enter do nothing.
- Text source is injectable (`register_with`); default reads loose files under `D2_GAME_DIR`.
- Tests: spec vectors (row 50 baseline 588 / 572 / 571 at draw 1 / 9 / 10, pair x 400 / 410, centred x 280, heading/pair/stop lines) and the flow tests.

## Left
- The host must draw `CreditsScreen::visible_rows` (the shared draw list has no per-tick text), with real FontFormal10 advances; read the text from the MPQ; no host window runs the front end yet.
- Check row counts on the shipped files (C rows 1,231 classic / 1,797 expansion).

## Local check
`cargo nextest run -p d2-client --lib credits; cargo nextest run -p d2-client --test front_end_credits` — all pass.
