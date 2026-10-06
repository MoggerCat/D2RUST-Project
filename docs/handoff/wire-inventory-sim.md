# Handoff: inventory wiring (`d2_sim::wiring::inventory`)

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here); the coordinator folds it (§1 row 3j/3k, §3 code map, §7 questions).

Cloud implementation session, 2026-10-06, task class: integration from
clear specs, medium (METHODS M14). Branch `claude/wire-inventory-sim`,
from `claude/tender-meitner-mphas3` at `edd9925`. Repo only, synthetic
tables, fixed game seed `0x1A7E`, no game files (M09): every claim below
holds on this branch.

## 1. State

**Wired, unverified** (M02): `items::inventory` (§1–§5) and
`items::moves` (§6–§11) are draft-spec implementations; the adapter adds
no rule of its own. No recording R1–R6 exists.

- New module `crates/d2-sim/src/wiring/inventory/` plus the line `pub
  mod inventory;` (and its doc bullet) in `wiring/mod.rs`. **No change to
  `items::inventory`, `items::moves`, any other module, `d2-server`,
  specs or dependencies** (Signature changes: none).
- `InvDesk` (borrows `wiring::economy::Economy`, the `InvTables`, an
  `InvState` and a rest `R: InvRest`) implements all four seams the move
  code needs: `items::inventory::InvWorld`, `items::moves::InventoryOps`,
  `MoveUnits`, `MovePending`, so `items::moves::handle(desk, …)` and
  `items::moves::player_update(desk, …)` run on real state.
- Tests: 26 integration tests in `wiring/inventory/tests/` (every handler
  through `items::moves::handle` with real message bytes, then the
  per-client update pass); the only fake is `Rest` (the `InvRest` seams,
  call log). Counts in §8.
- M08: `ground::pickup_position_follows_the_grid_record` (barbarian
  record 4 cut from 10 × 4 to 9 × 4: the same pickup lands at (8, 3)
  instead of (9, 3), every other field and message equal). By hand, each
  of 7 adapter routes broken in turn failed its tests: room list removal
  (`pick_to_cursor…`), write-back of the copied fields (5 tests), the
  deferred owner refresh (`lift_and_insert…`), the other-hand branch of
  `0x0063E490` (`two_handed_swap…`), "has durability"
  (`stack_merge…`), gold creation (`drop_gold_makes_a_real_pile`), item
  positions (3 tests).

## 2. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `wiring/inventory/mod.rs` | `InvState` (inventories per owner unit, the `InvItem` per item unit, ground expiry, errors), `InvRest` (seams without a provider), `InvDesk` (`new` fills the item copies; `with_inv` lends an inventory out for a mutating call, writes the copies back, runs the owner refreshes asked for; `update_done` = §6.1 rule 4), `InvError` | `inventory.md` §1.1, §6.1 |
| `wiring/inventory/inv_world.rs` | `InvWorld` for `InvDesk` | §1–§5 seams |
| `wiring/inventory/ops.rs` | `InventoryOps` for `InvDesk` (each op = the `items::inventory` function of its rule) | §1–§5 |
| `wiring/inventory/units.rs` | `MoveUnits` for `InvDesk` | §1.1, `units.md` §2, `generation.md` §1.3 |
| `wiring/inventory/pending.rs` | `MovePending` for `InvDesk`: 9 calls on providers, the rest forwarded to `InvRest` | §6–§10 |
| `wiring/inventory/tests/` | `mod.rs` (synthetic stat / item / inventory tables, `World` with a barbarian in one room, `Rest`, message builders), `ground.rs` (0x16, 0x17, M08), `buffer.rs` (0x18, 0x19), `equip.rs` (0x1A–0x1D), `belt.rs` (§8.1 step 6, 0x23–0x26), `stack.rs` (0x21, 0x22), `gold.rs` (0x50, §10.1) | |

## 3. Ownership (one owner per field)

