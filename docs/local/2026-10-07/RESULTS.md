# Local run 2026-10-07 (C46.1, G1)

Branch `claude/local-2026-10-07` from `origin/claude/specs-staging-7` @ `043d2be`.
`D2_GAME_DIR` = the 1.14d install (`game/`), `RUST_BACKTRACE=1`. Logs in this
folder are our programs' output only (counts, paths, Rust backtraces).

## Run 1: format sweep (LOCAL-RUN 2.11, C46.1)

`cargo test --release -p d2-formats --test game_sweep -- --ignored --nocapture` → `sweep-log.txt`

**9 passed, 2 failed** (44.85 s). Both failures are count assertions; neither names a file.

1. `ds1_every_file_parses`, `crates\d2-formats\tests\game_sweep.rs:284:5`:
   ```
   ds1: 2372 files, versions {3: 1, 8: 6, 12: 14, 13: 36, 15: 13, 16: 229, 17: 147, 18: 1926}
   assertion `left == right` failed
     left: 2372
    right: 2456
   ```
2. `dt1_every_live_file_decodes`, `crates\d2-formats\tests\game_sweep.rs:258:5`:
   ```
   dt1: 250 live files, 15873 tiles, version-4 [...]
   assertion `left == right` failed
     left: {1: 226996, 4097: 108905, 8197: 15712}
    right: {1: 226996, 4097: 110259, 8197: 15712}
   ```
   Block format 4097 (RLE) counts 1,354 fewer blocks than expected; formats 1 and 8197 match. 250 live files
   (cf. `render/camera.md` OQ7: 250 vs 251 files).

Passed: expfield_layout, animdata_real_vectors, animdata_matches_every_cof, font_tables_every_file_parses,
cof_every_live_file_parses, palettes_every_file_parses, string_tables_every_key_resolves, dc6_every_file_decodes
(1653 files, 26317 frames), dcc_every_file_decodes (21717 files, 3305132 frames).

## Run 2: client crash trace (first-playable gap G1)

`cargo run -p d2-client --release -- play --frames 3000` → `play-log.txt`; exit code **101**.

- Data line: `play: game data from D2_GAME_DIR (137 levels, 573 objects, waypoint object class 119; level files: 2043 DS1, 34 lvlsub DS1, 241 DT1)`
- Seed line: `single player: seed 1234, waypoint unit UnitId(0) (GUID 1)`
- Last progress line before the panic (≈5 s after the window opened): `frame 250: 104 server ticks, 1 units in the model, unowned S→C ids {}; … units_drawn: 0, units_hidden: 1 … gpu: true`
- **Panic:** `thread 'd2-server' panicked at crates\d2-sim\src\drlg\room.rs:152:44: live DRLG room`

Backtrace frames in our crates (top first):
```
 4: <d2_sim::drlg::level::Drlg>::level_rooms
 5: <d2_sim::drlg::outdoor::Outdoor>::reset_level
 6: <d2_sim::wiring::worldgen::levels::WorldTypes as d2_sim::drlg::seams::LevelTypes>::reset_level
 7: <d2_sim::wiring::worldgen::levels::SharedTypes as d2_sim::drlg::seams::LevelTypes>::reset_level
 8: <d2_sim::drlg::level::Drlg>::free_level_rooms
 9: <d2_sim::drlg::level::Drlg>::free_inactive_levels
10: <d2_sim::wiring::action::DrlgWorld>::free_inactive_rooms
11: <d2_sim::wiring::worldgen::dispatch::WorldSim<d2_client::app::single_player::LocalSeams> as d2_sim::tick::TickHooks>::free_inactive_rooms
12: d2_sim::tick::tick::<d2_sim::wiring::worldgen::dispatch::WorldSim<d2_client::app::single_player::LocalSeams>>
13: d2_server::adapters::handlers::world::wired::WorldHost<…>::run   (mangled in the log)
14: d2_server::adapters::sim::SimGame<…>                           (mangled)
15: d2_client::bridge::local::LocalLink<…>                         (mangled)
16: d2_client::app::server_thread::serve<…>                        (mangled)
```
Then the client side: `server: server thread stopped: no answer from the server thread` from
`d2_client::bridge::mirror::bridge_frame`, and the process exits with 101.

Reading: during the tick's inactive-room free, `free_level_rooms` calls the outdoor `reset_level`, which calls
`level_rooms` after (some of) the level's rooms were already freed; `level_rooms` expects every listed room to
be live (`room.rs:152`, `expect("live DRLG room")`). Order of free vs reset in `free_level_rooms` is the place to look.

## Run 3: game_core (LOCAL-RUN 2.6, C37)

`cargo test --release -p d2-sim --test game_core -- --ignored --nocapture` → `game-core-log.txt`

**12 passed, 0 failed** (0.07 s).
