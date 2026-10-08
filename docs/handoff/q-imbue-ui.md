# q-imbue-ui: Charsi's imbue dialog in the play preview

Branch `claude/q-imbue-ui`. Nothing is verified against 1.14d (rule 10). Open point: REC-145 (`docs/HANDOFF.md` §7).

## Links connected

| Link | Where |
|---|---|
| Imbue row in Charsi's (class 154) NPC menu | `ui/npc_menu_ui.rs` (`NpcMenuState::open`, `OptionKind::Imbue` in `ui/layout.rs`) |
| Row release opens the imbue dialog (menu closes, `npc_menu_up()` stays true so the auto 0x30 stays off) | `npc_menu_ui.rs`, state `NpcMenuState::imbue` |
| Dialog draw / hit / events (place cursor item, imbue → C→S 0x38 `[0][NPC][item]`, close → 0x30) | `ui/imbue_ui.rs`, driven by `NpcMenuUi` (same panel 0x103; its rect shrinks to the dialog so the inventory stays clickable) |
| S→C 0x58 codes 1/5/6/7 close it, 4 leaves waiting | `OriginalUi::imbue_output`, called from `msg_ui.rs` |

Tests: `tests/e2e_imbue.rs::menu_click_to_imbue_done` (menu click → row → 0x38 → server → S→C 0x58 `[npc][6]`, the buckler consumed, dialog closed by the 0x58); unit tests in `imbue_ui.rs`.

## PROVISIONAL (REC-145)

- Imbue row always shown; plain-text dialog, no art/cels/hover/note box/item cel; opened from the menu, not by a server 0x58 code 0.
- The e2e fixture's client model has no item stream, so the placing click is covered by the unit test and the e2e uses the `imbue_place` seam.
- 0x58 codes 6/7 close the dialog but the client does not send the 0x30 `EndInteractionFull` (the preview's chat-close handles it).

## Left

- Charsi and the Malus in the play world (see `q-a1-malus.md`); the quest text needs strings.
- The inventory is not opened automatically with the dialog: open it (`I`) to pick up the item.

## The user's local check

```
cargo test -p d2-client --test e2e_imbue
D2_GAME_DIR=<install> cargo run -p d2-client -- play --new amazon Test
```
In `play`, click Charsi: the box shows Talk / Trade / Imbue / Cancel. Imbue opens the dialog; open the inventory, pick up an item, click the item area, then the imbue button: C→S 0x38 is logged and the server answers 0x58 (result 7 unless the Tools of the Trade reward is pending).
