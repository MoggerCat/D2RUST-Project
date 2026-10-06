# Handoff: inventory model (`d2_sim::items::inventory`) — `claude/impl-inventory`

> Not yet folded into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited here); the coordinator folds it.

Cloud implementation session, 2026-10-06, task class: implementation from
a clear spec, medium (METHODS M14). Branched from
`claude/tender-meitner-mphas3` at `9b49081`, then merged with it at
`93b37c8` (includes the d2-proto regeneration `e909c15`). Spec: `specs/items/inventory.md`
§1–§5 (draft). Repo only (M09): every claim below holds on this branch.
The parallel session `impl-moves` owns §6–§11 (`items::moves`).

## 1. State

**Implemented, unverified** (the spec is a draft; T1–T9, B1–B5 and E1–E4
are synthetic; D1–D3 are queued, §6).

- New module `crates/d2-sim/src/items/inventory/` (`mod.rs`, `tables.rs`,
  `grid.rs`, `belt.rs`, `equip.rs`, `checks.rs`, `tests.rs`). The only
  other change: the line `pub mod inventory;` in `items/mod.rs`. No
  dependency, spec, `moves`, `wiring` or `d2-server` change.
- Tests: 33 new (32 run, 1 `#[ignore]` game-file test):
  `cargo test -p d2-sim` after the base merge: 1269 pass, 6 ignored;
  `cargo test -p d2-proto` passes.
- Coverage: `specs/items/inventory.md` 55 / 148 units claimed, unit tier;
  every §1–§5 unit except `§1.1` (the field table), `§1.4 r3` (cursor
  set: see Q1), `§2.4 text`, `§2.4 r5` (failed link check: Q2), `§4.3
  text`, `§4.6 r3` (not observable apart from step 2 with the seams'
  semantics), plus edge cases 2 and 5. §6–§11 are `impl-moves`'.
- Randomness: none (spec "Randomness": no draw in grid, belt, equip,
  requirement code). No `Seed` is taken.
- Gate (all pass): `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test -p d2-sim`, `cargo run -p
  depcheck` (determinism lint clean), `python3 tools/spec_index.py
  --check`, `python3 tools/methods.py check`, `python3 tools/coverage.py
  --check`, `python3 tools/coverage.py --selftest`.

## 2. Code map rows

| Path | What | Spec |
|---|---|---|
| `items/inventory/mod.rs` | constants (`SIGNATURE`, `NO_GUID`, `mode`, `page`, `grid_id`, `BODY_GRID`/`BELT_GRID`, `body`, `node`, `cmd` (item-actions.tsv flags), `iflag`, `ty`, `stat`); `UnitKind`; `InvItem` (item data fields); `Grid`; `Inventory` (item list, grids, weapon GUID, cursor, count, update list; `link`, `unlink`, `grid_or_create`, `body_item`, `belt_item`, `push_update`, `take_updates`); `InteractionTarget`; seam `InvWorld` | §1.1, §1.2, §1.4 |
| `items/inventory/tables.rs` | `InvTables::from_fixed` (inventory gridX/gridY, belts numboxes, items columns, itemtypes columns, equivalence); `is_type`, `size`, `numboxes` | Constants |
| `items/inventory/grid.rs` | `grid_record`, `page_grid_size`, `fits`, `in_bounds`, `weight`, `search`, `place_in_grid`, `place_at_page`, `place_at_body`, `find_free_position`, `place_in_page` (steps 1–9), `place_in_page_from_cursor` (steps 2–9) | §1.3, §2 |
| `items/inventory/belt.rs` | `belt_type`, `belt_numboxes`, `beltable`, `similar`, `free_belt_slot`, `auto_belt_gate`, `place_in_belt_slot`, `compact_belt` | §3 |
| `items/inventory/equip.rs` | `body_location_allowed`, `requirements_met`, `equip_check` (+ `res` codes, `other_hand`), `hands_compatible`, `stack_test`, `equip_put` (§4.6 step 5 with a flag argument, for 0x1B), `equip_from_cursor` → `EquipOutcome {ok, out}`, `auto_equip_location` | §4 |
| `items/inventory/checks.rs` | `cursor_item_check`, `stored_item_check`, `stored_or_equipped_check`, `owned_item_check`, `belt_item_check`, `ground_or_owned_check`, `busy`, `trading`, `targeting_reset`, `item_move_gate` | §5 |
| `items/inventory/tests.rs` | fake `InvWorld`, synthetic tables; T1–T9, B1–B5, E1–E4, rule tests; `real_grid_belt_and_type_tables` (D1–D3, ignored) | Test vectors |

