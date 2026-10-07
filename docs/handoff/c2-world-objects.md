# c2-world objects/hirelings coverage pass

Newly covered: hirelings-2 §15 r4, §edge-cases-original-bugs r5 (tests/c2world.rs); objects §edge-cases-original-bugs r25 (chests/tests/tests_c2world.rs); waypoints §edge-cases-original-bugs r7 (claim added to existing test).

Not covered, real gaps (no implementation or test yet): hirelings §8 r5 and edge r13 (room-deactivation of a dead hireling, 0x005752B0); hirelings edge r10 (life := max after restore, not in d2-sim); objects §12 r14 (iterate_players has no real implementation, only the trait default and a test fake); hirelings-2 §17 r3, §18 r4; vendors-2 §7.3.1 r6; cube §8 l2 r2; npc edge r13; objects-2 §22 r2-4, §23 r2, edge r7; objects-client §25 r4/r6/r7, §28 r1/r3, edge r5; hirelings §7.1 r3, §8 r6; hirelings edge r8, r12; vendors §8.1 r7.

## Exempt candidates
specs/world/waypoints.md	§4 r2	provenance table of 1.14d setter callers, owned by other specs
specs/world/waypoints.md	§8 r1	timing narration (message drain order)
specs/world/waypoints.md	§9	pointer to other specs
specs/world/waypoints.md	§10	open questions / provenance
specs/world/waypoints.md	§edge-cases-original-bugs text	edge text
specs/world/waypoints.md	§edge-cases-original-bugs r8	host wall-clock GetTickCount; not deterministic sim
specs/world/waypoints.md	§edge-cases-original-bugs r10	narration
specs/world/waypoints.md	§7 r6	spawn search belongs to drlg/levels and path-placement specs, needs level data
specs/world/cube.md	§10	provenance / narration
specs/world/hirelings.md	§6 r8	recording evidence narration
specs/world/hirelings.md	§8 r6	narration
specs/world/hirelings.md	§10 r9	narration
specs/world/hirelings.md	§11 r6	narration
specs/world/hirelings.md	§11 r8	narration
specs/world/hirelings.md	§14	pointer to monsters/ai-bodies-6.md
specs/world/hirelings.md	§edge-cases-original-bugs r12	1.14d undefined stack read, not reproducible
specs/world/hirelings-ai.md	§1	pointer to other specs
specs/world/vendors.md	§1 r6	unreachable with 1.14d tables, fatal paths
specs/world/vendors.md	§edge-cases-original-bugs text	edge text
specs/world/npc.md	§10	dead code in 1.14d
