# Mutation testing: inventory wiring and item-move handlers — `claude/mutants-wiring-inventory`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here); the coordinator folds it.

Cloud test session, 2026-10-06, METHODS M08. The base was
`claude/tender-meitner-mphas3` at `99b1e72`. The mutation runs used that
tree. The branch then merged the base at `756ce77`, which includes
unify-items. Repo only, synthetic tables, no game files (M09).

Only test code changed. Tests are in new files only:

- `crates/d2-sim/tests/mutants_wiring_inventory.rs` (31 tests)
- `crates/d2-sim/tests/mutants_wiring_inventory/fixture.rs` (the wiring
  fixture rebuilt on the public API; `Rest` logs every seam call with its
  arguments and takes its answers from `Answers`)
- `crates/d2-server/tests/mutants_moves.rs` (1 test)

`mutants.out` is not committed. The session ended early on the
coordinator's wrap-up order, so §5 lists what is still open.

## 1. Counts (`cargo-mutants` 27.1.0, pre-merge tree `99b1e72`)

These commands were used:

- **Sim:** `cargo mutants -p d2-sim --file 'crates/d2-sim/src/wiring/inventory/**' --timeout 60 -j 3 -- --lib`
- **Sim after:** the same command plus `--iterate`, with
  `-- --lib --test mutants_wiring_inventory`.
- **Server:** `cargo mutants -p d2-server --file 'crates/d2-server/src/adapters/handlers/items/moves.rs' --timeout 120 -j 3`

| Run | Mutants | Caught | Missed | Unviable |
|---|---|---|---|---|
| sim `wiring/inventory/**`, before | 474 | 200 | **249** | 25 |
| sim, after | 474 | 446 | **3** | 25 |
| server `items/moves.rs`, before | 25 | 16 | **4** | 5 |
| server, after | not run (see §5) | | expected **2** | |

Before, the sim survivors split by file like this: `inv_world.rs` 86,
`ops.rs` 33, `pending.rs` 92, `units.rs` 36 and `mod.rs` 15. All but 3
were killed in the sim after-run.

## 2. (a) Killed

**Seam forwarding, no claim.** About 150 mutants were in adapters that
only hand a call to `InvRest` / `MovePending` and return its answer. No
spec decides these adapters; the provider table in
`wire-inventory-sim.md` §4 does. Five tests kill them:

- `forward_pending_answers_{true,false}` and
  `forward_inv_world_answers_{true,false}` call every forwarder. Each
  boolean is checked both ways, and the tests assert the exact
  arguments in the log and the unchanged answer.
- `forward_inventory_ops` does the same for the `InventoryOps`
  forwarders.

**`InvRest` default bodies, no claim.** `inv_rest_defaults` checks the
defaults the trait doc names ("fails / none / no / 0 / 1"). The
server's `MoveRest` does not override them, so a real host runs on them.

**Spec outcomes, with `// Covers:` claims.** These tests go through
`items::moves::handle`, the `InventoryOps` / `MoveUnits` /
`MovePending` calls or the inventory model, and each asserts the
outcome of the rule it claims:

| Test | Kills | Claims |
|---|---|---|
| `place_removes_a_ground_item_from_its_room` | `InvWorld::remove_from_room` | `inventory.md` §2.2 |
| `place_unlinks_from_the_old_inventory` | `unlink_from` | §2.2, §1.4 r1 |
| `insert_clears_only_the_targetable_flag` | `clear_targetable` (`()`, `\|=`, `!`) | §2.4 r6 |
| `link_check_sockets_only_into_an_item` | `link_check` (both impls, the Item arm) | §2.4 r5 |
| `stash_size_follows_the_game_type` | `InvWorld::expansion` | §1.3 |
| `stack_test_compares_quality_file_index_ethereal_and_damage`, `stack_test_refuses_socketed_items` | `item_stat`, `quality`, `stack_file_index`, `stack_value`, `has_sockets`, `stack_test` | §4.5 |
| `requirement_percent_lowers_the_strength_needed` | `req_percent`, `percent_of`, `requirements` | §4.2 r2, r3 |
| `hand_result_7_needs_room_for_the_other_hand` | `fits_free_page0` | §4.3 r4 |
| `ground_check_measures_from_the_item_position`, `ground_check_other_act_is_2` | `item_pos`, `within_range` (all), `same_act` (all), `check_ground_or_owned` | §5.1 |
| `targeting_reset_clears_and_queues_0x3f` | `targeting_probe`, `queue_untarget`, `targeting_reset` | §5.3 |
| `inventory_getters`, `socket_getters` | `has_inventory`, `items`, `weapon_in_use`, `sockets`, `fillers` | §1.4 r1 |
| `beltable_reads_the_item_type` | `beltable` | §3 r3 |
| `auto_equip_takes_the_single_location` | `auto_equip` | §4.7 r3 |
| `stored_or_equipped_and_belt_checks` | `check_stored_or_equipped`, `check_belt` | §5.1 |
| `trading_is_an_interaction_with_a_player` | `trading` | §5.2 |
| `item_move_gate_refuses_with_player_data_4c` | `item_move_gate` | §5.4 |
| `queue_update_queues_the_unit_in_its_room` | `queue_update` | `unit-order.md` §6 r2 |
| `merge_allowed_needs_stat_152_below_1` | `merge_allowed` `<` → `<=` | `generation.md` §1.3 |
| `alive_is_not_dead` | `alive` (all) | `units.md` §2 |
| `move_units_fields` | the `MoveUnits` getters and setters (class, +0xC8, stored page, body location, item owner, code, quality, file index, quest, component, useable, expansion) | §1.1 |

