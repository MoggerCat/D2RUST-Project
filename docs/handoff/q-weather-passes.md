# q-weather-passes: weather frames, water floor, draw passes 4 and 9

Branch `claude/q-weather-passes`. Provisional note: **REC-272** (docs/HANDOFF.md §7).

## Links connected

1. `LevelRow` now carries `Rain` and `Mud` (`bridge/world.rs`, filled in
   `single_player::client_level_rows`).
2. `world_view/weather_view.rs` (new): `WeatherView` owns `Weather`, the
   floor context and the weather seed. `prepare` (called from
   `ModelFeed::prepare`) runs act load / level entry, makes the eight
   overlay DC6 sets (`Rain1-4`, `bubble1-4`) resident, and runs the §11.2
   weather update once per server tick.
3. `ModelFeed::weather_frame` lends the state; with weather on, the
   preview no longer dries the water floors, so the floor pass (§11.5)
   spawns splashes and bubbles.
4. `rules/draw_order/sky.rs` + `source.rs`: `WeatherFrame::sky_passes`
   runs `pass4` and `pass9` (replacing the "not wired" error when a
   `SkyFrame` is given); `OrderedSource::sky` carries the result.
5. `ViewFeed::sky_items` (default: refuses non-empty draws) and
   `build_frame` add the items at pass 4 / pass 9 keys and re-sort.
   `ModelFeed::sky_items` -> `WeatherView::items`: pool cels (mode 3,
   unlit), line pixels (1x1 frame mapped to the color), the flash
   rectangle (mode 5).

## Tests

`rules::draw_order::tests::ordered_source_runs_passes_4_and_9`,
`world_view::weather_view::tests::*` (rain level draws lines per tick,
no rain no particles, flash when lightning is on, pool cels at pass 4).

## PROVISIONAL / left

See REC-272. Lightning is never started in `play` (no caller specified,
`draw-order-2.md` OQ4). Sound for thunder is deferred.

## Local check

`cargo run -p d2-client --release -- play`, then go to a level whose
`Levels.txt` `Rain` is 1 (or `Mud` for bubbles; snow levels in Act 5):
rain / snow lines should fall and splashes appear on water floors. Log
lines starting `preview (d2rs-own, unverified): skipped: weather` name
a missing overlay file. Which levels have `Rain` / `Mud` was not checked
here (no game files).
