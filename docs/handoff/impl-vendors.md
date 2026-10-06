# Handoff: vendors (`d2_sim::world::vendors`) — `claude/impl-vendors`

> Folded into `docs/HANDOFF.md` (§1–§5, §7) and `docs/PLAN.md` as of this commit; this file stays as the detailed record.

Cloud implementation session, 2026-10-06, task class: implementation from
a clear spec, medium (METHODS M14). Branched from
`claude/bold-ptolemy-jvyvxy` at `a5b323a`. Spec: `specs/world/vendors.md`
(+ `vendors.tsv`). Repo only. For the coordinator to fold into
`docs/HANDOFF.md` / `docs/PLAN.md` (not edited here).

## 1. State

**Implemented, unverified** (the spec is a draft; its recorded facts are
reproduced only as synthetic vectors here, no game-file or trace check has
run on this code).

- New submodule `crates/d2-sim/src/world/vendors.rs` +
  `vendors/{price,store,gamble,trade}.rs` + `vendors/tests/`. The only
  other change is the line `pub mod vendors;` in `world/mod.rs`. No
  dependency, spec, `items`, `stats`, `units`, `wiring` or `npc` change.
- Tests: 60 new (`cargo test -p d2-sim`: 572 pass, 3 ignored): every
  synthetic test vector of the spec, the three recorded prices, the
  recorded Charsi store order (43 codes, item level 6, draw count), the
  recorded 0x32 / 0x33 bytes, every edge case, and the `vendors.tsv` check
  with a perturbation test (M05, M08).
- Coverage (`tools/coverage.py`): vendors.md 105 / 108 units claimed
  (unit tier). Unclaimed: `§9.2 l2 r5` (the empty rule "—"),
  `§edge-cases-original-bugs text` (the "Reproduced by default" line) and
  `§9.2 r0` (see §6, tool issue).
- Gate (all pass): `cargo fmt --all -- --check`, `cargo clippy -p d2-sim
  --all-targets -- -D warnings`, `cargo test -p d2-sim`, `cargo run -p
  depcheck`, `python3 tools/spec_index.py --check`, `python3
  tools/methods.py check`, `python3 tools/coverage.py --check`.

## 2. Code map rows

