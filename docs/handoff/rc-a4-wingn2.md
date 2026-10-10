# rc-a4-wingn2 hand-back (2026-10-10)

Task: act4.play `diablo-present-wingn2` stuck on staging-7 b56458388.
Base: origin/claude/integ-r18 (2d574b468), as the coordinator decided.

## Cause: a correct fidelity change moved the seal-boss GUIDs

- Bisect (`git bisect --first-parent d8d73357..b56458388`, current
  act4.play and playthrough.py pinned): first bad merge **5ba34fffd**
  (rc-mon-frame31, r15). 2487e67db reaches the milestone.
- Chain (seed 5): at f48 Venom Lord g125 now casts Inferno (mode 8)
  instead of walking, because `AiActs::skill_level` reads the init-given
  natural skill levels. Its missile (675, f59) takes a game-seed step at
  unit allocation, so the room populated at f61 rolls density differently
  (population.md §3) and spawns one extra pack (OblivionKnight + 3
  DoomKnights at (7929,5294)). Later monster GUIDs move by +4.
- 1.14d agrees: `AITHINK_Fn089_Megademon` (0x005E0C80) gates Skill1 on
  the unit's skill level (`0x006439B0(unit, Skill1, -1)`), which monster
  init gives every monster. No code change (rule 10).

## Fix: act4.play only (coordinator decision)

- `diablo-present-wingn2`: 1/146→1/150 (Infector), 1/156→1/160 (De Seis),
  1/162→1/166 (Vizier), with a comment. Same GUIDs on integ-r18 (De Seis
  hp 225280 and Vizier 267264 against minions' ~70k / ~120k).
- `@1:<class>` was not usable: it names the lowest-GUID unit of the class,
  and each boss shares its class (362, 312, 306) with its minions and with
  population monsters placed earlier.

## Playthrough (d2rs, release)
| file | base | reached |
|---|---|---|
| act4.play | integ-r18 + this | 13/13 |
| act4-blockers.play | integ-r18 + this | 6/6 |
| act4-forge.play | integ-r18 + this | 6/6 |
| act1 / act2 / act3 | staging-7 b56458388 | 17/17, 15/15, 15/15 |
| act5 | staging-7 b56458388 | 12/13 (nihlathak-killed, rc-a5-nihlathak) |

nihlathak-killed is stuck on 2487e67db too (no shared cause). Checks: none
settled, no ledger part; EQUAL counts unchanged.
