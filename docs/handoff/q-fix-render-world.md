# q-fix-render-world — world render rows of the render audit

Session `q-fix-render-world`, 2026-10-08, branch `claude/q-fix-render-world`
(from `claude/q-render-audit`). Cloud, no game files. Rows of
`docs/handoff/build-queue.tsv` from `docs/handoff/q-render-audit.md`.

## Fixed

| Row | Spec | Change | Test |
|---|---|---|---|
| `q-fix-render-edge-floors` (edge floors) | draw-order-2.md §14, OQ 2; levels.md §2 r3, §3 step 5 | `order_frame` adds the edge floors after the floor pass (extents from the room floors passing the whole-tile test, act edge record, `edge_key`), no more Open error; the act's base library and edge key (`d2_sim::drlg::tiles::ACT_EDGE_TILE`, `first_entry`), loaded by `WorldFiles`; `ClientDrlg::edge_tile`; `NearRooms::{edge, player_subtile}`; `TileArray::Edge` (DT1 entry in `MapState`); `FrameSize::resolution_mode` | `rules::draw_order::tests::edge_floors_follow_the_last_room_at_open_mode_0`; `bridge::msg::tests_drlg::near_rooms_come_from_the_client_drlg`; `d2-server` `world_files_load_every_named_file_once_by_its_table_string`; install: `game_world_data::act_edge_floor_records_resolve_in_the_base_libraries` |
| `q-fix-render-missile-blend` | blend-modes.md §4 | missiles `cel_ops(missile_mode(Trans))`, overlays `cel_ops(overlay_mode(Trans))`, no remap, unlit | `world_view::missiles::tests::missiles_and_overlays_draw_in_their_trans_mode` |
| `q-fix-render-missile-key` | draw-order.md §3 r4, §10; unit-composite.md §5 r1, r4, §10 | `Missiles::keyed`: a missile in its cell's unit list (after the walls, before the first unit whose client y is not below its own), an overlay with its host (back sub 0 before, front sub 255 after); host not drawn → overlay hidden; inserted into the sorted frame | `missiles_join_their_cell_and_overlays_their_host` |
| `q-fix-render-tile-block-mask` | shading.md §4 r4; lighting.md §11 r3 | DT1 frame sets hold each block alone after the assembled image (`DT1_BLOCK_FRAME0`); per-block draws use the block's own image at its position | `rules::tests::tile_blocks_draw_one_item_per_block`, `frames::tests::dt1_tile_uses_the_map_preview_tile_image` |
| `q-fix-render-unit-shadow-flag` | draw-order.md §3 r4, §5 r3 | `MapState` keeps flag 0x10000000 and flag-ex 0x80 per unit across rebuilds; the preview no longer forces 0x80 | `near_rooms_come_from_the_client_drlg`; `preview::tests::unit_facts_are_zero_and_visible` |
| `q-fix-render-ground-item-key` | draw-order.md §3 r4, §5 r3, §10; unit-order.md §5 r6 | `WorldFrame::slots` (the order's unit slots); ground items take their slot, not drawn → hidden. Seam found on the way: the model never linked ground items into room lists, so the order never saw them; 0x9C / 0x9D now do the item mode set's leave + insert (mode 3 / 5 → head of the room of the point) | `ground_items_take_their_slot_from_the_draw_order`; `bridge::msg::tests_drlg::ground_items_join_the_room_of_their_point` |
| `q-fix-render-unit-frame` | unit-composite.md §3 r2, r6 | objects (the model animates them) use +0x44 >> 8 as is, frame 0 included, no `% F` | `unit_rules::tests::an_object_loads_draws_and_animates` |
| `q-fix-render-cof-cull` | unit-composite.md §4 | `ViewRules::unit_box_visible`; `OriginalView` tests the COF box at the camera §4 position for types 0–2 | `rules::tests::units_failing_the_cof_box_pre_test_are_not_drawn` |
| `q-fix-render-act-load-clear` | composition.md §3 step 4 | `ClientWorld::act_loads` (each handled 0x03); `present::note_act_loads` sets the post-draw clear | `present::act_load_tests::the_frame_after_an_act_load_presents_all_index_0` |

Changed expectations (spec-driven, named in the commits):
`open_mode_3_and_level_backgrounds` (no Open 10 from `order_grid`; the
edge floors are `order_frame`'s), `an_object_loads_draws_and_animates`
(frame from +0x44, not ticks), `unit_facts_are_zero_and_visible` (no
forced 0x80), `tile_blocks_draw_one_item_per_block` (block image, not the
tile image under a clip), `units_are_not_culled_by_the_view` (COF box
reaching into the frame), `dt1_tile_uses_the_map_preview_tile_image`
(block frames).

## Not done

- **Level backgrounds (74, 120)** of the edge-floor row (draw-order-2.md
  §12): still an Open error in `order_grid`. Level 74 is blocked by the
  spec: §12 r3 names no initial star tick `last` (`BackgroundError::
  NoStarLast`). Level 120 needs a pass-1 art path in the world view (the
  `summit01` / `cloud01` cels, light −1 / 0xDDDDDDDD, draw modes 5 / 3)
  and its seed from the time value; a row of its own.
- Missiles stay unflat and not sight-tested (unit flag 0x10000 and the
  §5 r3 test for missiles need the client missile as a model unit);
  several missiles before the same unit keep creation order, not their
  own y order.

## Local run

HANDOFF §5 entry 101.
