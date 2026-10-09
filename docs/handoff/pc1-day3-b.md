# PC 1 day 3, session B — hand-back (branch `claude/local-pc1-day3-b`)

REC block REC-815..829. Items from the PC1-B prompt (2026-10-09).

## REC-680 — Blood Golem life share (answered)

- `0x005C6870` (ECX game, EDX U, stack v; `ret 4`): U's first type-3 pet
  (`0x00574EC0(game, U, 3, 0)`), if a monster of class 290, takes
  x = p·v / 100 of a life amount, where p = `Param6` of monstats 290's
  `Skill1` (BloodGolem, 25). Then v −= Heal(golem, x) (`0x005C5F10`:
  capped at the golem's max, returns what was applied). It applies to
  the whole potion amount before the stat-74 list spreads it; callers
  are use entries 3 / 4 / 5.
- Specs: `items/use.md` §3.1, `combat/events.md` §1 "Heal".
- d2rs differs: new row `q-fix-b680-golem-potion-share`.
