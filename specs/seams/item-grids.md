# Spec: Seam — Item and inventory grids (sim ↔ server ↔ wire ↔ client UI)

- **Status:** draft. Written from both sides' specs and code (seam audit
  2026-10-08, `docs/METHODS.md` M23); contract tests in
  `crates/d2-client/tests/seam_item_grids.rs`; nothing here is verified
  against 1.14d (rule 10).
- **Target version:** 1.14d
- **Crate/module:** `d2-sim::items::inventory` (grids, belt, pages),
  `d2-sim::items::bitstream` (writer), `d2-sim::wiring::inventory::bits`
  (stream view), `d2-sim::world::vendors::store` (store pages),
  `d2-server::adapters::handlers::{items, world}` (desks, store flush),
  `d2-proto::item_bits` (reader), `d2-client::bridge::{items, belt,
  item_lists}` (model), `d2-client::ui::{inv_grid, widget, shop_ui,
  hud_belt}` and `ui::panels::{inv_items, stash_items, cube_items, shop,
  control::belt}` (UI).
- **Related specs:** `items/inventory.md` §1–§3, `items/inventory-moves.md`
  §6, §7, §11, `items/bitstream.md` §2–§4.1, `client/msg-stats-items.md`
  §2, `ui/inventory.md` §1, §3, §5, §10, `ui/panels.md` §9.2, §11,
  §Test vectors, `ui/panels-2.md` §14, `ui/control-panel.md` §5,
  `world/vendors.md` §3.1, §4, `sim/client-messages.tsv`.

## Summary

An item's place crosses four owners. The sim owns it (inventory grids,
body locations, belt slots, the cursor; `items/inventory.md` §1). The
server desks only carry it (`InvDesk::stream_item` reads the sim's item
data into the writer's view). The wire carries it in the head of the
item bit stream of S→C 0x9C / 0x9D (`items/bitstream.md` §4.1 rules
1–3) and back in the fields of the C→S item intents
(`items/inventory-moves.md` §7). The client model keeps the last record
of each item and reads its place from the stream head
(`client/msg-stats-items.md` §2 r4); the UI turns a mouse pixel into a
grid cell, a belt box or a body location and sends an intent. This spec
states what each side must agree on; the owner specs hold the rules.

## Inputs

| Name | Type | Source |
|---|---|---|
| page | u8, 0–4 (0xFF none) | sim item data +0x45 (`inventory.md` §1.1) |
| grid cell (x, y) | column, row from the grid's top-left | sim item data, static path +0x0C / +0x10 |
| body location | u8, 1–12 | sim item data +0x44 (`inventory.md` §1.2) |
| belt slot | 0–15 | the item's x in grid 1 (`inventory.md` §3 r2) |
| store page | u8, `itemtypes.storepage` | `world/vendors.md` §3.1 r3 |
| `inventory.bin` records | 32 × (gridX, gridY, rectangles, cell size) | the user's tables, read by both sides |
| `belts.bin` records | 14 × (numboxes, boxes) | the user's tables |

## Outputs / state changes

None: a seam spec. Each side's outputs are its owner spec's.

## 2. Contract

### 2.1 Page ids

One numbering on every side: 0 inventory, 1 and 2 trade pages, 3 cube,
4 stash; 0xFF "no page" (cursor, belt, body; `inventory.md` §1.2). The
NPC store uses the same field for its store page, `itemtypes.storepage`
(0 armor, 1 weapons, 2 magic, 3 misc; `vendors.md` §3.1 r3), so a store
item's page is its shop tab (`ui/panels-2.md` §14, the client's
`shop_tabs`). The C→S page fields (0x18 page u32 @13) use the same ids.

### 2.2 Wire position fields

The stream head carries mode (3 bits), then for modes 3 and 5 the ground
sub-tile x, y (16 + 16 bits), else body location (4 bits), x (4), y (4)
and page + 1 (3 bits, page 0xFF → 0; `bitstream.md` §4.1 r2–r3). The
writer (`d2-sim::items::bitstream::put_location`), the proto reader
(`d2-proto::item_bits::read_location`, raw `page1`), the sim reader
(`bitstream::read::read_location`) and the client head reader
(`d2-client::bridge::items::peek`) must give back the same (mode, body,
x, y, page) for every value the sim stores. x and y are capped at 15 by
the writer (4 bits); no 1.14d grid is wider or taller than 10.

### 2.3 Grid space

A grid cell is (x, y) = (column, row) from the grid's top-left, row-major
storage cell y × width + x (`inventory.md` §1.1). An item's (x, y) is its
footprint's top-left cell; its footprint is items `invwidth` ×
`invheight` of its record (the first record of its code on the client).
The intents that name a cell (0x18 x @5, y @9; 0x1F x @9, y @13) carry
the footprint's top-left cell in the same order, never a pixel.

### 2.4 Grid record by page