| Field | Owner | `InvItem` copy |
|---|---|---|
| mode (unit +0x10), unit flags (+0xC4), update bits (+0xC8), class, GUID, act | `UnitRecord` | mode, guid, record |
| item flags (+0x18), page (+0x45), quality, file index | `ItemStore` (`Item<()>`) | flags, page |
| stats (quantity 70, gold 14, durability 72, sockets 194, …) | `StatLists` | — |
| command flags, body location, stored page, grid x / y (= the item's position), item owner GUID, owning inventory, node grid / kind | `InvState::items` (`InvItem`) | — |
| ground expiry (+0x24) | `InvState::expiry` | — |
| room membership | `UnitLists` | — |

`InvDesk::new` fills the copies from all item units of the lists;
`MoveUnits` setters write through; every mutating inventory call writes
the copies back. A desk is built per call (as `wiring::interaction::Desk`).

## 4. Provider table (seam → provider)

`InventoryOps` (all wired): every method → the `items::inventory`
function of its rule on the owner's `Inventory` (`place_at_page`,
`find_free_position` on a copy, `place_in_page_from_cursor` (§2.4 steps
2–9), `beltable`, `auto_belt_gate`, `free_belt_slot`,
`place_in_belt_slot`, `compact_belt`, `place_at_body`,
`requirements_met`, `equip_check`, `stack_test`, `equip_from_cursor`,
`auto_equip_location`, the six §5 checks, `busy`, `trading`,
`targeting_reset`, `item_move_gate`; list / cursor / update list on
`Inventory`). Plus: `link_check` → `InvWorld::link_check` below;
`clear_body_slot` (`0x0063BE30`) → nothing to do after the §1.4 unlink
(logged `BodySlotHeld` if the slot still holds an item);
`item_to_remove` (`0x0063E490`) → the item at the location, else the
two-handed item of the other hand (§7.7 text); `two_handed` →
`InvRest`; `link_into_item` → `InvRest`.

`MoveUnits` (all wired except three): `unit_exists` / `unit_class` /
`unit_flags` / `update_bits` → unit lists + `UnitRecord`; `stat` /
`set_stat` → `StatLists` unit total / unit set (layer 0); `expansion` →
`GameFields`; `frame` → `Game::frame`; mode / page / stored page / body
loc / command flags / item flags / expiry / item owner → §3 owners;
`pos` / `set_pos` of an **item** → its item data x, y (§2.2, §9.1 step
3); `is_type` / `code` / `quest` / `useable` / `component` →
`InvTables`; `quality` / `file_index` → `ItemStore`; `max_stack` →
`maxstack` + stat 254, ≤ 511 (`generation.md` §1.3); `sockets` → stat
194 (`cube.md` §4.1); `fillers` → the item's own inventory, link order.
**To `InvRest`:** `pos` / `set_pos` of non-item units (path),
`socket_filled` (`0x0055F590`), `socket_filler` (`0x0062BEB0`), `spell`
(`0x00627F80`).

`MovePending`, wired: `remove_from_room` → `UnitLists::room_remove`
(`0x0064C370`); `add_to_room` → `room_insert` (`0x0064C2C0`; an item
already in the spot's room, e.g. added by the allocator, is left; another
room is logged `OtherRoom`); `in_room`; `queue_update` →
`UnitLists::queue_update` (`0x0064C040`); `alive` → `!UnitRecord::is_dead`
(`0x005541B0`); `merge_allowed` → "has durability" `0x00629930`
(`generation.md` §1.3: `nodurability` 0, `durability` ≠ 0, stat list,
stat 152 < 1); `free_item` → `Economy::free_item` (`0x00555600`, item
data dropped; `FreedWhileLinked` logged for a linked item); `create_gold`
→ `Economy::create_item` with the request of `InvRest::gold_request` (code
`gld` looked up in the tables).

`InvWorld`, wired: `expansion`, `item` / `item_mut` (copies),
`item_by_guid`, `item_stat`, `unlink_from` (the other inventory, lent),
`remove_from_room` (notice + collision → `InvRest`, list →
`room_remove`), `clear_targetable` (+0xC4 &= ~2), `link_check` (owner not
an item → true, §2.4 step 5; item owner → `InvRest::socket_link`),
`owner_refresh` (deferred to `items::moves::owner_refresh` after the
call), `unit_kind`, `unit_stat`, `req_percent` (stat 91 unit total),
`fits_free_page0` (§2.3 on page 0, on a copy), `quality`,
`stack_file_index` (file index, `cube.md` §4.1), `stack_value` (ethereal
bit, `0x0062A8D0` per `cube.md` §4.1), `has_sockets` (stat 194 ≠ 0),
`queue_untarget` (S→C 0x3F via `layouts::use_stackable(0xFF, guid,
0xFFFF)` → `send`), `same_act` (unit +0x18), `within_range` (Chebyshev on
the player's position (rest) and the item's (item data)). Everything else
→ `InvRest` / `MovePending` unchanged.

