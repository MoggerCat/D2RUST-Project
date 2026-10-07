# impl-spec-answers: `specs/items/inventory.md` (second-pass answers)

Implementation of the answers changed in `specs/items/inventory.md` between
`b435f5a` and HEAD (OQ3–OQ17, OQ19; HANDOFF IV1–IV8, MV1–MV6, WN2, WN3,
PN1, IS1, IS2, GX2, GX3). Not in scope: OQ1 / `items/bitstream.md`, the
0x18 / 0x95 layouts, `combat/vitals.md` §5 / OQ18, anything in `path/`.
WN1 and GX1 were done by the parent.

## Answers implemented

| answer id | code change | test |
|---|---|---|
| IV1 / §1.4 r3 (cursor) | `Inventory::put_cursor` (set: +0x20 and the item's +0x5C only; none: unlink of the cursor item, node fields / owning inventory / weapon GUID cleared); `Inventory::unlink` accepts the cursor item (not in the list); `place_in_grid` unlinks the cursor item first; the rules (`grid.rs`, `equip.rs`, wiring `ops.rs` `set_cursor`) use `put_cursor`. `set_cursor` stays a raw field write for fixtures. | `answer_tests::put_cursor_links_nothing_and_none_unlinks_the_cursor_item` |
| IV2 / §2.4 r5 | link-check failure returns 0 with nothing undone (TODO replaced) | `answer_tests::failed_link_check_leaves_the_item_placed_in_mode_4` |
| IV3 / §2.4 r3 | page-2 trade hook moved after step 8, before step 9 | `tests::place_in_page_cursor_charm_and_passes` (order) |
| IV7, GX2 / §1.3 | `grid_record`: TODO replaced (types 3–5 → none; pages 0, 5–255 → class record) | `answer_tests::grid_record_other_pages_and_owner_types` |
| PN1 / §2.2 | `in_bounds`, `fits`, `Grid::set_rect`: signed 32-bit with wrap (wrapped end passes, loops run zero times); grids 0 / 1 skip the bound test | `answer_tests::signed_wrap_places_without_cells`, `prop_inventory::regress_place_near_i32_max` |
| IV4 / §3 r5 | TODO replaced (fall-through to the next column, already coded) | `answer_tests::full_similar_column_falls_through_to_the_next` |
| IV5 / §3 r8 | TODO replaced (item flags; unmoved items not flagged, already coded) | `answer_tests::compaction_flags_only_items_that_move` |
| OQ3 / §3 r7 | doc (slot ≤ 15 unsigned, no `numboxes` check anywhere) | `answer_tests::belt_slot_bound_is_fifteen_without_numboxes` |
| GX3 / §3 r1 | doc (no item-type test) | — (doc only) |
| OQ13 / §3 r9 | `moves::handlers::belt_change` (0x9C 0xF direct with flag 0x20, unlink, mode 4, item-skill unlink, page 0, §2.4 find-free, else ground drop at U, else detached); called from 0x1B / 0x1C (no new belt) and 0x1D (N). New `InventoryOps::belt_item`, `belt_boxes`; library `belt_boxes_of`. Seam `MovePending::belt_unequip` removed. | `answers::belt_change_moves_items_beyond_the_new_boxes`, `answer_tests::belt_boxes_of_a_belt_or_none` |
| OQ13 / §3 r10 | library `belt_removal_allowed`; `InvDesk::belt_remove_allowed` uses it (no longer forwarded to the rest) | `answer_tests::belt_removal_gate_refuses_only_a_trade_with_belt_items` |
| OQ4 / §4.2 r2 | `requirements_met` uses `combat::pct(req, p, 100)`; seam `percent_of` removed from `InvWorld` / `InvRest` | `answer_tests::requirement_percent_is_signed_and_truncating` |
| OQ5 / §4.8 | new `items::inventory::levelreq` (`level_requirement` over `LevelReqItem` / `LevelReqUnit`, affix value, crafted cap 98) | `answer_tests::level_requirement_by_quality` |
| IV8 / §4.4 r6 | TODO replaced (other unit types → no) | `answer_tests::hands_of_an_object_owner_never_dual` |
| OQ7 / §4.5 | `stack_test`: ethereal bits from item flags, `stack_quality_ok` (q ∈ 1–3); seams `stack_value`, `stack_quality_ok` removed | `answer_tests::stack_test_ethereal_and_quality` |
| IV6, OQ8 / §4.7 | TODOs replaced (primary type; right hand then left); `equip_profile` / `auto_equip_compatible` (ten-flag profile, `0x0055D670`); seam `auto_equip_allows` removed | `answer_tests::auto_equip_primary_tpot_and_quiver_hands`, `answer_tests::auto_equip_compatibility_profile`, `tests::auto_equip_rules` |
| OQ6 / §5.6 | library `active_inventory_item`, `usable`; `InvDesk` `active_item` / `is_active` use it | `answer_tests::active_inventory_item_and_usable` |
| OQ9, IS1 / §6.1 r4 | `deferred::update_list_reset` (`0x00597B00`: bit 0 only, body location 0, per-item reset tables `0x00738C70` / `0x00738C4C`, own lists, removal on cmd 0x1, list free), `item_reset`, `room_cleanup` (`0x00553220` flags); `InventoryOps::update_list_free`; `InvDesk::update_done` calls the reset; server update pass no longer clears bit 1 | `answers::update_list_reset_clears_by_the_tables`, `answers::room_cleanup_clears_unit_flags_and_bits`, server `pick_item_to_the_cursor` |
| MV1, rows 18–19 / §6.2 | dispatcher: a `to`-excluded row lets the walk go on; item-flag rows 18 / 19 end it | `answers::excluded_row_lets_the_walk_go_on`, `answers::item_flag_rows_18_19_end_the_walk` |
| MV2 / §11 (filler 0x9D 0x13) | fillers only for a socketed item and flags without 0x20; owner = parent item (4, GUID); flag argument | 0x8; seam `filler_owner` removed | `answers::no_fillers_with_flag_0x20`, `deferred::fillers_follow_their_parent` |
| MV3 / §9.2 | doc (absolute 0); 0x7D state already `flags & flag` | `answers::item_flag_rows_18_19_end_the_walk` |
| OQ10 / §7.1 r2 | 0x16 type 0 (`pick_player`: range, walk > 8, corpse pickup when dead and not trading, else interaction), type 3 → 1, type 5 (`pick_tile`: warp < 5, else walk); new seams `walk_to_unit`, `tile_warp`, `corpse_pickup`, `player_interact`, `MoveUnits::unit_mode`; `pick_other` removed | `handlers::pick_other_unit_types`, `handlers::pick_player_and_tile` |
| OQ11 / §7.4 r2, §7.24 | `layouts::cant_do_that` (0x5A, 40 bytes) sent for a cursor item; seam `resync` removed | `handlers::remove_from_buffer`, `handlers::item_to_belt_shift`, server `lift_and_insert` |
| OQ12 / §7.4 | TODO replaced (busy player: any page, already coded) | `handlers::remove_from_buffer` |
| WN2 / §7.6 | 0x1B: X stays the cursor (no cursor clear); failed put → N detached, result 1; failed link → 3; belt via §3 r9 | `handlers::swap_two_handed`, wiring `equip::two_handed_swap_and_removal_from_the_other_hand` |
| WN3, MV4 / §7.7 | doc; `0x0063E490` without an item → out 1 | `handlers::remove_body_item` (existing) |
| MV4 / §7.8 | E missing / not mode 1 → out 1; N's put or link failing → out 1; N of primary type 19 → belt change | `answers::swap_cursor_with_body_failures_and_belt` |
| OQ14 / §7.9 | 0x1E body `swap_1h_2h_body` (`0x00561220`, steps 1–7); seam `swap_1h_with_2h` removed | `answers::swap_1h_with_2h_body`, server `swap_one_handed_with_two_handed` |
| MV4 / §7.10 | C's link failing → out 1 | `answers::swap_cursor_buffer_link_failure` |
| OQ14 / §7.11 | 0x20 body `use_grid_body` (`0x0055E170`, steps 1–4 incl. `ass` / `xyz` / `tr2` / `toa`); seams `use_item_at`, `consume_item`, `item_skill`, `has_skill`, `skill_decrement`, `set_quest_flag`, `quest_item_used`, `quest_tr2_used`, `reset_skills_stats`; `use_grid_item` seam removed | `answers::use_grid_item_body_use`, `answers::use_grid_item_quest_items`, `handlers::use_grid_item_range`, server `use_grid_item` |
| OQ15, MV5 / §7.12 | TODO replaced (stat 72 durability); book test on the primary type | `handlers::stack_items` (existing) |
| MV4 / §7.16 | C's link failing → fatal `MoveFatal::Link` | `answers::switch_belt_item_link_failure_is_fatal` |
| OQ14 / §7.18 | 0x27 body `use_item_action_body` (`0x00561ED0`, steps 1–9); `use_item_action` seam removed | `answers::use_item_action_gates`, `answers::use_item_action_effects`, server `use_item_action` |
| MV4 / §7.19 | TODO replaced (only filler / mode 4 / target missing set out) | `handlers::socket_item` (existing) |
| MV5 / §7.20 | book test on the primary type; core `scroll_into_book` shared with the pickup | `answers::pickup_scroll_and_book_into_a_tome` |
| OQ16 / §7.23 | seam `not_dead` → `has_used_skill` (default yes: 0x61 does nothing) | `handlers::merc_item_gates` |
| OQ14 / §8.1 r4 | `ground::pickup_special` (scroll → tome via `tome_for` + 0x29 core; book onto a tome; auto-stack over body grid then page 0); new `InventoryOps::page_items` / `body_items`, `MoveUnits::primary_type` / `stackable` / `autostack` / `quiver`; `MoveFatal::NegativeQuantity`; seam `pickup_special` removed | `answers::pickup_scroll_and_book_into_a_tome`, `answers::pickup_auto_stack_fills_in_order`, `ground::auto_pickup_gold_specials_equip` |
| OQ19 / §8.4 r6 | `HELD_PAIRS` full list (+ `hst`/`vip`, `qf2`×4); walk stops at P; carry-one test reads X's quality / file index; corpse seam doc | `answers::held_pairs_corpses_and_the_stop_at_p` |
| OQ17 / §9.2 | `ground::expired_items` (reader `0x00558B90` selection), `EXPIRY_INTERVAL` | `answers::expiry_reader_takes_due_items_only` |
| IS2 | no code (spec: wiring difference stays as documented in the server update pass) | — |

## Test expectations changed (and why the spec says so)

- `items::inventory::tests::place_in_page_cursor_charm_and_passes`: page 1
  now ends with no cursor (§2.4 step 5 / §2.2 / §1.4 r3: the placement's
  unlink of the cursor item clears it); the trade hook is logged after
  step 8, not first (§2.4 step 3).
- `items::inventory::tests::auto_equip_rules`: the `allows` knob is gone;
  two one-hand swords for a sorceress now give none, for a barbarian the
  free hand (§4.7 compatibility profile). Fixture: shield type 2 made
  equivalent to `shld` (51); default item quality 2 (§4.5 stacks only
  1–3).
- `items::moves::tests::handlers::swap_two_handed` and wiring
  `equip::two_handed_swap_and_removal_from_the_other_hand`: X stays the
  cursor item (WN2, §7.6); the wiring test empties the cursor before 0x1C.
- `items::moves::tests::handlers::remove_from_buffer`,
  `item_to_belt_shift`, server `lift_and_insert`: a cursor item now sends
  the 0x5A "can't do that" (§7.4 step 2, OQ11).
- `items::moves::tests::handlers::pick_other_unit_types`: type 3 → 1, a
  missing player / tile → 1 (§7.1 step 2, OQ10).
- `items::moves::tests::handlers::remove_body_item`: the belt step is the
  belt change (§3 r9), not a logged seam.
- `items::moves::tests::handlers::swap_1h_with_2h`, server
  `swap_one_handed_with_two_handed`: the body is implemented; §4.3 ≠ 7 →
  nothing (§7.9 step 2).
- `items::moves::tests::handlers::use_grid_item_range`: the item must be
  `useable` (§7.11 step 1); server `use_grid_item`: the key is not
  `useable` → 3.
- server `use_item_action`: a cursor item exists → 0 (§7.18 step 2).
- `items::moves::tests::deferred::fillers_follow_their_parent`: the parent
  must be socketed and the filler's flag argument is 0x8 (§11, MV2).
- `items::moves::tests::ground::auto_pickup_gold_specials_equip`: the
  `special` knob is gone; an auto-stack merge is tested instead (§8.1
  step 4).
- server `pick_item_to_the_cursor`: +0xC8 bit 1 stays set after the
  clean-up (IS1, §6.1 r4).
- `prop_tests::handlers_on_arbitrary_payloads`: the 0x5A of a 0x19 / 0x63
  rejection is the one allowed effect; knobs `not_dead` → `used_skill`.
- `prop_inventory`: the model's bound test wraps (PN1), the cursor item's
  placement clears the cursor, the stack and auto-equip references follow
  §4.5 / §4.7; `regress_place_near_i32_max` now expects the wrapped
  placement (no cells) and the refusal of x + w = `i32::MAX`.
- `mutants_wiring_inventory`: forwarding checks of removed or now-computed
  seams dropped (`belt_unequip`, `use_grid_item`, `use_item_action`,
  `swap_1h_with_2h`, `pickup_special`, `not_dead`, `pick_other`,
  `resync`, `stack_quality_ok`, `auto_equip_allows`, `percent_of`,
  `belt_remove_allowed`, `is_active` / `active_item`, `filler_owner`).

## Open / not done

- §4.8 data plumbing: `InvWorld::level_requirement` still goes to the
  rest; gathering affix rows (`levelreq`, `class`, `classlevelreq` are not
  in `AffixRec`), set / unique `lvl req`, item data +0x30 version, the
  item's extended stat list (stats 97 / 107) and skills rows into
  `LevelReqItem` is a wiring task.
- §5.5 item-skill link / unlink and the cube recount `0x0055FA40`, §5.7
  inventory pass: still seams (`charm_relink`, `charm_unlink`,
  `inventory_pass`; names kept, docs updated). They need the skills and
  stat-list providers.
- §9.2 reader: `expired_items` selects; running it every 1,500 frames per
  act and the removal belong to the tick wiring (`sim/tick.md` §3 step 11).
- §6.1 r4 `room_cleanup` (unit flags) is not called by the tick wiring
  (IS2); the server calls only the update-list reset.
- Item-use effects `0x005BF240`, skill quantities (0x22), quest-record
  writes, consume `0x0055E000`, corpse pickup, player interaction and
  tile warp are new `MovePending` seams with narrow defaults.
- TODOs kept (not answered): §7.8 E's unlink failure (MV4 does not list
  it); §7.17 no hireling, §7.23 failed copy, §8.1 r5 / r7, §9.3 unlink,
  §10.2 failed creation (MV4 "not re-read"); `0x00557FD0` body.
- §7.9 step 3: "X missing → fatal" after `0x0063CB00` is read as part of
  the free-position test (a missing X gives out 1).
- `d2-client/tests/e2e_support/mod.rs` was edited (removed impls of the
  dropped `InvRest` / `MovePending` methods) but not compiled (no client
  build in this session); `d2-client` e2e tests not run.
- Unrelated d2-sim lib failures seen in the full run (missiles bodies,
  monsters AI catalogue, skills tables: 9 tests) are outside this scope.
