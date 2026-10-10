# rc-level-population: hand-back (2026-10-10, branch `claude/rc-level-population`)

## Checks (`suite.py --checks-dir traces/checks/gen --filter 'gen-lvl-*' --orig-cache`, 136 checks, channels state + rng)
| | before | after |
|---|---|---|
| checks with state and rng both equal | 96 | 112 |
| equal ticks (of 36,926) | 31,422 (85.1 %) | 33,786 (91.5 %) |
| runs MATCH / PARTIAL / DIVERGED | 104 / 96 / 72 | 121 / 112 / 39 |
No check went down. Newly equal: levels 13 14 25 38 42 48 49 76 85 92 93 116 119 133 134 136 (ledger part `ledger/rc-level-population.tsv`).

## Root cause: the object drops had no item pick rows
`DeathDrops::with_picks` was never called by a host, so the drop state's `ClassPicks` was empty and every object drop
(`objects-2.md` §20: `0x00559300` gold, armor stands, weapon racks, bookshelves, corpses) failed at `find_code` without
creating an item. In level population the gold placeholder (object 269, init 28 `0x0054F8C0`) makes 1.14d allocate
1-9 gold piles (game-seed draws `0x552E31` + item seed `0x552E9F` per pile); d2rs made none, so the game seed split at
frame 21 and every later population pick (objects 79/144/94/95/148/185/246 in q-run-objects §5) followed from it.
Fix: `d2-client` single player loads `GameTables::class_picks()` with the live tables and sets them on `object_drops`.
This should also clear q-run-objects cause 2 (16 operate-drop rows; not re-run here: `gen-obj-*`).
`d2-server` has no object drop state yet; when it gets one it needs the same call.

## Also fixed (read from `0x005559A0`, spec `monsters/population.md` §11.1)
First preset walk, per level: 133/135 never place object 397; 134 never places the waypoint 402 and skips the object
preset that brings game +0x1DE4 to init seed % 3 + 3 (new `Game::sands_preset_objects`); 136 never places 26/268/269.
Levels 133, 134, 136 now equal (L134's d2rs-only waypoint is gone).

## Open (state first divergence after the fix)
- L4 (frame 21, object 37 mode 1.14d 0 vs d2rs 2), L37/L94 (frame 23, monster seed), L55, L78 (frame 21 game seed:
  still a population draw-order difference; L78's two class-249 monsters and objects 251/252 absent after
  touring all rooms (q-fix-rd-act3) not looked at; L78 first diverges on the game seed at frame 21), size M each.
- L97 hp at 26, L98 missile at 78, L99 monster mode at 87, L102 monster seed at 51: later-tick causes, not population.
- Act IV render scene's extra Trapped Soul (class 380): not checked (draws channel, client build), size S-M.
- The preset "done" bit (+0x1C bit 0) is not on the drop seam's `PresetUnit`; d2rs relies on taking the list once.
- `traces/orig-cache/gen-lvl-*`: re-recorded locally to add the rng channel; not committed (brief).
