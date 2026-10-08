# q-a2-tyrael-door: the Lair's population, Tyrael's door and the portal walk

Branch `claude/q-a2-tyrael-door`. Nothing verified against 1.14d (rule 10). Open point: REC-238 (`docs/HANDOFF.md` §7).

## Links connected
| Link | Before | Now |
|---|---|---|
| Duriel, Tyrael, door 153 in the Lair | spawned by the test | host presets of level 73 (`synthetic_maze.rs`, positions in `synthetic_act2.rs`); rows for 153 (init 38) and 211 in `single_player.rs` |
| Door | never created | init 38 runs; mode 0 until Duriel dies, then animates |
| Walking through Tyrael's portal | `NoPortalDestination` (no partner, no spawn point) | `View::level_spawn_point` falls back to the DRLG spawn (type 12) of the destination level |

Test: `crates/d2-client/tests/app_a2_tyrael.rs` (placed units, door shut then open, portal walk ends in Lut Gholein).

## PROVISIONAL (REC-238)
Positions, the free-spot rule, Duriel's AI started by the test. All `d2rs-own, unverified`.

## What's left
Duriel placed with his AI by population; `portal_destination`'s exact free spot; Charge/Jab.

## The user's local check
```
cargo test -p d2-client --test app_a2_tyrael
cargo run -p d2-client --release -- play --new sorceress Test
```
In `play`: in Duriel's Lair Tyrael and a door stand in the first room; after Duriel dies the door opens; Tyrael's portal takes you to Lut Gholein. Record `portal` and `rejected` console lines.
