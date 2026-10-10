# rc-mon-tx hand-back

Checks (gen-mon-*, state + rng): re-run of the 17 `tx` rows.
- Stale, now EQUAL (state and rng): reanimatedhorde4-6 (439, 440, 698),
  slinger4 (376), slinger6 (395). Ledger part: ledger/rc-mon-tx.tsv.
- Not re-run: slinger7-9 (610-612), nihlathakboss (526).
- siegebeast1-3 (441-443): `tx` is gone (stale); the surviving first
  divergence was the possessed imp (unit 1:11, class 492) at frame 38.
  EQUAL count for the cluster: 0 -> 5 rows (siegebeast still DIVERGED).

Root cause (siegebeast): Imp Teleport (srvdo 129) could not place the imp
on its host: the exact placement `0x00554EA0(exact 1)` was a stub returning
"not placed", so possess and AI special state 16 never ran.

Changed (d2-sim):
- Exact placement wired to the path provider (`skill_rooms.rs`,
  `skill_use.rs`).
- Unit source link (`link_source` SourceFields effect applied; unit
  source map in ActionHooks; `0x00552FD0` reads it; AiUnits::source_unit).
- Special state 16 init `0x005E2CD0` and think `0x005E2D80`
  (`monsters/ai/bodies2.rs`), spec in `specs/monsters/ai.md` §3.3.
Result on 441-443: imp x now equal at frame 38; first rng divergence
moved frame 38 -> 50.

Open:
- Siegebeast 441-443: state `fr` (anim frame) 1.14d 0 vs d2rs 7936 at
  frame 38 for the placed imp (placement should reset the frame); rng
  frame 50 site `0x5e3262` (unit 1:10) missing. Also srvdo 147 (state
  143 follow each tick, bodies-4 §4.10) and the every-tick type-5 event
  are not wired.  Size: M.
- Other checks seen DIVERGED in the same run, not mine: 675/676 (m 14 vs 1,
  game draw 0x552e31), 679/69 (x), 692/693 (fr, 0x5a55ba), 695-697 (fr,
  0x5f4e91).
- Flaky: d2-sim prop_walk_motion::chase_a_moving_target passes/fails
  between runs on identical code (proptest), not touched by this change.
