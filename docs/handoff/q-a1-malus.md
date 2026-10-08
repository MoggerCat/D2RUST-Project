# q-a1-malus: Tools of the Trade, the imbue item flow

Branch `claude/q-a1-malus`. Nothing is verified against 1.14d (rule 10). Open point: REC-131 (`docs/HANDOFF.md` §7).

## The path, and which links are connected

| Link | State | Where |
|---|---|---|
| Malus object init / operate, the drop of `hdm `, the party/leave/return rules, reward pending bit | already wired and tested on the real host (fake rest) | `d2-sim` `world/quests/act1/q3.rs`, `wiring/economy/quest_objects.rs`, `d2-server .../tests/quests_act1.rs` |
| Returning the Malus (Charsi's quest message deletes `hdm `, sets reward pending) | already in the quest rules | `q3.rs` |
| C→S 0x38 (imbue) → `item_service` | reached, but `cursor_item` / `item_facts` / `remove_cursor_item` / `create_imbued` / `place_or_drop` were the rest's refusals | – |
| **Imbue on the real inventory** | **connected**: `Desk::inv` (`NpcInventory`, `NpcInv` on an `InvDesk`) is lent by `WiredWorld::npc` for the NPC call; facts from the item store and the item tables; create on the economy; the cursor item removed and freed; the new item placed (§2.4) | `d2-sim` `wiring/interaction/npc_items.rs` (new), `npc_world.rs` (4 delegations), `mod.rs` (the field); `d2-server` `wired.rs` (`desk_with`, `npc`) |
| Quest hook after the imbue (`imbue_granted`) | already called by the service; the e2e test now checks the pending bit is cleared | `services.rs` |

Test (synthetic, fails before): `crates/d2-client/tests/e2e_imbue.rs` — the bridge on the local link: pick up a buckler (C→S 0x19), C→S 0x38 action 0 with its GUID at Charsi. With the reward bit set: S→C 0x58 `[npc][6]`, the buckler unit is gone, a new item of the same record at item level base + 4 is in the backpack, no errors, the pending bit cleared. Without the bit: 0x58 result 7. (The synthetic item tables carry no rare data, so quality 6 downgrades in the test.)

## PROVISIONAL (REC-131)

See `docs/HANDOFF.md` §7. The imbued item copies only the record; affixes/durability/sockets of the input are not read.

## What is left (not done here)

- **Client imbue dialog** (UI state 0x0E, `specs/ui/messages.md` §11): the state machine exists (`ui/messages/socket.rs`), but no panel draws it, and no menu row opens it for Charsi, so the preview cannot yet send the 0x38. The server accepts it now.
- **Charsi and the Malus object in the play world**: `play` places no NPC but the synthetic Akara; with game files the Barracks (the maze level, q-act1-dungeons) carries the Malus stand from its DS1 objects; neither was run here (no game files).
- Dropping the imbued item when the backpack is full (ground drop), the personalized name copy, `put_back`.
- Charsi's quest text (needs strings, q-strings).

## The user's local check

```
cargo test -p d2-client --test e2e_imbue
D2_GAME_DIR=<install> cargo run -p d2-client -- play --new amazon Test
```
The test passes without game files. In `play` the Barracks stand can be reached with a character of level 8+; operating it should drop the Malus (look for `drop item` / the item on the ground). The imbue itself cannot be driven from the client yet (see above).
