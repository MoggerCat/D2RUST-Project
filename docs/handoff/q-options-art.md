# q-options-art: Esc menu art

Branch `claude/q-options-art`. Spec `ui/frontend-options.md` §O2 r3, §O4. Provisional points: REC-257 (`docs/HANDOFF.md` §7). d2rs-own, unverified (rule 10).

## Links connected

| Link | Where |
|---|---|
| Row → label / value image names and widths (§O2 r3) | `ui/esc_art.rs` `label`, `value` |
| Files registered once, resolved by the existing panel-art path (`*local\` prefix → `data\local\ui\eng\`) | `OriginalUi::new`, `world_view/panel_art.rs` `archive_name`, `hud::optional_file` |
| Draw: centred action labels, left label + right-aligned value, `OptBar`/`OptBarC` + `OptSkull` at X0 + t, `pentspin` both sides at the selected row | `esc_art::draw_row`, `draw_pents`, called from `esc_menu.rs` |

## Tests

`cargo nextest run -p d2-client -E 'test(esc_art) | test(the_menu_tree)'`: synthetic file table; label frames and positions, knob at the slider position, pentspin at the selected row (left spins the other way), right-aligned value. `the_menu_tree…` now checks the menu's image names instead of text (the spec says the labels are images, no font).

## PROVISIONAL / left

REC-257: no disabled look or dark slider rectangles; pentspin frame follows the tick; Window Mode stays text.

## Local check

```
cargo run -p d2-client --release -- play --new sorceress Tester
```
Esc: the DC6 labels (Options / Save and Exit Game / Return to Game), spinning pentagrams either side of the selected row; Options → Sound Options: bars with the skull knob moving with Left/Right and dragging; Video Options: value images (Low/Medium/High, On/Off) right-aligned at the right edge; Window Mode is plain text.
