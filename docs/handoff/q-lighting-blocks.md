# q-lighting-blocks: blocks-light flags, near-room fills, ambient overrides

Branch `claude/q-lighting-blocks`. `d2rs-own, unverified`; gaps are REC-250 (`docs/HANDOFF.md` §7).

## Links connected
| Link | Now |
|---|---|
| Blocks-light flags (`lighting.md` §4) | `PreviewLight::refresh` → `build_map_blocked` → `LightMap::fill_blocks`, flag = `ClientDrlg.drlg.collision_at(x, y) & 0x22` (the grid of q-client-collision, no second map) |
| Shadowed light (§7.3) | the player's record is kind 0 and takes `contribute::shadowed`; the others stay plain |
| Near-room fills (§3 r3) | `PreviewLight::scene`: the player room's adjacency array, each room's rectangle and own level ambient |
| Scripted overrides (§10) | `room_ambient`: `world.overrides.ambient(level, 0)` wins over level / act ambient when it has a colour |

Test: `preview_blocks::a_blocks_light_tile_between_the_light_and_a_floor_block_darkens_it`.

## Left (REC-250)
Quest byte 1 read as 0; non-player sources draw plain; no capture to compare.

## Your local check
```
cargo test -p d2-client --lib -- preview_blocks preview_light
D2_GAME_DIR=<D2 install> cargo run -p d2-client -- play --new sorceress Test
```
Expect: in a dungeon, walls cast shadow from your light (cells behind a wall stay dark); at room borders with different level ambients the lighting changes at the room rectangle. `D2RS_FULLBRIGHT=1` compares.
