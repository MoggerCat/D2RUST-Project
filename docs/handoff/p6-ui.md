# Handoff: Phase 6 C8, UI core (`d2_client::ui`)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Branch `claude/p6-ui`, based on `claude/bold-ptolemy-jvyvxy` at `978e6c4`
(2026-10-06, cloud). Spec: `specs/client/ui.md` §A2 (panel model), §A4
(input → actions: frame-coordinate mapping), Test vectors; d2rs-own
design draft. Task row C8 in `docs/PLAN.md` Phase 6.

## State

**Implemented; d2rs-own design, so its check is the spec's vectors.**
Every §A2/§A4 test vector of `ui.md` is a unit test (the §A6 vectors
belong to C9). Nothing here reproduces original behavior: every place
that needs it is a `TODO(spec: …)` hook with neutral behavior (list
below). No pixels are produced yet (no `DrawItem` until C4), so the
`ui` verify case of `render-pipeline.md` §A10 does not apply yet.

Gate run on this branch: `cargo fmt --all -- --check`, `cargo clippy -p
d2-client --all-targets -- -D warnings`, `cargo test -p d2-client`,
`cargo run -p depcheck`, `python3 tools/spec_index.py --check`,
`python3 tools/methods.py check`, `python3 tools/coverage.py --check`.

Changes outside `crates/d2-client/src/ui/`: one line in
`crates/d2-client/src/lib.rs` (`pub mod ui;`). No dependency, spec,
bridge, map, assets or other-crate change.

## Code map

