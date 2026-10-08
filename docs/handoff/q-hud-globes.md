# q-hud-globes: HUD globe numbers

## Links connected
- `ui/hud_tips.rs` `draw_globe_text` (new) calls the existing `globes::globe_numbers` (life / mana, §3 r6) and `globes::stamina_tip` (§4 r2) and pushes them as `UiDraw::Text` in font 1. Widths come from `sh.fonts` (`FontMeasure::width_a`), the same measure the cube tips use.
- `ui/hud.rs` `HudUi::draw` passes the smoothed life / mana / stamina values, the `Show HP/MP Text` toggles (`hud.input`), the mouse and state 136 (one call, after the existing tips).
- `globes.rs` gained `StaminaIn`.
- The experience tip was already bound (q-strings-bind).

## PROVISIONAL (REC-252, `// d2rs-own, unverified`)
- Font 1 for the stamina tip (the spec names it for the numbers only).
- Width 0 until `set_fonts` has bound the fonts.
- State group 24 (blue stamina) is not in the model.

## What's left
- 640 × 480 variants; none seen in the spec for these.

## Local check
`D2_GAME_DIR=<game> cargo run -p d2-client -- play`: hover the life or mana globe: "Life: x / y" / "Mana: x / y" over it; click the globe to toggle it permanently; hover the stamina bar: "Stamina: x / y". Tests: `cargo nextest run -p d2-client hud_tips`.
