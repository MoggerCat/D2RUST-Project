# q-save-full: items, skills and waypoints in the played character's save

Branch `claude/q-save-full`. Everything here is d2rs-own, unverified (rule 10).

## Links connected

| # | Link | Where |
|---|---|---|
| 1 | Save: player items → save entries (`SaveItems` over an `InvDesk`, `item_list`, `write_save`) | `WiredWorld::save_items` (`d2-server` `handlers/world/item_save.rs`); `SaveItems::stream` now carries unit +0x28 (the save-only 32 bits, as `InvDesk::save_view`) |
| 2 | Save: skill levels (class-list order) and the three waypoint records | `d2-client` `app/save_full.rs` `read_extra`, laid over the body by `apply_extra` (called from `save::apply_live`) |
| 3 | Load: waypoints | `ActionCharacter::set_waypoints` (was unapplied) |
| 4 | Load: save entries → item units placed in the inventory | `InvDesk::load_entry` (`d2-sim` `wiring/inventory/load.rs`: `item_from_record`, fillers through `insert_filler`, placement), `WiredWorld::load_items`, called from the client loader (`save_full::join_items`); the placements' 0x9C / 0x9D go to the join's item messages like the start items' |
| 5 | The copy's child-socketing was extracted to `InvDesk::insert_filler` and shared by the copy and the load | `wiring/inventory/copy.rs` |

Skills already loaded (`add_skill_level`); only the save half was missing. Cube
items are in the player's list (page 3), the stash page 4: they ride with the
inventory. Mouse skills and the act byte are NOT done (see below).

## Tests (synthetic)

- `d2-sim` `wiring::inventory::tests::load`: a stored item returns to its cell with the load's flags; a taken cell falls back to a free position; an equipped item returns to its body location; no inventory → `NoInventory`.
- `d2-client` `tests/app_save.rs`: `waypoints_round_trip_through_the_save` (the base has default waypoints, so only the running game can supply them), `extra_values_overlay_the_body`.
- Not covered: `WiredWorld::save_items` / `load_items` and the client loader on a game with item tables. The synthetic game has none; there is no cloud-side wired-host fixture with an inventory model. Queued below.

## PROVISIONAL points

REC-110 (placement of loaded items; what is not done). REC-105's item part is superseded by it.

## What is left

- Mouse skills, hotkeys and the act byte (`apply_header`, `set_mouse_skills`, `resolve_item_indices` stay unapplied; the loaded header passes through).
- Mercenary and golem items, the corpse (`hireling_items_loaded`); the stash gold.
- Runeword refresh after load (§8.2 rule 5).
- The new character's file has no start items on a synthetic game; with game files the start items are real items now saved.

## The user's local check

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=info"
git fetch origin claude/q-save-full; git checkout claude/q-save-full
cargo run -p d2-client --release -- play --new sorceress Test
```
1. Move an inventory item to a different cell, put another in the stash (see q-stash), pick up the first waypoint, close the window. Console: `play: saved the character to ...`.
2. `cargo run -p d2-client --release -- play --save "%USERPROFILE%\Documents\d2rs\saves\Test.d2s"`: the inventory, the stash and the waypoint are as left; the log has no `join: save load: items:` lines. A `Test.d2s.bak` appears after the second close.
3. Send the `join: save load` and `item` log lines if an item is missing or moved (placement is the preview's own, REC-110).
