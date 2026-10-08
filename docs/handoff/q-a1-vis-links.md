# q-a1-vis-links: Act I fixture links follow the level links

Branch `claude/q-a1-vis-links`. Open point: REC-259 (`docs/HANDOFF.md` §7). Nothing here is verified against 1.14d.

## Links connected

`ACT1_TREE` (`app/synthetic_chains.rs`) now follows 1.14d's chain: Stony Field → Underground Passage 1 (→ UP2) → Dark Wood → Black Marsh → Tamoe → Monastery Gate … Catacombs 4. Tristram is off the Stony Field, Burial Grounds off Cold Plains, Cave 2 off Cave 1 (its way on comes from the tree via `synthetic_tower::maze_links`), Hole off Black Marsh, Pit off Tamoe. The Blood Moor tiles to the Black Marsh and Burial Grounds (`single_player.rs`) were removed: Black Marsh and Burial Grounds now get slot 0 from the tree. Blood Moor → Cold Plains → Stony Field are borders.

## Expectations that moved (the fixture follows the spec now)

- `app_frame_loop`: joined units 30 → 28, handled 45 → 41, sight list's level 13 room `(24, 40)` → level 17 `(0, 24)` (Cold Plains's tile leads to the Burial Grounds), queued/drained after 300 ticks 5 → 6 (the Burial Grounds, now a neighbour, populate).
- `app_a1_dungeons`: walk Blood Moor → Cold Plains → Stony Field (put) → UP1 → Dark Wood → Black Marsh → Tamoe → … → Catacombs 4; branches re-parented.
- `app_tower_quest`, `app_play_bloodraven`: start in Black Marsh / Cold Plains by put, not by the removed tiles.
- `synthetic_chains` unit test: Dark Wood back = UP1, on = Black Marsh; the 1.14d order.

## PROVISIONAL (REC-259)

The specs hold the `Vis`/`Warp` column names only, not values; parents come from the task's chain. Slots, ids and tile places are made up.

## Left

Compare slot by slot with the real `levels.txt` (`data-tool tables`). Maze/preset builds for the flat dungeon levels.

## The user's local check

`cargo nextest run -p d2-client --test app_a1_dungeons` (no game files). With game files: `cargo run -p d2-client --release -- play --new sorceress Test`, walk Blood Moor → Catacombs 4 and note the level order.
