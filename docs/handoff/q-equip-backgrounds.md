# q-equip-backgrounds

Links connected: `ItemsUi::draw_tints` (`ui/panels/inv_items_tint.rs`), called by `InventoryUi::draw` (`ui/original.rs`) before `draw_panel`, paints a tint under each page-0 grid item (hovered green, refused red, unidentified, usable blue) and each equipped item (hovered, refused, unidentified, else none), through `esc_menu::push_fill` and the `d2rs\hudfill` frames (5-8 by tint index, new frame 9 for unidentified). Refused = the item's requirements fail against the local player's strength / dexterity / level (`ItemTips::can_use`) or item flag 0x4. Tests in `inv_items_tests.rs` (tile rectangles per frame; red fill frame pixels).

PROVISIONAL (REC-271, `docs/HANDOFF.md` §7): the tints are opaque, not the spec's translucent A2 blend; shooter / quiver, cursor-item and transmogrify terms are not read; empty-slot pictures are not drawn.

What's left: the translucent blend for UI fills; the empty-slot pictures (`panels.md` §9.4).

User's local check: `cargo nextest run -p d2-client tint` (passes). In `play`, open the inventory (I) with an item whose requirements you don't meet equipped or in the grid: a red block sits under it; usable identified grid items sit on blue; hovering an item turns its block green.
