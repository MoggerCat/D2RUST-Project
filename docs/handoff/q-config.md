# q-config: settings and key bindings files

Links connected (all `d2rs-own, unverified`; REC-172 in `docs/HANDOFF.md` §7):
- `app/config.rs`: strict `settings.toml` (`version`, `[video] resolution`, `window_mode`), atomic write, `controls.toml` created from the `dev` preset when missing and read back otherwise; folder = parent of the save folder.
- `app/play.rs`: loads both before the window opens (a bad file stops with the file and key named), builds the window from them, registers `config::apply_settings`.
- `apply_settings` (Update): hands the settings and the loaded key bindings to the UI once; on each Options change writes `settings.toml` and sets window mode / size.
- `ui/esc_menu.rs`: Options opens a page (Resolution, Window Mode, Controls, Previous); `OriginalUi::set_settings` / `take_settings_change`.
- Test changed: `original_tests` Esc-menu test no longer expects Options to do nothing; it now walks the page (REC-172 says Options opens it).

PROVISIONAL: dev preset instead of the original defaults; page art/layout; window-mode row; Controls row is a label only (no Configure Controls screen).
Left: Configure Controls screen, `Preset::Original`, sound / light / automap rows.

Local check: `cargo run -p d2-client -- play --new Amazon Test`; Esc, Options, click Window Mode (the window goes borderless) and Resolution; check `Documents/d2rs/settings.toml` and `controls.toml` exist; edit `controls.toml` (e.g. unbind `swap_weapons`) and restart: the key stops working; put `foo = 1` into settings.toml: play refuses to start.
