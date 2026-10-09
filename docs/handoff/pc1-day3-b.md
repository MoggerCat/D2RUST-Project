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

## REC-665 — mode request to the unit's own position (answered)

- A mode-2 request at U's own cell is still made. The path compute
  (`0x00649970`) hits start = target for type 13 and computes no path,
  so the path has 0 points. It retries as type 15 (flags |= 1, unit
  queued) with the same result.
- The walk start `0x005A7520` returns 0, so the unit goes to neutral
  (`0x005A73E0`, mode 1) and never enters walk.
- Next think at f + aidel (+45 with state 21). One S→C 0x67 code 7 is
  sent; no 0x68 and no RNG draws.
- Spec: `monsters/ai.md` §7.5 rule 8 ("Zero-length walk"), §7.2 row
  `0x005DE6D0`.
- d2rs differs (`walk_in_radius` makes no request): new row
  `q-fix-b665-zero-walk`, which depends on `q-fix-p3-walk-in-radius`.
