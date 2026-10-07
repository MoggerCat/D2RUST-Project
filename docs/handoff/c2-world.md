# c2-world coverage handoff

Result: 5 world rules newly claimed (claims 12507 -> 12512, 0 errors); no code fixes.

Newly covered: quests-act1-rest §4.1; hirelings-2 §15 r4, edge r5; objects edge r25; waypoints edge r7.

Exempt candidates and the remaining detail: `c2-world-quests.md`, `c2-world-objects.md`.

Real gaps (no implementation in d2-sim): quests.md §9.2, §9.6 (init 46 trapped soul, init 59, operate 43);
hirelings §8 r5, edge r10, r13; objects §12 r14 (iterate_players).

Rules left uncovered and not examined: quests-act1-rest §5 l2 r1, §8 r1/r4, §9 r3/r6/r12;
quests-act2-2 §1 r11/r14-16, edge r2/r5; quests-act2 §5.9 r1/r3/l2 r1; quests-act4 edge r19/r21;
quests-act5-2 edge r12; quests-act5 edge r11; quests-helpers §8 r1, edge r2;
hirelings-2 §17 r3, §18 r4; vendors-2 §7.3.1 r6; cube §8 l2 r2; npc edge r13;
objects-2 §22 r2-4, §23 r2, edge r7; objects-client §25 r4/r6/r7, §28 r1/r3, edge r5; hirelings §7.1 r3, edge r8.
