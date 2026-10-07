# c2-items-drlg coverage session

Time-boxed (cloud toolchain install + d2-sim build consumed most of it).

## Counts
Before: specs/items + specs/drlg had ~96 uncovered units (see
`python3 tools/coverage.py` at base). After: 2 claims added (below).

## Claims added
- `specs/drlg/levels.md §edge-cases-original-bugs r4` on
  `spawn_tile_without_match_reads_record_0` (spec says the other half
  cannot occur in 1.14d).
- `specs/items/inventory-moves.md §7.10 r4` on
  `swap_cursor_buffer_placement_fails` (+ asserts C stays in mode 4).

## Code fixes
None.

## Exempt candidates
specs/drlg/preset.md	§2 r3	assumption note (expansion install)
specs/drlg/preset.md	§3.1 r4	pointer to levels.md §6
specs/drlg/preset.md	§5.2 r4	"skipped": nothing implemented
specs/drlg/preset.md	§5.2 r11	"anything after the last section is ignored"
specs/drlg/preset.md	§6 text	intro/callers list
specs/drlg/preset.md	§12	run-time pop presentation (client)
specs/drlg/preset.md	§edge-cases-original-bugs r3	no bounds checks, unreadable slack bytes
specs/drlg/maze.md	§edge-cases-original-bugs r9	unreachable rows
specs/drlg/outdoor-act3-act5.md	§5 text	pointer to outdoor.md
specs/drlg/outdoor-act3-act5.md	§edge-cases-original-bugs r6	memory leak only
specs/drlg/outdoor-act3-act5.md	§edge-cases-original-bugs r7	size never used
specs/drlg/outdoor-tilesub.md	§edge-cases-original-bugs r4	reads whatever follows the table
specs/drlg/outdoor.md	§3 text	intro
specs/drlg/outdoor.md	§7 text	level-id range note
specs/items/treasure.md	§9 text	intro/calling convention
specs/items/quality.md	§2	pointer to treasure.md §6
specs/items/quality.md	§8 r1	format 0 durability x5 is generation.md OQ 1 (unspecified)

## Rules left
All other uncovered units listed by `python3 tools/coverage.py` for
specs/items/* and specs/drlg/* (notably inventory.md §4.9, generation.md
§10.3/§12, inventory-moves.md §8.5, rooms.md rules): not reached.
