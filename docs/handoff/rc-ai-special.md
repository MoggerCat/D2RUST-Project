# rc-ai-special hand-back (REC-1925..1929 unused)

Two monster-AI gaps from `q-run-gen-wp-shrine.md`. 1.14d was recorded fresh
(the orig-cache held neither check). Staging landed overlapping work meanwhile
(per-class mode records, mode-end target, tentacle spawn / wait); the merge
took staging's version where it overlapped.

## Checks before -> after (state ticks equal of 460 / 160; rng of 145)

| Check | Before | My change alone | After merge |
|---|---|---|---|
| gen-wp-18 (class 359) | 36, frame 37 (1:6 seed) | 402, frame 153 | 445, frame 434 (class 254 `tx`) |
| gen-wp-19..26 | 36, frame 37 | 341, frame 153 | 399, frame 400 (game seed) |
| gen-lvl-132 state | 96, frame 97 (seed) | 143, frame 144 | 160/160 PARTIAL |
| gen-lvl-132 rng | 140, `0x552e31` missing | 145 MATCH | 145 MATCH |

`gen-wp-*` (39) re-run after my change: none worse. EQUAL verdicts: none yet.

## What changed (d2-sim; specs in our words, addresses in them)

- SpecialState06 `0x005E7C10` (`ai-bodies.md` §9.33): the class 359 cause.
- Frame advance `0x00623E00`, sequence-less branch, for every monster event 0
  (`anim::advance_frame`); Baal S2 `fr` 18688 -> 1184. The provisional "+0x4E
  := timer arg" (REC-701) is gone; 6 tests feed AnimData bytes instead.
- Source-unit link +0x94/+0x98 stored; owner `0x00552FD0` reads it (without
  it the tentacle AI killed the unit at once).
- Mode end `0x005A8030`: state-54 / dead guard (`ai.md` §1.4).
- Tests: SpecialState06 x3, frame advance x3; all gates pass (5158 tests).

## Open

- gen-wp-18 frame 434: class 254 `tx` 0 vs 5085 (path target, M).
- gen-wp-1..17, 19..38 frame 400: game seed after waypoint travel (shared
  cause named in the q-run brief, M).
- Mode end does not clear the used-skill entry (`0x00620210(U,0)`): the host
  seam `set_current_skill` takes a skill id (0 = Attack); needs a "none" seam
  in d2-client `LocalSeams` (S).
- `prop_walk_motion::chase_a_moving_target` is flaky on staging (~1 in 3,
  `(0,0)` vs `(1,1)`, line 352); path code, not touched here (S).
- `re/exports/names.tsv` is absent in the private repo: no names appended.
  d2-client not built (Bevy); its tests only gained `mode_chart: false`.
