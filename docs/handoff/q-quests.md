# q-quests: quest log and Den of Evil in the play preview

Branch `claude/q-quests`. Nothing is verified against 1.14d (rule 10). Open points: REC-104, REC-105 (`docs/HANDOFF.md` §7).

## Links connected

| Link | Where |
|---|---|
| Click Akara → C→S 0x13 → server NPC start (S→C 0x27 with the quest text list, 0x29, 0x28) | `app/rest.rs` (real NPC/quest seams), `app/npc_seams.rs` (position snapshot written by `sync_seams`, 0x27 list encoder), `WiredWorld::interact_classes` (`d2-server .../world/wired.rs`) |
| Synthetic town NPC Akara (monstats rows, unit path) | `app/single_player.rs` (`synthetic_monstats`, `AKARA_X`) |
| Client 0x28 → dialog branch → C→S 0x2F + 0x31 (message from the list), then the chat close C→S 0x30 | existing `msg_ui`/`bridge`; `bridge/chat_end.rs` + `present.rs` `deliver_with` (play preview only) |
| Talk 1 = Akara's introduction (msg 12), talk 2 = Den of Evil (msg 64, quest started) | `tests/app_play_quests.rs` |
| Q → ui 0x0F + C→S 0x40 → S→C 0x28/0x50/0x52 → rows | `ui/quest_log_ui.rs` (panel, inputs sync), `ui/original.rs`, `ui/msg_ui.rs` (one wrapper) |

Den clear and the reward (+1 skill point, message 76) already run on the real host in the server tests (`quests_act1.rs`); the play world has no level-8 monsters, so it can't be driven from the client yet. The NPC menu (Talk/Trade/Leave) is not built (stitch-npc2 item 1); the quest path doesn't need it.

## Tests
`cargo test -p d2-client --test app_play_quests`, `cargo test -p d2-client --lib quest_log_ui`, `npc_seams`. Existing tests adjusted for Akara: `app_frame_loop.rs`, `app_single_player.rs`.
d2-server: three game-file-style tests (`world_data_tables` and two others) fail here with a string.tbl I/O error; not touched by this change.

## Local check
```
D2_GAME_DIR=<install> cargo run -p d2-client -- play --new amazon Test
```
Click Akara (town): she is talked to twice in a row (click again after the first greeting); press Q: the Act I rows show, the Den of Evil in progress. Text shows only with strings bound (q-strings).