| Path | What | Spec |
|---|---|---|
| `world/vendors.rs` | constants (columns, caps, codes, classes, stats, flags), per-NPC switches (`column_of`, `global_column_of`, `GAMBLERS`, `FLAGGED`, `HIRE_CLASSES`, `REPAIRERS`), `store_level`; `VendorTables::from_fixed` (items with raw vendor columns, itemtypes, equivalence, magic / unique / set / skill / stat cost columns, books, monstats levels and `interact` classes, `npc.txt`, `difficultylevels` gamble odds, low-quality names, gamble index); `Column`, `GlobalLists`, `VendorRecord` (+ `GambleList`, `ChainNode`, `EventNode`); seams `NpcLink`, `VendorWorld`; `Transaction` (0x2A) | §1, §2, Constants |
| `world/vendors/price.rs` | `cost` (`0x0062EFB0`), (A) item skills, (B) bonus stats, (C) `charged_skills`, `gamble_price` (§9.4), predicates `repairable`, `durability_applicable`, `max_stack`, `charges_not_full`; `PriceItem`, `PriceCtx`, `PriceFatal` | §9.2–§9.4 |
| `world/vendors/store.rs` | `quality_draw`, `range`, `generate` (`0x00576980`), `make_store_item` (`0x00576330`, upgrade, tries, page, park), `mark`, `open` (`0x00579430`), `clear_record`, `refresh_act`, `level_changed`, `client_left` | §2–§4, §6 |
| `world/vendors/gamble.rs` | `level_draw`, `quality_draw`, `make_list` (`0x00578790` + upgrade `0x005786A0`), `drop_list`, `identify_gamble` (0x37) | §5 |
| `world/vendors/trade.rs` | `price_ctx`, `pay`, `receive`, `repair_item` (`0x005761C0`), `is_permanent`, `buy` (0x32), `sell` (0x33), `repair` (0x35); message parsers `BuyMsg`, `SellMsg`, `RepairMsg` | §7, §8, §9.1 |
| `world/vendors/tests/` | `mod.rs`: synthetic tables (the vectors' costs, levels, multipliers) and the fake world; `price`, `store`, `gamble`, `trade`, `tsv` | Test vectors, edge cases |

## 3. Design points

1. **Record ownership.** The vendor fields of an NPC record (`npc.md`
   §1.1 +0x04 store items, +0x08 gamble lists, +0x0C, +0x14, +0x18, +0x1C,
   +0x20, +0x24/25, +0x27, +0x28, +0x2C–0x38, +0x40) are
   `VendorRecord`; `world::npc` should embed one per record, built with
   `VendorRecord::new(class, act, trader, &GlobalLists)` at game creation
   (`GlobalLists::build` once per server start). `class`, `act`, `trader`
   are copies of the NPC table fields. +0x21 (hire list made) stays with
   the NPC module (`NpcLink`).
2. **NPC-control seed** stays with the NPC module; every draw function
   takes it (`StoreCtx { tables, seed }`). Item seeds come from the game
   seed inside `VendorWorld::create_item` (§3.2).
3. **Messages** are parsed here (`BuyMsg::parse` etc.); results are the
   handler return values; S→C 0x2A leaves through
   `VendorWorld::send_transaction` (gold read after the transaction).
4. **Wall clock** (`GetTickCount`) is an input (`now`, ms) to `generate`,
   `open`, `refresh_act`, `level_changed`, `client_left` (edge case 10).
5. **Fatal paths** (no `npc.txt` row, missing `books` row) return
   `PriceFatal` instead of exiting (Open question 5 is a Ruleset
   decision; the caller decides).
6. `VendorItem` is a projection of its own (cost, gamble cost, vendor
   columns, upgrades, …) because `items::tables::ItemRec` lacks those
   columns and the items module is not this session's to edit. The
   vendor columns are read from the raw record bytes at 326 + 17·f + i
   (§1 rule 1).

## 4. Seams (trait methods → expected provider)

`NpcLink` (narrow, `world::npc`, parallel session): `npc_by_guid`,
`npc_class`, `is_interact_unit` (`npc.md` §2 start step 4),
`interaction_empty` (§6 rule 3), `hire_list_made` /
`set_hire_list_made` / `make_hire_list` (`npc.md` §7.1). The NPC module
calls `store::open` for menu actions 1 / 2 (`npc.md` §4),
`gamble::drop_list` when 0x30 empties the interaction list (`npc.md` §3).

`VendorWorld: NpcLink`:
- game fields: `difficulty`, `expansion`, `item_format`, `game_type` →
  game creation.
- units / stats: `guid`, `player_by_guid`, `item_by_guid`, `stat`,
  `base_stat`, `set_stat` → units / `StatLists`; `quest_slot` →
  `world::quests::PlayerQuests`; `players_in_level`, `player_level_id` →
  units / rooms (`0x005538D0`); `town_entered` → `quests.md` §6.7;
  `gold_cap`, `stash_cap`, `drop_gold` → player spec (not written);
  `last_bought` / `set_last_bought` (player data +0x6C); `has_cursor_item`.
- items: `create_item` (`0x00559CE0`, request owner NPC, mode 4) →
  `wiring::economy::Economy::create_item`; `copy_item` (`0x0055A2A0`),
  `recharge` (`0x0055FE80`), `repair_broken` (`0x0055F900`) → not
  specified (economy handoff §5 lists the same three); item fields
  (`item_record`, quality, file index, flags, unit +0xC8, mode, page,
  `has_filled_sockets`); `price_item` (the `PriceItem` snapshot,
  including the stat 107 / 204 entries and the (B) bonuses of
  `0x00625560`, Open question 1); `identify` (`0x00562590`);
  `send_item_stat` (0x3E), `send_transaction` (0x2A) → transport.
- NPC inventories: `new_store_inventory`, `place_in_store`
  (`0x00560200`), `remove_store_item` (`0x00536510`), `take_from_store`
  (`0x005766D0`), `place_in_gamble`, `remove_gamble_item`,
  `refresh_npc_inventory` (`0x00621000`), `add_trade_inventory`
  (`0x0063CC70` / `0x00576C30`, 0x9C action 11) → inventory spec (not
  written).
- player inventories: `owns_item`, `in_inventory`, `equipped_items`
  (13 body locations), `find_tome`, `add_to_tome`, `find_partial_stack`,
  `can_belt`, `put_in_belt`, `equip_ammo` (OQ3), `place_in_backpack`,
  `take_from_cursor`, `lower_book_skill` (`0x00576E40`, S→C 0x22),
  `remove_stored`, `unequip` → inventory / player specs (not written).

## 5. Open questions and narrowest readings (TODO in code)

Each has a `TODO(specs/world/vendors.md …)` at the site unless noted.

- V1 §7.2 rule 7: the mask at `0x006CE270` (unique flag byte +0x2C) is
  not written: `VendorTables::unique_nosell_mask` = 0 (no unique
  refused).
- V2 §9.2 rule 4: an empty affix slot (id 0, also auto affix 0) adds no
  delta.
- V3 §9.2 (A) / (B): a layer without a `skills` row is skipped / reads
  (0, 0).
- V4 §9.2 (B) encode 4: "(min, max) of the by-time value" read as the
  two packed fields `stats::by_time` decodes (low, high).
- V5 §9.2 rule 6: "cost/2" read as the socketed item's record `cost` / 2.
- V6 §9.4: an item without a normal-code record prices 0. The uber /
  ultra validity test is the literal "≠ 0 and ≠ `0   `" (the store
  upgrade uses "≠ 4 spaces").
- V7 §3.1 rule 1: an upgrade code missing from the code map keeps the
  base code; §3.1 rule 2: a null creation counts as a failed try.
- V8 §3 step 1 / permanent list: a list code missing from the code map
  is skipped without a draw and without a failure count.
- V9 §5.1: no gamble index → no list; an index past the list ends it;
  `rin` / `amu` missing keep the drawn id; in an expansion game a missing
  record is passed to creation (the stop rule is written for classic
  only).
- V10 §7.1 rule 2 and every 0x2A whose GUID the spec does not write
  (codes 9, 11, 12 and the repair codes): GUID −1.
- V11 §7.2 rule 8: "quantity := max stack" is applied to every restored
  copy (a non-stack gets `maxstack` 0 + stat 254).
- V12 §8.1: handler results are written only for rule 4's first refusal
  (3); other paths return 0.
- V13 (no TODO, reading): "flag 1" / "flag 2" of §3.1 rule 5, §4 rule 3
  and §7.1 rule 9.8 are the item-flag values 0x1 / 0x2; "store item" of
  §7.1 rule 12 is an item of `VendorRecord::store`.
- V14 (no TODO): the order of the NPC inventory for §4 rule 3 is the
  creation order kept in `VendorRecord::store` (the grid order belongs
  to the inventory spec).
- Spec Open questions 1–7 untouched (OQ5 surfaces as `PriceFatal`).

## 6. Tool issue found (for a tools session)

`tools/coverage.py --rules specs/world/vendors.md` lists `§9.2 r0` (the
list under §9.2 starts at `0.`), but a claim `§9.2 r0` is rejected as
malformed ("want … r<N>"). Either the claim grammar should accept `r0`
or the rule reader should number from the list's first item; until
then rule 0 of §9.2 cannot be claimed (it is tested by `price::rule_zero`).

## 7. Checks to queue (local run queue, `docs/HANDOFF.md` §5)

1. **Column lists on live data** (the spec's game-file test; needs a
   home with MPQ access: `data-tool` or `crates/conformance`, since
   `d2-sim` has no `d2-formats` dependency): `GlobalLists::build(
   &VendorTables::from_fixed(&fixed))`; Charsi's column 2 holds exactly
   the rows with `Charsi*` values (`aqv`, `cqv` permanent; `axe` Min 1
   Max 1 MagicMin 1 MagicMax 1 MagicLvl 1); `npc` rows equal §9.3;
   `difficultylevels` odds 10000 / 100 / 50 / 90 / 33.
2. **Store generation on live data + recording**: with the live tables
   and the recorded NPC-control seed of
   `traces/raw/20261006-015956-packets.jsonl`, `generate` for Charsi
   (frame 899) and Akara (frame 1779) must reproduce the recorded 0x9C
   action 11 code lists in order (once `create_item` has the economy
   provider).
3. **Prices on recordings**: the three recorded transactions (buy 56,
   sell 500, buy 40) through `buy` / `sell` with real providers; then
   the spec's OQ6 / OQ7 recordings (magic item price, gamble open and
   purchase) decide V2–V5.
