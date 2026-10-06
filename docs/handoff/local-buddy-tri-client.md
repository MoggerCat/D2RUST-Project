# local-buddy tri-client (2026-10-07)

Lane A re-run of the d2-client rows of `triage-game-findings.md` (origin/main 06a9726), 1.14d install, AMD RX 9070 XT.

## 1. game_assets (release, --ignored, --test-threads 1)

6 passed, 0 failed (exit 0, 39 s). Per test, all PASS:
`cpu_compositor_on_real_frames` (24 items, 804 map rows, 40858 non-zero pixels; now passes),
`each_loader_loads_a_real_file`, `every_listed_name_canonicalizes_and_reads_back`,
`every_pl2_cof_and_tbl_loads`, `frame_sets_and_residency_on_a_real_dcc` (16 directions), `gpu_compositor_on_real_frames`.

- Adapter: `AMD Radeon RX 9070 XT (Vulkan, DiscreteGpu, driver AMD proprietary driver 26.8.1 (LLPC))`.
- Names: 32502 listed, 32502 read back identical, 0 refused.
- every_pl2_cof_and_tbl_loads: 16 .pl2, 3512 .cof (tolerated failure: `am/cof/amblxbow.cof`), 36 .tbl (14 font, 20 strings). The test printed `Format(Truncated)` TblLoader errors for font tbls `DEFAULT.TBL` (offset 2303) and `FONTER.TBL`, yet the test passes (tolerated / print-only); worth a look.

Claim lines in `crates/d2-client/tests/game_assets.rs` (5) changed from "Intended claim (unconfirmed...)" to `// Covers:`; `py tools/coverage.py --check`: 5242 claims, 0 errors.

## 2. play --frames 1500 (RUST_BACKTRACE=1)

FINDING: the panic recurs. Exit code 101 (cargo "process didn't exit successfully"); window closed, no process left.

- `play: game data from D2_GAME_DIR (137 levels, 573 objects, waypoint object class 119; level files: 2043 DS1, 34 lvlsub DS1, 241 DT1)` -> n=137, m=573, k=119, a=2043, b=34, c=241.
- `single player: seed 1234, player unit UnitId(1), waypoint unit UnitId(0) (GUID 1)`.
- Only log line before the panic (frame 250): 106 server ticks, 0 units in the model, last view bridge_frame 249 / server_tick 105, items 0, gpu true, node frames 82, audio presented 106, decodes 0, load_errors 0, engine_errors 0. The panic came after that, before the frame-500 line; the exact tick is not printed (earlier cloud estimate ~104; consistent with roughly 106-110).
- Panic: thread `d2-server`, `crates\d2-sim\src\drlg\room.rs:144:44` "live DRLG room". Then the client reports `server thread stopped: no answer from the server thread`.
- Frames above room.rs:144 (innermost first): `Drlg::level_rooms` <- `Outdoor::reset_level` (drlg/outdoor) <- `WorldTypes::reset_level` / `SharedTypes::reset_level` (wiring/worldgen/levels) <- `Drlg::free_level_rooms` <- `Drlg::free_inactive_levels` <- `DrlgWorld::free_inactive_rooms` (wiring/action) <- `d2_sim::tick::tick::<ActionSim<LocalSeams>>` <- `LocalLink` (d2_client bridge/local) <- `server_thread::serve`. (The release backtrace had no file:line for these.)
- Freed room's level id and the caller's identity were not printed (the panic message carries no id).
- Reading: `Outdoor::reset_level` calls `level_rooms` on a level while `free_level_rooms` is freeing it, so a room id (likely one already freed earlier in the same free pass, or a stale one in the level's list) is looked up after freeing. Consistent with the triage suspect (outdoor level reset after rooms freed); not yet confirmed which level.

Logs: `C:\Users\zffit\Desktop\D2test\out-tri-client\` (t1.log, p.log, p.err; not committed).