## 3. Public API (for the `impl-moves` wiring)

Model: items are units by `UnitId`; the inventory keeps the item list,
grids (cells hold `UnitId`), cursor, weapon GUID, count and the update
list (GUIDs). The item data fields of §1.1 (`mode`, `cmd_flags`,
`flags`, `body_loc`, `page`, `stored_page`, `x`, `y`, `owner_guid`,
owning inventory `inv`, `node_grid`, `node_kind`, plus `guid` and
`record`) are an `InvItem` the wiring keeps per item unit and hands out
through `InvWorld::item` / `item_mut`. The inventory's owner is
`Inventory::owner` (`UnitId`) with its `UnitKind` and GUID.

Signatures (all generic over `W: InvWorld + ?Sized`; `t: &InvTables`):

| Spec | Function |
|---|---|
| §1.2 | `Inventory::grid_or_create(&mut self, g, w, h) -> Option<&mut Grid>`; `grid(g)`; `item_at(g, x, y)`; `body_item(loc)`; `belt_item(slot)` |
| §1.3 | `grid_record(UnitKind, page, expansion) -> Option<usize>`; `page_grid_size(t, UnitKind, page, expansion) -> Option<(u8, u8)>` |
| §1.4 | `Inventory::link(&mut self, w, item, grid: Option<usize>)`; `unlink(&mut self, w, item) -> bool` (false = not linked: the callers' fatal); `cursor()`, `set_cursor(Option<UnitId>)`; `push_update(guid)`, `is_listed(guid)`, `update_list()`, `take_updates()`; `items()`, `contains(item)` |
| §2.1 | `grid::fits(&Grid, x, y, w, h)`, `grid::in_bounds(...)` |
| §2.2 | `place_in_grid(inv, w, item, g, x, y, (iw, ih), (gw, gh)) -> bool`; `place_at_page(inv, w, t, item, page, x, y) -> bool`; `place_at_body(inv, w, item, loc) -> bool` |
| §2.3 | `search(&Grid, w, h, player) -> Option<(i32, i32)>`; `weight(&Grid, x, y, w, h) -> u8`; `find_free_position(inv, w, t, item, page) -> Option<(i32, i32)>` |
| §2.4 | `place_in_page(inv, w, t, item: Option<UnitId>, x, y, find_free, send) -> bool`; `place_in_page_from_cursor(...)` (steps 2–9, for §9 "§2.4 steps 2–9") |
| §3 | `belt_type(inv, w, t) -> usize`; `belt_numboxes(inv, w, t) -> Option<u8>`; `beltable(t, record)`; `similar(t, a, b)`; `free_belt_slot(inv, w, t, item) -> Option<u8>`; `auto_belt_gate(inv, w, t, item) -> bool`; `place_in_belt_slot(inv, w, t, item, slot) -> bool`; `compact_belt(inv, w, t, slot) -> Vec<(u8, u8)>` |
| §4.1–4.5 | `body_location_allowed(t, record, loc)`; `requirements_met(w, t, item: Option<UnitId>, unit, equipping)`; `equip_check(inv, w, t, unit, loc, item: Option<UnitId>, skip) -> u8` (`equip::res::*`); `hands_compatible(w, t, unit, a, b)`; `stack_test(w, t, a, b)` |
| §4.6 | `equip_from_cursor(inv, w, t, item: Option<UnitId>, loc, skip) -> EquipOutcome`; `equip::equip_put(inv, w, item, loc, cmd_flag) -> bool` |
| §4.7 | `auto_equip_location(inv, w, t, item, skip) -> Option<u8>` |
| §5 | `*_check(inv, w, guid) -> u8` (six checks); `busy(inv, w)`; `trading(inv, w)`; `targeting_reset(inv, w)`; `item_move_gate(inv, w) -> bool` |

`inv` is always the player's (owner's) inventory; the cursor, targeting
reset and update list are the owner's. Placement into an inventory other
than the owner's (the optional inventory of `0x00560200`) is not
modelled: no §1–§5 caller needs it; the socket case goes through
`InvWorld::link_check`.

