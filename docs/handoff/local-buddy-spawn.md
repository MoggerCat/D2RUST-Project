# Local buddy: monster spawn hooks (2026-10-07)

Branch `claude/spec-spawn-hooks-buddy` (base origin/main + merge of
`claude/tender-meitner-mphas3`).

## Delivered

- `specs/tools/original-hooks-spawn.md`: callable spawn entry points of
  the 1.14d `Game.exe` (normal `0x005B2F20`, with GUID `0x005B30E0`,
  random boss `0x005A43E0`, superunique `0x005A49B0`, champion mark
  `0x005A48C0`, room lookup `0x00463740`) with registers, stack, `ret`
  sizes and refusals; how to read game, player, room and position; which
  call gives which kind of monster; every RNG draw site of the spawn
  path; the debugger call procedure at the tick-return hook `0x0052FD1E`.
  Links `tools/original-hooks.md` (PC 1, branch
  `claude/spec-scenario-hooks`) for injection, seed and tick hooks.
- `tools/trace-recorder/spawn.py` (subclass of `record_rng.Recorder`,
  stdlib only) and its README section. Output `traces/raw/<time>-spawn.jsonl`
  (`spawn-raw-1`, readable by `check_rng.py`).

## Commands run (game lock held 02:30–02:32)

```
py tools/trace-recorder/spawn.py --game <main>/game/Game.exe --tick 60 --class 5
py tools/trace-recorder/spawn.py ... --tick 60 --class 19
py tools/trace-recorder/spawn.py ... --tick 60 --class 19 --kind champion
py tools/trace-recorder/spawn.py ... --tick 60 --class 19 --kind unique
py tools/trace-recorder/spawn.py ... --tick 60 --class 58 --kind boss
py tools/trace-recorder/spawn.py ... --tick 60 --superunique 0
py tools/trace-recorder/check_rng.py traces/raw/<each>-spawn.jsonl     # all OK
```

Default start: `Game.exe -w -ns -nosave -name bdAma -ama`, menu forced at
4.1 s (as `dump_tables.py`); the save `bdAma` loaded with no keyboard
input, first tick ~4.2 s, spawn at frame 60 in the Rogue Encampment,
player at (5473, 4708), target +4/+4. Each run ~8 s.

| Run | Created | Draw records / seed steps |
|---|---|---|
| zombie1 normal | 1 | 21 / 21 |
| fallen1 normal (party 3) | 4 | 38 / 38 |
| fallen1 champion (umod 16) | 3 | 32 / 32 |
| fallen1 unique (umods [17], 4 minions) | 5 | 50 / 49 |
| fallenshaman1 random boss (unique, [9]) | 4 | 43 / 42 |
| superunique row 0 Bishibosh ([8, 9, 22]) | 3 | 45 / 42 |

Draw records count helper calls with n = 0 (d = 0 rings), which do not
step. No draw from another thread during any call. Sequences per run:
spec Test vectors.

## Findings

- The appearance-variant builder `0x005BDB20` (`population.md` §2.4)
  draws from the new unit's seed for the first monster of a class in a
  level region; it was missing from the static site list and is now in
  spec §4.
- The first AI setup `0x005B0E00` drew nothing for zombie1, fallen1,
  fallenshaman1 (`monsters/init.md` Open question 2, partly settled; not
  edited there).
- A champion made by `0x005A48C0` also raises its party minions (minion
  list via SetBoss) to level 4, xp 90 (`init.md` §18 step 2).
- The room seed before the spawn was identical in all six runs (save map
  seed); the game seed differed (clock, `tools/original-hooks.md` §2).

## Problems / not done

- No Blood Moor run: the save starts in town, and the town spawn worked,
  so walking out was not needed.
- The 0xAC timing (same tick or next) is spec Open question 1.
- The `game.lock` was held by the saves worker from 01:16 to 02:30; this
  work waited and did Part 1 meanwhile.
