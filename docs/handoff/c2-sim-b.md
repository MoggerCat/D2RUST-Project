# c2-sim-b handoff

Short session (cold toolchain install ate most of the time).

## Counts
Before: scenario.md 7 uncovered. After: 6 (edge r4 claimed, if its test passes; see commit).
All other files unchanged.

## Code fixes
None.

## Added
- `crates/conformance/tests/tests_c2simb.rs`: `snapshot every` > `end` gives ticks 0 and `end`
  (`Covers: specs/tools/scenario.md §edge-cases-original-bugs r4`).

## Exempt candidates
specs/sim/tick.md	§5.7	pointer table to units.md (ownership), no behaviour of its own
specs/sim/tick.md	§8	host-only wall-clock parts, stay out of d2-sim by definition
specs/sim/tick.md	§1 r6	host driver load ratio (host-only)
specs/sim/tick.md	§edge-cases-original-bugs r3	out-of-bounds read for a null-unit timer; no caller, open question 2
specs/sim/tick.md	§6 text	client pass: heartbeat/host-only narration
specs/sim/stats.md	§edge-cases-original-bugs r5	checked by tools check_stats.py --files against game data
specs/sim/units.md	§edge-cases-original-bugs r4	wall-clock state in the original, open question 3
specs/tools/scenario.md	§edge-cases-original-bugs r1	needs a live/original trace (unresolved unit on one side)

## Rules left
damage.md §7.1 (r1,r2,r4,r5,r6,text), vitals.md §4.8 (text,r1,r2) and §5.1 r4, pathing.md
(§9.6 r2, §10 r4/r5, §12.2, §12.7/§12.8, edge r1/r2), path-placement.md (§10, §12.2, §13, edge r10),
units.md §3.4 r3 and §4.5, tick.md §6 r1-r3, scenario.md §4 text/r1/r3/r9 and edge r2/r3.
These need substantial new code or fixtures; not attempted.