No existing public signature changed.

## 4. Seams (`InvWorld`)

Every routine the spec names without stating its behavior is a seam
method (doc comment names the address / open question): unit and item
lookups (`item`, `item_mut`, `item_by_guid`, `item_stat`, `unit_kind`,
`unit_stat`), `unlink_from` (an item linked in another inventory),
`remove_from_room`, `clear_targetable`, `link_check` (`0x0063B210`),
`charm_relink` (OQ6), `active_item` (`0x0062FF70`), `stat_refresh`,
`socket_filled` (`0x0055F590`), `owner_refresh` (`0x00621000`),
`inventory_pass` (OQ6), `trade_hook`, `weapon_in_use_update`,
`stat_link`, `weapon_bookkeeping`, requirements (`req_percent`
`0x00625500`, `percent_of` OQ4, `item_active_on` `0x00625820`,
`own_contribution` `0x0062B450`, `level_requirement` OQ5), hands
(`two_handed` `0x006289C0`, `one_or_two_handed` `0x0062A1E0`,
`ammo_type` `0x0062E6F0`, `quiver_type` with a table default per
`generation.md` §1.3, `fits_free_page0` `0x0063CB00`), stack test
(`quality`, `stack_file_index`, `stack_value`, `stack_quality_ok`,
`has_sockets`; OQ7), auto-equip (`has_allowed_location` `0x0062FDF0`,
`quiver_kind` `0x00628480`, `auto_equip_allows` OQ8), targeting
(`targeting_probe` `0x0044BE50`, `queue_untarget` 0x3F), players
(`interaction`, `clear_interaction`, `player_data_4c`, `player_data_50`,
`npc_talking`, `player_trade_gate` (out of scope), `same_act`,
`within_range` `0x00548EF0`), `expansion`.

## 5. Questions (`TODO(spec: …)` seams in the code)

1. `0x0063C180` (set cursor): does it link the item into the item list?
   Only the field is written (`Inventory::set_cursor`). §1.4 rule 1 implies
   the cursor item is linked but not counted.
2. §2.4 step 5: the result when the link check fails (code returns 0).
3. §2.4 step 3: where the page-2 trade hook runs ("afterwards"; code: right
   after a successful placement).
4. §3.5: a similar column with no empty slot below `numboxes`: try the next
   column (code) or go straight to the autobelt rule? B1 does not decide it.
5. §3.8: compaction sets *item* flags 0x400 and 0x1 (as written; 0x400 is
   also the PutInBelt *command* flag of item-actions.tsv row 11: confirm
   which field); items that do not move are not flagged (code).
6. §4.7 step 1 "type ≠ 38": primary type (code) or the equivalence test?
   Step 2 "an equipped hand weapon": which hand (code: right, then left).
7. §1.3: grid record of an item-owned (socket) inventory and of other
   owner types (code: none).
8. §4.4 step 6 for an object owner (code: no).

## 6. Local checks to queue (`docs/HANDOFF.md` §5)

- **D1–D3** (group C, game files): `D2_GAME_DIR=<game> cargo test -p
  d2-sim --lib real_grid_belt_and_type_tables -- --ignored`. Expect pass:
  `inventory.bin` 32 records with the §1.3 sizes for 0–15, `belts.bin`
  numboxes `12, 8, 4, 16, 8, 12, 16, 12, 8, 4, 16, 8, 12, 16`, and the D3
  itemtypes codes (space-padded, `bow `, `axe `, `h2h `). Expected values
  are the spec's measurements; this test has not run yet, so it carries
  no coverage claim (`COVERAGE.md` §3 lesson).
- R1–R6 (recordings, group A) belong to the spec; the inventory parts
  (grid positions in R2, belt slots / compaction in R4) need the moves
  wiring first.

## 7. Gate

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p d2-sim
cargo run -p depcheck
python3 tools/spec_index.py --check
python3 tools/methods.py check
python3 tools/coverage.py --check
python3 tools/coverage.py --selftest
```
