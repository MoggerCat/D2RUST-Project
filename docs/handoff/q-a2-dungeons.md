# q-a2-dungeons: the Act 2 dungeons in the play preview

Branch `claude/q-a2-dungeons`. Nothing is verified against 1.14d (rule 10). Open point: REC-142 (`docs/HANDOFF.md` §7).

## Links connected

The maze generator, the Act II tomb choice and the special builders (sewers, tombs, Arcane spiral) existed; the synthetic world had only the Act 1 maze levels and nothing to enter them from in Act 2. Reused: `synthetic_maze` (`SyntheticTypes`, floor DS1, maze rows) and `synthetic_tower` (`maze_links`, `maze_levels`), no new level builder.

| Link | Where |
|---|---|
| Warp-pair table for Act 2 | `app/synthetic_act2.rs`: lines (sewers, Halls, Temple, Lair, Stony Tomb, Sanctuary, Canyon → seven tombs), `edges()` assigns lvlwarp ids 29 up, `add_levels` fills `levels.txt` view (level types, vis/warp slots) |
| Maze levels | `synthetic_tower::maze_links` / `maze_levels` fall through to `synthetic_act2`; `synthetic_maze` rows give tombs 6 cells (staff ×3, boss ×2, `maze.md` §5.2) |
| Flat parents | `SyntheticTypes::generate_flat`: Lut Gholein's room gets the warp slot flags, the Canyon stand-in (46) is a flat room; their tiles come from `preset_units` |
| `synthetic_drlg_data` | one call `synthetic_act2::add_levels`, ids appended to the warp rows |
| True tomb | `DrlgWorld::staff_tomb(act)` → `HostQuests::true_tomb_level` (0x3E clue → 0x50) and `LocalSeams::object_staff_tomb` (set after act creation in `build`) |

Test: `tests/app_act2_dungeons.rs`: to Act II, in and out of every line (all 31 dungeon levels built and visited, the client follows each time), the staff tomb is a tomb ≠ the boss tomb, the objects seam has it, and the staff tomb has the most rooms, the boss tomb next.

## PROVISIONAL (REC-142)

Where each line starts (the fields are other tasks'), tile places, which maze room holds the exits, the Canyon stand-in. All `// d2rs-own, unverified`.

## What's left

- Duriel's Lair (73) and the boss tomb's entrance; the orifice / Horadric Staff flow; population and objects in these levels (waypoint of Halls 2 / Sewers 2, chests, Radament).
- The Arcane Sanctuary is a spiral but entered by a plain warp (the original: Palace Cellar 3 portal).
- With game files the live `WorldTypes` builds all of these from the DS1 warp units; not exercised here.

## The user's local check (game files)

```
cargo run -p d2-client --release -- play --new sorceress Test
```
Act 2 needs the act-travel route (`q-act-travel`). Expect in the preview synthetic world only; with game files: enter the sewers from Lut Gholein, Halls of the Dead from Dry Hills, etc., and note the console lines if a level fails to build or the player lands off-floor.
