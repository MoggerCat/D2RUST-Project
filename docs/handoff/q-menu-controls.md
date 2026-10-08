# q-menu-controls: Configure Controls screen

Branch `claude/q-menu-controls`. Spec `ui/frontend-options.md` §O9; provisional points in REC-231 (`docs/HANDOFF.md` §7).

## Links connected

| Link | Where |
|---|---|
| Screen state: 15 rows from `menu_table` (62 exp / 51 classic), selected row, column One initial, latch, edit, Delete, Esc/Space = Cancel, Left/Right, Down/Up skipping separators, wheel −2n rows clamped | `screens/controls.rs` `ConfigureControls` |
| Two-key rule, allowed keys, error 3978/3979 for 2 s | reuses `controls::original::assign_key` |
| Default / Accept / Cancel (snapshot restore) | `default_all`, `accept`, `cancel` |
| §O9 r2 layout with colours, blinking edit cell, textslid scroll bar | `draw_list` |
| Shell adapter: a key-only control per virtual key, click-to-edit key cells (r4 row rule), 3 buttons, scroll arrows | `ControlsScreen` |
| Flow: `CONTROLS` + Exit/Ok → `OPTIONS` | `flow.rs` (one arm) |
| Persistence: Accept → `controls.toml` (`<config dir>/d2rs/`, `D2RS_CONFIG_DIR` overrides), reloaded on entry | `save_table`, `load_table` |
| Play input uses the saved file | `app/ui.rs`: `saved_bindings()` before the `dev` preset |

## Tests

`cargo nextest run -p d2-client --test front_end_controls`: Inventory ← C clears Character slot 0; Cancel restores; Accept persists and reloads; Esc/Enter latch; error message window; layout numbers (headings 108/298/488, buttons 193/399/605); blink 5 of 16 draws hidden; flow arms.

## Left

- The host must draw `ControlsScreen::model().borrow_mut().draw_list(..)` (font 13) and feed wheel / mouse-middle / key-up events (REC-231).
- Options needs an arm into `CONTROLS` (the Options session owns it).

## Local check

```
cargo nextest run -p d2-client --test front_end_controls
```
Expect 13 passes. Nothing visible in `play` until the front-end host exists; after Accept, `~/.config/d2rs/controls.toml` holds `[bindings]`, and `play` reads it.
