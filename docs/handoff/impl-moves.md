# Handoff: item-move intents and deferred item messages (`d2_sim::items::moves`) — `claude/impl-moves`

> Not folded into `docs/HANDOFF.md` / `docs/PLAN.md` yet; for the coordinator (§1 row 3d / 3k, §3 code map, §7 questions). This file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: implementation from
a clear spec, medium (METHODS M14). Branch `claude/impl-moves` from
`claude/tender-meitner-mphas3` at `9b49081`. Spec:
`specs/items/inventory.md` §6–§11 (+ `items/item-actions.tsv`). Repo only;
every claim below holds on this branch.

## 1. State

**Implemented, unverified** (the spec is a draft; no recording R1–R6
exists; the unit tests prove the spec's synthetic vectors and the
handlers' validation order on a fake only).

- New module `crates/d2-sim/src/items/moves/` (`mod.rs`, `seams.rs`,
  `deferred.rs`, `handlers.rs`, `ground.rs`, `layouts.rs`, `tests/`). The
  only other change is the line `pub mod moves;` in `items/mod.rs`. No
  dependency, spec, `items::inventory`, `wiring`, or `d2-server` change.
- Independent of the parallel `impl-inventory` session: every §1–§5
  operation is a method of the seam `InventoryOps` (§4 below).
- Tests: 59 new (`cargo test -p d2-sim`: 1291 pass, 5 ignored on this
  branch): test vectors G1, G2, G3, X1; edge cases 1, 3, 4, 6, 7, 8; every
  handler's validation order and result codes; the dispatcher's row
  order, recipients and conditions; ground items; category; message
  sizes; TSV checks with perturbation tests (M05, M08):
  `item_actions_match_tsv` (all 9 columns of `item-actions.tsv` against
  `ITEM_ACTIONS`), `layouts_match_server_tsv` (every built S→C layout
  0x19, 0x1D–0x1F, 0x3F, 0x42, 0x47, 0x48, 0x7D, 0x9C, 0x9D read back
  through the `server-messages.tsv` `layout`/`size` columns),
  `handled_ids_match_client_tsv` (ids, sizes, kind and gate of the 23
  handled ids in `client-messages.tsv`). Each perturbation test changes
  one cell and asserts the exact single report.
- Coverage (`tools/coverage.py`): inventory.md 84 / 148 units claimed
  (unit tier), every unit of §6–§11 and of my edge cases except §6.1 r4
  (OQ9, not implemented) and §7.21 (the cube's). The 64 uncovered units
  are §1–§5 (the inventory session's) plus those two. Tool note: the
  wrapped line "15000. Who reads +0x24…" in §9.2 is read as list item
  `§9.2 r15000`; it is claimed with `§9.2 text`. A reflow of that line
  in the spec would remove the artefact (and break the claim: fix both
  together).

## 2. Code map rows

| Path | What | Spec |
|---|---|---|
| `items/moves/mod.rs` | `Guid`, `NONE_GUID`, `Owner`; constants `mode`, `page`, `cmd`, `iflag`, `uflag`, `res`, `ty`, `stat`, `sound`, ranges, masks, caps; `MoveFatal`; `Outcome` (helper result + out flag → handler result) | §1.1, §7 text, Constants |
| `items/moves/seams.rs` | `InventoryOps`, `MoveUnits`, `MovePending`, `MoveWorld`, `Spot` | §1–§5 calls, Related specs |
| `items/moves/deferred.rs` | `ITEM_ACTIONS` (+ `Test`, `To`, `Cond`, `ItemAction`), `owner_refresh`, `mark`, `category`, `dispatch` (`0x005973F0`), `player_update` (`0x00580860` / `0x00597890`), `ground_update` (`0x0055BED0`), `send_item_page` (`0x0053D010`), `send_to_belt` (`0x0053EE70`) | §6, §11 |
| `items/moves/handlers.rs` | `handle` (size check, dispatch by id), `HANDLED`; one function per id: `pick_item` 0x16, `drop_item` 0x17, `insert_item` 0x18, `remove_from_buffer` + `to_cursor` 0x19, `equip_item` 0x1A, `swap_2handed` 0x1B, `remove_body_item` 0x1C, `swap_cursor_with_body` 0x1D, `swap_1h_with_2h` 0x1E, `swap_cursor_buffer` 0x1F, `use_grid_item` 0x20, `stack_items` 0x21, `unstack_items` 0x22, `item_to_belt` + `to_belt` 0x23, `item_from_belt` 0x24, `switch_belt_item` 0x25, `use_belt_item` 0x26, `use_item_action` 0x27, `socket_item` 0x28, `scroll_to_book` 0x29, `drop_gold` 0x50, `merc_item` + `merc_take` + `merc_give` 0x61, `item_to_belt_shift` 0x63 | §7 |
| `items/moves/ground.rs` | `leave_room`, `pickup_auto` (§8.1), `pickup_to_cursor` (§8.2), `refused_pickup` (§8.3), `can_pick` + `held` + `HELD_PAIRS` + `QUEST_PICKUP` (§8.4), `drop_spot`, `ground_expiry`, `ground_place`, `drop_cursor_item`, `cube_spill` (§9), `gold_limit`, `gold_pickup`, `gold_piles` (§10) | §8–§10 |
| `items/moves/layouts.rs` | byte builders `item_world` 0x9C, `item_owned` 0x9D, `item_state` 0x7D, `relator1` 0x47, `relator2` 0x48, `clear_cursor` 0x42, `use_stackable` 0x3F, `gold` 0x19 / 0x1D / 0x1E / 0x1F | §10.3, §11 |
| `items/moves/tests/` | `mod.rs` fake world (`Fake`, `Knobs`: every seam, with a call log); `deferred`, `handlers`, `ground`, `tsv` | Test vectors, edge cases |

## 3. Public API (all new; no existing signature changed)

- `items::moves::handle<W: MoveWorld>(w, player: Guid, msg: &[u8]) ->
  Option<Result<u32, MoveFatal>>`: `None` for an id outside `HANDLED`
  (0x4C included: `world/cube.md` §10), else the handler result 0–3 or a
  fatal assert. The size check (→ 3) is first.
- `items::moves::player_update(w, client: Guid, player: Guid) ->
  Result<Vec<Vec<u8>>, MoveFatal>`: the per-client update pass (item
  messages in update-list order, then 0x47, 0x48). `dispatch(w, client,
  inv: Owner, item)` for one item; `ground_update(w, item)` for a mode-3
  item (the caller decides when the item unit update runs).
- `mark`, `owner_refresh`, `category`, the `ground` functions and the
  per-id handler functions are `pub` for the wiring and other callers
  (`world/vendors.md`, `world/cube.md` use §2.4 / direct sends).
- Messages queued "now" (0x63, cube spill, 0x42 of a stack merge) leave
  through `MovePending::send`; deferred ones are returned by
  `player_update`.

## 4. Seams (trait methods → expected provider)

`InventoryOps` (provider: `items::inventory`, §1–§5; no defaults):
`has_inventory`, `cursor`, `set_cursor`, `items` (§1.4 link order),
`unlink` (false = missing / mismatch), `update_list`, `update_list_add`,
`weapon_in_use`; `place_at` (§2.2), `find_free` (§2.3), `place_in_page`
(§2.4 steps 2–9: callers do step 1), `link_check` (`0x0063B210`),
`link_into_item` (socket link); `beltable` (§3.3), `auto_belt_gate`
(§3.6), `belt_free_slot` (§3.5), `belt_place` (§3.7), `belt_compact`
(§3.8); `body_item` (`0x0063DD90`), `place_body` (`0x0063BDB0`),
`clear_body_slot` (`0x0063BE30`), `item_to_remove` (`0x0063E490`),
`two_handed` (`0x006289C0`), `requirements` (§4.2), `equip_check` (§4.3),
`stack_test` (§4.5), `equip_from_cursor` (§4.6, returns (result, out)),
`auto_equip` (§4.7); `check_cursor_item`, `check_stored`,
`check_stored_or_equipped`, `check_owned`, `check_belt`,
`check_ground_or_owned` (§5.1), `busy`, `trading` (§5.2),
`targeting_reset` (§5.3), `item_move_gate` (§5.4).

`MoveUnits` (provider: units / item records through the wiring; no
defaults): `unit_exists`, `unit_class`, `pos` / `set_pos`, `unit_flags` /
`set_unit_flags` (+0xC4), `update_bits` / `set_update_bits` (+0xC8),
`stat` / `set_stat`, `expansion`, `frame`; item data `mode`, `page`,
`stored_page`, `body_loc`, `cmd_flags`, `item_flags` (each with a
setter), `set_expiry` (+0x24), `item_owner` (+0x5C), `is_type`
(itemtypes equivalence), `code`, `quality`, `file_index`, `quest`,
`useable`, `component`, `max_stack`, `socket_filled` (`0x0055F590`),
`socket_filler` (`0x0062BEB0`), `sockets`, `fillers`, `spell`.

`MovePending` (defaults = narrowest reading, logged here as Pending):

| Group (owner) | Methods (default) |
|---|---|
| path / placement (`sim/path-placement.md`, being written) | `distance` (i32::MAX → 0x16 result 1), `collides` (false), `walk_to_item` (no-op), `room_at` (false), `free_spot` (none → no drop, no pile), `in_town` (false → 0x18 page 4 refused) |
| rooms (`units.md`, `unit-order.md`) | `room_delete_notice`, `free_collision`, `remove_from_room`, `add_to_room`, `room_change_notice`, `queue_update` (no-ops), `in_room` (false → no ITEMDROPPED) |
| stat lists (`stat-lists.md`, OQ6) | `stat_refresh`, `stat_refresh_unlink`, `stat_link`, `charm_relink`, `charm_unlink`, `inventory_pass`, `weapon_in_use_update`, `weapon_bookkeeping`, `body_leave_effects` (`0x0062A360` + `0x0063D2B0`), `hireling_owner_pass` (no-ops), `is_active` (false) |
| belt with potions (OQ13) | `belt_unequip` (no-op), `belt_remove_allowed` (false → 0x1C / 0x1D on location 8 do nothing) |
| sounds | `sound`, `pickup_sound`, `requirement_sound`, `merc_sound` (no-ops) |
| quests (`world/quests.md`) | `quest_flag` (clear), `quest_item_picked`, `quest_item_dropped` (no-ops), `carry_one` (false; mask `0x006CE270` unwritten), `held_test_units` (none; OQ19) |
| item creation / ownership (`generation.md`) | `create_gold` (none → no pile), `free_item`, `give_cursor_item`, `set_owner`, `party_share`, `owned_gold_pickup`, `rest_pile`, `book_count_changed` (no-ops), `copy_item` (none), `consume_one` (false), `pile_owner` (none), `query_0044be50` (false), `party_share_id` (−1), `merge_allowed` (`0x00629930`, false → no merge) |
| item use (unwritten; OQ14) | `use_grid_item`, `use_item_action`, `swap_1h_with_2h` ((false, false) → 0), `use_item` (false), `charge_update`, `remove_used` (no-ops), `pickup_special` (false), `equip_picked` (`0x00562E00`, false) |
| sockets (`properties.md` §9–§10) | `filler_linked` (no-op), `runeword` (false) |
| hirelings (unwritten) | `hireling` (none), `not_dead` (false), `alive` (false), `owns_hireling` (false) → 0x61 does nothing; `equip_on_merc`, `merc_after_take` (no-ops) |
| NPC / object picks | `pick_npc`, `pick_object`, `pick_other` (0), `resync` (no-op) |
| transport | `send`, `send_item_stat` (0x3E, layout unwritten) (no-ops), `item_bits` (empty; OQ1), `store_messages` (none), `filler_owner` ((6, −1)) |

## 5. Readings and questions (M1, M2, M5 and the seamed OQs carry `TODO(spec: …)` at their sites; the others are readings recorded here)

- M1 §6.2: a row whose flags (and condition) match but whose `to`
  excludes the client is read as ending the walk (nothing sent, later
  rows skipped); a failed condition is read as "row does not match".
  Settle: R1/R3 with two clients, or Ghidra `0x005973F0`.
- M2 §11: the dispatcher's flag argument to the bit stream (0 used); the
  owner fields and flag argument of a filler's 0x9D action 0x13
  (`filler_owner` seam).
- M3 §6.2 row 18–19: 0x7D `state` = item flags & the row's flag.
- M4 §9.2: "quest items → 0 (never)" read as the stored expiry 0 (not
  frame + 0).
- M5 unwritten failure results, each read as "nothing" (result 0, out
  0): §7.7 `0x0063E490` without an item; §7.8 E not in mode 1, failed
  placement of N, E's unlink; §7.10 target not in mode 0, the "link"
  step; §7.16 the link; §7.17 no hireling; §7.19 the mode / filler /
  socket checks; §8.1 r5 a failed `0x00562E00`; §8.1 r7 the link; §9.3
  the unlink; §10.2 a failed pile creation (stops); §7.23 a failed copy.
- M6 "of type T": every type test uses itemtypes equivalence
  (`MoveUnits::is_type`); the spec states it only for 0x26 / 0x61
  potions.
- M7 §7.12: max stack read from dst; both quantities announced dst
  first.
- M8 §7.16: both items join the update list (needed for "both send 0x9C
  action 0x10"; the spec lists no update-list step for B).
- M9 §8.2: the refused pickup ends the routine (no pickup sound).
- M10 §10.1: the rest pile's sound is part of the `rest_pile` seam.
- M11 0x26 is 13 bytes (`client-messages.tsv` `partial`); bytes 9–12
  are not read.
- Open in the spec and seamed: OQ1, 6, 9, 10, 11, 12, 13, 14, 15, 16, 17,
  19.

## 6. Local checks

None new: no game file is read here. R1–R6 (spec Test vectors) remain
the conformance checks once recorded; replaying them needs the wiring
of all three seams.

## 7. Gate (all pass on this branch)

`cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --
-D warnings`; `cargo test -p d2-sim` (1291 pass, 5 ignored); `cargo run
-p depcheck` (determinism lint clean); `python3 tools/spec_index.py
--check`; `python3 tools/methods.py check`; `python3 tools/coverage.py
--check` (0 errors) and `--selftest` (ok).

Update: merged `origin/claude/tender-meitner-mphas3` at `93b37c8` (includes
the regenerated `d2-proto` tables, `e909c15`) into this branch; `sh
tools/gate.sh --no-client` → GATE: PASS on the merge.
