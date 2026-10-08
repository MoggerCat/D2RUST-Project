# q-sockets: sockets, gems, runes, runewords

## Links connected
- Server (`d2-sim` `wiring/inventory`): the 0x28 SocketItem handler (`items::moves::socket_item`) was already there but three desk seams answered "no" by default, so every insert was refused. Now: `ops.rs` `link_into_item` creates the target's inventory and links the filler (mode 6); `units.rs` `socket_filler` (gem / rune / jewel types); `inv_world.rs` `socket_filled` (own inventory not empty, drives item flag 0x1 and the grid rules).
- `queries.rs` `apply_filler_properties`: after the gem / rune properties land in the filler's `ListKey::ITEM` list, that list is attached to the target with `StatLists::equip` (`properties.md` §9 rule 4). The runeword step (`socket_runeword`, runes table) already ran and is now reached.
- Client: `ui/panels/inv_items_socket.rs` (new) decides the cursor socket (`SocketFacts`, `can_socket`) from the model (header flags, socket count of the target record, fillers listed as mode 6 under the target) and builds C→S 0x28. `inv_items.rs` sets `cursor_can_socket`, maps `GridMsg::Socket` to the intent, and sends 0x28 over a worn item. `inv_items_tip.rs` appends each filler's name and property lines to the target's tip. `item_tip.rs` got `bits` and `is_socket_filler`.
- The server already sends the fillers as 0x9D action 0x13 owned by the target; the model keeps them with `owner = target` (`bridge/items.rs` `mode::SOCKETED` added).

## PROVISIONAL (REC-121)
- Filler test `0x0062BEB0` (gem / rune / jewel), the tip layout of filled sockets, and the client reading `sockets` from the target's full record.

## What's left
- Stats of a socketed gem reaching a wearer depend on the equip wiring attaching the target's list to the player (`StatLists::equip` has no caller outside `apply_filler_properties`); not this task.
- Larzuk / Horadric socketing and the cube recipes are other code paths.
- Items dropped in the preview rarely have sockets; use the test items or a save with a socketed item.

## Local check
`D2_GAME_DIR=<game> cargo run -p d2-client -- play --new <class> <name>` with a save or drop that has a socketed (white/blue) item and a gem or rune: pick the gem up, click the socketed item. The gem leaves the cursor, the item's tip lists the gem's name and bonus, and a runeword shows once the last rune goes in (name from the runes table).
Tests: `cargo nextest run -p d2-sim wiring::inventory::tests::socket`, `cargo nextest run -p d2-client inv_items_socket` (4 + 3 synthetic).
