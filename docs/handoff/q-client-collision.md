# q-client-collision

Nothing here is verified against 1.14d (rule 10). PROVISIONAL: REC-248 (`docs/HANDOFF.md` §7). Synthetic fixtures only.

## Links connected
- Trace: skill start (`lineofsight`) / Double Swing scan → `UseWorld::line_clear` (`d2-sim` `wiring/interaction/skill_use.rs`) → was `LocalSeams::line_clear` (always clear, no grid).
- The client already holds the DRLG rooms' collision grids (built by d2-sim, `drlg/collision.rs`) and turns the path provider on (`app/single_player.rs` `enable_paths`). `UseView::line_clear` now calls `rooms_line_clear` (`skill_rooms.rs`): `path::line::line_test` from the caster's room and path position to the point, under the skill's mask. No second collision map: the one the walk paths, `line_blocked` and `box_collides` use.
- Test: `use_line_clear_sees_a_wall_on_the_rooms_once_paths_are_on` (a wall bit between two points blocks; without the provider the seam answers).

## PROVISIONAL / left
- Unit sizes and the stop cell of `units_line_blocked` are not applied; mask use unchecked against `use.md` §5.
- Other client walkability asks (click paths) already go through the same grids on the server side; no separate client query existed to switch.

## Your local check
`cargo run -p d2-client -- play` with a ranged `lineofsight` skill: aim across a wall in a dungeon, it should refuse; in the open it fires.
