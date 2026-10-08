# q-a1-cain: Act I "Search for Cain" glue and Cain's identify

Branch `claude/q-a1-cain`. Nothing is verified against 1.14d (rule 10). Open point: REC-124 (`docs/HANDOFF.md` §7).

## The path, with the links that were missing

The quest rules (Akara's scroll, Cairn stones, tree, gibbet, Cain portal, town Cain; `world/quests/act1/q4.rs`), the object module's quest routes (`wiring/economy/quest_objects.rs`) and the host lending (`WiredWorld::lend_quests`) already ran on the wired host. What fell through to `AppRest` (logged, returned nothing):

| Quest call | Used by | Before | Now |
|---|---|---|---|
| `spawn_monster` / `spawn_monster_flags` | town Cain, Tristram Cain (gibbet event 7) | rest: log, `None` | `HostQuests::spawn_unit`: `View::allocate` of a monster in a DRLG room (NPC classes allied) |
| `open_portal` | red portal to Tristram (Cairn stone timer), Cain's way out | rest: log, `None` | portal object (mode 1) in a free spot, `interact` = level, owner GUID |
| `find_object_near` | stone operate (§10.6) | rest: `None` | first object of the class in the object's act |
| `create_object` | marker / portal objects | rest: log | `View::create_object` |
| `remove_monster` | Cain leaves Tristram | rest: log | S→C 0x0A to the players, `View::remove` |
| Cain identify C→S 0x34 | Cain NPC menu | `AppRest::identify` logged; `inventory_entries` empty | the NPC call stages each player's entries from the inventory model (`InvDesk::npc_entries`), the rest records Cain's `identify`, the host applies it (`InvDesk::identify_unit`: flag 0x10, 0x9D 0x15 update) |

Tests (synthetic): `d2-client/tests/app_cain_quest.rs` (spawn, portal, lookup, removal on the built play game; nothing reaches the rest's log), `wiring::inventory::tests::identify::cain_entries_and_identify_unit`, `app::rest::tests`.

## PROVISIONAL (REC-124)

- Removal is immediate (no removal-mode animation); spread and spawn flags are not applied; the object search covers the act, not the room list; `open_portal`'s body is unwritten, so the town portal's owner/level fields are reused.
- Cain's identify effect is REC-113's.

## What is left

- No end-to-end test of C→S 0x34 through the server glue (`WorldHost::npc` in `wired.rs`): the synthetic game has no item tables or Cain NPC, and the existing e2e fixtures use their own rest. The pieces are tested separately.
- The synthetic world has no Stony Field, stones, tree or gibbet; the chain (Akara's scroll, stones in order, portal, gibbet, rescue) runs only on a live install.
- The Tristram level and its Cain come from the live DRLG; nothing is checked against a recording.

## The user's local check (game files)

```
D2_GAME_DIR=<install> cargo run -p d2-client --release -- play --new amazon Test
```
Cain identify: walk to Deckard Cain (needs the town Cain; he appears in the Rogue Encampment after the rescue), left-click, choose Identify in the NPC menu (`q-npc-menu`) with an unidentified item in the backpack: gold drops by 100 per item, the item's tip shows its properties. Quest chain: finish the Den of Evil, talk to Akara (message 97 starts the quest and gives the scroll), click the stones in the Stony Field in the order the log prints, click the red portal, free Cain from the gibbet in Tristram. Send log lines with `quest`, `portal`, `unhandled` and say where it stopped.
