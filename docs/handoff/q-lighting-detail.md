# q-lighting-detail: per-block tile light and Levels ambient

Branch `claude/q-lighting-detail`. `d2rs-own, unverified` (rule 10); the gap
list is REC-247 in `docs/HANDOFF.md` §7.

## Links connected

| Link | Before | Now |
|---|---|---|
| Per-block wall / floor / roof light | one flat value per tile (`tile_chain`) | `world_view/preview_blocks.rs` `block_shades`: wall points → `wall_block_shades`; floor / roof 8×8 grid → `floor_block_light` / `floor_block_chain` (CPU rules of `rules/lighting/draws.rs`, `rules/shading.rs`) |
| Block grid coordinates | not kept | `TileAssets::grids` (DT1 block bytes 6, 7) |
| Carrying the shades to the draw | n/a | `Preview::tile_art` stores them by `DrawKey`; `ModelFeed::tile_blocks` answers them (the existing `OriginalView::tile_draws` places each gradient at its block) |
| Levels ambient | environment only | `LightRows::levels` (leveldefs), `PreviewLight::refresh` takes it when Red/Green/Blue ≠ 0 (§3.1 r2); roofs keep the environment (§11 r4) |

The GPU path (`gpu_compositor`) already has byte-for-byte tests of the
gradient items; the new CPU values are the same `ShadeChain` gradients.

## Not done

- Blocks-light flags (`lighting.md` §4): the client holds no collision grid, so
  no light is shadowed.
- Near-room ambient fills and scripted overrides (§3 r3, §10).
- Roof blocks: floor-grid path (provisional), wall fade state 0.

## Your local check

```
cargo test -p d2-client --lib -- preview_blocks preview_light block_feed
D2_GAME_DIR=<D2 install> cargo run -p d2-client -- play --new sorceress Test
```
Expect: walls and floors near the player fade smoothly across each 32-pixel
block instead of one flat tone per tile; interiors with a Levels.txt colour
(dungeons, Act 4 town) use that ambient. `D2RS_FULLBRIGHT=1` compares.
