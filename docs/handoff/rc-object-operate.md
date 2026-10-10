# rc-object-operate: hand-back (branch `claude/rc-object-operate`)

## Checks (gen-obj-*, state compared with `--ignore q`; items and rng are the hard channels)
- Before (q-run-objects): 0 MATCH of 431 rows (408 PARTIAL, 23 DIVERGED). EQUAL in the ledger stays 730: object state is
  PARTIAL by construction (fields one side does not write), so no row can reach EQUAL on state alone.
- After, the 23 rows in scope: 20 now items MATCH + rng MATCH + state PARTIAL (104 105 550 551 106 107 548 549 179 180 269
  149 354 355 356 405 406 407 397 298); still DIVERGED: 267, 385 (walk), 194 (monster mode). Ledger DIVERGED 1049 -> 1029.
- Not re-run: the other 408 object rows (no code path they use changed except the quest-chest gate and portal 34).

## Changed
- `d2-client` `LiveData` -> `GameParts.picks`: the drop state now holds the combined-items pick table
  (`DeathDrops::with_picks`). Without it every stand / rack / bookshelf / corpse / altar pick found no candidate and made
  no item (root cause of the 16 rows; the game-seed + 0x552e31 draws followed from the missing item creation, so the draw
  site itself needed no change and rc-wp-walk-tx was not touched).
- `HostQuests::quest_chest_gate` (0x545850) runs on the host's object view (Mode1, open animation); the inner economy
  world could not see them, so the quest chests 354-356 / 405-407 never changed mode.
- Operate 34 (Arcane / Palace portal 298) mode 0: mode 1 + event 1 at frame + (FrameCnt1>>8) + 1, no quest call
  (`specs/world/quests-act2.md` §6.9).
- `suite.py`: the duplicate `--checks-dir` option left by the merge removed. `q-run-objects` merged (conflicts in
  check_gen.py / ledger files resolved by union).
- Ledger: `ledger/rc-object-operate.tsv` (20 rows); the same rows removed from `q-run-objects.tsv`
  (two session parts: first by name wins).

## Open
- 267 bank / 385 urn (size M): 1.14d `0x548B00` case 2 walks the player (`0x548A50`, run mode 3) when the object is not in
  interact range R (`0x623660`) or the line test `0x622B50(P,O,0x804)` is nonzero. d2rs: client `object_in_range` = true and
  `object_preview_range` (d2rs-own, 5) replace both; `View::units_line_blocked` exists. Needs `object_approach` wired
  with R + line test + the server-side walk. Why the line test is nonzero with the object on the player is not yet known.
- 194 stair (size S-M): after the operate a town NPC (monster 1:7 class 155) takes a different wander target at frame 69
  (tx/ty 4868,4229 vs 4869,4230), game seed equal: suspect the stair footprint (collision) after mode 1, not the operate.
- clippy (d2-sim, d2-client), nextest d2-sim (4737 passed), coverage, spec_index, ledger --check: all clean.
