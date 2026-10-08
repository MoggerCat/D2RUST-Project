# Handoff: q-belt-stash (branch `claude/q-belt-stash`)

## Links connected
- Belt draw list (`HudBelt::draw_list`, `ui/hud_belt.rs`): key labels `1`–`4` (font 1, colour 4, boxes 0–3 with a usable item) and the hover rectangle (green, 29 × 29) are in the list; labels are painted. Test: `inv_items::tests::belt::the_belt_draw_list_has_key_labels_and_the_hover_rect`.
- Belt hover tip: `HudBelt::hover_tip` gates on the spec's hover rule (`hover_text`) and `BorderUi` draws the item tool tip at the belt text position.
- Stash gold: the stash gold button release opens the withdraw dialog (kind 4); the inventory gold button with the stash open opens the deposit dialog (kind 3, pre-filled). OK sends C→S 0x4F 0x13 / 0x14 (`gold_dialog::open_dialog`, `close`). Test: `original::tests::stash_gold_withdraw_and_deposit_send_0x4f`. Server amounts and limits: existing `d2-sim` `world/stash` tests (`withdraw_vectors`, `deposit_vectors`).

## PROVISIONAL (REC-240, docs/HANDOFF.md §7)
Default key labels; hover tip is the item tip; rectangles not painted (no rectangle primitive); dialog text in English.

## Left
Paint the rectangles once a rectangle primitive exists; bind the key names to the controls table; string ids 4049 / 4050.

## Local check
`cargo run -p d2-client -- play` (synthetic), buy potions, put them in the belt: labels 1–4 show over the slots, hovering a potion shows its tip. Open the stash, click its gold button: a dialog opens; type an amount and press Enter; the stash gold line and carried gold change.
