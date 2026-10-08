# q-equip-backgrounds

Links connected: `ItemsUi::draw_panel` (`ui/panels/inv_items.rs`) now pushes a `UiDraw::Tint` (new, `ui/draw.rs`) over the box of each equipped item whose requirements fail (client model → `ItemTips::can_use` with the local player's stats 0 / 2 / 12) — tint 0 (red) — or that is unidentified (tint 4); a usable identified item gets none. Tests in `inv_items_tests.rs`.

PROVISIONAL (REC-271, `docs/HANDOFF.md` §7): see the entry. The scene has no fill primitive, so `world_view/ui_bind.rs::ui_items` skips `UiDraw::Tint`; the tint is in the draw list but not yet painted. Hover tint, grid tints and empty-slot pictures are not done.

What's left: a fill primitive in the scene (blend kind 2 with the `A2` table, `inventory.md` §2 r3), then paint `Tint`.

User's local check: `cargo nextest run -p d2-client equipped` (passes). In `play` nothing red appears yet (the fill is not painted).
