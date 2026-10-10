# rc-gen-skill-2 hand-back

Base: staging-7 + integ-r17 (merged). Checks: the 8 gen-skill checks still DIVERGED
after rc-gen-skill (state channel, 70 ticks each, cached 1.14d).

## Checks
- Before: 349/560 ticks equal, 8 DIVERGED.
- After: 400/560 ticks equal, 7 DIVERGED (gen-skill-dru-243: 19 -> 70 ticks equal, PARTIAL).
  All 30 gen-skill-dru-* re-run: no regression (27 PARTIAL, 3 DIVERGED = the vines).

## Changed (REC-2215)
- Cause: 1.14d refuses a skill whose `restrict` is 2 (Shock Wave: State1 bear) outside the
  form (use state 4, shape test 7 `0x00644060`), so the click sends nothing; d2rs read test 7
  as passing and cast it (mode 10, mana spent).
- d2-client `bridge/use_state.rs` `shape_ok` (restrict 0: not in a form; 1: any; 2: in a
  form and one of State1..3), `SkillRow.restrict/shape_states`, `StateRow.restrict`
  (states.txt flag bit 17 = the 0x0063A510 mask group). Unit test
  `shape_restriction_is_use_state_4`. Spec: `specs/skills/use.md` §2 (shape test),
  `specs/ui/controls.md` §6 r9.1.
- Server-side `use_state` has no shape test either (the sim's server path never rejects a
  forged 0x0C for a restricted skill): not touched (no check shows it).

## Open (sizes)
- dru-222/231/241 (3 checks, S-M): f67 vine (class 425/426/427) path target (5143,4264) vs
  (5144,4266) and anim speed sp 302 vs 192, unit seed differs. Traced: Vines
  (`0x005EC6C0`) runs at f54 (pet move k0) and f61 (k5: velocity (7,0,0) then wander near
  the owner, point (5149,4260), path type 7) in d2rs; the target change at f66/67 comes from
  the path engine, not from another Vines call. Control flow of `0x005EC6C0`, `0x005E45D0`,
  `0x005E3EA0` case 5, `0x005DF530` (draws, signs, centre) matches the code line by line; the
  difference is after the request (straight path / velocity consumption, `pathing.md` §6 and
  §8.1: sp 302 hints the velocity or anim rate differs when method 7 is consumed). PC 1 item
  queued in pc1-data.md Step 4.
- ass-257 (S: game seed f28), bar-155 (S: needs `0x0055B800` skill-stat callback wiring,
  see q-fix-skills-bda.md), ass-279 (S: monster mode 7 vs 1 at f48, Shadow Master think,
  PC 1 question already queued), nec-93 (S: missile ty off by 1 at f32): not looked at.
- Environment: `$HOME` is /root here; the game is in /home/user/game (symlinked to /root/game
  for prepare_saves.sh).