Two more tests check the wiring's own readings and carry no claim:

- `clear_body_slot_logs_a_held_slot` (`BodySlotHeld`)
- `add_to_room_by_room` (the room guards, `in_room`, and `note_list`
  logging a list error)

**Server.** `move_ids_are_exactly_the_handled_ids` checks `is_move_id`
over all 256 ids against `HANDLED` and `MOVE_IDS`. It kills both
`is_move_id` mutants. It carries no claim, because it checks the id
table.

## 3. (b) Survivors that are equivalent or unobservable (5)

- **`inv_world.rs:188` `!=` → `==` in `stack_value`.** This mutant is
  equivalent. The value only feeds the §4.5 equality between A and B,
  and the mutant inverts both sides.
- **`inv_world.rs:181` delete `-` in `stack_file_index`.** This changes
  the `map_or` default for an item that has no item-store entry. That is
  unreachable, because `sync_in` only creates item data for units that
  have both a record and a store entry.
- **Server `moves.rs:251` `&` → `|` / `^` in the update-pass clean-up
  guard (`bits & 1 == 0`).** Both are equivalent on reachable states:
  - §6.1 r1 sets bit 0 and (for players) bit 1 together, and the pass
    clears them together.
  - The update list only grows together with the owner refresh.
  - So a player whose bit 0 is clear has bit 1 clear and an empty list,
    and running the clean-up on that player changes nothing.

  Which bits the clean-up clears is reading R1 in
  `wire-inventory-server.md`; the spec does not write it.

## 4. (c) Spec gap, no test (1)

- **`ops.rs:132` `auto_belt_gate` → `true`.** §3 r6 says the gate
  "returns true for every item". The adapter returns false only when the
  owner has no inventory or the GUID has no item, and the spec does not
  cover either case. The mutant may match 1.14d as well as the code does.
  Settle with Ghidra `0x00628BA0` called with a null item.

No mutant showed the code disagreeing with its spec, so there is no code
fix to describe.

## 5. Merge (`756ce77`, unify-items) and what is left

**Ported.** unify-items added inherent `InvDesk::check_stored` and
`check_ground_or_owned` methods (in `host.rs`, taking `UnitId`). They
shadow the trait calls, so the tests now call
`InventoryOps::check_*(d, owner, guid)`. After the port, all 31 sim
tests and the server test pass on the merged tree.

**Not done** (the coordinator stopped the session):

1. **Mutate `crates/d2-sim/src/wiring/inventory/host.rs`.** unify-items
   added it and it is in scope; it has not been mutated:
   `cargo mutants -p d2-sim --file crates/d2-sim/src/wiring/inventory/host.rs --timeout 60 -j 3 -- --lib --test mutants_wiring_inventory`.
   unify-items' own `wiring/inventory/tests/host.rs` exists.
2. **Re-run the server after-run on the merged tree.** On that tree,
   `moves.rs` gained `InvParts::desk` and has 45 mutants:
   `cargo mutants -p d2-server --file crates/d2-server/src/adapters/handlers/items/moves.rs --timeout 120 -j 3`.
   The expected outcome is that only the two §3 equivalents survive.
   This is not verified.
3. **Run `sh tools/gate.sh all`.** It was not run. Run: `cargo fmt
   --check` (clean), `cargo clippy -D warnings` on both new test targets
   (clean), the new tests (pass), and `python3 tools/coverage.py --check`
   (4172 claims, 0 errors).
