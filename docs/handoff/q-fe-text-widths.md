# q-fe-text-widths

Links connected: `ui/front_end/glyphs.rs::text_width` (font-table advances, `ui/text.md` §6) -> `FrontArt::text_width` (font cache, `app/front_host.rs`) -> `FrontEnd::overlay` in `draw`. `provisional_adv` deleted.

PROVISIONAL: REC-258 (width 0 when no art / font missing).

Left: nothing for widths; credits colour and render check remain (REC-231).

Test: `glyphs::tests::credits_rows_centre_with_the_font_advances` (synthetic font table; column A ends at x 400).

Local check: `cargo run -p d2-client -- play` (front end) -> Credits; names in column A should end at the centre gap, columns C centred by true text width.
