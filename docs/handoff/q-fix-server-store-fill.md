# q-fix-server-store-fill (`claude/q-fix-server-store-fill`)

> Fix session 2026-10-09 (REC block 520–529: none used). Rows: `q-fix-seam-store-grid` server half, Ctrl-click sell.

- Store fill: `InvDesk::store_place` / `store_unlink` (`wiring/inventory/host.rs`) put store items on the NPC's inventory (monster record 5, per page, find-free, no send; `vendors.md` §3.1 r4, `inventory.md` §2.4) and unlink on take / clear (§7.1 r10, §6 r4). Called from `VendorDesk` through the lent inventory (`NpcInventory::store_place` / `store_unlink`, open-trade path) and from `InvVendors` (buy, sell, refresh paths; `VendorDesk::grid_npc` finds the class's NPC when no call NPC is set).
- Real install: `play_smoke::the_shop_finds_each_store_item_at_its_server_cells` passes (Akara's ten items on distinct cells, e.g. (9,0)…(9,7), (0,0), (0,2)) and is un-ignored to the standard real-data attribute.
- Ctrl-click sell: `inv_items` sends 0x33 (`GridInfo::sellable`: `quest` column and type 39; the 0x1000 flag is not in the stream, the server refuses it; `store_npc` set by `open_shop`, cleared at close). Real install: `play_smoke::ctrl_click_sells_a_backpack_item_to_the_open_store` passes (0x33 sent, no 0x19, item leaves the backpack, gold rises).
- Left: a real-install stack-click test (0x21) needs two stacks of one item in the backpack; the starting character has none.
- Finding: `Run` tests that call `no_findings` fail on `q-fix-real-client-path` (run leg drawn (4929, 4209) vs server (4924, 4209)); not from this branch.

## Rows from q-prov-recording (same session)

- `q-fix-real-newchar-hand-item`: done. A new character's two join hands carry item 0 (`PlayerRecord::new_character`); `d2s-load.md` §8 r3 PROVISIONAL replaced by the recording; `play_smoke::the_play_path_steps_and_speaks` (real install) still walks.
- `q-fix-real-tp-town-cast`: done. A town cast makes nothing (`create_town_portal` returns none in a town; `last_field_level` removed; REC-243 (1) settled in HANDOFF); the item path already refused it: real install `play_smoke::a_town_portal_scroll_used_in_town_is_refused_without_cost` (3F/7C bytes, no 0x22, scroll stays); `app_town_portal` test renamed and follows.
- `q-fix-real-potion-effect`: not done, binary-only: `pc1-data.md` Step 4 item 25.
- `the_session_flow_creates_the_game_...`: the test was stale, not the code. The install joins a new sorceress with three 0x23 (select before 0x0B, then the two hands after 0x5F) as `facts/join/a1-new-sor.tsv`; the unapplied steps are new character set-up / has skill / mouse skills / quest entry. Expectations follow the recording.
- `q-fix-real-exit-target`: done. After a game the front end opens the main menu (`Entry::AfterGame` removed; `main.rs` uses `Entry::MainMenu`); `smoke_frontend::after_a_game_the_front_end_opens_at_the_main_menu` (runs in CI) and the save-and-exit test follow the recording. Finding (resolved as `q-fix-real-front-save`): see below.
- `q-fix-real-front-save`: done, test-side. `difficulty_popup_...`: `save_with_status` left a stale header checksum and cleared the stub's NEW flag, so the game's load (not the select scan) failed at 0xC / the quest section; it now re-sums (`formats/d2s.md` §3 r1) and keeps NEW. `play_cli_...`: GameData is Live only, so the `--save` leg writes a real stub and expects the load to succeed (origin `character from ...`). `esc_options_...` passes under nextest (the gate's runner); under plain `cargo test` the parallel tests race on `D2RS_CONFIG_DIR` (process-wide `set_var`), which is the only cause of the missing controls.toml. All 5 `smoke_frontend` ignored tests pass: `cargo nextest run -p d2-client --test smoke_frontend --run-ignored only`.
- `q-fix-real-den-entrance`: done. Root cause: `LevelTypes::warp_unit` (the warp tile preset `0x0066E1C0`, `path-placement.md` §12.1) had no live implementation, so no exit cell ever added a tile preset and no tile unit existed in any tile level (cave entrances, dungeon stairs). `WorldTypes::warp_unit` now adds the type 5 preset (class = lvlwarp Id, `OffsetX/Y` from the table, new `DrlgData::warp_offsets`) to the preset room; the trait gained `data` and the exit type. The Blood Moor's DenEnt stamp gives class 2 at the spec's (14, 21). `app_level_warp` walks to the entrance (the tile exists only near it; `warp_ids` lists the four lvlwarp ids into the Den, the stamp decides which) and passes.
- `q-fix-real-border-room-lag`: not a client bug: the bridge-only rig had no prediction, and the play app relinks the local player after the bridge frame (`walk_room::preview_walk_room`, order bridge_frame → walk → recache), so there is no draw frame without a room. `seam_drlg_coords` now routes into the Blood Moor on the install, recaches as play does, and checks every frame; passes. Finding: `app_level_border` still uses the synthetic town's coordinates and fails on the install (q-fixture-migrate-2's).

## Area G: items-vendor-akara-buy (scenario-diff)

State channel: items and player equal (fi ignored, item 26). Fixed on the way: the buy needs the player's inventory
(`ensure_inventory` in `vendor_inv.rs`: a character without start items had none, so equip and auto-place failed);
a decoded item copy keeps the allocation's item and start seeds (`item_records.rs`); the taken store item is freed
after its 0x9C action 12, which goes out in the next tick's unit work after the 0x2A (`vendors.md` §7.1 rule 10) in mode 4.
Open on the packets channel:
- frame 16: 0x27 entry order for Akara (1.14d 0x40 then 0x0b; d2rs the reverse), quest/NPC list builder (not items).
- frame 17: 0x31 answer missing headless (tools).
- frame 20: the 7th store item (magic, charged skill, guid 8): charges current value 1.14d 67 = max, d2rs 65
  (`properties.md` §5 r9, `roll(c - c/8)`: r 58 vs 56 with equal final item seeds, so the order of the rolls inside
  the item differs); pc1-data Step 4 item 27.
- frame 24/25: the gold 0x1E goes out in the tick in d2rs, after the flush in 1.14d (stat messages of the client pass).
