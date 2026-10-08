# q-fe-start-flow: front-end game start and loading

Branch `claude/q-fe-start-flow`. New: `crates/d2-client/src/app/front_start.rs`, `app/loading_overlay.rs`, `tests/front_start.rs`. Small additive edits: `front_host.rs` (`FrontHost::with_front`), `play.rs` (`PlayConfig::start_flags`, `send_create_game_flags`, one `add_loading` call), `main.rs` (`play`, `play_once`). PROVISIONAL: REC-236 (HANDOFF §7).

## Links connected

- `registry(save_dir, &StartHandles)`: every screen, with character select reading the real Save folder and writing its choice to `selection`, and create writing to `created` (name-taken check on the folder). `play` builds the `FrontEnd` from it.
- `StartChoice::resolve(GameLoad, handles, save_dir)`: saved character (name, class, status, save path, difficulty from the box, Normal without the box) or the created one. `create_request()` = `create_request_for` with `flags = start_flags(status)` (4 | 0x800 hardcore | 0x100000 expansion). `character(data)`: the save with live data, else the named stand-in.
- `play_once` takes the choice: character, save path, hardcore, and the 0x67 flags (`PlayConfig::start_flags` → `send_create_game_flags`). Without a choice, behaviour is unchanged (`--new` / `--save` shortcuts).
- Dead hardcore: character select already shows message 5304 and stays (test `dead_hardcore_shows_5304_and_stays`).
- Loading: `LoadFeed` turns the model's act / in-game edges into `LoadEvent`s (game start is the first draw), `LoadingState` runs `LoadingScreen` (`Loading{frame}` → `Black` → `World{act}` once the local player has a room). `loading_overlay::add_loading` covers the game window in black with `loadingscreen.dc6` frame n (Loading palette) while it is active.

## Tests

`cargo test -p d2-client --test front_start --lib front_start`: saved character at Nightmare → 0x67 name, class, difficulty 1, flags 0x100004; hardcore flag 0x100804 and no-box start; dead hardcore 5304; loading events to the world.

## Left

- 0x61 act videos and a repeated 0x03 of the same act are not seen (REC-236).
- Start act is reported by `start_act`, not used to place the player.
- Loading art placement/size and the overlay on a real window are unverified.

## Local check

```
D2_GAME_DIR=... cargo run -p d2-client --release -- play
```
Single Player → pick a saved character with Nightmare open → Nightmare. The game window starts with that character's name (log line `play: front-end character <name>`, `play: difficulty 1`), after the loading art (frame advances) on black; a hardcore character starts hardcore. A dead hardcore character shows message 5304 and stays on select.
