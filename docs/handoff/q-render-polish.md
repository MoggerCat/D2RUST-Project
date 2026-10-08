# Handoff: q-render-polish (stitching session)

Branch `claude/q-render-polish`. Everything below is `d2rs-own, unverified`
until a render capture compares it (REC-165 in `docs/HANDOFF.md`).

## Links connected

| Piece | State | Where |
|---|---|---|
| Unit shadows (`blend-modes.md` §5) | **connected** | `rules/draw_order/source.rs` records the kind-2 shadow entries (`DrawEffects.shadows`) instead of failing the frame; `ViewSource::unit_shadow_slot`; `ViewRules::unit_shadows` (default none; `OriginalView` supplies the slot; `UnitRules` draws); `world_view/unit_shadow` (derived `#shadow` frame sets made when a component file loads, `unit_assets::store_file`; draws with `unit_shadow_ops` = chain `[Z]` + `A0`); `ViewAssets.shades` holds the act's tables (set by `Preview::prepare`); preview facts give players, monsters and objects flag-ex 0x80 |
| Hover outline | **connected** | `present.rs` picks the unit under the cursor (`bridge::hover::pick`) → `ViewFeed::set_hover` → `PreviewLook.hover` → `ComponentLook.hovered` (draw mode 7 through `LitRules`) |
| Wall transparency | already wired | draw order §8 fade (instant on the GDI clock) → `OrderedTile.alpha` → `preview::tile_ops` (`IndexTableSrcRow`); only levels whose walls carry `Logicals` fade (not the town) |
| Screen-edge clip | already wired | `OriginalView::with_edge_clip`, on in the preview (`play-units.md`) |
| Unit draw order on edges | no change | the pass-6 keys come from the draw order; nothing found to fix without a capture |

## PROVISIONAL points

REC-165: unit height 0, no no-shadow flag / state 146, no COF box pre-test,
no single-cel shadow for items and missiles, hover only with light on.

## Tests

`world_view/unit_shadow/tests.rs` (shape, degenerate cels, end-to-end draw
with position, ops and key), `preview_light` (`only_the_hover_target_is_highlighted`),
`rules/draw_order/tests.rs` (a shadow entry is recorded). Changed expectations
(the feature is now wired): the resolve test no longer expects an error for a
unit-shadow entry; the preview unit-facts test expects flag-ex 0x80 for a
player; the unit art test counts the derived shadow frames.

## Left

Motion-record height for flying/jumping units; item and missile shadows;
wall fade check in a level with `Logicals`; the edge cases at the top-down
cel rows.

## Local check (needs `D2_GAME_DIR`)

```
cargo run -p d2-client --release -- play --new amazon Test
```
Look for: a dark sheared shadow, half the sprite's height, left of and
under the player and each town NPC; moving the mouse over an NPC or
monster brightens it (not with `D2RS_FULLBRIGHT=1`); no `frame not drawn`
warning mentioning "unit shadow".
