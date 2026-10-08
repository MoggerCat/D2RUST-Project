# q-menu-options: Esc options menu

Branch `claude/q-menu-options`. Spec `ui/frontend-options.md` §O1–§O8; provisional points in REC-187 (`docs/HANDOFF.md` §7). Everything is d2rs-own, unverified (rule 10).

## Links connected

| Link | Where |
|---|---|
| Tree: Game → Options → Sound / Video / Automap, Previous rows, rows, kinds, enabled tests (O8 stubs disabled) | `ui/options_menu.rs` |
| Opens on Return to Game; entering a menu selects its last row | `OptionsMenu::open`, `go` |
| 7-entry input: Down/Up wrap past titles and disabled rows, Left/Right wrap (choice) or clamp (slider), Enter, click on release, hover, drag with latch | `key_*`, `press`, `release`, `moved`, `drag` |
| Row-under-pointer rule (y only, `y ≥ y_top + n`), strict bounds | `row_at` |
| Slider math: position ↔ value (Sound/Music/Bias 5p, Gamma 55 + 10p, Contrast p 99 ↔ 100), drag formula | `pos_from_value`, `value_from_pos` |
| Draw at the §O4 positions (800 × 600): label left, value right, slider bar + knob, pentagram squares | `ui/esc_menu.rs` `EscMenuUi::draw` |
| Keys → panel: arrows and Enter as `UiEvent::Char` | `ui/edge.rs` `key_chars` |
| Save and Exit Game, Return to Game (closes the whole menu); Esc anywhere closes the whole menu | `esc_menu.rs`, `OriginalUi::game_menu_key` |
| Settings: every O7 key in `settings.toml` (`[audio]`, `[video]`, `[automap]`), strict ranges, written on each change | `app/config.rs` `INT_KEYS`, `parse_settings`, `write_settings` (host `apply_settings` unchanged) |
| Configure Controls arm | `OriginalUi::take_controls_request`; `flow.rs` `OPTIONS` + `Trigger::ConfigureControls` → `CONTROLS` |

## Tests

`cargo nextest run -p d2-client -E 'test(options_menu) | test(the_menu_tree) | test(config) | test(front_end_controls)'`: spec vectors for navigation, row tops, slider keys, drag x 352/400/617/618, strict press bound, Sound-menu pointer rows, Gamma 155 → 165, Contrast 100 → 98, light quality wrap, fade cycle; a Window Mode change round-trips through `write_settings` / `parse_settings`; the old Esc-menu test was rewritten for the tree (its Options page, three-row "Options / Resolution / Window Mode / Controls" list, no longer exists: the spec's Options menu replaces it).

## PROVISIONAL / left

REC-187: text instead of DC6 labels, bar, skull and pentspin art; extra Window Mode row in Video; Gamma / Contrast / Sound / Music values are stored only. Light Quality, Blended Shadows and the Automap rows are stored here but their effects (render lighting, shadows, automap) are not rewired by this row. Nothing opens the CONTROLS screen in play yet (no in-game host): `controls_requested` is set and flow has the arm. Sound is deferred (the sound names are only `CursorPass` / `CursorSelect` events; Select plays the existing click sound).

## Local check

```
cargo run -p d2-client --release -- play --new sorceress Tester
```
Esc: Options / Save and Exit Game / Return to Game, selected row coloured (blue), gold squares either side; Up/Down/Enter and the mouse work. Enter on Options → Video Options → Left/Right on Light Quality, drag the Gamma knob, Window Mode changes the window. Check `<config dir>/d2rs/settings.toml` holds `[audio]`, `[video]` and `[automap]` with the values (Gamma 55 + 10p). Esc inside a sub-menu closes the whole menu.
