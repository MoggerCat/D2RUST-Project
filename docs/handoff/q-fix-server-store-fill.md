# q-fix-server-store-fill (`claude/q-fix-server-store-fill`)

> Fix session 2026-10-09 (REC block 520–529: none used). Rows: `q-fix-seam-store-grid` server half, Ctrl-click sell.

- Store fill: `InvDesk::store_place` / `store_unlink` (`wiring/inventory/host.rs`) put store items on the NPC's inventory (monster record 5, per page, find-free, no send; `vendors.md` §3.1 r4, `inventory.md` §2.4) and unlink on take / clear (§7.1 r10, §6 r4). Called from `VendorDesk` through the lent inventory (`NpcInventory::store_place` / `store_unlink`, open-trade path) and from `InvVendors` (buy, sell, refresh paths; `VendorDesk::grid_npc` finds the class's NPC when no call NPC is set).
- Real install: `play_smoke::the_shop_finds_each_store_item_at_its_server_cells` passes (Akara's ten items on distinct cells, e.g. (9,0)…(9,7), (0,0), (0,2)) and is un-ignored to the standard real-data attribute.
- Ctrl-click sell: `inv_items` sends 0x33 (`GridInfo::sellable`: `quest` column and type 39; the 0x1000 flag is not in the stream, the server refuses it; `store_npc` set by `open_shop`, cleared at close). Real install: `play_smoke::ctrl_click_sells_a_backpack_item_to_the_open_store` passes (0x33 sent, no 0x19, item leaves the backpack, gold rises).
- Left: a real-install stack-click test (0x21) needs two stacks of one item in the backpack; the starting character has none.
- Finding: `Run` tests that call `no_findings` fail on `q-fix-real-client-path` (run leg drawn (4929, 4209) vs server (4924, 4209)); not from this branch.
