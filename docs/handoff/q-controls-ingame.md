# q-controls-ingame: Configure Controls over the game

Branch `claude/q-controls-ingame`. Spec `ui/frontend-options.md` §O9; provisional points in REC-256 (`docs/HANDOFF.md` §7). Everything is d2rs-own, unverified (rule 10).

## Links connected

| Link | Where |
|---|---|
| Esc → Options → Configure Controls sets the request; the host opens the screen | `OriginalUi::service_controls` (called from `world_view/present.rs`) |
| Screen model reused, not copied | `ui/front_end/screens/controls.rs` `ConfigureControls`, `load_table`, `save_table`, `table_to_bindings` |
| Draw over the game (Esc panel draws it instead of the menu), pointer events by §O9 r2 geometry | `ui/controls_host.rs`, `ui/esc_menu.rs` |
| Raw keys while open; game bindings get none | `OriginalUi::controls_key`, `present.rs` `ui_input` |
| Cancel / Accept: back to Options with Previous selected; Accept saves `controls.toml` and replaces the in-game bindings | `EscState::close_controls`, `OptionsMenu::return_from_controls`, `take_accepted_bindings` |
| Closing ui 9 any other way drops the screen | `OriginalUi::set_ui` |

## Test

`cargo nextest run -p d2-client --lib -E 'test(configure_controls)'`: request → screen open and drawn (Font 13 list, Cancel / Default / Accept); rebinding Inventory to C then Cancel leaves nothing accepted and writes no file; then Accept gives bindings with C for Toggle Inventory and writes the file.

## PROVISIONAL / left

REC-256: rectangles instead of art, English stand-in strings, click on press, expansion flag false, no mouse-button capture.

## Local check

```
cargo run -p d2-client --release -- play --new sorceress Tester
```
Esc → Options → Configure Controls: the key list appears over the game. Down/Up, Enter on a row, press C: the cell shows C. Accept returns to Options with Previous Menu selected; leave the menu and press C in game: the inventory toggles (and I no longer does if it was the only key). Cancel drops the edits. `<config dir>/d2rs/controls.toml` is written on Accept.
