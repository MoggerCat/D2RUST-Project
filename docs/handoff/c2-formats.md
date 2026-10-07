# Coverage session c2-formats (specs/formats, specs/data)

Cut short by the deadline: only the cheap rules were done.

## Counts (unit-covered / rules, before → after)

| Spec | Before | After |
|---|---|---|
| formats/animdata.md | 10 / 18 | 11 / 18 |
| formats/wav.md | 10 / 12 | 11 / 12 |
| formats/d2s.md | 98 / 133 | 106 / 133 |
| data/patch-layers.md | 17 / 22 | 19 / 22 |
| data/loading.md | 30 / 44 | 31 / 44 |

Claims added in `d2-formats/tests/tests_c2fmt.rs`,
`d2-formats/src/d2s/tests_c2fmt.rs` and
`d2-data/src/patch/tests_c2fmt.rs`. `coverage.py --check`: 0 errors.

## Code fixes

None. Every new test passed against the existing code.

## Exempt candidates

specs/formats/animdata.md	§5	composer 0x0064F5B0 name building for unit/class/mode lives with monstats speeds (fixups.md §8); player/object cases are Open question 2
specs/formats/animdata.md	§7	debug speed setter; d2rs does not implement it
specs/formats/animdata.md	§expfield-d2	expfield.d2 is read by d2-sim path tables, only checkable against game file
specs/formats/wav.md	§5	client WavDecoder hook description; checked in client/audio
specs/formats/d2s.md	§4 r4	pointer to world/quests.md
specs/formats/d2s.md	§7.1 r8	measurement of fresh saves from game, needs game files
specs/formats/d2s.md	§8.1 r8	measurement of fresh saves
specs/formats/d2s.md	§8.1 r9	list of original call sites of the item writer
specs/formats/d2s.md	§8.3 r6	measurement on one save
specs/formats/d2s.md	§8.3 r7	measurement / original-bug narration
specs/formats/d2s-load.md	§3 r1	original join handler addresses; server join (Iron Golem re-summon) needs a server test, not format

## Rules left

- d2s.md: §2.4 r5, §8.1 r6/r7, §8.2 r1/r3/r5, §8.3 r1, §8.4 r3, §8.5 r5, §10 r3 and
  edge cases r3, r6, r9, r11–r15.
- d2s-load.md: 12 rules (server load effects, d2-server work).
- data/calc-expressions, loading, patch-layers: a few each, not read.
