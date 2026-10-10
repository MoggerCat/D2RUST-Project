# rc-mon-lvl hand-back (2026-10-10, REC-2235)

## Root cause (one)
The 18 "lvl" divergences were not the monster's own level: the first
divergence was the monster's **missile** (3:n, 1.14d lvl 3..9, d2rs 1).
The client's monster AI mirror (`d2-client/app/monster_ai.rs`
`used_skill`) took the used skill's base level only from the summon
entries (`monster_skills`) and fell back to 1; the levels monster init
step 14 gives (`Sk<i>lvl` + skill bonus, `ActionHooks::natural_skills`)
were never passed. Missiles from the used skill therefore got level 1.

## Changed
- `d2-client/src/app/single_player.rs`: levels handed to the AI mirror =
  `natural_skills` overlaid with `monster_skills`.

## Checks (--no-playthrough, 1.14d reused from orig-cache)
- 14 checks that diverged first on missile lvl (613, 614, 722, 304-306,
  692, 693, 636-638, 720, 133, 134): lvl now equal in all; 613/614/722
  are now PARTIAL with state 100% (rng MATCH). Others moved on:
  304/305/306/692/693 player `m` (4 vs 5, f98/74/59), 133/134 monster `hp`
  (f45, 1.14d 43264/40960 vs 0), 636-638/720 game seed (f114).
- EQUAL (state MATCH/PARTIAL) before -> after: 0 -> 3 of those 14.
- Baseline of the full gen-mon cluster not obtained (orig-cache holds no
  gen-mon recordings; each check records 1.14d for ~36 s).
- clippy d2-client, coverage, spec_index clean. Unit/nextest of d2-client
  not run (no logic test covers the mirror).

## Open
- fr: gen-mon-117 (f100 3328 vs 4608), 509/510 (f97 23296 vs 3077) not
  read (3 rows, as in rc-mon-fr.md).
- New first divergences: player `m` after the missile (5 rows), monster
  `hp` 0 (133/134, probably a death/dmg path), game seed f114 (4 rows).
- 4 of the 18 lvl rows listed by the coordinator were not identified in
  the rc-mon-fr ledger (only 14 named there).
