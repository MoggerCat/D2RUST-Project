# q-fix-seam-movement: movement and world/screen seam fixes

Branch `claude/q-fix-seam-movement` (from `claude/q-seam-audit`, staging-7
merged). Rows of `docs/handoff/build-queue.tsv`, findings of
`docs/handoff/q-seam-audit.md`. Gate: fmt, clippy (d2-sim, d2-server,
d2-client, test-fixtures), nextest 7279 passed, coverage, spec_index.
Nothing here was run on the real install (local queue).

| Row | Status | Test |
|---|---|---|
| q-fix-seam-check-own-position | done: the check's (cx, cy) and tolerance mode are the client's own walk (`ClientWorld::predicted`, `LocalWalk { pos, mode }`) | `seam_movement_tests::a_prediction_on_the_servers_path_is_never_corrected`, `…_that_parts_from_the_server_is_corrected` |
| q-fix-seam-monster-velocity | done: 0x67 / 0x68 field read as stat 67; path velocity by the server's `mode_velocity` on monstats `Velocity` (`motion::monster_step`) | `seam_movement_tests::a_monster_walk_moves_as_far_on_the_client_as_on_the_server` |
| q-fix-seam-local-pos | done: `ClientWorld::local_position` / `local_cell`, read by corpse / hover / label / overhead cameras, automap, near rooms, click | `seam_world_screen::with_a_prediction_every_camera_is_the_draw_camera` |
| q-fix-seam-click-distance | done: `ModelClick` position and path distance from the own position | `bridge::click::tests::an_npc_click_measures_from_the_predicted_position` |
| q-fix-seam-predict-velocity-stats | done: `predict::MoveStats` (stat 67 / 96 totals), `Speeds::step_with`, `ClientPath` `Own` | `seam_movement_tests::the_predicted_tick_distance_reads_stats_67_and_96` |
| q-fix-seam-stamina-scale | **open**: the wire carries stamina >> 8, so raw 1..255 cannot be recovered; whether 1.14d's client drains locally is `client/model.md` OQ2 (REC-51). Needs a spec answer first. | — |
| q-fix-seam-pick-anchor | done (with staging's `hover::unit_feet`) | `seam_world_screen::a_units_pick_anchor_is_its_draw_anchor` |
| q-fix-seam-shake-camera | ground items and missiles use the frame's camera; the pick cameras still build theirs unshaken (no shake starts yet) | `ground_items_tests::a_ground_item_moves_with_the_frames_shaken_camera` |
| q-fix-seam-anim-frame | done: objects always use the model frame | `unit_rules::tests::an_object_loads_draws_and_animates` (expectation now the model frame) |
| q-fix-seam-present-offset | done: `Presentation::centre_offset` | `seam_bridge_app::the_presented_image_sits_where_the_click_mapping_reads_it` |
| q-fix-seam-room-order | code done (write-back on every filled frame); spec line in `client/bridge.md` §1 r1 left to a spec session; no test | — |
| q-fix-seam-act-cache | done: `MapState` drops draw state on a new client DRLG (act, init seed) | `tests_drlg::a_new_client_drlg_drops_the_tile_draw_state` |
| q-fix-seam-staged-act | done: the tick refreshes staged units' act | `waypoints::a_staged_players_act_follows_the_world_each_tick` |
| q-fix-seam-tick-order | done: `WiredWorld::collect_sent` after each step, one outbox | `trade_quests::a_ticks_messages_leave_in_production_order` |
| q-fix-seam-grid-facts | partly: the cube grid passes mode 0x0E; the stack / tome / cube / ctrl-sell facts need item tables in the UI (overlaps q-fix-ui-play-wiring's beltable) | — |
| q-fix-seam-stash-fallback | done | `seam_item_grids::the_stash_fallback_is_the_measured_record`, fallbacks added to `every_accepted_pixel_names_an_in_grid_cell` |

Changed test expectations (each follows the seam specs):
- `motion` unit tests and `app_play_combat`: the 0x67 field is a percent;
  the class gets monstats `Velocity` 16 (same 1 sub-tile / tick).
- `unit_rules` object test: the model's 8.8 frame, frame 0 included.
- `app_play_visibility` diagonal walk: the own cell rarely differs from
  the 0x96 on both axes, so the test now adds a server point one sub-tile
  off on both axes to reach rule 6.
- `smoke_travel` rig: installs an all-visible predicate (it draws no frame
  and has no unit art; on screen the walk-out of a waypoint arrival is
  visible, REC-288).
- Callers of `hover::feet` with a unit type use `unit_feet`.
