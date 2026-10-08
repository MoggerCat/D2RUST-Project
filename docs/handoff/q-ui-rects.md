# q-ui-rects: rectangle primitive and belt key labels

Branch `claude/q-ui-rects`. Provisional points: REC-264 (`docs/HANDOFF.md` §7).

## Links connected
- `BeltDraw::Box` (belt hover / red rectangles) is painted: `hud_belt::fill_rect` tiles the synthetic `d2rs\hudfill` file with frames 5–8 (red, green, blue, yellow; made by `hud::fill_frames`). Test: `a_hovered_belt_slot_paints_its_highlight_rect`.
- Belt key labels follow the play bindings: `HudBelt::set_keys` from `Bindings::inputs(BeltSlotN)`, called every frame from `world_view/present.rs` (`OriginalUi::set_belt_keys`), so Configure Controls → Accept relabels the belt. Unbound: no label. Test: `rebinding_a_belt_key_changes_its_label`.

## PROVISIONAL / left
Opaque fill, no mode 0 table blend; label is the key's portable name, not the controls table's `vk_name`, no cut to width 28. 4049 / 4050 are the bank gold strings (`panels-2.md`), not belt keys, and are not bound.

## Local check
`cargo run -p d2-client --release -- play --new sorceress Tester`: buy potions, put them in the belt, hover one: a green 29 × 29 box shows behind it. Esc → Options → Configure Controls, rebind Belt 1 to F, Accept: the slot-1 label reads F.
