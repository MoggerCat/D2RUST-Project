# q-pending-audit

## Audit (`ui/original.rs` PENDING, `world_view/model_feed.rs` PENDING)
Deleted (wired), each with its covering test:
- Stash and cube panels: `stash_ui`, `cube_ui` — `ui/cube_ui_tests.rs`, `ui/panels/stash_input/tests.rs`, `tests/app_play_stash.rs`.
- NPC menu, NPC shop: `npc_menu_ui`, `shop_ui` — `tests/e2e_vendor.rs::shop_panel_buys_and_closes`.
- Cursor jump: now wired (below).
Trimmed to the true remaining input: waypoint (tab gates only; strings are wired, `tests/app_play_npc.rs`), stash gold kinds 3 / 4 wired (`original_tests.rs::stash_gold_withdraw_and_deposit_send_0x4f`), quests hotkey wired (`quest_log_ui_tests.rs`), `ViewFeed::light` (preview light is wired: `preview_light` tests; the per-unit look inputs remain), shift-spend removed from the character entry.

## Wired
- Shift: `CharacterUi` release reads the host's shift flag (`set_shift`); spends all points in 32-point chunks (spec count: `panels.md` §8 r5; not 5). Test `a_stat_button_spends_one_point_and_shift_spends_all_in_chunks_of_32`. Skill tree: no spec'd Shift spend.
- Cursor jump: `OriginalUi::take_cursor_warp` → `world_view/present.rs` sets the window cursor (`edge::frame_to_window`, `Presentation::from_frame`). Tests `a_cursor_jump_effect_becomes_a_cursor_warp_to_the_new_x_at_the_same_y`, `from_frame_is_the_inverse_of_to_frame`. PROVISIONAL REC-268.

## Local check
`cargo run -p d2-client --release -- play`: press I with the mouse at the right of the screen — the cursor jumps left to the inventory; with stat points, Shift+click a "+" button spends them all.
