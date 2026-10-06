# Handoff: Phase 6 world view (`d2_client::world_view`)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Scope: branch `claude/p6-world-view`, from `main` at `fd37fba`
(2026-10-06, cloud, repo only). Specs: `specs/client/render-pipeline.md`
§A1 (stages 1–5), §A6–A9; `specs/client/bridge.md` §5, §7 rule 4, §8;
`specs/client/ui.md` §A2, §A4. These are d2rs-own design drafts. No owner
spec of §B exists, so this session reproduces no original behavior.

## State

**Implemented: the client's model is now connected to its presentation.
Checked by synthetic vectors in CI. No original behavior.** Each frame
runs: bridge frame (`PreUpdate`, existing) → UI (input routed, intents
forwarded, draw requests) → `world_view::build` (map tiles, units through
the C7 COF composite, UI items, then the C4 stable sort) → compose (the
GPU compute compositor on Bevy's device when there is one, else the CPU
reference) → an 800×600 RGBA image shown by a sprite at the integer
presentation scale. Every original rule goes through a hook trait. The
neutral implementation `Unspecified` draws nothing the model does not
state. With a rule it does not have, it errors; it never guesses. Tests:
`cargo test -p d2-client --lib world_view` (9 pass). Full crate: 205 lib
tests pass, 5 ignored.

Changes outside `crates/d2-client/src/world_view/`:
- `lib.rs`: `pub mod world_view;` and two doc lines.
- `app.rs`: in `Mode::View`, `add_plugins((BridgePlugin,
  WorldViewPlugin::default()))`. Both plugins stay inert until
  `BridgeResource` and `WorldViewState` exist, and today no app code
  inserts them. The map preview and verify paths are unchanged.

No edits to bridge, scene, composite, frames, assets, ui or
gpu_compositor internals. No new accessors were needed: every input is
already public (`Bridge::world`, `ClientWorld` fields, `composite::build`,
`scene::{order, compose_rgba, bin}`, `gpu_compositor::{pack, Gpu}`,
`Atlas::insert_set`, `UiRoot::{pump, forward, draw}`). No dependency,
spec, `docs/HANDOFF.md` or `docs/PLAN.md` edit.

## Code map rows

