# q-chest-drops: opened chests drop items the client hears (`claude/q-chest-drops`)

Nothing here is verified against 1.14d (rule 10). Open point: REC-260 (`docs/HANDOFF.md` §7). No audio.

## Links connected

| Link | Where |
|---|---|
| Chest drop (`object_treasure`, chest operate, drop helpers) found no spot (`NoSpot`) | `StartSpot` in `wiring/action/objects.rs`, `wiring/economy/quest_host.rs` (as the monster drops, REC-108) |
| Ground items were never sent to the client | `update_pass` (`items/moves.rs`) announces each ground item of the client's adjacent rooms once (0x9C, `announce_item`), tracked in `SimGame::announced_ground` |
| Synthetic chest had no treasure class | `synthetic_items::drop_tables`: chest table entry names a class picking the hammer row |

Test: `crates/d2-client/tests/app_play_objects.rs` `clicking_an_object_walks_to_it_and_operates_it` (now ends with: one ground item `hfh ` in the client model). It failed before (0 ground items). The test uses `DEFAULT_SEED + 1`: with the default seed the chest's 25% empty roll hits.

## What is left
- No collision search: the item lands on the chest's start offset; several picks stack on one cell.
- Monster drops are now announced by the same path (not separately tested here).

## The user's local check
```
cargo nextest run -p d2-client --test app_play_objects
cargo run -p d2-client --release -- play --new amazon Test
```
Open a chest: its items appear on the floor next to it (the 0x9C line in the log); click one to pick it up.
