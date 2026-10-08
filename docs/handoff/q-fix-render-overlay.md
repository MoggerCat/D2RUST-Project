# q-fix-render-overlay: one camera, one local position, automap, belt pop-up

Branch `claude/q-fix-render-overlay` (from `claude/q-render-audit`, merged
with `claude/specs-staging-7`). Cloud, no game files. Rows worked:
`q-fix-render-one-camera`, `q-fix-render-overlay-local-pos`,
`q-fix-render-fade-player-tile`, `q-fix-render-automap-pass`,
`q-fix-render-automap-blend`, `q-fix-render-belt-popup`.
`q-fix-render-ui-blend` was **not** done here: it is the same
`ImageRequest` mode / remap plumbing as `q-fix-ui-draw-sink` (its own
session, which owns `ui/draw.rs`, `panel_art.rs` and the callers); that
session was told by message.

## Local-player position

`q-fix-seam-movement` had not landed a single position when this branch was
cut, so every reader takes **the predicted position the frame camera
already used** (`ModelFeed::position_of`: the walk prediction in the
preview, else the model sub-tile centre). It is decided once per drawn
frame as `rules::camera::FrameAnchor` (player position + shake,
`world_view::frame_anchor`) and turned into the frame's one camera by
`world_view::camera_at` after the UI has set the open mode. When the seam
row lands its own position source, only `ModelFeed::position_of` /
`frame_anchor` need to read it.

| Reader | Before | Now |
|---|---|---|
| hover pick, object labels, corpse clicks (`present.rs`) | `corpse_click::camera_for` (model cell, no shake) | the frame's camera, also handed to `build_frame_placed` |
| automap toggle facts, automap view | own camera from the model cell | `frame.camera`; the player marker at the anchor's position |
| overhead bubbles (`ui/overhead_ui.rs`) | own camera from the model cell | `OriginalUi::set_frame_anchor` before the UI frame; local player's bubble at the anchor |
| ground items, missiles (`add_to_frame`) | own camera, shake (0, 0) | `frame.camera` (§2.6 shake included) |
| missile cast origin, follow and state overlays | model cell | `feed.unit_position` (the predicted 16.16 for the local player) |
| wall / roof fade centre (`near_rooms.rs` `player_tile`, `player_logical`) | model cell | the predicted sub-tile (preview) |

`corpse_click::camera_for` stays for the strict path's tests
(`seam_world_screen.rs` §2.2 vector without prediction).

## Automap

- Pass: keyed `(UI, UI_AUTOMAP_MAJOR = 0, n)`; the UI root's panel draws are
  now `(UI, UI_PANELS_MAJOR = 1, 0)` (`scene::order::pass`), so the map draws
  after every world pass (rain / snow of pass 9, screen fade 10) and before
  every panel (`ui/panels.md` §5 r3, `draw-order.md` OQ14).
- Blend: each cel's §10 r4 mode through `rules::blend::cel_ops` with the act's
  `ShadeTables` (0/1/2 → A2/A1/A0, 5 opaque), unlit. Without loaded tables
  (non-preview feeds) opaque (d2rs-own). Open mode 3 draws no automap (§10 r1).
- Still missing: header and name texts (§11 r7, §13).

## Belt pop-up

`ui::panels::control::belt::hover_text` follows `control-panel.md` §5 r8 as
now written: T = `Prefix(S, 3)` + `Prefix(N, 0)` (stat lines first, name
above), S cut to 256 units and empty for quality 3, T to 384, a price after
an LF. `TopUi` draws it through `hud_tips::popup_request` →
`ui::text::popup_text` (r14 placement, font 1). d2rs-own (module doc of
`hud_belt.rs`): N / S come from the item tool tip lines, not the
`0x0048C060` / `0x004E6410` strings; no shop price. The r14 backing box
still waits on the UI rectangle primitive (`q-fix-ui-draw-sink`).

## Changed expectations (spec-driven)

- `belt.rs` `hover_text_rules`: the old order (name colour 3, LF, stats
  colour 0; price after a space) → the spec vectors of §5 r8 / Test vectors.
- `automap_view_tests`: automap items are in the UI pass, not pass 9
  (`draw-order.md` OQ14).
- `inv_items_tests::hovering_a_belt_item_yields_its_tip_at_the_box`:
  `hover_tip` returns the r8 hover text (point, colour, centring, name
  part) instead of tool-tip lines; the anchor (475, 562) is unchanged.
- `ground_items_tests` / `missiles_tests` frames now carry the frame camera
  (`framed`), as `present.rs` hands them.

## Checks

New tests: `seam_world_screen::with_a_prediction_every_camera_of_the_frame_is_the_draw_camera`,
`missiles_tests::the_local_players_overlays_follow_its_predicted_position`,
`overhead_ui::anchor_tests::the_local_players_bubble_stands_on_the_drawn_player`,
`tests_drlg::the_fade_player_tile_is_the_predicted_sub_tile`,
`automap_view_tests::the_automap_draws_after_every_world_pass_and_before_the_panels`,
`automap_view_tests::automap_cel_modes_map_to_the_alpha_tables`.
All unverified against 1.14d (no capture): local run entry 100 (`play` on
the install) covers the screen.
