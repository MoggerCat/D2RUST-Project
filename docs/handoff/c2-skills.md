# c2-skills handoff

Counts (specs/skills/descriptions.md): covered 11 → 32 of 78. Other skills specs untouched (bodies*/levels/use were already ≥ 96%).

Code: new `crates/d2-client/src/ui/skill_desc_more.rs` (pure helper arithmetic of §2.1–2.11 and entries 11/12/14/15), tests in `ui/tests_c2skills.rs`. No fixes to existing code needed.

## Rules left (46)
Need unit-state access (weapon in use, stat lists, hand swaps, draw calls): §2.3 r1/r2/r4/r5, §2.4 r4/r5, §2.6, §2.7 r1, §2.8, §2.10, §3 rows 1,3–7,13,16–24, §4 rows 1–4, edge-case r5.

## Exempt candidates
specs/skills/descriptions.md	§1 r2	pointer: which callers reference the tables
specs/skills/descriptions.md	§1 r3	register/stack call convention of the original
specs/skills/descriptions.md	§1 r4	register/stack call convention of the original
specs/skills/descriptions.md	§2.1 r4	screen placement/drawing (pixel check only)
