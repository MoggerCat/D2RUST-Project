# play-drlg: the live "live DRLG room" panic (gap G1)

Session S-C1 of `docs/handoff/first-playable-scope.md`. Branch
`claude/play-drlg`. Files touched: `crates/d2-sim/src/drlg/outdoor/mod.rs`
(fix), `crates/d2-sim/src/drlg/outdoor/cold_plains_tests.rs`,
`crates/d2-sim/src/drlg/tests/{mod,walk}.rs` (tests),
`crates/d2-client/tests/app_level_border.rs` (new test).

## Cause

Reproduced from the code, no game files needed.

`Drlg::free_level_rooms` (`levels.md` §9.4, `0x00642010` keep = 1) frees
every room of the level, **then** resets the level type data. During
that reset the level's room list (`first_room` / `next`) still names
the freed rooms; it is cleared only after the reset. The outdoor type's
reset (`Outdoor::reset_level`, reached live through
`wiring/worldgen/levels.rs:319`) walked that list with
`drlg.level_rooms(l)`, which calls `Drlg::room()` on the first freed
room: `expect("live DRLG room")` panics (`room.rs:144` in the live run,
`room.rs:152` now).

Why ~104 ticks: game creation streams the first room of Cold Plains
(level 3, an outdoor level) and the player never goes there. Its level
activity stays 0. Tick step 9 (every 12 frames) deactivates the room
after its inactivity count passes 10; tick step 10 (every 11 frames)
counts the level's 10 inactive frames down, the free test (§9.3) passes,
and the free panics in the outdoor reset. Preset and maze resets never
walk the room list, so the synthetic game (Cold Plains is a preset level
there) never showed it.

The other suspects of the scope doc (near arrays, warp links, adjacency
arrays, status lists, the build cursor) were checked against the code
and the spec: the free test clears the near arrays and warp links of
every room that can name the freed level (§9.3 bullet 1), and a freed
level's rooms are never in a status list or active. The walk test
asserts all of them every tick.

## Fix

`Outdoor::reset_level` no longer walks the level's room list. It drops
the outdoor records of rooms that are no longer live
(`rooms.retain(|r, _| drlg.try_room(r).is_some())`). The order of §9.4
(free the rooms, then reset the type data) is unchanged. A room's
outdoor record belongs to the room (`outdoor.md` §12.2, room +0x20), so
it goes when the room is freed; the grids, polygon, paths and build list
are reset as before. The existing TODO (what `0x006754C0` frees exactly)
stays.

## Tests

- `d2-sim` `drlg::tests::walk::walking_across_levels_for_3000_ticks_never_reads_a_freed_room`:
  three generated levels on the synthetic fakes; level types that keep
  an outdoor record per room through the real `Outdoor` code. A client
  walks every room of level 2, across the level border through every
  room of level 3, then stays in the far level 4 (2 and 3 lose their
  activity and are freed), and back, for 3000 ticks, with tick step 9
  every 12 ticks and step 10 every 11. After every tick: no level list,
  near array, warp link, adjacency array, status list, active index or
  outdoor record names a freed room. Levels 2 and 3 are freed ≥ 2 times
  and generated ≥ 3 times. **Without the fix it panics** in
  `Outdoor::reset_level` with the live message (checked).
- `d2-sim` `drlg::outdoor::cold_plains_tests::cold_plains_rooms_free_with_their_outdoor_records`:
  the synthetic Cold Plains outdoor build, freed through `OutdoorTypes`:
  no panic, every room and record gone, the level list empty.
- `d2-client` `tests/app_level_border.rs`
  (`the_app_game_runs_3000_ticks_while_an_unvisited_level_is_freed`):
  the app's own synthetic game, headless, for 3000 server ticks: the
  server keeps ticking, the client stays in game in the town with no
  rejected message, and step 10 freed the never-visited Cold Plains.
  The synthetic data has no two adjacent levels, so it does not walk
  across a border (see "What's left").

## Local checks for the user (game files)

1. The live run that panicked (LOCAL-RUN 4.2):

   ```
   RUST_BACKTRACE=1 cargo run -p d2-client --release -- play --frames 3000
   ```

   Expect: no panic and no `server thread stopped`; log lines every 250
   frames with ≈ 25 server ticks per second up to ~3000 frames; exit 0.
   If it still panics with `live DRLG room`, record the whole backtrace
   (the frames above `d2_sim::drlg::room` name the caller) and the
   level: add `RUST_LOG=debug`.

2. The outdoor generation check on the user's tables (unchanged test;
   it touches the changed module, so re-run it):

   ```
   cargo test --release -p d2-server outdoor_levels_generate_through_the_dispatcher -- --ignored --nocapture
   ```

   Expect: passes as before.

3. Town → Blood Moor on foot (G18), once S-A/S-D land: start `play`,
   walk out of the Rogue Encampment's east gate into the Blood Moor and
   back, wait in town ≥ 30 s. Expect no panic; the Blood Moor / Cold
   Plains rooms free and regenerate silently.

## What's left

- A walk across a real level border in the app needs synthetic data with
  two adjacent levels (vis + warp −1 border rooms). That data is in
  `crates/d2-client/src/app/single_player.rs` (`synthetic_drlg_data`,
  `synthetic_types`), owned by S-D. Seam request: add a level 2 room
  next to the town room (tile (8, 0, 8, 8)), `vis` 1 ↔ 2, and let
  `app_level_border.rs` walk the player into it with C→S 0x01.
- `levels.md` §9.4 does not say whether the original's outdoor reset
  frees per-room data; the record removal is the smallest change that
  keeps §9.4's order. Spec question for the next RE session: what
  `0x006754C0` frees (the existing TODO in `Outdoor::reset_level`).

## Gate run in this session

`cargo fmt --all --check` clean; `cargo clippy -p d2-sim -p d2-client
--all-targets -- -D warnings` clean; `cargo test -p d2-sim` 4465 passed,
0 failed; `cargo test -p d2-client --test app_level_border` passed
(85 s in a debug build: 3000 ticks through the Bevy app);
`python3 tools/coverage.py --check` 0 errors.
