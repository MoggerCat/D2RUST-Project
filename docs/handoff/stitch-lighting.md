# stitch-lighting: real lighting in the `play` preview

Branch `claude/stitch-lighting`. Everything here is `d2rs-own, unverified`
(rule 10): nothing is compared with 1.14d yet.

## Links traced and connected

Path: model → `ModelFeed::prepare` → light map → `Preview::tile_art` /
`ViewFeed::light` → `build_lit` (`LitRules`) → frame → draw.

| Link | Before | Now |
|---|---|---|
| Light map build | `rules/lighting/*` existed, nothing called it in `play` | `world_view/preview_light.rs` `PreviewLight::refresh`, called from `ModelFeed::prepare` (`model_feed.rs`) once per frame |
| Player light | no record created by any unit code | radius 13 + stat 89, colour stat 90 (`sources.rs`), plain contribution at the predicted sub-tile |
| Act ambient | not fed | `Environment` (model's record, or fresh) updated once per frame; `ambient()` fills the map (§3) |
| Tile light | `tile_ops` gave `ShadeChain::EMPTY` | `Preview::tile_art` takes `PreviewLight::tile_chain` (not shadow tiles) |
| Unit light | `ModelFeed::light` returned `None` (rules' shade) | returns the frame light + `PreviewLook`; `build_lit` wraps the rules in `LitRules`, so unit `shade`/`blend` follow the light cell at the unit's sub-tile (§11 r1) |
| PL2 shade tables | pushed by `TileAssets::ensure_shades` | used as `FrameLight.tables` |
| Full bright | the only mode | `D2RS_FULLBRIGHT=1` (read in `Preview::new`) |

## Preview behaviour with no spec yet (listed per rule)

- A tile gets **one flat light value** (cell at its centre sub-tile), not the
  per-block wall/floor gradients (§11 r2–r3).
- Ambient is the environment's only: no `Levels.txt` ambient, no near-room
  fills, no scripted override (the level row has no Intensity/RGB fields).
- No blocks-light flags: no light is shadowed.
- Unit look inputs (ghostly, override, hover, remap) are all "none".
- Roof (height ≠ 0) = environment intensity; floor material 0x100 = 0xFF (§11 r3, r4, implemented).

## Left

- Per-block gradients for walls/floors (needs block (gx,gy) in `BlockRect`).
- Other light records (monsters, missiles, objects): only the player's is created.
- Blocks-light flags and shadowed lights (needs collision point test).
- Compare against a recorded light map (`render/capture.md`).

## Your local check

```
cargo test -p d2-client --lib preview_light
D2_GAME_DIR=<D2 install> cargo run -p d2-client -- play --new sorceress Test
```
Expect: the Rogue Encampment dim away from the player with a lit circle
(radius about 13 sub-tiles) that follows the character when walking; units
inside the circle are bright, outside darker. Daytime ambient may keep the
town bright; try `D2RS_FULLBRIGHT=1` to compare (flat full-bright as before).
