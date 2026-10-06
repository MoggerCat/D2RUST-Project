# Handoff: Phase 6 C4 — `d2-client::scene`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Scope: branch `claude/p6-scene`, from `claude/bold-ptolemy-jvyvxy` at
`978e6c4`. Spec: `specs/client/render-pipeline.md` (d2rs-own design
draft) §A3–A8 and the bins of §A9. Cloud session, repo only (no game
files, no GPU).

## State

**Implemented; CPU half proven by synthetic vectors (CI).** This is d2rs
infrastructure (part (a) of the spec): its proof is the spec's synthetic
test vectors, which all pass as unit tests (`cargo test -p d2-client
scene`). Nothing here reproduces original-game behavior yet: every §B
point is a `TODO(spec: …)` hook (below). Link 2 (CPU → GPU) waits for C5;
link 1 (original → CPU) waits for the §B owner specs.

Changes outside the module: `pub mod scene;` plus two doc lines in
`crates/d2-client/src/lib.rs`. No dependency, spec, or other-module
edits.

## Code map rows

| Path | What | Spec |
|---|---|---|
| `crates/d2-client/src/scene/mod.rs` | constants (`FRAME_WIDTH/HEIGHT` 800×600, `BIN_SIZE` 32, `MAX_SHADE` 4), `Rect` (i64 edge math, `intersect`, `contains`), `SceneError`, re-exports | `client/render-pipeline.md` |
| `crates/d2-client/src/scene/item.rs` | `DrawItem`, `FrameId`, `FrameView`, `FrameSource` (seam to C3), `FrameImage`, `MapId`, `MapTable`, `ShadeChain`, `BlendOp::{Opaque, IndexTable}`, `ItemTag`; per-item validation (`resolve`) and per-pixel formula (`pixel`) | §A2–A5 |
| `crates/d2-client/src/scene/order.rs` | `DrawKey` (`pass:4│major:28│minor:24│sub:8`, checked packing), `order` (stable sort) | §A6 |
| `crates/d2-client/src/scene/bins.rs` | `Bins`, `bin` (32×32 bins over the view, item indices in list order) | §A9 (CPU part) |
| `crates/d2-client/src/scene/cpu.rs` | `compose` (index framebuffer), `compose_binned` (per-pixel bin walk, the GPU's model), `compose_rgba`, `to_rgba` | §A8 |
| `crates/d2-client/src/scene/tests.rs` | every §Test vectors row that has a CPU half, strict-input perturbations | §Test vectors |

## Public API (for C5, C6, C7)

- `DrawItem { frame: FrameId, x, y: i32, flip_x: bool, clip: Rect, shade:
  ShadeChain, blend: BlendOp, key: DrawKey, tag: ItemTag }`;
  `DrawItem::new(frame, x, y)` = opaque, unshaded, clip = `Rect::FRAME`,
  key 0. `x, y` is the image's top-left on screen (offset → position is
  the builder's job, §B1).
- `Rect { x, y: i32, width, height: u32 }`, `Rect::FRAME`, `intersect`,
  `contains`.
- `FrameSource` trait: `fn frame(&self, FrameId) -> Result<FrameView,
  SceneError>`; `FrameView::new(w, h, &pixels)` checks `len == w*h`.
  Implemented for `[FrameImage]` / `Vec<FrameImage>` (`FrameId(n)` =
  element n).
- `MapTable`: `push(row) -> MapId`, `push_table(&[[u8;256];256]) -> MapId`
  (base), `get`, `rows()` (what the GPU uploads as one storage buffer).
- `ShadeChain::new(&[MapId]) -> Result` (≤ 4), `EMPTY`, `maps()`.
- `BlendOp::Opaque` (`dest = src`), `BlendOp::IndexTable(base)` (`dest =
  map[base + src][dest]`; needs rows `base..base+256`).
- `DrawKey::new(pass, major, minor, sub) -> Result`, accessors,
  `DrawKey(u64)` raw; `order(&mut [DrawItem])`.
- `bin(items, frames, maps, view) -> Result<Bins>`; `Bins::{cols, rows,
  list(c, r), rect(c, r), view, item_count}`.
- `compose(items, frames, maps, view) -> Result<Vec<u8>>` (indices,
  `view.width × view.height`, pixel 0 = screen `(view.x, view.y)`);
  `compose_binned(items, &bins, …)` (same result); `compose_rgba(…,
  &Palette, view)`; `to_rgba(indexed, &Palette)` (alpha 255).
- Per pixel, in list order: frame index `0` → unchanged; else `s =
  chain(src)`, `dest = blend(s, dest)`. Framebuffer starts at index 0.

Every item is validated before any pixel is drawn (strict, M07): missing
frame, pixel-count mismatch, unknown map, blend table out of range,
`flip_x = true` are `SceneError::Item { index, error }`; key fields out of
range and chains over 4 are errors at construction. Off-screen items are
validated too. Bins built for another view or list length are rejected.

## Seams

| Seam | Where | Provider |
|---|---|---|
| `FrameSource` / `FrameId` | `item.rs` | C3 (`IndexFrame`, frame store): implement the trait; atlas slot lookup by `FrameId` is C3/C5's, not stored in the item (spec §A3 says "atlas slot + IndexFrame id"; the id is enough to find the slot) |
| `MapTable` rows | `item.rs` | C1 PL2 loader: push each PL2 map / 256×256 table, keep the returned `MapId`s |
| Building the list (`scene::build`, `FrameParams`, §A1 stage 1) | not here | needs bridge snapshot + §B owner specs; C7 builds unit items with `sub` = slot |
| `Bins` + `MapTable::rows()` + item list | `bins.rs` | C5 uploads these; `compose_binned` is the per-pixel reference for its WGSL |

## `TODO(spec: …)` hooks (narrowest neutral behavior)

| Hook | Neutral behavior | Owner |
|---|---|---|
| Mapped shade result 0 (§B3) | drawn as index 0 (only the source 0 is transparent, as §A4 states) | `render/shading.md` |
| Palettes per screen region (§B3) | one palette per frame | `render/shading.md` |
| Composition domain, `Rgb` op (§B2) | indexed framebuffer only; no `Rgb` variant | `render/composition.md` |
| Frame clear value (§B2) | index 0, mapped through the palette like any index | `render/composition.md` |
| Which blend op each draw uses (§B5) | caller's choice | `render/blend-modes.md` |
| `flip_x` (§A3 reserved) | `true` is an error | `render/sprite-placement.md` / `render/unit-composite.md` |
| Pass numbers and key rules (§B6) | plain numbers, no pass enum | `render/draw-order.md` |

## Open questions

1. `to_rgba` maps index 0 through the palette; `map::cpu::to_rgba` paints
   0 black. When C6 ports the `map` case to this compositor, either the
   act palette's entry 0 is black (then images agree) or the case needs a
   clear rule — decide with §B2 (`render/composition.md`); verify will
   show it.
2. Rust's slice `sort_by_key` is the stable sort used; the GPU never
   sorts (it consumes the ordered list), so no second sort to match.

## Checks to queue (local)

None needing game files. When C5 lands: `d2-client verify` synthetic
cases compare GPU output to `compose` (GPU half, local).

## Gate run

All pass on this branch: `cargo fmt --all -- --check`, `cargo clippy -p
d2-client --all-targets -- -D warnings`, `cargo test -p d2-client` (51
lib tests, 20 in `scene`), `cargo run -p depcheck` (8 crates OK),
`python3 tools/spec_index.py --check`, `python3 tools/methods.py check`
(21 OK), `python3 tools/coverage.py --check` (0 errors;
`render-pipeline.md` 6 of 16 units covered at unit tier: §A3, §A4, §A5,
§A6, §A8, edge cases).
