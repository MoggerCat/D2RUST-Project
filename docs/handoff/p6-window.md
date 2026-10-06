# Handoff: the client on a real window (`claude/p6-window`, 2026-10-06)

> Not folded into `docs/HANDOFF.md` and `docs/PLAN.md` yet (a docs session folds it); this file stays as the detailed record.

Scope: HANDOFF §2 steps 4–5 (in-app wiring of the single-player game and
the GPU compositor). Cloud implementation session, task class:
integration from clear specs, medium (METHODS M14). Base: `main` at
`edad871`. Specs: `specs/client/bridge.md` §3, §7, §8;
`specs/client/render-pipeline.md` §A1, §A9; `specs/world/waypoints.md`
§5.1 (the waypoint object row). These are d2rs-own design drafts plus one
1.14d rule (§5.1 rule 1). **No original rendering rule is added**: every
§B point stays a `TODO(spec: …)` hook (`world_view::Unspecified`).

## State

**Implemented, checked in CI by headless tests; the GPU half is checked on
Mesa llvmpipe only (unverified on a real GPU, METHODS M02).**

`cargo run -p d2-client` (subcommand `play`, now the default) opens a
window that runs the local single-player game in-process:

- the game is `SimGame<ActionSim<LocalSeams>, ActionWorld>` (the wiring
  of `bridge/local_tests.rs`: two-act synthetic DRLG, a sorceress beside a
  Cold Plains waypoint, 0x49 on its real provider), behind
  `d2_server::host::Host` in `bridge::local::LocalLink`, on a **server
  thread** (`app::server_thread::ThreadLink`, see Send + Sync below),
  boxed into `BridgeResource`;
