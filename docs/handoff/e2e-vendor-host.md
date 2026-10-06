# Handoff: vendor world host on the server — `claude/e2e-vendor-host`

Cloud implementation session, 2026-10-06, task class: integration from
clear specs, medium (METHODS M14). Base: `claude/bold-ptolemy-jvyvxy` at
`35484d4` (= main `b966c6b` + `wire-open-seams`, PR #18). Repo only,
synthetic tables, fixed seeds, no game files (M09). For the coordinator
to fold into `docs/HANDOFF.md` / `docs/PLAN.md` (neither is edited
here). Closes `e2e-single-player.md` §2 row 5c up to the stubs listed in
§3.

## 1. State

**Wired, unverified** (M02): every spec on the path is a draft; this
layer adds no rule, it routes the existing `d2-sim` modules onto one
unit world.

- New server world host `d2_server::adapters::handlers::world::TradeWorld<R>`
  (`crates/d2-server/src/adapters/handlers/world/trade.rs`). It
  implements `WorldHost<D>` for any `D: ActionEvents` (`ActionSim` or
  `WorldSim`) with `D::X: Outbox`:
  - `npc` → `NpcControl` handlers on `wiring::interaction::Desk`
    (`NpcWorld + NpcVendors`): C→S 0x13, 0x2F, 0x30, 0x34, 0x36, 0x38,
    0x62 now run on real units / stats / timers / quests;
  - `vendors` → the trade functions on `wiring::interaction::VendorDesk`
    (`VendorWorld + NpcLink`, with the NPC control block): 0x32, 0x33,
    0x35, 0x37; the vendor records are lent out of `InteractionState`
    for the call;
  - `waypoints` → delegated to its embedded `ActionWorld` (0x49 as
    before); faults go to `ActionWorld::faults`;
  - `quests` → not provided (0x31, 0x40, 0x58 stay stubs, as on
    `ActionWorld`).
- **One unit world.** The `Economy` is built per call from the action
  sim's own `sys.units`, `sys.stats`, `sys.data`, `sys.hooks`
  (`ActionHooks<X>` as `LifecycleHooks`) plus the host's `GameFields`,
  `ItemTables`, `ItemStore`. The game seed's one owner stays
  `ActionHooks::game_seed`; it is lent to `GameFields::seed` for the
  call and written back (`TradeWorld::with_economy`, public, also used
  by the test to create the player's items).
- `TradeRest` = `NpcRest + VendorRest + QuestRest + PlayerQuestsRef +
  Outbox` (blanket impl): the unwritten seams plus the outbox of the
  rests' messages (`QuestRest::send`; `VendorRest::send_transaction` as
  `d2_sim::world::npc::transaction` bytes). `take_sent` returns the
  action outbox, then the rest's.
- `SimGame::with_world(game, events, world)` added (`with_events` now
  calls it with `W::default()`), for a host without `Default`.
- New test `crates/d2-client/tests/e2e_vendor.rs` (4 tests, all pass):
  bridge → `LocalLink` → `Host` → `SimGame<ActionSim<_>, TradeWorld<_>>`.

## 2. Vendor steps end to end

| # | C→S | Result | S→C (asserted bytes) | Stops at |
|---|---|---|---|---|
| 1 | 0x13 type 1, Akara | 0 | 0x27 (40: `27 01` GUID + 34 bytes of the `encode_text_list` stub), 0x29 (97: `29` + the game quest record), 0x28 (103: `28 01` GUID `00` + the player's record) in that order (`npc.md` §2 step 5, `quests.md` §1.5) | — (0x27 bytes 6–39 are the stub's: `server-messages.tsv` 0x27 `partial`) |
| 2 | 0x2F | 0 | none | — (heal hook, nothing to heal) |
| 3 | 0x38 action 1 | 0 | none | — store generated (`vendors.md` §3, §4): buckler entry (Min 1, Max 3) drawn on the NPC-control seed, then the permanent cap; items made by `Economy::create_item`, game seed stepped exactly 2 per item; identified, unit +0xC8 |= 4, page 0. 0x9C action 11 is the item spec's (`add_trade_inventory` stub) |
| 4 | 0x32 (gold = price − 1) | 0 | `2a 00 0c 00000000 ffffffff` gold (§7.1 rule 4) | — |
| 5 | 0x32 (gold 5000) | 1 | `2a 00 09 00000000 ffffffff` gold | **`VendorRest::copy_item`** (`0x0055A2A0`, no items spec): its null runs §7.1 rule 9.2's refusal; nothing paid |
| 6 | 0x33 player's buckler (stored) | 3 → `Malformed` | `2a 00 09 …` GUID −1 | **`VendorRest::copy_item`** (§7.2 rule 8, buckler is not a permanent code) |
| 7 | 0x33 player's cap (stored) | 0 | `2a 03 01 00000000` item GUID, gold + price (§7.2 rule 10) | runs: a cap is Akara's permanent code, so rule 8 makes no copy; rule 9's removal is the `remove_stored` stub (inventory spec); price received on real stats (§9.1) |

Prices are checked by hand from §9.2 (not by calling `cost`): buy
S = 100·AC/5 (armor rule 2, sell mult 1024); sell B = (100·AC/5)·512/1024.
AC read from the item's base stat 31.

Also: `same_seed_same_run` (frames, seeds, store rows incl. item seeds,
gold, stub log, identical twice); `one_gold_more_changes_only_the_gold_fields`
(M08: +1 gold → exactly byte 11 of the 0x2A in frames 4–6 differs,
nothing else); `other_seed_other_store`. No world fault, interaction
error or unit error in the run; no id fell back to `SimGame::unhandled`.

Fixture answers (staged, not behaviour): distance 3, the interact unit
record, quest records, the player's inventory set, NPC grid always has
room, carried-gold / stash caps 100 000, item format 1, `copy_item`
null. Five idle frames before the repeated buy: the client's duplicate
filter drops identical bytes within 200 ms (`bridge.md` §4 rule 5).

## 3. Stubs reached and what they need

| Stub | Needed for | Owner |
|---|---|---|
| `VendorRest::copy_item` (`0x0055A2A0`) | buy rule 9 (every purchase), sell rule 8 (re-sellable non-permanent items) | no items spec writes it (economy WE / vendors notes list the same) |
| `place_in_store`, `add_trade_inventory`, `refresh_npc_inventory`, `new_store_inventory` | store grid, S→C 0x9C action 11 | inventory / item-message spec |
| `owns_item`, `remove_stored`, `place_in_backpack`, `can_belt`, cursor | sell rules 3 / 9, buy rule 9.6–9.7 | inventory spec |
| `gold_cap`, `stash_cap`, `drop_gold` | §9.1 receive / pay | player spec |
| `encode_text_list` | 0x27 bytes 6–39 | `0x00661480`, 0x27 `partial` |
| `distance`, `axis_check`, `unit_check`, `clear_path` | 0x13 / 0x2F / 0x38 gates | unit / path spec |
| quest callbacks `unhandled` (chains 1–6, 37 on Akara's talk) | quest text on talk | Act I quest callbacks (`quests.md`) |

## 4. Findings

1. **Two owners of the player's interact unit.** The action wiring's
   `Pending::{set_interact, reset_interact, interact_guid}` (waypoints)
   and `NpcRest::{interact_unit, set_interact, reset_interact}` (NPCs)
   hold the same player field (+0x64/+0x68). On `TradeWorld` they are
   separate; a single player-data provider should own it.
2. **Duplicate game fields.** `GameFields::{difficulty, game_type,
   ladder}` (economy) and `ActionHooks::ai_info` (AI) both hold game
   +0x6D / +0x6A / +0x74. Only the seed is unified here.
3. `NpcRest::item_format` holds game +0x78, a game field, not a rest
   call.
4. Store generation output order: permanent codes last (as §3); the
   test asserts it.

## 5. Signature changes

None to an existing seam. Additions only: `SimGame::with_world`;
`handlers::world::{TradeWorld, TradeRest, Parts}`. No `d2-sim` file
touched; `wiring::action` and `e2e_single_player.rs` untouched.

## 6. Next

- Make the e2e server type (`e2e_single_player.rs`, other session) use
  `TradeWorld` instead of `ActionWorld` when it adds an NPC: the host is
  a drop-in (`ActionWorld` is its `action` field).
- `QuestCall` on `TradeWorld` via `EconomyQuests` (same desk) would
  unstub 0x31 / 0x40 / 0x58.
- The copy `0x0055A2A0` (items spec) unblocks buy and the store copy of
  a sale; then 0x2A kind 4 / the store copy run.

## 7. Gate

`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --
-D warnings`, `cargo test --workspace`, `cargo run -p depcheck`,
`python3 tools/spec_index.py --check`, `python3 tools/methods.py check`,
`python3 tools/coverage.py --check`: results in the commit message.

## 8. Checks to queue

When the trade recording replay (`wire-interaction.md` §8 item 1) runs,
run it through this host (`Host` + `TradeWorld`) and compare S→C bytes
per frame (0x2A bytes 3–6 masked).
