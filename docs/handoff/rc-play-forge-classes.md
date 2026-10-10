# rc-play-forge-classes hand-back

Task: act4-forge `forge-smashed` (quest 27.1) and classes `summon-follows-wp`
plus two other classes cells.

## Result
No code change was needed. After merging `origin/claude/integ-r17`
(staging-7 + r17 fixes), all three playthrough files pass on a fresh build:

| file | before (staging-7 table) | after (r17 merge) |
|---|---|---|
| act4-forge.play | 4/6, forge-smashed stuck f450 | 6/6 (27.1 set f210, 27.0 set f340) |
| classes.play | 7/10 and 8/10 in the two earlier tables | 10/10 |

The earlier failures (cast-then-walk, aura-applies, summon-follows-wp,
forge-smashed) were fixed by commits already on r17; I did not isolate which.
Per-file runs: `playthrough.py <file> --build` (forge), plain (classes).
`docs/handoff/playability.md` regenerated with `tools/coord/playtable.py` at
head 956cf372: 95/99 reached.

## Open (owned elsewhere, as briefed)
- act 4 `diablo-present-wingn2` (stuck f2900)
- act 5 `nihlathak-killed` (stuck f1000)
- act 1 `den-of-evil-done`, `andariel-done`/`act2-open` still stuck (act 1: 15/17)

No ledger rows settled; no spec change; no new RE.