| Path | What | Spec |
|---|---|---|
| `crates/d2-client/src/world_view/mod.rs` | `ViewAssets` (COFs by `CanonicalPath`, resident `FrameSet`s by `FrameSetKey`, `MapTable`, palette), `UnitPose`, `TileDraw`, trait `ViewRules` (hooks), `Unspecified`, `FrameTable` (FrameId = n-th distinct (set, frame) in build order) + `BoundFrames` (scene `FrameSource`), `WorldFrame`, `build`, `compose_cpu`, `GpuAtlas` (`ensure`, `pack`, `compose`) + `FrameSlots` (`SlotSource`), `ViewError`, `VIEW` | render-pipeline §A1, §A6–A9 |
| `world_view/ui_bind.rs` | trait `UiRules` (hooks), `UiSprite`, `ui_items` (UI requests → items, key `(ui_pass,0,0,0)`, emission order), `clip_rect`, `UiQueue` (`UiInput`), `UiFrame`, `run_ui` (pump with `UiCtx` from the bridge's world, `forward` through the bridge, `draw`) | ui §A2, §A4 |
| `world_view/present.rs` | Bevy edge: `WorldViewPlugin { gpu }`, `WorldViewState` (assets + `Box<dyn WorldRules + Send + Sync>` + `FrameStats`), non-send `WorldViewUi` (root, strings, queue), `WorldViewGpu` (`Gpu::from_device` on Bevy's `RenderDevice`/`RenderQueue` + `GpuAtlas`), `WorldViewTarget`; systems `init_gpu → ui_input → world_view_frame → present_scale` in `Update`, run only when `BridgeResource` and `WorldViewState` exist | render-pipeline §A1 stages 4–5, §A9 |
| `world_view/tests.rs` | 9 tests (below) | |

## Design choices (ours, inside the specs' latitude)

- Build order: tiles, then units in `ClientWorld::units` key order
  (BTreeMap), each unit in COF slot order, then UI requests in emission
  order. After that, `scene::order` does a stable sort. Draw order is the
  key order; ties keep build order.
- `FrameId`s are assigned per frame in build order. They do not depend on
  what else is resident, so the same model gives the same list.
- Residency: `ViewAssets` holds what is resident. A missing COF, set or
  frame index is a `ViewError` naming it (render-pipeline §Edge cases:
  never a skipped draw). Loading through `assets::cache::Pool::resolve` is
  not wired (see seams).
- GPU path: `GpuAtlas` inserts whole frame sets on first use, in frame-id
  order (deterministic packing). `pack` re-bins with `scene::bin`. A full
  atlas is an error: page eviction belongs to C2 and is not wired. A
  failed GPU frame is an error with no CPU fallback (M07). Only one image
  is computed per frame: the GPU one when `WorldViewGpu` exists, the CPU
  one otherwise.
- Presentation: a sprite on its own render layer (31) and its own camera
  (order 1). Scale = `ui::Presentation::scale / window.scale_factor()`,
  nearest sampling, centered. This is outside the verify boundary (§A9).
  Bars come from Bevy centering, not the rounded-down bars of
  `Presentation`. See open question 3.
- UI input edge: cursor in frame coordinates (`ui::edge`), reported once
  per change. Pointer buttons are routed as-is. A window below 800×600 is
  treated as "outside the frame" (no input) instead of an error each
  frame. Keyboard actions (C9 controls → `UiEvent::Action`) are not wired
  in this edge.
- The UI context `tick` is `ClientWorld::frames` (bridge frames,
  bookkeeping only, `bridge.md` §5 rule 3).
- Events no panel takes are returned in `UiFrame::unhandled` and counted.
  They are never turned into world intents here.

## `TODO(spec: …)` hooks (narrowest neutral behavior of `Unspecified`)

| Hook | Neutral behavior | Owner |
|---|---|---|
| `ViewRules::tiles`: which map tiles, placement, keys, shade, blend (the model holds no map yet) | no tiles | `render/draw-order.md`, `render/camera.md`, DRLG (§B6, §B7, §B10) |
| `ViewRules::unit_pose`: COF, COF direction and frame a unit shows | `None`: unit not drawn (the model states nothing about appearance) | `render/unit-composite.md` (§B4) + owner specs of the S→C messages |
| `ViewRules::unit_params`: pass/major/minor, clip | error | `render/draw-order.md`, `render/camera.md` (§B6, §B7) |
| `ViewRules::component_frame`: component file, file direction, frame; whether a slot is drawn | error | `render/unit-composite.md` (§B4) |
| `ViewRules::place`: unit position + frame offsets → screen top-left | error | `render/sprite-placement.md`, `render/camera.md` (§B1, §B7) |
| `ViewRules::shade` | error | `render/shading.md`, `render/unit-composite.md`, `render/lighting.md` (§B3, §B4, §B8) |
| `ViewRules::blend` | error | `render/blend-modes.md` (§B5) |
| `UiRules::ui_image`: `ImageRef` → DC6 frame, offsets vs `at`, shade, blend | error | `ui/panels.md` (§B1, §B6) |
| `UiRules::ui_text`: `layout_text` | error | `ui/text.md` (§B3) |
| `UiRules::ui_pass` | error (asked only when a panel draws) | `render/draw-order.md` (§B6) |
| `ViewAssets::palette`: one palette per frame | one palette | `render/shading.md` (§B3) |
| unhandled UI events → world intents | reported, not acted on | `ui/controls.md` (§B4) |
| panels | none: `ui.md` defines no d2rs-owned panel; original panels are owner specs | `ui/panels.md`, `ui/inventory.md`, `ui/text.md`, `ui/automap.md` |

So today the client with `Unspecified` presents the cleared frame (index 0
through the palette) whatever the model holds, and any panel that draws
is an error until its rules exist.

## Tests (`cargo test -p d2-client --lib world_view`)

- `draw_list_is_ordered_by_key_with_cof_slots`: a synthetic world with
  units (0,7), (1,4), (1,9), a synthetic 2-direction 2-layer COF, one tile
  and one UI image. The exact list is checked (frame set and frame,
  position, key, tag): tile first, then unit (1,4) before (0,7) (its major
  key is smaller although it was built second), per-direction slot order,
  then the UI. Also checks FrameId assignment order and the reuse of an
  id.
- `equal_keys_keep_build_order`: equal pass/major/minor; `sub` decides,
  and equal full keys keep unit-key build order.
- `unspecified_rules_draw_nothing_and_refuse_ui`: the neutral rules give
  an empty list and the cleared frame. A UI request is an `Unresolved`
  error naming `render/draw-order.md`.
- `missing_assets_and_text_are_errors`: a missing set (unit, names the
  file), a missing tile set, a missing COF, a text request → errors.
- `cpu_frame_hash_is_golden`: FNV-1a 64 of the CPU-composited RGBA frame
  is `0x03866d4be03b3500`, plus pixel spot checks of overlap order.
- `frame_hash_catches_perturbations` (M08): one source pixel changed →
  exactly that screen pixel differs and the hash changes. One item moved
  1 px → exactly its old and new pixel. Two overlapping items swapped →
  exactly the overlap pixel.
- `gpu_packing_emulates_to_the_cpu_image`: `GpuAtlas::ensure` + `pack`
  through `gpu_compositor::pack::emulate` (the shader on packed bytes, on
  the CPU) equals `scene::compose`. This proves the FrameId → atlas slot
  wiring without a GPU. A second `ensure` does not repack.
- `ui_binding_routes_forwards_and_draws`: a test panel over a bridge with
  a recording link. A cursor move outside the panel is returned
  unhandled. A press becomes an intent sent through
  `Bridge::send_bytes` (C→S Walk bytes reach the link). The draw requests
  build the same list as above.
- `bevy_frame_presents_the_cpu_image`: a windowless `App` (Minimal +
  Asset plugins, `BridgePlugin`, `WorldViewPlugin { gpu: false }`). After
  one update the units arrive through a synthetic 0x0E handler, the UI
  press is forwarded, the stats are `(bridge frame 1, 6 items, 2 drawn,
  1 hidden)`, and the presented image's bytes equal `compose_cpu`. The
  second update overwrites the same image handle.

## Seams for other sessions

- **Owner specs (local RE):** each hook above becomes an implementation
  of `ViewRules` / `UiRules` written from its spec. Only those trait
  methods change. Once a §B spec lands, its `unit`/`scene`/`ui` verify
  cases (render-pipeline §A10) compare that rule's output with captures.
- **Model fields:** `ClientUnit` holds only its key, and `ClientWorld`
  has no map. When S→C owner specs add fields, the rules read them through
  `unit_pose` / `tiles`. world_view itself needs no change.
- **Residency (C2):** `ViewAssets::sets` and `cofs` are plain maps.
  Wiring `Pool::resolve` needs the frame's key list before `build`. The
  options are a first pass over `unit_pose` + `component_frame`, or
  catching `SetMissing` and retrying. Either way, a stall is counted,
  never skipped (assets §A4). Atlas page eviction belongs here too.
- **App wiring:** no code yet builds a `LocalLink` single-player game, a
  `BridgeResource` or a `WorldViewState` in the app (HANDOFF §2 step 4).
  The session that adds that inserts the three resources (plus an
  optional `WorldViewUi` with `insert_non_send`).
- **GPU in app:** `Gpu::from_device(render_device.wgpu_device().clone(),
  (**render_queue.0).clone())` is called once in `init_gpu`. It
  type-checks against Bevy 0.19.1's `RenderDevice`/`RenderQueue`. It has
  not run on a real window: the CI test uses `gpu: false`. The compositor
  reads back to the CPU each frame and the image is re-uploaded through
  `Assets<Image>`. A render-graph node writing straight to a texture is
  the later optimization (HANDOFF §2 step 5). It cannot change pixels.

## Open questions

1. `UiRules::ui_pass` gives every UI item one key, so emission order is
   draw order (the C8 design). If `ui/panels.md` needs UI items
   interleaved with world passes (e.g. a cursor pass), the key layout
   stays and only the hook changes.
2. Tiles are one hook that returns complete `TileDraw`s, not one hook per
   question as for units, because placement, key, shade and blend of a
   tile all come from the same draw-order / DRLG specs. Split it if those
   specs answer them separately.
3. Presentation bars: Bevy centers the scaled sprite in logical units, so
   an odd remainder can land half a pixel off from `Presentation`'s
   rounded-down bars (C8 open question 2). This is outside verify. The
   cursor mapping uses `Presentation`, so a one-pixel mismatch between
   pointer and image is possible on odd remainders until the present
   step places the image in physical pixels (render-graph node).

## Checks to queue (local)

1. GPU in app (real window, any GPU): run a dev build that inserts a
   `BridgeResource` and a `WorldViewState` with test rules, and confirm
   that `FrameStats::gpu` is `true` and the presented frame equals the
   CPU frame. There is no command yet: it needs the app wiring session
   (seam above). Until then, the synthetic verify cases (`d2-client
   verify`, `gpu_compare`) cover the compositor itself.

No game-file check: synthetic data only.

## Gate run (this branch)

All pass: `cargo fmt --all -- --check`, `cargo clippy -p d2-client
--all-targets -- -D warnings`, `cargo test -p d2-client` (205 lib pass, 5
ignored; 3 other), `cargo run -p depcheck` (8 crates OK), `python3
tools/spec_index.py --check`, `python3 tools/methods.py check` (21 OK),
`python3 tools/coverage.py --check` (3172 claims, 0 errors; adds unit
claims for render-pipeline §A1, §A7 text, §A9, §A8 and ui §A2).
