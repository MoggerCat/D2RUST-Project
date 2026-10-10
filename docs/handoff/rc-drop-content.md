# Hand-back — rc-drop-content (branch claude/rc-drop-content)

## Result
`items-drops-nor-*` (14 checks, orig-cache, this container): **6 EQUAL -> 8 EQUAL**
(before: 00 02 03 05 12 13; after: + 09, 11). 04 06 07 08 10 still DIVERGED, 01 PARTIAL.
Other families (`cha`, `hel`, `nig`, `rbo`, `uni`): not measured. Their 1.14d
recordings time out here (cha/rbo: ERROR at 680 s with 3 workers), so no
before/after; the change touches combat/AI answers, so run them on PC 1.

## Cause (REC-1985)
Not the treasure walk. The item content differed because the dropper's unit
seed had been advanced before its death. Method: the `rng` channel on a copy of
check 11 (`rng_diff.py`): Griswold's (unit 1:20) draws had the same values as
1.14d but at frames 31/41/46 instead of 51/61/66, plus a bolt hit at 59.
Griswold is a monstats `boss`; precheck C's boss sound (ai.md §2.4 step 1: idle
20 once, no draw) defers its first think to 51. `Pending::is_boss/is_demon/
is_undead/is_prime_evil` (0x0063E9F0/E940/E990/EDC0) defaulted to false in
production (only test fakes implemented them), so d2rs thought at 31, walked
into a +x bolt and drew hit rolls on the seed.

## Changed
- `wiring/action/monsters.rs`: `ActionHooks::is_boss/is_demon/is_undead/
  is_prime_evil` read the monstats row of the unit's class (monster data
  present), else the `Pending` answer. Call sites (ai.rs, combat.rs, missiles.rs,
  monster_death.rs, worldgen/umod_host.rs) use them.
  Side effect: combat now sees undead/demon/boss/prime-evil monsters
  (damage.md rows for those); other check families may move, both ways.
- Test `monstats_flags_answer_boss_demon_undead_prime_evil`; d2-sim nextest
  4750/4750 pass, clippy -D warnings clean.
- Specs: ai.md §2.4 (recorded Griswold timing), treasure.md §3.7 (the
  nor-* lag is not only the death lag; REC-1453 assumption refined).
- Ledger part `docs/handoff/ledger/rc-drop-content.tsv` (2 rows, MATCH).

## Open (not mine; sizes)
- 07 (class 121 134 156 160 161): first item order/content differs from the
  first walk; 1.14d 7 items, d2rs 11. Needs `rng` diff (run it with one worker:
  3 parallel rng recordings time out at 580 s). Size M.
- 08 (163 164 173 174 211): d2rs drops a gold at 108 that 1.14d drops at 144
  (death timing, then 1 extra gold). Size S-M.
- 04, 06: death/hit timing (see rc-drops-chickens.md: bolt 2 hit gate). Size M.
- 10: 1.14d drops gld (183) and qui (187) from monsters d2rs does not kill/drop
  at that time. Size S-M.
- 01 PARTIAL: no items on either side. Size ?.
