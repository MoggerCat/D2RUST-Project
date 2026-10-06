# Handoff: Phase 6 C7 — `d2-client::composite` (COF composite mechanics)

Scope: branch `claude/p6-cof`, from `claude/bold-ptolemy-jvyvxy` at
`ed7236e` (2026-10-06, cloud, repo only). Spec:
`specs/client/render-pipeline.md` §A7 (d2rs-own design draft), on top of
`specs/formats/cof.md` (`d2_formats::cof`), C4 `scene` and C3 `frames`.

## State

**Implemented; mechanism proven by synthetic vectors (CI). No original
behavior.** Slot order per (COF direction, frame) and one `DrawItem` per
slot, keys `pass/major/minor` shared, `sub` = slot. Every §B question is a
method of the `ComponentResolver` trait (a `TODO(spec: …)` hook); the
module itself picks no file, frame, position, shade or blend. Tests:
`cargo test -p d2-client --lib composite` (11 pass, 1 ignored game-file
test).

Changes outside `crates/d2-client/src/composite/`: `pub mod composite;`
plus two doc lines in `crates/d2-client/src/lib.rs`. No dependency, spec,
`scene/`, `frames/` or other-crate edit.

## Code map rows

| Path | What | Spec |
|---|---|---|
| `crates/d2-client/src/composite/mod.rs` | `Slot`, `slot_order` (§A7 step 2, strict COF checks), `ComponentRequest`, `ComponentFrame` (C3 `FrameSetKey` + frame index), `ComponentResolver` (hooks), `UnitParams`, `ComponentDraw`, `build` (§A7 steps 2–4), `CompositeError` | render-pipeline §A7 |
| `crates/d2-client/src/composite/tests.rs` | synthetic COF bytes built per `cof.md`, slot order per (d, f), `cof.md` vector, errors, `build` → items → `scene::compose` back-to-front check, 1 ignored live-COF test | §Test vectors |

## Public API

- `slot_order(&Cof, dir, frame) -> Result<Vec<Slot>>`; `Slot { slot: u8,
  component: u8, layer: usize }` (layer = index of the record carrying
  the component).
- `build(&Cof, dir, frame, &UnitParams, &impl ComponentResolver) ->
  Result<Vec<ComponentDraw>>`, in slot order (= key order, so
  `scene::order` keeps it). `ComponentDraw { slot, frame: ComponentFrame,
  item: DrawItem }`: the `ComponentFrame` is what residency (C2) loads.
- `UnitParams { pass, major, minor, clip, tag }`.
- `ComponentResolver`: `frame`, `frame_id`, `place`, `shade`, `blend`,
  each taking a `ComponentRequest { cof, dir, frame, slot, layer }`.

Strict (M07): direction/frame out of range, layer-record count ≠ header,
draw-order length ≠ D·F·L, layer component ≥ 16 (hand-built `Cof`s; the
parser already refuses these), **a draw-order component with no layer
record**, **two layer records with one component** (ambiguous which record
a slot means), any hook error (whole unit fails, no partial composite),
key fields out of range. Duplicate components *within one frame's draw
order* are accepted (mechanically defined: two items).

## `TODO(spec: …)` hooks (narrowest neutral behavior)

| Hook | Neutral behavior | Owner |
|---|---|---|
| `ComponentResolver::frame`: component file path (token, armor class variant, mode, weapon class), file direction for the COF direction, frame inside it | caller-supplied; module never builds a path | `render/unit-composite.md` (§B4) |
| Whether a slot is drawn at all (e.g. empty variant) | every slot is drawn | `render/unit-composite.md` (§B4) |
| Unit direction → COF direction; frame source (animdata vs COF rate) | caller passes COF indices; out of range is an error | `render/unit-composite.md` (§B4) |
| `ComponentResolver::place`: offsets → screen top-left | caller-supplied; module does no offset arithmetic | `render/sprite-placement.md` (§B1) |
| `ComponentResolver::shade`: light level, per-component colormaps, selection | caller-supplied | `render/shading.md`, `render/unit-composite.md` (§B3/§B4) |
| `ComponentResolver::blend`: COF translucency override → blend op | caller-supplied (layer record is in the request) | `render/blend-modes.md` (§B5) |
| `UnitParams::{pass, major, minor}` | caller-supplied; `sub` = slot is ours (§A7 step 4) | `render/draw-order.md` (§B6) |
| `UnitParams::clip` | caller-supplied (`Rect::FRAME` if nothing else) | `render/camera.md` / `render/draw-order.md` |
| `flip_x` | always `false` (scene rejects `true`) | `render/sprite-placement.md` / `render/unit-composite.md` |
| COF shadow / selectable fields | not read here; available to hooks via `ComponentRequest::layer` | `render/blend-modes.md`, `render/unit-composite.md` |

## Open questions

1. Do live COFs ever list a draw-order component without a layer record,
   or two records for one component? This module refuses both; the queued
   test answers it. If they occur, §B4 must state the rule.
2. Residency: `frame_id` is a hook here because no store maps
   `(FrameSetKey, index)` → `scene::FrameId` yet (C2/C3/C5). When it
   exists, `frame_id` can become a plain lookup outside the trait.

## Checks to queue (local run queue, `docs/HANDOFF.md` §5)

1. Game files: every live COF gives a slot order for every direction and
   frame.
   `D2_GAME_DIR=<game> cargo test -p d2-client --lib composite::tests::all_live_cofs_give_slot_orders -- --ignored --nocapture`
   Expect: pass; printed line gives COF count (3,604 + the junk
   `amblxbow.cof` as the one parse error, `cof.md` §Edge cases), frame
   count, failures 0. A failure lists file, direction, frame and error.

(HANDOFF.md not edited per task scope; the integrating session adds this.)

## Gate run

All pass on this branch: `cargo fmt --all -- --check`, `cargo clippy -p
d2-client --all-targets -- -D warnings`, `cargo test -p d2-client` (168
lib tests, 2 ignored), `cargo run -p depcheck` (8 crates OK), `python3
tools/spec_index.py --check`, `python3 tools/methods.py check` (21 OK),
`python3 tools/coverage.py --check` (392 claims, 0 errors;
`render-pipeline.md` 10 of 16 units at unit tier, now incl. §A7 r2–r4).