## 5. Pending (no provider in d2-sim; `InvRest` / `MovePending` defaults)

| Group | Seams | Owner / why |
|---|---|---|
| path / placement | `distance`, `collides`, `walk_to_item`, `room_at`, `free_spot`, `in_town`, `pos` / `set_pos` of players and monsters | `sim/path-placement.md` (`d2_sim::path`, implemented in parallel); a later wiring connects positions and the free-spot search |
| rooms | `room_delete_notice` (`0x0061A270`), `free_collision` (`0x00623830`), `room_change_notice` (`0x0063BCF0`) | no spec states their body |
| stat lists | `stat_refresh`, `stat_refresh_unlink`, `stat_link`, `charm_relink`, `charm_unlink`, `is_active`, `inventory_pass`, `weapon_in_use_update`, `weapon_bookkeeping`, `body_leave_effects`, `hireling_owner_pass`, `item_active_on`, `own_contribution` | `stat-lists.md` does not write these routines (`inventory.md` OQ6) |
| requirements / hands / stack / auto-equip | `percent_of` (OQ4), `level_requirement` (OQ5), `two_handed`, `one_or_two_handed`, `ammo_type`, `stack_quality_ok` (OQ7), `has_allowed_location`, `quiver_kind`, `auto_equip_allows` (OQ8) | no rule in `generation.md` §1.3 or `inventory.md` |
| player data / interaction | `interaction`, `clear_interaction`, `player_data_4c`, `player_data_50`, `npc_talking`, `player_trade_gate`, `quest_flag`, `held_test_units` | player data is not in d2-sim; `wiring::interaction::InteractionState` holds NPC lists, but the player's interaction field has no d2-sim home |
| belt / sounds / quests hooks | `belt_unequip`, `belt_remove_allowed` (OQ13), `sound`, `pickup_sound`, `requirement_sound`, `merc_sound`, `quest_item_picked`, `quest_item_dropped`, `carry_one` | sound and quest hook transport; `0x006CE270` |
| item creation / use | `gold_request` (`0x00559CE0` layout, economy WE9), `copy_item` (`0x0055A2A0`), `give_cursor_item`, `consume_one`, `set_owner` (`0x00621CE0`), `pile_owner`, `query_0044be50`, `targeting_probe`, `party_share*`, `owned_gold_pickup`, `rest_pile`, `book_count_changed`, `use_grid_item`, `use_item`, `charge_update`, `remove_used`, `use_item_action`, `swap_1h_with_2h`, `pickup_special`, `equip_picked` (`0x00562E00`) | item-use spec unwritten (OQ14) |
| sockets | `socket_link`, `link_into_item`, `socket_filled`, `socket_filler`, `spell`, `filler_linked`, `runeword` | `properties.md` §9–§10 not wired to the inventory |
| hirelings | `hireling`, `not_dead`, `owns_hireling`, `equip_on_merc`, `merc_after_take` | mercenary spec unwritten |
| picks / transport | `pick_npc`, `pick_object`, `pick_other`, `resync`, `send`, `send_item_stat` (0x3E), `item_bits` (OQ1), `store_messages`, `filler_owner`, `trade_hook` | `world/npc.md`, waypoints, OQ10–OQ11, item serialization unwritten |

Consequences on this branch: auto pickup of an item with a body location
(§8.1 step 5) stops at `equip_picked` (default false: the item left the
room but stays in mode 3); 0x26 calls `use_item` and `remove_used` but
the item stays in the belt; no gold pile is made without
`gold_request`; ground items without a `free_spot` never drop.

## 6. Questions (for the spec owner; each pinned by a test)

