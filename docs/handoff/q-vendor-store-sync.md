# q-vendor-store-sync: vendor buy, store grid and bought copy

Stitching session `q-vendor-store-sync`, branch `claude/q-vendor-store-sync`. Nothing here is verified against 1.14d (rule 10). No new PROVISIONAL point; REC-262 was not needed.

## Finding

Both links `q-tp-gaps` listed as open already work; the note was stale. No code change was needed, only tests that pin them.

| Link | State | Where |
|---|---|---|
| Bought copy, S→C 0x9C action 4 | sent in the buy's tick update pass (`place_in_backpack` → `place(.., send)` queues the put-in-container update; `update_pass` emits it, `vendors.md` §7.1 rule 10) | `d2-server` `handlers/items/vendor_inv.rs`, `items/moves.rs` (`update_pass`) |
| Action 12 on the client | `bridge::items::item` returns `None` for a last record of action 12, so the item leaves `store_items`, which the shop panel (`ui/shop_ui.rs` `page_items`, `tabs_of`) reads | `bridge/items.rs` (`removed`) |

## Tests (synthetic)

- `tests/e2e_vendor.rs` `buying_a_store_item_sends_0x9c_action_12`: now also asserts the bought buckler is gone from the client's `store_items`. Its player has no room, so the update pass (action 4) does not run there.
- `tests/e2e_single_player.rs` (step 20, room-backed): the bought copy is a stored backpack item (`mode 0`, page 0), not a store item, in the client's model. It checks the model directly because that harness has no local player to own the item.

## Left

Nothing for this row. The shop panel's own redraw after action 12 is covered only through the shared `store_items` filter, not a pixel check.

## Local check

```
cargo run -p d2-client --release -- play --new sorceress Test
```

Buy a non-permanent item (a buckler or similar, not a potion/scroll) from Akara or Charsi: it leaves the shop grid and appears in the backpack.
