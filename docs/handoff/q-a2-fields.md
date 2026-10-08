# q-a2-fields: Act II outdoor levels

Stitching session `q-a2-fields`, branch `claude/q-a2-fields`.

## Finding

The Act II generator (`d2-sim` `drlg/outdoor/acts.rs`, `outdoor.md` §8) and the A2 / A2C placer rows were already in the sim and unit-tested, but nothing ran them end to end: the synthetic play world and the fixture sets are Act I only (`test-fixtures` `act1.rs`, `GameData::drlg_world` creates act 0 only).

## Links connected

| Link | Before | Now |
|---|---|---|
| Act II-shaped synthetic set | none | `crates/test-fixtures/src/act2.rs`: levels 0..=46, DrlgType and LevelType 16 per the spec, recorded sizes and offsets, lvlprest `Def` = row to 413, Lut Gholein 56 × 56 |
| Placer (`Drlg::create(1, ..)`) over it | untested end to end | `tests/act2_game.rs::the_act_placer_links_the_desert_chain`: levels 40..46 allocated, Def rects of 40 and 46, Valley of Snakes 32 × 32 |
| Generate + stream of every Act II level | untested | `every_desert_level_generates_and_streams`: rooms exist, outdoor rooms in 41..46, no preset-only outdoor level, no provider error, and exactly Dry Hills / Far Oasis / Lost City carry a waypoint room flag |

Small edit in `act1.rs`: `n`, `clear`, `floor_preset` are `pub(crate)` for reuse.

## PROVISIONAL

REC-133 (`docs/HANDOFF.md` §7). Made-up data; no `Covers:` claim.

## What's left

- Population (monsters in 41..46) and a player walk Lut Gholein → Rocky Waste: `host::Session`, `Setup` and `GameData::drlg_world_in` are hard-wired to act 0 (a town waypoint search, `ensure_act(0)`); generalising them touches every existing test's `Setup` literal.
- The waypoint object itself (outdoor waypoints are room flags here; the object comes from lvlsub / preset content), so waypoint travel to Act II fields is untested.
- Synthetic play world (`single_player.rs`) has no Act II outdoor levels; with game files the live `WorldTypes` already builds them.
- Level 134 (Pandemonium 2) is not in the set.

## The user's local check (game files)

```
cargo run -p d2-client --release -- play --new sorceress Test
```
Take the town waypoint to Lut Gholein (a save that knows it, or Warriv), walk out of the gate to the Rocky Waste, then Dry Hills. Expect: desert tiles with no void at the level borders, monsters present, a waypoint object in Dry Hills, no panic, no `rejected` line. Note the console lines if a level fails to generate (the log names the outdoor error).
