# q-a2-duriel: the Horadric Staff, Duriel's Lair and the Duriel kill in the play preview

Branch `claude/q-a2-duriel`. Nothing is verified against 1.14d (rule 10). Open point: REC-163 (`docs/HANDOFF.md` §7).

## Links connected

The quest rules (`act2/q6.rs`: orifice operate, `item_to_object`, `hand_in`, the lair timer, Duriel's kill, the lair warp check) existed, as did the staff tomb choice and `StaffAssembled`. Missing links:

| Link | Where |
|---|---|
| C→S 0x44 had no provider (`Pending::staff_in_orifice` default `None`) and `q6::item_to_object` no caller | `LocalSeams::staff_in_orifice` queues `QuestEvent::InsertItem`; `wired/quest_events.rs` resolves the item GUID and calls `item_to_object` |
| Host seams the hand-in reads: `set_room_portal`, `spawn_quest_object`, `player_busy`, `missile_range` (all "unhandled" in the play host) | `HostQuests` in `wiring/economy/quest_host.rs` |
| The quest warp gate (`quests.md` §8.2) was never consulted (`LevelView::quest_gate` default 0) | `Pending::warp_quest_gate` read by `wiring/path/place.rs`; `Pending::set_lair_open` fed from `lair_warp_open` each tick; `LocalSeams` closes level 73 unless open and the source is the staff tomb |
| Duriel not linked to chain 13 | Kill of monster class 211 adds the link (as Andariel), `quest_events.rs` |
| Synthetic world: no level 73, no orifice | `synthetic_act2.rs` (`DURIELS_LAIR`, a way from every tomb, seven ways back, level type 17), `synthetic_maze.rs` (back tiles, the orifice preset in the staff tomb's first room), `single_player.rs` (object rows 100 and 152) |

Tests: `d2-server` `quests_act2::the_staff_put_in_the_orifice_is_handed_in` (0x44 event: a wrong item refused, the staff handed in); `d2-client` `tests/app_a2_duriel.rs` (staff tomb has the orifice, the Lair tile is refused while closed, opens, Lair reached, Duriel's death: chain 13 state 3 and killed). `app_act2_dungeons` stops its chain walk at the Lair (new world, not a weaker assertion).

## PROVISIONAL (REC-163)

See HANDOFF §7. All `// d2rs-own, unverified`.

## What's left

- Duriel's AI and fight in the Lair; the test spawns and kills him.
- Tyrael (class 251), his door (153) and the portal to town; Jerhyn's (msg 442) and Meshif's (450) travel east need `q-act-travel` for those NPCs.
- The staff as an item: the synthetic game has no item tables, so the client test sets `lair_open` as the hand-in would; the hand-in itself is the `d2-server` test.
- The entrance object (class 100) the lair timer creates; the way in is the warp tile.

## The user's local check (game files)

```
cargo run -p d2-client --release -- play --new sorceress Test
```
Reach Act II, assemble the Horadric Staff in the cube, walk to the true Tal Rasha tomb (the one with the orifice), click the orifice with the staff held: the staff is consumed (quest log: staff done), the Lair entrance opens; step in. Before the hand-in the Lair's warp must be refused. Kill Duriel: log moves, Tyrael appears (not wired here). Send console lines mentioning `0x44`, `0x58` or `quest`.
