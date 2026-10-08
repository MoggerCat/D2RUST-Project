# q-a2-town: Lut Gholein in the play preview

Branch `claude/q-a2-town`. Nothing is verified against 1.14d (rule 10). Open point: REC-136 (`docs/HANDOFF.md` §7).

## Links connected

| Link | Where |
|---|---|
| Synthetic Act II town room holds the NPCs (Warriv, Atma, Drognan, Fara, Greiz, Jerhyn, Elzix, Lysander, Meshif) and a waypoint object | `app/single_player.rs` (`ACT2_NPCS`, `ACT2_NPC_X0/Y`, `ACT2_WAYPOINT_XY`, allocation next to Akara's) |
| Their `monstats` rows (`npc` + `interact`) so the server's `interact_classes` takes them; the client's unit rows so the 0xAC creates the units | `synthetic_monstats`, `synthetic_unit_rows` (one list, `synthetic_npc_classes`) |
| Greiz's hire list | one synthetic `hireling` row in `GameParts::synthetic`; without it his interaction ended in `NoHirelingRow` and no 0x27 / 0x28 came |
| The NPC menus (Talk / Trade / Gamble / Heal / Hire / Travel) | existing `npc-menus.tsv` path of `q-npc-menu` (`ui/npc_menu_ui.rs`); nothing new |
| Act change into Lut Gholein | existing (`q-act-travel`) |

Test: `tests/app_a2_town.rs` (`lut_gholeins_npcs_and_waypoint_are_there_and_talk`): act change by the hook queue, the waypoint and all nine NPCs are in the client model, each NPC (one fresh game each) answers C→S 0x13 with S→C 0x27 / 0x29 / 0x28. It fails before the change (no NPC of Act II in the model).

## PROVISIONAL / left

- REC-136: positions are d2rs-own; real ones come from the town presets.
- Harem / palace entrances: not built (the synthetic DRLG has no such levels or warp rows).
- A second chat in one headless game does not start after a raw C→S 0x30 without the UI's 0x2F / 0x31 before it (the Akara tests go through the UI); the test gives each NPC its own game. Not investigated further.
- Vendor stock needs game files (`q-vendor-items`).

## The user's local check (game files)

```
D2_GAME_DIR=<install> cargo run -p d2-client --release -- play --new sorceress Test
```
Take Warriv's "Go East" (or use a save that has Lut Gholein), then click each NPC: expect the menu box (Talk / Trade / Heal / Gamble / Hire as the class allows) and no `rejected` lines. With game files the NPCs stand where the town presets put them, not in a row; record any that are missing or silent.
