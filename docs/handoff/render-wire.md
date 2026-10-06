# Handoff: the original's camera and placement wired into the app and the capture `scene` case — `claude/render-wire`

> Waiting to be folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md`; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: integration from
clear specs, medium (METHODS M14). Branch `claude/render-wire`, from
`claude/tender-meitner-mphas3` at `729c76e` (render-camera-placement,
render-composition, render-capture, p6-integrate merged). Repo only: no
`game/`, no recording, GPU = Mesa llvmpipe. Specs: `specs/render/camera.md`
(§1–§4, §8–§10), `specs/render/capture.md` (§3, §4, §6), with
`sprite-placement.md` through `rules::`. No spec, HANDOFF, PLAN, `rules/`,
`scene/`, `verify/mod.rs` or `Cargo.toml` edit.

## State

**Wired; checked in CI on synthetic data; unverified against 1.14d**
(M02: the capture runs are queued below).

1. **App world view through `rules::OriginalView`.** Every frame is built
   by `world_view::build_frame`: the camera once (camera §3) from a
   `ViewFeed`, then `world_view::build` over `OriginalView(camera, rules,
   feed)`. `WorldViewState` gained `feed: Box<dyn ViewFeed>`; `rules`
   keeps answering the delegated hooks (pose, component frame, draw keys,
   shading, blend, UI). Placement is no longer a hook in the app: the
   rules' own `tiles` / `place` are not called.
2. **The feed (`world_view/feed.rs`).** `ViewFeed: ViewSource` adds what
   the client world model does not hold: `player` (local player position,
   `None` = the model states none), `open_mode` (`ui/panels.md`), `shake`
   (`RunningShake { shake, start_tick }`) and `player_seed` (camera OQ6).
   All are `TODO(spec: …)` hooks. `NoFeed` (the app's placeholder): no
   player, no map tiles, no shake; open mode, seed, unit positions and
   offsets refuse. `frame_shake` runs the envelope on the d2rs time base
   (camera §9): `t = Shake::time_of(server_ticks − start_tick)`, `a = 0`
   or ended → `(0, 0)` with no draw, else `shake_offsets(a, seed)` (two
   draws). A shake starting after the frame's tick and the original's
   `t3 = 0` division are errors.
3. **No player → no camera (`NoCamera`).** Without the §3 origins nothing
   can be placed: any listed map tile and any unit the rules draw is an
   error naming `render/camera.md`; UI passes through. With `NoFeed` and
   `Unspecified` the app's frame stays the empty list (black), as before.
4. **One frame per tick (camera §9).** `world_view_frame` draws only when
   `ClientWorld::server_ticks` advanced since the last drawn frame (and
   never before tick 1): a Bevy frame without a tick keeps the presented
   image, positions are the snapshot after the tick, nothing is
   interpolated. UI input is queued until the next drawn frame (d2rs
   mechanism: the UI frame is part of the drawn frame). `FrameStats`
   gained `server_tick`.
5. **Capture `scene` case has a real scene source**
   (`verify/capture_case/scene_source.rs`, wired in `main.rs` `verify`):
   `WorldScene<W: CaptureWorld>` (a) builds the frame's camera from the
   record: player path `fixed` (16.16, camera §2), `open_mode`, frame
   size (800 × 600 only), recorded shake `(dx, dy)` as input (§9,
   capture.md §4); (b) checks the recorded `view_rect`, `shift_x`,
   `tile_origin`, `unit_origin` against `Camera::new` — a difference
   fails the frame (`SceneOutcome::Differs`, one line per value), before
   any pixel; (c) asks the `CaptureWorld` for units, positions, map tiles,
   UI, rules and assets; (d) builds through `OriginalView` +
   `world_view::build` into the compared `Built`. **Today's recordings
   stop at (c)**: `NotRecorded` returns `SceneOutcome::Seam` with
   `RECORDER_GAP` (below), reported `SCENE NOT WIRED` (exit 2, never a
   pass) after the camera check ran. `SceneNotWired` stays for tests.

## Seams (stopped, not invented)

- **Recorder gap** (`scene_source::RECORDER_GAP`, printed per case):
  `record_frames.py` records the camera only. To draw a frame it must add
  per frame: (1) act and level of the player (act palette `pal.pl2`,
  `composition.md` §4; level tile files; BlankScreen); (2) the map tiles
  drawn: cell, list (floor / wall / roof + DT1 roof height, camera §6),
  DT1 file, orientation, main, sub index; (3) the client units drawn:
  type, GUID, position (moving: path 16.16; static: subtiles), the extra
  offsets of `0x004DA0B0/0x004DA0D0/0x004DA0F0` (camera OQ3), mode, COF
  direction and frame, component tokens (`unit-composite.md`); (4) the UI
  drawn (control panel, open panels: image file and frame, position;
  text). Owner: capture.md §3 (spec session) + recorder. Draw order,
  shading and blend stay `TODO(spec)` after that.
- **App feed hooks**: local player position (S→C owner of positions),
  open mode (`ui/panels.md`), shake start (effect specs), player seed
  (camera OQ6), unit positions, unit offsets (`unit-composite.md`), map
  tiles (`draw-order.md`, DRLG → client). Until then the window is black.
- **Frame-cycle base in the compare**: `capture_case::compare_one`
  composes from index 0 (`scene::compose`). The original keeps the bottom
  47 rows of the previous frame (`composition.md` §3, capture.md edge
  cases). Once a world is recorded, the CPU half must use
  `scene::compose_frame` with `FramePlan` (BlankScreen of the level) and
  the previous captured draw as base (`SceneJob::previous`), and
  `verify::GpuJob` needs `base` + `plan` (`verify/mod.rs`, `verify/gpu.rs`:
  not owned here). Until then a frame whose bottom rows are not fully
  drawn differs there.
- **Pause**: the original draws every loop pass while a single-player
  game is paused (camera §9); d2rs draws only on ticks. Pause is not
  wired in the app; when it is, the gate needs a "paused" input.
- **Recorded `player.client` (path `+8/+0xC`)** is read but not checked:
  no spec ties it to camera §2. A spec session can state the relation.

## Code map rows

| Path | What | Spec |
|---|---|---|
| `crates/d2-client/src/world_view/feed.rs` | `ViewFeed`, `NoFeed`, `RunningShake`, `frame_shake`, `frame_camera`, `build_frame`, `NoCamera` | camera §3, §8, §9, §10 |
| `crates/d2-client/src/world_view/feed/tests.rs` | no player → no camera / errors; camera from the feed; shake on the tick time base | camera §3, §8, §9 |
| `crates/d2-client/src/world_view/present.rs` | `WorldViewState { feed }`, `FrameStats.server_tick`, draw once per tick through `build_frame` | camera §9; render-pipeline §A1 |
| `crates/d2-client/src/app/play.rs` | `add_game` inserts `NoFeed` | bridge §8 |
| `crates/d2-client/src/verify/capture_case/scene_source.rs` | `WorldScene`, `CaptureWorld`, `RecordedScene`, `WorldAnswer`, `NotRecorded`, `RECORDER_GAP`, `recorded_camera`, `camera_differences`, `built` | capture §3, §4, §6; camera §1–§3 |
| `crates/d2-client/src/verify/capture_case/scene_tests.rs` | synthetic 800 × 600 capture end to end (CPU + GPU), perturbation, camera differences, seam | capture §6 |
| `crates/d2-client/src/verify/capture_case.rs` | `SceneOutcome::{Seam, Differs}`, `SceneSource::take_notes` | capture §6 |
| `crates/d2-client/src/main.rs` | `verify` scene cases on `WorldScene::new(NotRecorded)` | render-pipeline §A10 |

## Signature changes

- `WorldViewState::new(assets, rules, feed)` (was 2 args).
  `FrameStats` has `server_tick`.
- `SceneOutcome` has `Seam(String)` and `Differs(Vec<String>)`;
  `SceneSource::take_notes` (default empty). `capture_tests.rs` unchanged.
- `rules/` unchanged (no hook signature needed changing).

## Tests (CI and llvmpipe)

- `world_view::feed::tests` (3), `verify::capture_case::scene_tests` (5
  + 1 ignored GPU), `world_view::tests::bevy_frame_presents_the_cpu_image`
  (now through `TestFeed`), `app_frame_loop.rs`: existing loop test
  updated to the tick gate (no draw before tick 1, frame 3 without a tick
  keeps tick 1's frame); new `frame_loop_draws_each_tick_through_the_original_view`
  (walking player, 5 ticks with 5 tickless frames between: the feed asked
  once per tick `[1, 2, 3, 4, 5]`; presented image = hand-painted from
  camera §3/§6 coordinates) and `frame_loop_shakes_on_the_tick_time_base`
  (a = 4, 8, 10 at ticks 1–3; seed advanced two draws per drawn frame;
  presented image = CPU reference with tick 3's origins); GPU node and
  text tests now through `OriginalView`.
- The synthetic capture (4 draws of a walk, 3 compared): expected frames
  painted from the spec's coordinates, not from the pipeline. CPU `0 of
  480000`, palette 0 of 256; `--perturb 1 / 7 / 64` → every frame `CPU`,
  `GPU indices`, `GPU` exactly N; a recorded tile origin or view rect off
  by one → FAIL naming the value; a frame painted one pixel off → `CPU:
  12 of 480000`.
- llvmpipe (Mesa 25.2.8, LLVM 20.1.2, Vulkan): ignored
  `gpu_half_matches_the_synthetic_capture` → `GPU indices: 0`, `GPU: 0 of
  480000 pixels differ`, perturb 7 → 7 on all three; `app_frame_loop`
  6 pass (GPU node 0 differing pixels).
- M08 by hand: removing the tick gate fails 3 app tests; dropping the
  shake offsets from the camera fails `frame_loop_shakes_on_the_tick_time_base`.

`Covers:` claims: camera §3, §4, §6, §8, §9, §1; capture §3, §6 (unit tier,
synthetic).

## Local run queue (add to `docs/HANDOFF.md` §5)

1. **Window with real files** (A/C, no player needed):
   `D2_GAME_DIR=<game> cargo run -p d2-client --release -- play --frames 1500`.
   Expect as `p6-integrate.md` check 1 (black window: `NoFeed` states no
   player), and in the log lines `last view Some(FrameStats {
   bridge_frame: <f>, server_tick: <t>, items: 0, … })` with `t` equal to
   that line's server ticks (one drawn frame per tick), no error, exit 0.
   Record the node frame count (now at most the tick count).
2. **Capture, camera check today** (A, needs the player, Windows): record
   each case of `capture.md` §8 with `py
   tools/trace-recorder/record_frames.py --seconds 30` (camera-0001: Rogue
   Encampment, walk 5 s, run 5 s, stand; placement-0001: stand, open and
   close the inventory, stand), then right after each:
   `cargo run --release -p d2-client -- verify --cases
   crates/d2-client/capture-cases --case camera-0001` (then
   `placement-0001`, `composition-0001`). Expect: no ERROR; on the first
   frame `camera: recorded view rect, shiftX, tile origin [..] and unit
   origin [..] equal camera.md §1, §3`, then `scene: seam: the recording
   holds the camera only …`; `frames: 0 match, 0 differ or fail, N scene
   not wired`; `SCENE NOT WIRED <id> (scene)`; exit 2. Inventory frames
   of placement-0001 must pass the camera check with view rect
   `[-200, 0, 600, 560]`, shift −200 (mode 1). **Any FAIL** with lines
   `tile origin (camera.md §3): recorded …, rule …` (or view rect, shiftX,
   unit origin) is a camera.md finding: record the lines, the draw and
   the frame's `shake` and `open_mode`; do not loosen the check. A
   `camera: … no player record` ERROR means the recorder took a frame
   without the player (record the draw). `--perturb N` cannot fail these
   frames (no pixel is compared at the seam); it applies once a world is
   recorded.
3. **Real GPU**: `cargo test -p d2-client --lib
   verify::capture_case::scene_tests::gpu_half -- --ignored --nocapture`
   → `adapter: <real GPU>`, pass; `cargo test -p d2-client --test
   app_frame_loop -- --nocapture` → 6 pass, 0 differing pixels.
4. **After the recorder gap closes** (capture.md §3 change + recorder):
   the same verify commands → `PASS`, and `--perturb 5` → every frame
   `CPU: 5 of 480000 bytes differ`, FAIL, exit 1. Needs the frame-cycle
   base seam above first.

## Gate

`sh tools/gate.sh all`: see the commit message for the result.
