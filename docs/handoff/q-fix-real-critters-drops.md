# q-fix-real-critters-drops: town critters and the monster drop spot

Cloud session, 2026-10-09. Branch `claude/q-fix-real-critters-drops`.
Implementation-side (CLAUDE.md rule 3). Both sides recorded with
`python3 tools/scenario-diff/scenario_diff.py traces/checks/a1-town-arrival-ama.check`
(1.14d under Wine, d2rs headless).

## Row 2: monster drop spot (q-fix-real-monster-drop-spot) — fixed

- Cause: the play host never loaded `data\global\ExpField.D2`, so
  `ActionHooks::paths.field` was `None` and every drop went through the
  `FreeSpot` seam (`StartSpot`: the start spot as is, no collision, no
  walk-back). The sim's floor drop (`wiring::economy::death::Spots`,
  `path-placement.md` §9) was already specified and tested with a
  synthetic field.
- Fix: `LiveData::expfield` (read through `GameFiles`, `ExpField::from_cells`)
  and `build_with` sets it on the path provider (`app/single_player.rs`).
  Monster, chest and quest drops of the play game now search their spot.
- Recording (`facts/items/a1-cold-plains-poke-kills.tsv`): drops at
  (5172, 4663) and (5172, 4662) for deaths at (5170, 4660) and
  (5171, 4659): (x + 2, y + 3) and (x + 1, y + 3). The first is the start
  spot; the second is the ring search moving off a taken start cell.
- Pin: `crates/d2-client/tests/app_floor_drop.rs` (real data, ignored):
  the live game's path provider holds the 256 × 256 field and the floor
  drop at the player's town position lands at (+2, +3); it fails with the
  field unset. The ring search with units on the cells is covered in
  `d2-sim` (`wiring/action/tests/death.rs`, vectors with `sign_field`).
- Not done: a `.check` with a kill. `state-dump` takes no input and the
  poke directives have no `kill`, so the d2rs side cannot kill a poked
  fallen; a `.check` needs a `kill` poke on both sides (poke.md) or input
  on `state-dump` (`scenario-diff.md` OQ 2). Left for the tool rows.

## Row 1: town critters (q-fix-real-town-critters) — found, not fixed

The chickens are **not** a server population step.

- Both sides' server unit lists are equal in class and count (25 units to
  frame 90, no class 149); `population.md` §11.3 ("critters are not
  placed") is confirmed by d2rs's preset path, which sees two class-149
  presets in the town and drops them.
- The `GUIDs 93–95` of the scene (`draws.tsv` rows 180–187, units `1:93`
  … `1:95`) are client-only monsters (set C): `0x00466730(149, x, y, 1, …)`
  from return address `0x0046C316`, GUID from the client counter
  `[0x00711F30]`, which 190 object creations at `0x00466862` (river
  objects, classes 40–65) inflate before the chickens of the scene.
- Source data is `Levels.txt` row 1: `C1` = 149 (critter class), `CA1` = 30.
  The level record the creator reads starts `95 00 ff ff ff ff ff ff 1e`
  (id 0x95?, 0x1E = CA1) followed by the level name.
- Call chain: client room function `0x0044C750` → `0x0044C774` (also runs
  the preset pass `0x00466820`) → `0x0046C54D` → `0x0046C316`. Three
  chickens per group (the chicken row's MinGrp = MaxGrp = 3).
- Draws per group (`record_rng.py`, seeds are the room client seeds):
  3 steps at `0x0046C4AC`, `roll(0)` via `0x0045C3E0` at `0x0046C516`,
  then per chicken a step at `0x0046C257` (x) and `0x0046C29C` (y).
- Observed groups, frame 2: (4827, 4195) (4820, 4198) (4832, 4191);
  (4927, 4198) (4939, 4198) (4956, 4195). Frames 89–95: (4814, 4256)
  (4827, 4269) (4834, 4275); (4871, 4242) (4877, 4254) (4849, 4267).
- No spec states this function, so an implementation would be a guess
  (rule 10). Queued for PC 1 as a `pc1-data.md` Step 4 item
  ("[q-fix-real-critters-drops] Town critters …"): the body of
  `0x0046C257`–`0x0046C54D`, then a `client/` rule and the client-side
  creation in `d2-client::bridge` (`ClientObjects.set_c`, `next_guid`).
