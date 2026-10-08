# q-strings-bind: string ids bound in the panels

## Links connected
- `ui/waypoint_ui.rs` and `ui/stash_ui.rs` passed `NoStrings` to their panels; they now pass `ctx.strings` (the `TableStrings` that `install_strings` binds). The waypoint tab / row text and the stash `GoldMax` line (string 4051) resolve.
- Stash cap: `d2_sim::world::stash::STASH_CAP` (2 500 000, the fixed stash limit) replaces the 0 placeholder.
- `ui/hud_tips.rs` (new, called once at the end of `HudUi::draw`): the control-panel tool tips `run_tip` (4179 / 4178), `menu_tip` (4167 / 4168), `tip_800` (3986 / 3987) and `exp_tip` (4163) now draw as centred text, resolved by id through `ctx.strings`.
- Already bound before this task (checked, no change): NPC menu (`npc_menu_ui.rs`, `ctx.strings`), shop tabs, character panel, hire list, item tips (`item_tip.rs` over `TableStrings`).

## PROVISIONAL (`// d2rs-own, unverified`, REC-241)
- The tip font (1, the globe-number font) is not named by `control-panel.md`.
- The globe numbers (§3 r6) are still not drawn: they need text widths (`width_a`), which a panel does not have.
- The 640 × 480 variant of the new-stats tips is not drawn (800 × 600 only).

## What's left
- Globe numbers, once a text measure reaches the panel draw.
- Item-name composition (magic / set / unique / rare names) lives in `item_tip.rs`; no change here.

## Local check
`D2_GAME_DIR=<game> cargo run -p d2-client -- play`: hover the run button, the menu button, the experience bar: the tip text appears over the strip. Open a stash: "Gold Max: 2500000" shows top-left. Tests: `cargo test -p d2-client --lib hud_tips stash_gold_max`.
