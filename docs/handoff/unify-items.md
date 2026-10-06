# Handoff: one item store and one inventory per game — `claude/unify-items`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here); the coordinator folds it (§1 state, §3 code map, §7 questions).

Cloud implementation session, 2026-10-06, task class: wiring
architecture, high (METHODS M14). Base: `claude/tender-meitner-mphas3`
at `bbb06c1` (the merged host with the item-move handlers), with
`origin/claude/tender-meitner-mphas3` at `ad23c70` merged in at the end.
Repo only, synthetic tables, fixed seeds, no game files (M09): every
claim below holds on this branch. Closes `host-merge.md` W-5 and
`wire-inventory-server.md` §5 F1 / F2.

## 1. State

**Wired, unverified** (M02): no rule is added; the same `d2-sim` modules
run, now on one item store and one inventory model per game.

- **One item store (W-5, F1).** The store's home is the action wiring:
  `ActionHooks::items` (`wiring::economy::ItemStore`), lent to an
  economy for a call and written back. `DeathDrops::items` is gone:
  `economy::death::monster_death_drop` creates the kill's items in that
  store (`h.items` lent for the drop), and `WiredWorld` no longer holds
  a store of its own (`with_economy` lends the hooks'). So the kill's
  gold / item can be picked up (C→S 0x16), sold (0x33) and cubed (0x2A):
  the item-move code, the vendors and the cube read the item data the
  drop wrote.
- **One inventory per unit (F2).** The inventory model
  (`WiredWorld::inventory`: `InvParts` = inventory tables, `InvState`,
  the item-move rest) is the only inventory on the host. The cube's
  `Staged::inventories` (list + cursor) and `Inventory` are deleted; the
  vendor rest's `owns_item` / `in_inventory` / `equipped_items` /
  `has_cursor_item` / `place_in_backpack` / `remove_stored` are answered
  from the model by the new `handlers::items::InvVendors` wrapper, before
  they reach a rest. An item placed by 0x18 is sellable at a vendor and
  usable in the cube, and an item bought or transmuted lands where the
  moves see it.
- **The model's host API** (new `d2_sim::wiring::inventory::host`): reads
  on `InvState` (`of`, `items_of`, `cursor_of`, `holds`, `body_items`,
  `fillers`) and the rules on `InvDesk` (`place` = §2.4, `remove` = the
  §1.4 unlink, `free` = `0x00557FD0`, `reset_targeting` = §5.3,
  `check_stored` / `check_ground_or_owned` = §5.1, `send_item_page` = the
  §6.4 direct 0x9D), all keyed by `UnitId`. `InvParts::desk(econ)` builds
  the desk; the vendor and cube seams use it.
- **The cube** (`handlers::items::cube_world::ServerCube`) now answers
  `inventory`, `socketed`, `place`, `remove_cube_item`,
  `targeting_reset`, `put_item_check` and `cube_check` from the model
  (its economy and the inventory parts sit in `RefCell`s: the `&self`
  checks need a desk, which borrows the economy mutably). `UnitFacts` are
  no longer read there: §5.1's act comes from the unit record and the
  distance from the player's position (the rest) and the item's item
  data. `ItemPending` loses `place`, `remove_cube_item` and `socketed`;
  `Staged` loses `inventories` and `targeting_resets` (the reset runs the
  real rule, which queues 0x3F through `MovePending::send`).
- **Tests.** d2-sim: 5 new integration tests
  (`wiring/inventory/tests/host.rs`) on the model's host API (reads after
  the moves, §2.4 placement with and without "send" and a ground item
  refused, removal + free with `FreedWhileLinked`, the §5.1 checks and
  the §5.3 reset, the direct 0x9D's exact bytes). d2-server: the cube
  tests (`handlers/items/tests.rs`) ported to the model (the cube and the
  ring stored through §2.4, the grid cells asserted, the transmute's 0x9D
  action 5 exact); `prop_handle.rs`'s item and trade hosts carry the
  model and its vendor calls `unreachable!` in the rests. 163 d2-server
  tests pass (unchanged count).
- **e2e** (`crates/d2-client/tests/e2e_single_player.rs`, 3 tests pass):
  the single-player loop now runs end to end on one store and one
  inventory, 36 frames (was 32). Step 5b: the kill's gold is **picked
  up** (0x16 cursor 0 → §8.1 → §10.1: stat 14 += the pile's gold, the
  pile freed; result 0 where it was "item missing" 1). Then the cap on
  the ground: 0x16 → 0x18 → 0x19 → 0x1A → 0x1C → 0x17 → 0x16 → 0x18
  (every frame's 0x9C / 0x9D / 0x47 / 0x48 bytes exact), **sold at
  Akara** (0x33: 0x2A kind 3, gold credited, the cap freed, its grid
  cells cleared), a **buy** (0x32, stops at the item copy `0x0055A2A0`:
  0x2A code 9), the stored buckler's sale (same stop), the stored cap
  sold; then the **cube**: a ground ring picked (0x16), put in (0x2A →
  §2.4 placement, 0x9C action 4 in the pass) and **transmuted** (0x4F:
  the ring's 0x9D action 5 now, the amulet placed on page 3, identified,
  its 0x9C action 4 in the pass). `same_seed_same_run` and
  `other_seed_other_run` pass; the transcript adds the player's final
  inventory. `e2e_vendor.rs` (4 tests) runs its buckler and cap through
  the model too.

## 2. Code map rows (for `docs/HANDOFF.md` §3)

| Path | What | Spec |
|---|---|---|
| `crates/d2-sim/src/wiring/inventory/host.rs` | the model for the other item systems: `InvState::{of, items_of, cursor_of, holds, body_items, fillers}`; `InvDesk::{place, remove, free, reset_targeting, check_stored, check_ground_or_owned, send_item_page}` | `inventory.md` §1.4, §2.4, §5.1, §5.3, §6.4 |
| `crates/d2-sim/src/wiring/inventory/tests/host.rs` | 5 integration tests of that API | `inventory.md` §2.4, §5.1, §5.3, §6.4 |
| `crates/d2-server/src/adapters/handlers/items/vendor_inv.rs` | `InvVendors`: the vendor world with its player-inventory calls on the model | `vendors.md` §7, §8; `inventory.md` §1.2, §1.4, §2.4 |
| `crates/d2-sim/src/wiring/action/mod.rs` | `ActionHooks::items`: the game's one item store | `generation.md` §2–§3 |

## 3. Signature changes

`d2-sim` (public):

- `wiring::action::ActionHooks`: field `items: wiring::economy::ItemStore`
  added (empty from `new`; lent out during an economy call).
- `wiring::economy::DeathDrops`: field `items` **removed** (the drop
  creates into `ActionHooks::items`). Behaviour change: a dropped item's
  item data is now in the game's one store.
- `wiring::inventory`: new module `host` (additions only, listed in §2);
  `InvState::of` / `items_of` / `cursor_of` / `holds` / `body_items` /
  `fillers`, `InvDesk::place` / `remove` / `free` / `reset_targeting` /
  `check_stored` / `check_ground_or_owned` / `send_item_page`.

`d2-server` (public):

- `handlers::items`: `Inventory` **removed**; `Staged` loses
  `inventories` and `targeting_resets`; `ItemPending` loses `place`,
  `remove_cube_item` and `socketed`; `ItemError::Move(MoveFatal)` added;
  `CubeCall::call` takes `inv: Option<&mut InvParts>` (fourth argument,
  before `interact`); re-exports `InvParts` and `InvVendors`.
- `handlers::items::moves`: `InvParts::desk(&mut Economy)` added (the
  module-private `desk` helper is gone); `take_sent` is `pub(crate)`;
  `mod tests` is `pub(crate)` (the cube tests reuse `MRest`).
- `handlers::world::WiredWorld`: field `items: ItemStore` **removed**
  (the store is the action wiring's); private field `inv_sent` added.
  `Parts` gained `inventory: Option<&'p mut InvParts>`.
- `handlers::world::wired`: `WiredWorld::vendors` now wraps its world in
  `InvVendors`; `take_sent` appends what the inventory rules queued in
  vendor calls after the rest's messages.

`d2-client` test files only: `tests/e2e_support/mod.rs` gained
`inv_tables`, `inv_parts`, `store` (shared fixtures) and its `Rest`
lost `inventory` (the vendors' inventory calls are `unreachable!` there);
`e2e_single_player.rs`, `e2e_vendor.rs`, `prop_worldsim.rs` use them.

## 4. Readings and findings

- **R1** (`vendor_inv.rs` `owns_item`, `TODO(spec)`): `0x00557FF0`'s body
  is not written. Read as the owned-item test of `inventory.md` §5.1
  (`0x00549220`: in the player's item list or its cursor item) without
  the lookup the caller already did; `in_inventory` (§8.1 rule 4) reads
  the same.
- **R2** (`vendor_inv.rs` `remove_stored`, `TODO(spec)`): §7.2 rule 9's
  "item cell := page, item update message" names no routine or layout, so
  no message is queued; the removal is the §1.4 unlink plus the free
  (`0x0055DF10` → `0x00557FD0`, as `cube.md` §8 step 1 reads it).
- **R3** (`wired.rs` `take_sent`, `TODO(spec)`): the order of a vendor
  call's inventory messages (a targeting reset's 0x3F, a placement's) and
  its own 0x2A is not written; the inventory ones follow.
- **F1**: the vendors' remaining inventory calls have no rule to wire:
  `take_from_cursor` (`0x0055EEA0`), `unequip` (`0x00560CD0` by item, not
  by location: §7.7 is by location), `can_belt` / `put_in_belt`
  (`0x0055E9B0` with the vendors' arguments), `equip_ammo` (OQ 3),
  `find_tome`, `add_to_tome`, `find_partial_stack`, `lower_book_skill`.
  They stay the rest's; the e2e paths do not reach them.
- **F2**: the vendor's store and gamble inventories (`place_in_store`,
  `add_trade_inventory`, `new_store_inventory`, …) are still the rest's.
  `InvState` can hold an NPC inventory (`add_inventory` takes any unit
  kind), so a later task can route them to the model as well — the
  store's own grid record (`inventory.md` §1.3, `vendors.md` §3.1 step 3
  store page) is the missing piece.
- **F3**: the cube still has no owner for the inventory pass
  (`0x0055FA40`, `cube.md` OQ 8) and the item routines no items spec
  writes (`duplicate`, `tempered_affix`, repair, recharge, quest hooks):
  `ItemPending` keeps exactly those.
- **F4** (e2e): the kill's gold needs the pile's item record in the
  game's one item table, so the e2e's drop tables are now the game's
  (`gld ` beside the vendor and cube items). The gold's own inventory
  size (1 × 1) is synthetic; `inventory.bin` has no gold row to measure.

## 5. Local checks to queue

None new runnable now. The queued replays gain one step: when the
item-move recordings R1–R6 (`inventory.md` Test vectors) exist, replay a
pick-up of a **dropped** item (the W-5 path) through `Host` +
`WiredWorld` and compare the result code and the S→C bytes; the vendor
replay (`e2e-vendor-host.md` §8) now runs with the inventory model, so
its §7.2 rule 9 removal and §7.1 rule 9.7 placement are compared too.

## 6. Gate

`sh tools/gate.sh all` on this branch: **GATE: PASS** (spec_index,
methods, coverage + selftest, trace checkers, hook selftest, fmt,
depcheck + determinism, clippy workspace, tests d2-sim + conformance,
rest, d2-client, doc-tests). d2-server 163 tests pass; d2-sim's inventory
wiring 31; `e2e_single_player` 3 and `e2e_vendor` 4.
