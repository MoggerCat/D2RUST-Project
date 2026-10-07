# last-sim coverage session

Branch `claude/last-sim`. Short session (deadline moved up); most rules were not reached.

## Covered
- specs/sim/tick.md: 47 -> 50 of 62 (§5.2 r4, §5.3, §7), new `crates/d2-sim/src/tick/tests_lsim.rs`.
- specs/monsters/population.md §14 r1: claim added on the existing `spawn_class` test (`skills/use_/bodies/tests_b3a.rs`).
- Code fixes: none.

## Left (not examined unless a reason is given)
- tick.md: §1 r6, §5.5 r4, §5.7, §6 text/r1-r3, §8 (host/server-side or narration). Edge-case r3 (timer on unit type 6: original indexes out of bounds, code returns `NoTimerClass`) needs a decision.
- rng.md: §3 r6 (fastcall narration), §5.5 rows 1-2 (client-only seeds), §7 rows (which system draws from which seed; needs cross-crate checks/traces), §5.4 rows 3-5 (DRLG seed derivations, testable, not reached).
- damage.md §2 r1, §7.1 (reaction branches): `Pending::reaction` is a no-op host seam; implementing the monster/player mode-request tree needs mode-request wiring in the wiring crate/module (larger change). §7.1 r3 already claimed.
- vitals.md §4.8, §5.1 r4; monsters/* (init, umod-callbacks, ai-bodies-7 §27, population §1 r2/r3, §14 r2-r6 [narration]); sim/intents-events, pathing, path-placement, units, stats; drlg/*, missiles/*, skills/*: not reached (see the drlg agent's section below if present).
- Note: subagent worktrees were created from a stale base; the sim agent rebased onto this branch.