- **WV1** (`inventory.md` §6.3 vs `units.md` §2 / §3.1 step 5): the
  allocator sets unit flag 0x10 ("seed set") on **every** unit, and §6.3
  sends a ground message only for items *without* unit flag 0x10, so on
  real units `items::moves::ground_update` never builds 0x9C action 2 /
  3. Either §6.3 reads another field or bit, or the units table's 0x10
  is wrong. Pinned by `ground::assert_ground_update` (with the bit
  cleared by hand the spec's 0x9C action 2 comes out). Settle: Ghidra
  `0x0055BF30` / `0x0055BED0` (which field the test reads), or R1.
- **WV2** (`inventory.md` §7.6): X "becomes the cursor item", then N goes
  to the location "as §4.6 step 5", whose text includes "cursor :=
  none"; read literally (as `items::moves::swap_2handed` does), X ends in
  mode 4, unlinked and not the cursor item. An original quirk (like
  §7.24's) or the order misread? Pinned by
  `equip::two_handed_swap_and_removal_from_the_other_hand`. Settle: R3
  (two-handed weapon over sword + shield) or Ghidra `0x00563D20`.
- **WV3** (§7.7): "empty location → 0" comes before §4.3, so §4.3
  result 4 (other hand two-handed) and `0x0063E490`'s other-hand branch
  are unreachable from 0x1C; a client taking a two-handed weapon off by
  the left hand does nothing. Pinned by the same test. Confirm with R3.
- Readings (no rule; each has a doc comment / `TODO(spec: …)` at its
  site): `0x00557FD0` free = the unit removal `0x00555600`
  (`pending.rs`); `0x0063CB00` = §2.3 on page 0 (`inv_world.rs`);
  `0x0063BE30` = nothing after the §1.4 unlink (`ops.rs`); "room added"
  of `0x00558AA0` = the room list insert (`pending.rs`); §6.1 rule 4 =
  `InvDesk::update_done` (command flags := 0, update list freed; OQ9);
  a unit without an inventory is checked against an empty one (§5).

## 7. What the server handler session needs

- Build per message: `InvDesk::new(&mut econ, &inv_tables, &mut
  inv_state, &mut rest)`, then `items::moves::handle(&mut desk,
  player_guid, msg)` (`None` = not an item id; `Some(Ok(code))` = the
  handler result for `intents-events.md` §2.3; `Some(Err(MoveFatal))` =
  the original's fatal assert). The `None`-owner rows of `ITEM_IDS`
  (`d2_server::adapters::handlers::items`) except 0x4C (`cube.md` §10:
  item-use spec) are exactly `items::moves::HANDLED` (23 ids; checked by
  reading both lists on this branch); their owner becomes
  `specs/items/inventory.md` §7.
- Per client in the player unit update (`unit-order.md` §6, `tick.md`
  §6): `items::moves::player_update(&mut desk, client, player)` returns
  the 0x9C / 0x9D / 0x7D and 0x47 / 0x48 bytes in order; then
  `desk.update_done(owner)` in the room clean-up (§6.1 rule 4). Ground
  items: `items::moves::ground_update` from the item unit update (WV1
  first). Messages queued "now" (0x3F, 0x42, 0x63's direct sends) leave
  through `MovePending::send`.
- State to own per game: `InvState` (add an inventory with
  `InvState::add_inventory(unit, UnitKind, guid)` when a player /
  hireling / NPC is created; `0x0063ABD0`), `InvTables::from_fixed` once
  per table set, the economy parts (already in `TradeWorld`'s desk), and
  an `InvRest` value (the path wiring will supply `distance`,
  `free_spot`, `room_at`, `pos`).
- `InvState::errors` collects provider errors (list, economy) and the
  logged readings; the host should surface them.

## 8. Gate

`sh tools/gate.sh all` on this branch: **GATE: PASS** (every step:
spec_index, methods, coverage 3,542 claims 0 errors + selftest, trace
checkers, hook selftest, fmt, depcheck, clippy workspace, tests, doc
tests). nextest: d2-sim + conformance 1,445 passed, 99 skipped; rest (no
client) 553 passed, 68 skipped; d2-client 304 passed, 15 skipped. The
26 new tests are in the d2-sim count.
