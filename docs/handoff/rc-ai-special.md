# rc-ai-special hand-back (REC-1925..1929 unused)

Two monster-AI gaps from `docs/handoff/q-run-gen-wp-shrine.md`. Checks run with
`suite.py --filter ... --orig-cache traces/orig-cache`, 1.14d recorded fresh
(the cache held neither check). Staging moved while I worked (parallel work
landed the same per-class mode records, mode-end target and tentacle spawn);
the merge took staging's version where it overlapped.

## Checks before -> after (state ticks equal of 460 / 160, rng of 145)

| Check | Before | After my change alone | After merge with staging |
|---|---|---|---|
| gen-wp-18 (class 359) | 36, diff frame 37 (1:6 seed) | 402, frame 153 (class 245 `tx`) | 445, frame 434 (class 254 `tx`) |
| gen-wp-19..26 | 36, frame 37 | 341, frame 153 | 399, frame 400 (game seed after waypoint) |
| gen-lvl-132 state | 96, frame 97 (game seed) | 143, frame 144 (Baal `tx`) | 160/160 PARTIAL |
| gen-lvl-132 rng | 140, site `0x552e31` missing | 145/145 MATCH | 145/145 MATCH |

`gen-wp-*` (39) re-run after my change: none got worse. EQUAL verdicts: none
(no check has all channels MATCH yet; gen-lvl-132 is PARTIAL, state+rng equal).

## What changed (d2-sim; specs in our words with addresses)

- SpecialState06 `0x005E7C10` (new, `ai-bodies.md` §9.33): GoodNpcRanged with
  the attack choice from the main search (walk flags 7 / A1). The class 359
  (act3hire) cause.
- Frame advance `0x00623E00`, sequence-less branch (`anim::advance_frame`,
  `ActionHooks::refresh_unit_animation`) for every monster event 0 refresh;
  Baal's S2 `fr` 18688 -> 1184. The provisional "+0x4E := timer arg"
  (REC-701) is gone; 6 tests now feed AnimData bytes.
- Source-unit link +0x94/+0x98 stored (`BodyEffect::SourceFields`), owner
  `0x00552FD0` reads it: without it the tentacle AI killed the unit at once.
- Generic mode end `0x005A8030`: state-54 / dead guard (target part is
  staging's); `ai.md` §1.4.
- Tests: SpecialState06 x3, frame advance x3; fmt, clippy, d2-sim + d2-server
  5158 tests, coverage / spec_index / ledger checks pass.

## Open

- gen-wp-18 frame 434: class 254 `tx` 0 vs 5085 (path target, M).
- gen-wp-19..26, 1-17, 28-38 frame 400: game seed after waypoint travel (the
  separate shared cause in the q-run brief).
- Mode end does not clear the used-skill entry (builder `0x00620210(U,0)`):
  host seam `set_current_skill` takes a skill id (0 = Attack); needs a "none"
  seam in d2-client `LocalSeams` (`monster_ai.rs`) (S).
- `prop_walk_motion::chase_a_moving_target` is flaky on staging (about 1 run
  in 3, `(0,0)` vs `(1,1)` at line 352); path code, not touched here (S).
- `re/exports/names.tsv` is absent in the private repo, so no names were
  appended (`0x005E7C10` SpecialState06, `0x00623E00` frame advance).
- d2-client not built (Bevy); its tests only gained `mode_chart: false`.