- each Bevy frame: `BridgePlugin` pumps the server in `PreUpdate`
  (drain → tick → flush → receive, `bridge.md` §8), then the world view
  builds the frame from the client world model in `Update`, packs it, and
  hands it to a **render-graph system** that runs the compute compositor
  on Bevy's own device (`Gpu::from_device(render_device.wgpu_device()
  .clone(), (**render_queue.0).clone())`) and copies its RGBA rows
  straight into the presented texture (no readback);
- with `D2_GAME_DIR` set (and no `--synthetic`), the `levels` and
  `objects` tables come from the user's files (`d2_data::bin::load`) and
  the waypoint object is the first `objects` row with operate function 23
  and init function 17 (`waypoints.md` §5.1 rule 1). The DRLG stays
  synthetic: live level generation needs the DS1 providers (HANDOFF §2
  step 7), so no level is generated from game files.

What the window shows: black. No S→C id has an owner spec, so the model
stays empty; `Unspecified` draws nothing; the frame palette is all zeros
(`ViewAssets::palette`, TODO(spec: render/shading.md) §B3). The log line
every 250 frames shows bridge frames, server ticks, unowned S→C ids, the
last `FrameStats` and the node's frame count.

Measured here (cloud, Xvfb + llvmpipe, `xvfb-run cargo run -p d2-client --
play --frames 1000`, synthetic): 1000 frames, 419 server ticks (25 per
second at ~60 fps), `gpu: true`, node frames 999, exit 0.

Tests: `cargo test -p d2-client` (lib 205 pass / 5 ignored; `app_frame_loop`
2; `app_single_player` 4 + 1 ignored; e2e unchanged); `cargo test
--workspace` 0 failures; on llvmpipe the ignored GPU tests
(`gpu_matches_cpu`, `gpu_perturb_reports_exactly_n`,
`gpu_half_matches_cpu_on_every_synthetic_case`) and `gpu_compare`
(12/12, `--perturb 7` exactly 7 each) still pass after the `device.rs`
refactor.

### Send + Sync of the game (HANDOFF §2 step 4)

`SimGame<ActionSim<_>, ActionWorld>` is **not `Send`**:
`d2_sim::wiring::action::DrlgWorld` holds `Box<dyn TileSource>` and
`Box<dyn LevelTypes>` without a `Send` bound, and the world-generation
wiring shares its level types as `Rc<RefCell<WorldTypes>>`
(`wiring::worldgen::SharedTypes`), so `WorldSim` cannot be `Send` either.
Instead of changing `d2-sim`, the game is built on and never leaves a
dedicated thread: `ThreadLink<L>` is `Send + Sync` for any `L` and
forwards each `ServerLink` call synchronously (request, then answer), so
call order is the caller's and the thread adds no concurrency or clock
(`bridge.md` §3 rule 3: the link inside owns the host clock). Tested:
`the_link_is_send_and_sync`.

### Bug found and fixed in the staging (METHODS M21)

The first window run panicked on the server thread after ~110 ticks:
`units/lists.rs` `r()` "linked room exists" from `queue_update` in the
tick's client update. Cause: I copied `bridge/local_tests.rs`'s join
(`SimGame::join(…, Some(room), …)`), which skips the client room switch
(`rooms.md` §4.1); the DRLG then counts the player's room as clientless,
and tick step 9 (`rooms.md` §7.2, §8) frees it under the player.
`e2e_single_player.rs` joins with room `None`, so the first tick runs the
switch; the app now does the same. Caught where: a manual window run.
Check that catches it at the source: `app_frame_loop` now runs 300 ticks
(it panics with the old join; confirmed). Latent elsewhere (not my file):
`bridge/local_tests.rs` keeps `Some(rooms[0])` but runs ≤ 2 ticks.
Question Q1 below for the sim.

## Code map rows

| Path | What | Spec |
|---|---|---|
| `crates/d2-client/src/app.rs` | module doc; `pub mod play, server_thread, single_player` (map preview / verify unchanged) | `render/map-preview.md` |
| `crates/d2-client/src/app/server_thread.rs` | `ThreadLink<L>` (`spawn(build)`, `with(f)`, `ServerLink`), `ThreadStopped` | `client/bridge.md` §3, §8 |
| `crates/d2-client/src/app/single_player.rs` | `Sim`, `Link<C>`, `LocalSeams` (`Pending` + `Outbox`: stored positions / interaction, outbox, warp and arrival logged), `WaypointTables::{synthetic, live}`, `GameData`, `build`, `LocalGame`, `start` → (`ThreadLink`, `Started`), `BuildError`; constants `COLD_PLAINS`, `ACT2_TOWN`, `DEFAULT_SEED`, … | `client/bridge.md` §3, `world/waypoints.md` §5.1, §6 |
| `crates/d2-client/src/app/play.rs` | `add_game(app, link, gpu)` (bridge + world view + progress log, shared by window and tests), `unspecified_palette`, `PlayConfig`, `run` | `client/bridge.md` §7, §8; `render-pipeline.md` §A1, §A9 |
| `crates/d2-client/src/world_view/node.rs` | `ComposeJob` (extracted resource: seq, packed, pages + version, palette, target), `NodeRuns`, `add_node`; render-graph system `compose_node` (`RenderGraph` schedule, `RenderGraphSystems::Render`, before `camera_driver`): owns the atlas array (re-upload on version change), `Gpu::encode_rgba` into Bevy's encoder, one buffer→texture copy per row | `render-pipeline.md` §A9 |
| `crates/d2-client/src/world_view/present.rs` | GPU path now: main world `WorldViewGpu { atlas, pages, sets, seq }` packs and inserts a `ComposeJob`; the presented image is a target texture (+ `COPY_SRC`) the main world never rewrites; CPU path unchanged (writes `image.data`) | `render-pipeline.md` §A1, §A9 |
| `crates/d2-client/src/main.rs` | `play` subcommand (default): `--seed`, `--frames`, `--synthetic` | |
| `crates/d2-client/tests/app_frame_loop.rs` | headless frame loop (CPU path; server ticks, 0x49 → S→C 0x0D received as unowned, 300 ticks, presented image = CPU reference); GPU node test (DefaultPlugins without winit; node output read back = CPU reference byte for byte; skipped with a message when no adapter) | |
| `crates/d2-client/tests/app_single_player.rs` | `Send + Sync`, failing builder, deterministic build on the thread, live tables (ignored) | |

## Signature changes (outside my files; METHODS M21)

- `gpu_compositor::device::Gpu` gained `atlas_texture(&self, pages) ->
  wgpu::Texture` and `encode_rgba(&self, encoder, packed, atlas, pages,
  palette) -> Result<Option<wgpu::Buffer>, GpuError>`. `run` (behind
  `compose` / `compose_rgba`) now calls the shared private `encode`; its
  behavior is unchanged (gpu tests and `gpu_compare` re-run on llvmpipe).
- `world_view::GpuAtlas::sets()` (new; `mod.rs`).
- `world_view::present::WorldViewGpu`: field `gpu: Gpu` removed (the
  compositor lives in the render world now); fields `pages`, `sets`, `seq`
  private. Nothing outside `present.rs` used it.
- `d2-client` `Cargo.toml`: `d2-sim`, `d2-data` moved from
  dev-dependencies to dependencies (the app builds the game). depcheck OK.
- `cargo run -p d2-client` without a subcommand now runs `play`, not
  `view` (`view` stays as a subcommand).

## Seams reached (stopped, not invented)

- Every §B rendering rule: `Unspecified` (no tiles, no units drawn, UI
  hooks error), palette all zeros (§B3). The window is black by design.
- No S→C id has an owner spec: the model stays empty; 0x0D is counted as
  unowned.
- Live levels: the DRLG is synthetic even with `D2_GAME_DIR` (DS1
  providers, HANDOFF §2 step 7). Only `levels` / `objects` are live.
- Session spec (create / join / leave): the game is built and joined
  before the link, as in the tests (`PendingSession`).
- Input: no keyboard / mouse intent leaves the client (C9 controls are
  not wired to intents; UI events go to `UiRoot`, none of which exists in
  play mode).
- Atlas eviction (C2) is not wired: a full atlas is an error.
- Frame store: this branch builds none. Frame ids come from the existing
  `world_view::FrameTable` and `composite::ComponentResolver::frame_id`
  hook. The parallel session `verify-map` owns the frame store
  ((FrameSetKey, index) → `scene::FrameId`); after that merge, the app's
  world view should switch to it.

## Questions

- Q1 (sim, `rooms.md` §7.2 / §8, tick step 9): should removing a room
  that still holds units be possible? `free_room` documents "its units
  must be gone", but step 9 freed the player's room (clientless in the
  DRLG's view) and the next client update panicked on the dangling room.
  If 1.14d cannot reach this, the sim should report it as an error
  instead of panicking; if it can, the units' fate belongs in the spec.
- Q2: presentation stays as in `p6-world-view` (sprite on layer 31, own
  camera, integer scale); the node writes the texture before the camera
  driver. Whether a dedicated present pass (no sprite) is wanted is open.

## Local checks to queue

1. Window on a real GPU, synthetic tables:
   `cargo run -p d2-client --release -- play --synthetic --frames 1500`.
   Expect: a black 800×600 view in the window; log lines every 250
   frames with `server ticks` ≈ 25 per second, `gpu: true`, `node frames`
   = frames − 1 (or −2), no error; exit 0. Record the adapter line
   (`bevy_render::renderer: AdapterInfo { … }`).
2. Window with the user's tables: `D2_GAME_DIR=<game> cargo run -p
   d2-client --release -- play --frames 1500`. Expect the line `play: game
   tables from D2_GAME_DIR (<n> levels, <m> objects, waypoint object class
   <k>)`, then as in 1. Record n, m, k.
3. `D2_GAME_DIR=<game> cargo test -p d2-client --test app_single_player
   -- --ignored`. Expect `live_tables_give_a_waypoint_object` to pass.
4. GPU node byte check on the real GPU: `cargo test -p d2-client --test
   app_frame_loop -- --nocapture`. Expect `adapter: <name> (<backend>,
   <driver>)` (not llvmpipe) and both tests `ok` (0 differing pixels).

## Gate results (this branch, cloud)

`cargo fmt --check` OK; `cargo clippy --workspace --all-targets -- -D
warnings` OK; `cargo test --workspace` 0 failures; `cargo run -p
depcheck` OK (8 crates); `python3 tools/spec_index.py --check` OK;
`python3 tools/methods.py check` 21 OK; `python3 tools/coverage.py
--check` 3198 claims, 0 errors; `--selftest` ok. Coverage claims added:
`bridge.md §8 r1, §8 r2, §8 r3` (`frame_loop_ticks_the_server_and_feeds_the_world_view`:
one bridge frame per update with frames / ticks counted, no tick in a
short frame and one tick in a long one, an intent sent between frames k
and k+1 answered in frame k+1). No `§a9` claim: the GPU test checks the
node's output, not the whole §A9 unit (presentation scaling is not
checked).

Environment note: the GPU half ran on Mesa llvmpipe 25.2.8 installed
with `apt-get install mesa-vulkan-drivers libvulkan1` (not part of
`tools/cloud-setup.sh`); the window run used `xvfb` and
`libxkbcommon-x11-0` from apt. Without them the GPU test prints
`skipped: no GPU adapter` and passes.
