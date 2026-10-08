# q-menu-host: the front-end Bevy host

Branch `claude/q-menu-host`. Module `d2-client::app::front_host`; `main.rs` `play` loop. PROVISIONAL points: REC-231 (HANDOFF §7).

## Links connected

- `FrontHost` (non-send resource, screens are not `Send`) wraps `FrontEnd`: `feed_input` (window cursor scaled to 800×600, left button, `KeyboardInput` → VK + UTF-16 chars) → `drive` (frame time → whole 40 ms ticks) → `draw` (`FrontEnd::draw()` → DC6/sky-palette CPU image on a sprite) → `finish` (outcome → `AppExit`).
- `main.rs`: `play` with no `--new` / `--save` / `--frames` runs `run_front_end`, then `play_once` (the old body) on `Outcome::GameLoad` (menu difficulty applied), then back to the main menu (`start(false)`); `Outcome::Exit` ends the program. The shortcuts skip the front end.
- Saves-found = `*.d2s` in `--save-dir` / default save dir.

## PROVISIONAL / left

- Text is not drawn (a bar per text item); no glyph path from `DrawItem::Text` yet.
- Game starts with the default character: select/create screens report no choice yet.
- Game window is a second Bevy app in the same process: unverified on the user's OS.
- Art: frame `frame` of the DC6 at the bottom-left; multi-frame backgrounds (tiled) not stitched.

## Test / local check

`cargo test -p d2-client --test front_host --test front_end` (headless, no game files: trademark → main menu → game start; Esc exits; key and cursor mapping).

Local, with game files: `D2_GAME_DIR=... cargo run -p d2-client --release -- play`. Expect an 800×600 window with the trademark art, a click/Enter to the main menu, and (Placeholder screens) bars where text goes; closing the game window returns to the menu.
