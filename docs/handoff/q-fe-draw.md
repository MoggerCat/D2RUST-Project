# q-fe-draw: front-end shared draw

Links connected (`ui/front_end/mod.rs` `FrontEnd::draw`, new `ui/front_end/glyphs.rs`, `app/front_host.rs`):

- **Pressed frame.** `draw()` knows the pressed control (`Down` over an enabled button); it draws `tiles…2·tiles−1`. Disabled buttons never press.
- **Fire overlay.** `DrawItem::Blend { file, frame, at, mode }` (mode 3) follows each logo half's base, same frame (`((now−built)/40) mod 29`). The create screen's `FrontEnd\fire` is a Blend itself. Host: additive `min(255, d+s)`, DC6 offsets added.
- **Text glyphs.** `DrawItem::Text` carries `label: Option<Label>` (button geometry). `glyphs::text_quads` runs `layout_text`/`OriginalText` (the in-game text code); the host draws the quads from the font DC6 and `.tbl`. `FrontArt::with_strings` resolves string ids (wired in `main.rs` from `TableStrings`).
- Existing assertion changed: `version_text_and_draw_order` now expects the Blend items after each logo base (spec F1.5 r2: base, then fire). Nothing weakened.

PROVISIONAL: REC-189 (HANDOFF §7). Tests: `tests/front_end_draw.rs`, `tests/front_end_main_menu.rs`.

Not done (other rows): per-screen text positions, hover/create/credits hookups (q-fe-host-screens).

Local check (needs `game/`): `D2_GAME_DIR=<game dir> cargo run -p d2-client --release -- play`. The main menu should show the logo with fire flickering at 25 fps, button labels in the Exocet font centered in their buttons, and a button that sinks (pressed art, label 2 px lower) while the mouse button is held.