The sim's record (`inventory.md` §1.3, `grid_record`) and the client's
record for drawing and hit tests (`ui/inventory.md` §1 r2; the client's
`original::inventory_record`, `stash_items::stash_record`,
`cube_items::cube_record`) name the same `inventory.bin` record for a
player owner: page 0 class 0–4 → 0–4, 5 → 14, 6 → 15; page 3 → 9; page
4 → 12 in an expansion game, 8 in a classic one. The client adds 16 at
800 × 600 (same grid size, `inventory.md` §1.3). Both sides index the
compiled `inventory.bin` (32 records, the `.txt` `Expansion` row not
compiled), never `.txt` rows.

### 2.5 Pixel → cell

A cell (c, r) of a record covers left + cellW·c … + cellW − 1, top +
cellH·r … + cellH − 1 (`ui/inventory.md` §1 r3). The click test is
`0x00483AB0` (left ≤ x < right, top ≤ y ≤ bottom, §4 r1) and the cell is
§1 r4. For every pixel the click test accepts on a measured record, the
cell is inside the grid (c < gridX, r < gridY); so the client never
names a cell the sim's bounds test (§2.2) refuses for a 1 × 1 item.
The cursor cell of a cursor item (§5 r3) is the footprint's top-left,
clamped so the footprint stays in the grid; it is what 0x18 / 0x1F send.

### 2.6 Body locations

1 head, 2 neck, 3 torso, 4 right hand, 5 left hand, 6 right ring, 7 left
ring, 8 belt, 9 feet, 10 gloves, 11 / 12 swap right / left
(`inventory.md` §1.2). The client's equipment boxes are the
`inventory.bin` rectangles in that order (`ui/panels.md` §9.2: rArm 4,
lArm 5, rHand 6, lHand 7); 0x1A / 0x1B / 0x1D / 0x1E bodyloc u8 @5 and
0x1C bodyloc u16 @1 carry these ids; the intents accept 1–10.

### 2.7 Belt slots

Slot s = column (s & 3) + 4 × row, row 0 the bottom row (`inventory.md`
§3 r2). The sim stores the slot as the item's x (y = 0); the wire
carries it in the 4-bit x; the client keys its belt by x
(`bridge::items::belt`), indexes `belts.bin` boxes by slot, and sends
the box index as 0x23's slot. The column-ready bytes are written for
x < 4 only (`msg-stats-items.md` §2 r6).

### 2.8 Beltable

Whether an item may go to the belt is one fact on both sides: itemtypes
`beltable` of the item's type (`inventory.md` §3 r3, `0x0062BAD0`), and
for placement 1 × 1. The client's Shift-click test (`ui/inventory.md`
§10 r3.4) and the belt click (`ui/control-panel.md` §5) call the same
`0x0062BAD0` on the client's tables.

### 2.9 Store positions

A store item's place in the NPC's grid (monster record 5, 10 × 10) is
the sim's (`vendors.md` §3.1 r4, `inventory.md` §2.3 non-player search);
the client places it at the stream's (x, y) on its store page (the 0x9C
action 0x0B handler places the item in the NPC's grid,
`msg-stats-items.md` §2 r5.3 row 0x0B).

## Edge cases & original bugs

- The 4-bit x / y cap is the writer's (`bitstream.md` §4.1 r3); a
  crafted 0x18 with x = 0x7FFFFFFF still places (`inventory.md` §2.2)
  and goes out as x = 15.
- 0x23 / 0x25 accept any slot 0–15 whatever the belt (`inventory.md` §3
  r7); the client only offers the boxes of its belt record.

## Test vectors

| Rule | Check | Test |
|---|---|---|
| §2.1, §2.2 | sim writer → client `peek` and proto reader, pages 0–4 and 0xFF, modes 0, 1, 2, 4, 3 | `seam_item_grids::wire_place_round_trips_to_the_client_model` |
| §2.4 | sim `grid_record` = client record picks, classes 0–6, pages 0, 3, 4, both game types, both resolutions | `seam_item_grids::grid_record_by_page_agrees` |
| §2.5 | every accepted pixel of the measured records 0 / 16, 12 / 28 → a cell inside the grid; cell rect ↔ mouse cell | `seam_item_grids::every_accepted_pixel_names_an_in_grid_cell` |
| §2.7 | a belt item written at slot s is the client's belt slot s | `seam_item_grids::belt_slot_is_the_stream_x` |
| §2.1 | store page p → shop tab p | `seam_item_grids::store_page_is_the_shop_tab` |

## Provenance

Read from the owner specs listed above and both sides' code (seam audit,
2026-10-08). No `re/`, no `../refs/`.

## Open questions

1. The client's store grid geometry (`shop_ui::grid_record`) is d2rs-own;
   the original draws the NPC grid through a layout record (monster
   owner, `ui/inventory.md` §1 r2) whose store values are not measured.
2. Known breaks of this contract (seam audit 2026-10-08, no test yet):
   §2.8 (the client's belt test is a code list, not `beltable`) and §2.9
   (the client repacks store items and drops the stream's x, y);
   build-queue rows `q-fix-seam-beltable`, `q-fix-seam-store-grid`.