| Path | What | Spec |
|---|---|---|
| `crates/d2-client/src/ui/mod.rs` | module doc, re-exports | §A2, §A4 |
| `ui/geom.rs` | `Point` (the spec's `IVec2`, kept Bevy-free), `Rect` (i32 origin, u16 size, half-open `contains`), `FRAME` 800×600 | §A1, §A5 |
| `ui/frame.rs` | `Presentation` (integer scale, centered bars), `to_frame` → `FramePos::{Inside, Outside}`, `FrameError::TooSmall` | §A4 |
| `ui/edge.rs` | the only Bevy part: `presentation(&Window)`, `window_pixel` (floor of Bevy's f32 cursor), `cursor_frame_pos(&Window)` | §A4 |
| `ui/draw.rs` | `UiDraw::{Image, Text}`, `ImageRef {file, frame}`, `TextStyle {font, color}`, `UiDrawSink` (impl for `Vec<UiDraw>`) | §A2 (draw items) |
| `ui/panel.rs` | `PanelId`, `WidgetId`, `ActionId`, `PointerButton`, `UiEvent`, `UiInput`, `ClientIntent` (encoded C→S bytes), `UiResponse`, `StringLookup`/`NoStrings`, `UiCtx`, trait `Panel` | §A2 |
| `ui/root.rs` | `UiRoot` (fixed order; `add/open/close/toggle/is_open/open_panels`, `draw`, `hit`, `hovered`, `dispatch`, `pump`, `intents/take_intents`, `forward` to `Bridge`), `PanelRules`/`NoPanelRules`, `Routed`, `UiHit`, `UiError` | §A2 |
| `ui/widget.rs` | trait `Widget`; `Button`, `FrameImage`, `Label`, `CellGrid` (+`Cell`), `ScrollList`, `TextInput`; `WidgetError` | §A2 widgets |
| `ui/tests.rs` | 20 tests: the 3 §A2/§A4 vectors + routing, rules hook, intents, widgets, Bevy edge | Test vectors |

## Rules as implemented (ours, within §A2/§A4)

- Draw: open panels bottom-most first (list order); dispatch: top-most
  first. A pointer event is offered to each open panel whose `rect()`
  contains its point, other events (`Char`, `Action`, `CursorLeft`) to
  every open panel; the first answer other than `Ignored` ends the walk.
  If none takes it, `Routed::Unhandled` and `pump` returns it for the
  world input.
- `hit(p)`: top-most open panel whose rect contains `p`, plus its widget.
  `hovered()` = `hit(last cursor)`, so it follows open/close at once.
- Strict (M07): duplicate panel id, unknown panel id, a `PanelRules`
  answer naming an unknown panel or the opening panel itself: errors,
  state unchanged. Zero-size grids/lists and grids over `u16` pixels:
  errors. Window smaller than 800×600: `FrameError::TooSmall`.
- Frame mapping: `scale = min(w/800, h/600)`, bars `(w − 800·scale)/2`
  and `(h − 600·scale)/2` rounded down, `frame = (p − bar) / scale`
  (integer), clamped; bar or outside-window pixels → `Outside`.
- Intents: `UiResponse::Intent(ClientIntent)` is queued by the root only;
  `UiRoot::forward` sends them in order with `Bridge::send_bytes`; on a
  refused intent it stops, keeping the refused one and those after it.

## Seams (for the parallel C-tasks)

- **C4 scene (`DrawItem`)**: panels emit `UiDraw` requests to a
  `UiDrawSink`, not `DrawItem`s (the spec's `draw(&self, ctx, out: &mut
  Vec<DrawItem>)` became `out: &mut dyn UiDrawSink`). The scene
  implements the sink: `ImageRef` → `FrameRef`, pass `ui`, keys in
  emission order (stable sort keeps it), `clip` copied. All widgets use
  `clip = FRAME` today.
- **§A3 text**: `UiDraw::Text` carries UTF-16 units, an origin and an
  opaque `TextStyle`; the sink runs `layout_text` (fonts live there, so
  `UiCtx` has no font field).
- **C9 controls / input**: narrow trait `UiInput::drain(&mut Vec<UiEvent>)`.
  The controls map bindings to their closed `Action` enum and send it as
  `UiEvent::Action(ActionId(index))`; mouse buttons as
  `Press/Release{PointerButton}`; the cursor via `edge::cursor_frame_pos`
  (`Outside` → `CursorLeft`, pointer input there dropped).
- **C5 present**: must place the image with `Presentation` (same scale,
  same rounded-down bars) so the inverse mapping here matches.
- **Bridge**: read-only use of `ClientWorld` in `UiCtx`; sending through
  the public `Bridge::send_bytes` / `intent::encode`.

## `TODO(spec)` hooks (original behavior not invented)

| Hook | Neutral behavior now | Owner spec |
|---|---|---|
| `NoPanelRules` | opening closes nothing | `ui/panels.md` §B2 |
| panel background stopping world clicks | the panel's own answer (`Ignored` passes through) | `ui/panels.md` §B2 |
| `ImageRef` files/frames, frame offsets vs `at`, button pressed/hover art | opaque ids; `at` = rect origin; image never changes | `ui/panels.md` §B1 |
| `TextStyle`, label alignment/baseline, caret | opaque ids; `at` = rect origin; no caret | `ui/text.md` §B3 |
| `CellGrid` cell size, gaps, art, highlight | caller's sizes, adjacent cells, draws nothing | `ui/inventory.md` §B5 |
| `ScrollList` rows per wheel step | caller passes rows | `ui/panels.md` §B2 |
| `PointerButton` meaning, modifiers | routed only | `ui/controls.md` §B4 |
| hit test by rect vs opaque pixels | rect | `ui/panels.md` §B1 |
| cursor drawing (pass `cursor`) | not drawn by the UI | `ui/panels.md` §B6 |

## Open questions

1. Window smaller than 800×600: `render-pipeline.md` §A9 has no scale
   below 1; we refuse (`TooSmall`). Decide (refuse, or crop at scale 1)
   with C5.
2. Odd bar remainder: we round the left/top bar down (extra pixel on the
   right/bottom). C5 must agree; not stated in §A9.
3. §A4 "then clamped": inside the image the quotient is already in range,
   so the clamp is a no-op by construction; positions past the image are
   `Outside`, not clamped. Confirm this reading.
4. `UiResponse` has no "close me / open panel X" answer; a panel's close
   button cannot close it through the closed three-variant enum. Needs a
   design decision (extend `UiResponse` or a root command list) when the
   first real panel lands.
5. Keyboard focus: non-pointer events go to all open panels top-most
   first; whether the original has a focus model (chat box) is
   `ui/panels.md` §B2 / chat.

## Checks to queue

None for the local run queue: this task contains no original behavior
and no pixels. Once C4 exists and real panels are written from
`ui/panels.md`, each panel gets a `ui` verify case (`render-pipeline.md`
§A10) with its capture comparison and `--perturb`.
