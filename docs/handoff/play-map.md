# S-A Map: the play preview draws the map

> Implementation session, 2026-10-07, branch `claude/play-map`. Plan:
> `docs/handoff/first-playable-scope.md` (S-A). Decision D1 (preview
> fills) as approved in `docs/PLAN.md`. Read only `specs/`, `docs/`,
> `crates/`, `tools/`. Nothing here is verified against 1.14d (rule 10):
> every fill is marked `// d2rs-own, unverified`.

## What changed

| File | Change |
|---|---|
| `world_view/tile_assets.rs` (new) | `TileAssets`: reads the DT1 of each near-room record entry (`rooms.md` §9.3 path + index) from the user's archives on demand (archive name = `d2_server::world_data::archive_name`, `DATA\GLOBAL\TILES\` prefix), makes `FramePart::Tile(i)` resident in `ViewAssets.frames`, keeps each tile's blocks (camera §7 culling). Failures (no archive, bad parse, index past the file, tile without blocks) are remembered per entry, never retried. Also pushes an act's PL2 palette-table block (`ShadeTables::push`) into `ViewAssets.maps` once per act (G10). |
| `world_view/preview.rs` (new) | D1 fills: tile art full bright (empty shade chain) with the owner spec's blend by kind and alpha (`blend-modes.md` §5, §6: shadow tiles blended `A0`, translucent walls / roofs `IndexTableSrcRow`, alpha < 0x40 not drawn); unit facts zero with sight "visible" (so no unit-shadow entries); unit offsets `(0, 0)`; water bit cleared on floors (no weather state is fed); a 1×1 transparent skip frame for tiles that cannot be drawn; `log_once` (each skip message once, `warn!`). |
| `world_view/model_feed.rs` | `ModelFeed.preview: Option<Preview>` and `with_preview()` (turns the map on). With a preview: `tile_art`, `unit_facts`, `unit_offset`, `near_rooms` (facts, dried floors, a failing build → logged, frame without the map) and `prepare` answer from the preview. Without one every answer is the strict one; `PENDING` unchanged (it describes the strict path). |
| `world_view/feed.rs` | New `ViewFeed::prepare(world, &mut assets)` hook (default: nothing), run before each frame build. |
| `world_view/present.rs` | Calls `feed.prepare` before `build_frame`. `WorldViewState.preview`: when set, a failing frame build is logged (each distinct message once) and the frame is not presented, instead of returning the error to Bevy. Default `false` (strict). |
| `app/play.rs` | `add_preview(app, levels, tiles)`; `run` turns it on (live: archives + the five act PL2s; synthetic: no archives, every tile skipped and logged). `add_game` alone stays strict, so the existing headless tests are unchanged. |
| `world_view/mod.rs` | Two `pub mod` lines (`preview`, `tile_assets`). |
| `bridge/msg/tests_model.rs` | `preview: None` in one `ModelFeed { .. }` literal (compile fix, test unchanged). |
| `tests/app_play_preview.rs` (new) | Headless: synthetic game + client DRLG + preview with a synthetic one-tile DT1 in a `MemorySource`; frames build, items > 0, the tile is resident. |

Unit tests: `tile_assets` (keys, load once, failures kept, no archives,
shade tables once per act), `preview` (tile ops per kind / alpha, zero
facts + visible, skip frame, log dedup).

## Gaps closed (preview half; all unverified)

G2 (map on), G3 (tile art, D1 full-bright light), G4 (DT1 loader into
the frame store), G5 (unit facts, D1 zero), G8 (unit offsets, D1 zero:
no motion record in the model), G10 (PL2 shade / blend rows in
`ViewAssets.maps`), G19 preview half (per-tile skip + log; a whole-frame
failure logs instead of stopping the app).

## Seams for other sessions

- **S-B (units):** draw through `ViewRules` as planned. The feed in play
  is `ModelFeed<NoFeed>` with a `Preview`; `unit_offset` answers `(0, 0)`
  and `unit_facts` zero flags with `sight_hidden = Some(false)`, so every
  listed unit not skipped by `draw-order.md` §5 r1 is ordered as drawn
  and no `UnitShadow` item appears. Unit shade / blend stay the rules'
  (the feed states no `light`): for full bright return
  `ShadeChain::EMPTY` / `BlendOp::Opaque`. The act shade tables are
  pushed into `state.assets.maps` by the preview (`TileAssets::shades(act)`
  is the preview's; if S-B needs them, add an accessor on `Preview` /
  `ModelFeed` or push its own copy). A unit draw error still fails the
  frame (the preview then logs it and skips the frame): per-unit skip
  belongs in S-B's rules.
- **S-D (walk motion):** the camera follows `world.local()`'s
  `position`; moving it in the model moves the view, nothing else needed.
- **Coordinator:** if S-B also adds modules to `world_view/mod.rs`, the
  merge is two adjacent `pub mod` lines.

## Local checks for the user (game files; not done until run)

1. Unit + headless tests (no game files needed, also run by CI):
   `cargo test -p d2-client --lib world_view:: && cargo test -p d2-client --test app_play_preview`
   → all pass.
2. The done-check (Windows, 1.14d files):
   `set D2_GAME_DIR=C:\path\to\Diablo II` then
   `cargo run -p d2-client --release -- play --frames 2000`
   (until S-C's G1 fix is merged the server may still panic at about
   tick 104; `--frames 90` shows the map before that).
   Look for: the Rogue Encampment's floor and walls (tents, palisade,
   ground), centred on the (invisible until S-B) player, full bright, in
   the act 1 palette. The log has `play:` / `frame N:` progress lines with
   `last view Some(FrameStats { items: <hundreds> .. })`.
   Expected `WARN preview (d2rs-own, unverified): skipped: ...` lines:
   a few are fine (tiles with no blocks); many `in no archive` or
   `past the file's N tiles` lines mean the DT1 path or index mapping is
   wrong: copy them into `docs/HANDOFF.md`. Any
   `frame not drawn: ...` line is a whole-frame error: copy it too.
3. Optional sweep before (2): the ignored DT1 sweeps of HANDOFF item 52
   (`cargo test -p d2-formats --test game_sweep -- --ignored`) catch
   decoder refusals the preview would otherwise skip tile by tile.

## What is left

- Everything drawn is unverified: light (full bright), the tile blends,
  draw order without real unit facts, no water effects, no unit shadows.
  The checks to queue (HANDOFF §5) are the capture-compare cases once a
  `SceneSource` exists, and the lighting / draw-order recordings.
- Per-block shading (`ViewSource::tile_blocks`) is not answered: tiles
  shade as a whole (full bright makes that equal).
- DT1 files stay resident for the whole run (no eviction, `assets.md`
  §A5 budgets not applied): fine for town and the Blood Moor.
- No CLI switch for the strict path in `play` (by design, D1: "a later
  flag can turn it off").
