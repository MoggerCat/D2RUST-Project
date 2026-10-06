# Handoff: Phase 6 C3, indexed frames and atlas (`d2_client::frames`)

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Branch `claude/p6-frames`, based on `claude/bold-ptolemy-jvyvxy` at
`978e6c4` (2026-10-06, cloud). Specs: `specs/client/render-pipeline.md`
§A2, `specs/client/assets.md` §A3 (+ the atlas half of §A5, §A7); both
d2rs-own design drafts.

## State

**Implemented; infrastructure only, no original behavior.** `IndexFrame`,
per-direction / per-tile `FrameSet` from DCC, DC6 and DT1, the
deterministic shelf packer with 1-pixel gutters over 2048² R8 pages, an
atlas check (slot bytes + gutter ring) with perturbation tests (M08), and
the R8Uint page upload into `Assets<Image>`. All plain Rust except
`frames/upload.rs`. Unit tests run in CI without game files or a GPU
(the upload test uses `Assets<Image>` only). The game-file check below
is queued; the GPU half (pages drawn by the compositor) belongs to C5/C6.

Changes outside `crates/d2-client/src/frames/`: one line in
`crates/d2-client/src/lib.rs` (`pub mod frames;`). No dependency, spec,
`bridge/`, `map/`, `assets`, `render` or other-crate change.

## Code map

| Path | What | Spec |
|---|---|---|
| `crates/d2-client/src/frames/mod.rs` | `IndexFrame` (strict `new`: pixels = w×h), `FramePart::{Dir(u8), Tile(u32)}`, `FrameSetKey` (refuses non-canonical paths), `FrameSet::{from_dcc, from_dc6, from_dt1, byte_size}`, `FrameSource` (`frame_set`, `part_count`), `FrameError` | render-pipeline §A2, assets §A3 |
| `crates/d2-client/src/frames/atlas.rs` | `Atlas` (`new(max_pages)`, `insert_set` all-or-nothing, `clear_page`, `read`, `check`, `take_dirty`), `AtlasSlot` (+ `EMPTY`), `AtlasPage` (`pixels`, `dirty`, `generation`), `CheckReport`, `AtlasError`; constants `PAGE_SIZE`, `GUTTER`, `MAX_SIDE` | render-pipeline §A2, assets §A5 (atlas eviction) |
| `crates/d2-client/src/frames/upload.rs` | Bevy edge: `AtlasTextures` resource (one `Handle<Image>` per page, stable), `upload_dirty` (whole dirty pages via `render::index_image`), `UploadError` | render-pipeline §A2 |
| `crates/d2-client/src/frames/tests.rs` | 22 synthetic tests (rule claims via `// Covers:`) + 1 ignored game-file test | |

## Design decisions taken inside the spec's latitude (d2rs-own)

- Offsets, unchanged from the format: DC6 `offset_x/offset_y`; DCC the
  frame box top-left `(x_min, y_min)` (`dcc.md` §Boxes, as `DccFrame`
  already stores it); DT1 the tile image `(x0, y0)` from
  `map::tiles::assemble` (reused, not copied).
- Packing rule (first fit, documented at the top of `atlas.rs`): pages
  in order, shelves in creation order, first shelf tall enough with room;
  else a new shelf below; else next page; else a new page up to
  `max_pages`; else `AtlasError::Full` (caller evicts and retries). Frame
  sets go in whole or not at all.
- Gutter: slots start at `GUTTER` and keep a ring of index 0 on every
  side including the page edge, so largest frame side = 2046
  (`TooLarge` above; the queued game-file test measures whether any live
  frame exceeds it).
- Empty frames (0 wide or high, legal in DC6) get `AtlasSlot::EMPTY`
  and take no space. An empty DT1 tile gives an empty `FrameSet`.
- `clear_page` zeroes the page, bumps its `generation` (slots don't carry
  a generation; the residency cache compares) and restarts packing there.
- Upload re-sends whole 4 MiB pages when dirty. Sub-rectangle upload is
  an optimization for later; it cannot change pixels.

## Seams with other tasks (narrow types defined here)

- **C1 (paths):** `FrameSetKey` holds a `String` and only *checks* the
  canonical form of assets §A1; replace with C1's `CanonicalPath` when
  merged (one field type change).
- **C2 (residency):** owns which `FrameSet`s stay, LRU over
  `FrameSetKey` (ordered), budgets via `FrameSet::byte_size`, and atlas
  page eviction by calling `Atlas::clear_page` on `AtlasError::Full`
  and tracking `AtlasPage::generation`.
- **C4 (scene):** `DrawItem.frame: FrameRef` = an `AtlasSlot` plus a
  reference to the `IndexFrame` (the CPU reference reads `IndexFrame`
  bytes, never the atlas). No `FrameRef` type is defined here to avoid
  two definitions; C4 composes it from these two types.
- **C5 (GPU compositor):** binds `AtlasTextures::pages` (R8Uint,
  `textureLoad` by `(x, y)`).
- **DCC per-direction decode:** assets §A3 wants one direction decoded
  without the others; `d2_formats::dcc::Dcc::parse` decodes the whole
  file today. `FrameSet::from_dcc` takes one direction from it, so a
  per-direction parser can replace the input later without touching
  this module (would be a `d2-formats` task).

## TODO(spec) hooks

- `TODO(spec: render/sprite-placement.md)` on `IndexFrame::x_off/y_off`
  (render-pipeline §B1): how offsets become a screen position; the
  module never interprets them.

## Open questions

1. Is any live frame wider or higher than 2046 (one page minus gutters)?
   If yes, §A2 needs a rule for oversized frames (split, or a larger
   dedicated page). Answered by the queued test.
2. Whether DC6 `flip = 1` frames reach `IndexFrame` already top-row-first
   (the DC6 parser's contract says yes; `dc6.md` OQ 2 confirmed it
   visually) — nothing here re-flips.

## Checks to queue (local run queue, `docs/HANDOFF.md` §5)

1. Game files: every live DCC/DC6/DT1 builds all its frame sets and each
   set packs and checks clean in a 64-page atlas.
   `D2_GAME_DIR=<game> cargo test -p d2-client --lib frames::tests::all_live_frame_sets_build_and_pack -- --ignored --nocapture`
   Expect: pass; the printed line gives files, parse errors (should be
   the 7 known leftovers of `mpq-tool formats`, 6 of them DT1),
   frame-set and frame counts, and the largest frame (must be ≤ 2046).
2. GPU: pages sampled through `textureLoad` give the CPU bytes: covered
   by the C6 verify cases once C5 exists (no separate command here).

## Gate (this branch)

`cargo fmt --all -- --check`, `cargo clippy -p d2-client --all-targets
-- -D warnings`, `cargo test -p d2-client`, `cargo run -p depcheck`,
`python3 tools/spec_index.py --check`, `python3 tools/methods.py check`,
`python3 tools/coverage.py --check`: all clean (`cargo test -p d2-client`:
53 pass, 1 ignored; coverage 315 claims, 0 errors).
