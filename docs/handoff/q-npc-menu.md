# q-npc-menu: NPC menu box in the play preview

Branch `claude/q-npc-menu`. Nothing is verified against 1.14d (rule 10). Open point: REC-123 (`docs/HANDOFF.md` §7).

## Links connected

| Link | Where |
|---|---|
| S→C 0x28 delivered → NPC menu opens from the class's `npc-menus.tsv` record (reset + builder edits) | `ui/msg_ui.rs::npc_dialog` → `ui/npc_menu_ui.rs` (`OriginalUi::open_npc_menu`) |
| Rows drawn, row release | `ui/npc_menu_ui.rs` (`NpcMenuUi`, panel 0x103, installed in `ui/original.rs`) |
| Talk: shows the 0x27 text list's kind-0 strings | `Open::speech`, `talking` |
| Trade → C→S 0x38 action 1 `[1][GUID][0]` (the vendor panel is `q-vendor-items`' `open_shop` seam) | `panels/npc.rs::option_intent` |
| Gamble / Identify / travel → their `option_intent` (0x38 action 2, 0x34, ...) | same |
| Hire → hire list (`open_hire_list` seam); the 0x4F auto-open is cleared when the menu opens | `npc_menu_ui.rs` |
| Cancel row or a press outside → C→S 0x30; the preview's automatic 0x30 is off while the menu is up | `world_view/present.rs` |

Tests: `tests/app_play_quests.rs` (`akaras_menu_offers_talk_trade_and_cancel`, `leaving_the_menu_sends_the_chat_end`; `talk_to_akara` now leaves through Cancel).

## PROVISIONAL / left

- REC-123: fixed anchor, plain text rows, no box art/selection colour, no NPC name line, Talk has no scroll widget and sends nothing.
- A press outside the box closes it and is consumed (the click does not also walk).
- Cain's identify (C→S 0x34, `AppRest::identify` only logs): not done; the rest has no inventory access (`docs/handoff/q-identify.md`).
- Trade shows no panel until `q-vendor-items` lands.

## Local check
```
D2_GAME_DIR=<install> cargo run -p d2-client -- play --new amazon Test
```
Click Akara: the box shows Talk / Trade / (cancel). Talk shows her text (needs strings bound); Trade logs C→S 0x38; cancel or a click elsewhere ends the chat.
