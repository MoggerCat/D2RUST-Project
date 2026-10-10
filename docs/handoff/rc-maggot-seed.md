# rc-maggot-seed hand-back (2026-10-10)

Task: 19 rows diverging on `seed` (maggotegg1-6, sandmaggot2-6, uberandariel).

## Checks (EQUAL = rng channel / state channel)
Before: gen-mon-190..194, 681 state DIVERGED@41 (seed), rng DIVERGED@41.
After: rng channel MATCH on gen-mon-190..194, 681, 679, 716, 72
(100%); the `seed` field no longer diverges on those. State still DIVERGED
on a later field (below). No check went fully EQUAL yet.

## Root cause (1.14d `0x005CAFA0`, spec bodies-3.md §5.7)
The egg hatch was dead in d2rs, so the game seed never advanced:
1. `MonsterSpawn::Near` (`0x005B23C0`) was not routed (host default None):
   now `MonsterWorld::spawn_near` -> population `place_near`.
2. `BodyEffect::KillBy` (`0x0057CCB0`) was dropped: now `reaction::kill_by`.
3. `BodyEffect::DeadFootprint` (`0x00649F70`) was dropped, so the egg
   blocked its own ring-0 placement test: now `dead_body_footprint`.
4. Monster init's `give_skill` was a no-op, so the egg's skill level fell
   back to 1 (hatches 1 baby, 1.14d hatches 2 at Sk1lvl 2): the init host
   now fills `ActionHooks::monster_skills` (Sk level + difficulty bonus).
Item 4 changes `ai_skill_level` / `ai_skill_entry` answers for every
init-created monster (they used to fall to the default None). Other AI
checks may move; not run here.

## Open (not fixed, separate causes)
- Egg dead-mode animation frame: `fr` 1.14d 4608 vs d2rs 10496 at frame
  41 (gen-mon-190..194, 681, gen-ai-maggotegg). Size S.
- Sandmaggot laid egg position `x` 5151 vs 5147 (gen-mon-72/679/716,
  gen-ai-sandmaggot: `LAY_PAIRS` offset or placement, frame 85/135). Size S.
- gen-mon-69/70/71 (sandmaggot2-4): rng DIVERGED at frame 100, a missing
  game-seed allocation `0x552e31` (a second laid egg not spawned). Size S-M.
- gen-ai-sandmaggotqueen: seed diverges at frame 43. uberandariel not
  re-run (no gen-mon check name found; `gen-ai-andariel` exists). Size M.
- Ledger rows in `ledger/rc-maggot-seed.tsv` stay DIVERGED (state).
- One flaky d2-sim test seen once (`prop_walk_motion chase_a_moving_target`),
  passed on re-run with --no-fail-fast (4754/4754).
