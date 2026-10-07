# c2-world quests coverage handoff

## Exempt candidates

specs/world/quests-act2.md	§6.10	pointer: section moved to quests-act2-2.md §2
specs/world/quests-act2-2.md	§2 text	narration: list of chain 11 extra field offsets
specs/world/quests-act3.md	§edge-cases-original-bugs r17	dead code: functions have no caller in 1.14d
specs/world/quests-act4.md	§edge-cases-original-bugs r18	dead code: functions have no caller / no effect in 1.14d
specs/world/quests-act5-2.md	§edge-cases-original-bugs r10	dead code: functions have no caller in 1.14d
specs/world/quests-act5-2.md	§edge-cases-original-bugs r14	statement that the radius arguments are unused (nothing to test)
specs/world/quests-act5.md	§edge-cases-original-bugs r7	dead code: init 70 and 0x005875D0 do nothing reachable
specs/world/quests-act5.md	§1.4	table of object init/operate function numbers (pointer table to rules in other sections)
specs/world/quests-act3-2.md	§11.7 text	narration: describes the init record layout

## Not done (needs real work)

- specs/world/quests.md §9.6 (init 46 trapped-soul placeholder, rules 1-8, §9.6 r1-r6, l2 r1-r7) and §9.2: no d2-sim implementation found (init 46 / operate 43 / init 59 bodies); needs a new module, not a claim.
