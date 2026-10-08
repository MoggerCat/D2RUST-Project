# q-esc-menu: the Esc game menu in `play`

Branch `claude/q-esc-menu`. Everything is d2rs-own, unverified (rule 10).

## Links connected

| Link | Before | Now |
|---|---|---|
| Esc → action | `Preset::Dev` binds Esc to `GameMenu`; nothing took the action | `OriginalUi::after_event` (`ui/original.rs`) handles the unhandled `GameMenu` action by `controls.md` §3 row 56: menu open → close; else any open panel → close them (jump 1); else open ui 9 (gate: no player refuses, dead player respawns) |
| ui 9 panel | no panel | `ui/esc_menu.rs` `EscMenuUi`, installed last (top-most, full-frame rect: modal, takes every click): dark box, "Options", "Save and Exit Game", "Return to Game" (hover colour 3) |
| Return | – | closes ui 9 |
| Save and Exit | – | sets a flag; `OriginalUi::take_exit_request` is read in `world_view/present.rs` and calls `save::request_save_and_exit` (play saves on the way out) |
| Mini panel "Game Menu" button | `MiniAction::GameMenu` ignored | opens ui 9 (`ui/hud.rs`) |
| World click while open | `game_menu_open: false` | `ClickView.game_menu_open` = ui 9 open (`present.rs`) |

## PROVISIONAL (REC-QESC-1 in HANDOFF §7)

Art (dark tiles of the synthetic fill file, frame 4), layout, English strings, Options = no-op, the set of panels Esc closes.

## Left

Options screen; real art/strings once specified; Esc handling while the chat box is open.

## Local check

```
cargo run -p d2-client --release -- play --new sorceress Tester
```
Press Esc: a dark box with three entries appears; Esc again or "Return to Game" closes it. Open the inventory (I), press Esc: the inventory closes, the menu does not open. Click "Save and Exit Game": the window closes and `play: saved the character to ...` prints.
