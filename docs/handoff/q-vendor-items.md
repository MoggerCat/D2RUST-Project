# q-vendor-items: vendor store items in the shop panel

> Stitching session, 2026-10-08. Read only `specs/`, `docs/`, `crates/`,
> `tools/`. Nothing here is verified against 1.14d (rule 10). No audio.

The row asked for store lists "generated per `world/vendors*.md` on game
start and refresh". The generation itself was already wired (the trade
open `0x00579430`, `vendors.md` §3–§4, runs on the NPC-control seed in
`WiredWorld`; `e2e_vendor.rs` pins it). The spec generates a store when a
trade opens and refreshes it by level change and timer (§6), not at game
start, so that is what runs. What was missing was everything between the
generated store and the screen.

## Links connected

| Link | Before | Now | Where |
|---|---|---|---|
| Store items to the client (S→C 0x9C action 11, `vendors.md` §4 step 3) | `add_trade_inventory` was a log-only stub; no message | the NPC call collects the items a trade open added (`InteractionState::shown`, deduped, in add order) and sends one 0x9C action 11 per item to the opening player through the inventory model's item stream | `d2-sim` `wiring/interaction/{mod,npc_vendors,vendor_world}.rs`, `wiring/inventory/host.rs` (`send_item_world`), `d2-server` `handlers/world/wired.rs` (`flush_shown`) |
| Client model keeps the store apart from the player's items | a 0x9C record with owner = local player landed in the inventory | `ItemView::store` (action 0x0B), `store_items`, `local_items` skips them, action 0x0C removes | `bridge/items.rs`, `bridge/msg/stats_items.rs`, `bridge/world.rs` (`store_serial`, `ItemRecord::seq`) |
| Shop panel (ui 0x0C) | `ShopPanel` rules existed, nothing installed or drew them | `ShopUi`: art, tabs (with their start page), store grid items, action buttons, hover price; opens when store items arrive (nearest trader) or by `OriginalUi::open_shop`; closes with the UI state and sends C→S 0x30 | `ui/shop_ui.rs` (new), `ui/original.rs` (4 lines), `world_view/ui_bind.rs` (one call) |
| Buy (C→S 0x32) | no sender | right click (spec quick buy) or left click on a store item → `ShopTx::click` → 0x32; the server pays, copies the item, answers S→C 0x2A and the item messages | `ui/shop_ui.rs` |
| Sell (C→S 0x33) | no sender | a cursor item clicked on the store grid → 0x33 (`menus.md` §4.5) | `ui/shop_ui.rs` |
| Repair (C→S 0x35) | no sender | the repair-all button (frame 18) → 0x35 with item 0 / 0x80000000 | `ui/shop_ui.rs` |
| Buy price | none | the host computes `cost(BUY)` for each shown item and publishes it (`VendorRest::store_price` → `ShopPrices`, shared with the client in-process); shown beside the cursor when the fonts are bound | `wired.rs`, `app/rest.rs`, `app/single_player.rs` (`Started::prices`), `app/play.rs`, `app/ui.rs` |

Tests (synthetic): `crates/d2-client/tests/e2e_vendor.rs`
- `vendor_end_to_end` (step 3 now asserts one 0x9C action 11 per store
  item in store order; it failed before: nothing was sent).
- `shop_panel_buys_and_closes`: trade open → the shop opens → a right
  click on the permanent cap leaves as C→S 0x32 → the server pays and
  places the copy (S→C 0x2A kind 4) → closing the shop sends C→S 0x30.

## PROVISIONAL (REC-109 in `docs/HANDOFF.md` §7)

- Grid geometry, left-click buy without the confirm dialog, button hit
  areas, the epoch counter and the host-published price are
  `d2rs-own, unverified` (see REC-109).
- The NPC menu (`q-npc-menu`) is not merged: nothing in `play` opens a
  trade yet, because there is no NPC in the play world and no Trade menu
  option. The panel reacts to the store items, so it works as soon as
  the Trade option sends C→S 0x38 action 1 (or calls
  `OriginalUi::open_shop`).
- Stale store items: the client keeps the item units of a closed trade;
  the panel hides records older than the last close (`floor`), and a
  re-trade re-sends them. The server does not send 0x9C action 12 for an
  item taken out of the store, so a bought non-permanent item stays drawn
  until the next trade open (the purchase gold and the copy in the
  backpack are right).

## What is left

- Single-item repair (the repair button + a click on the player's item),
  the confirm dialog, sell-by-Ctrl-click, gamble shops (0x37) and the
  hire (0x36) shop buttons.
- The 0x9C action 12 on a taken store item, and 0x2A result notes.
- A `play` NPC for the town vendors (q-npc-menu), then a local run.
- Not run against game data (cloud): item art for the store grid comes
  from the same rows as the inventory (`ItemsUi::art`); the grid position
  needs a screenshot check.

## The user's local check (after q-npc-menu merges)

```powershell
$env:D2_GAME_DIR = "C:\Program Files (x86)\Diablo II"
$env:RUST_LOG = "warn,d2_client=debug"
cargo run -p d2-client --release -- play --new sorceress Test
```

Walk to Akara, click her, choose Trade. Expected: the shop panel opens on
the left (art, tabs Armor / Weapons / Misc, the item grid) beside the
inventory; hover an item for its price; right-click it to buy (gold drops,
the item appears in the backpack); drop a cursor item on the grid to sell;
Esc or the Close button closes the panel. Copy any `ui:` or `shop` log line
and a screenshot of the grid into `docs/HANDOFF.md` (REC-109).
