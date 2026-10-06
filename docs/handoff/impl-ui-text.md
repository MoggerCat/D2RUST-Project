# Handoff: UI text (`ui/text.md`) in `d2-client`

Branch `claude/impl-ui-text`, based on `main` at `62fcef6` (2026-10-06,
cloud). Spec: `specs/ui/text.md` (+ `text-fonts.tsv`), the 2026-10-06
updates of `formats/font-tbl.md`. Status: **implemented, unverified**
(no game files in the cloud; checks queued below).

## What changed

| Path | What | Spec |
|---|---|---|
| `crates/d2-client/src/ui/text.rs` | `NoTextRules` replaced by `OriginalText` (the one `TextRules`): draw call (pen right by `width`, `LF` moves **up** one line step, centering in a block of max width + 8), `ÿc` codes (k = unit − 0x30, ≥ 13 → 0, negative kept, `ÿ`/`ÿc` at the end ends drawing, `ÿ` + other = glyph 255), horizontal window, no-color variant; vertical window returns `Unspecified` (OQ 1) except `skip < 0` (empty). Measures `width_a/b/c`, `line_width`, `max_width`, `text_height`, `line_step`; `centered_span_x` (`0x004A7080`); `framed_text` / `framed_text_tight` (§8); `wrap` (§10). `FONTS` table (+`font_info`). `GlyphLookup::record` is Latin by position (record `c`, or 0 above 0xFF); `AmbiguousGlyph` removed; `MissingGlyph { code, record }` only for a font with fewer records than the position (malformed input). `TextOpts` is now the call kind (`Draw { centered, block_w, mode }`, `Horizontal { s, w }`, `Vertical { skip, lines }`, `NoColor`): no clip rect (CG2, §12). Placement color is `i32`. | §1, §3, §5–§13 |
| `ui/draw.rs` | `TextRequest` gains `opts: TextOpts`; `at` is the pen (bottom row of the first-drawn line); `clip` stays and is the frame for text (§12) | §12, §13 |
| `ui/widget.rs` | `Label`/`TextInput` send `TextOpts::default()`; doc updates (Label position → `ui/panels.md`, caret → OQ 3) | §13 |
| `world_view/ui_bind.rs` | `TextHooks::glyph_look(color: i32, mode: u8)`; `text_sprites` uses `req.opts` and turns the pen into the sprite top-left with `rules::placement::draw_position` (DC6 bottom anchor: `y − h + 1`); `TextColors::push` (PL2 maps 1–12 from offset 439,847 into the `MapTable`); `OriginalTextHooks { colors }`; `original_text_font` (font id → `.tbl` path + DC6 `FrameSetKey` dir 0). `Unspecified` now answers text from the spec (font, rules, color 0); colored glyphs need `OriginalTextHooks` with the frame's PL2 maps. Glyph look: color 0 → no remap, k 1–12 → map k, opaque; mode ≠ 5 → `render/blend-modes.md`; k outside 0–12 → OQ 2 error. | §4, §13 |
| `crates/d2-formats/src/font.rs` | `font-tbl.md` correction: header `unknown: u16` + `count: u16` (bytes 6–9), record `frame: u16` (bytes 8–9; `unknown4` gone). Test `frame_is_one_byte` replaced by `frame_is_a_u16_and_count_is_header_byte_8` (the spec changed the rule). | font-tbl.md |
| `crates/d2-client/tests/app_frame_loop.rs` | fixture font now 256 records (by-position lookup); `glyph_look` signature; `placeholder_hooks_refuse_text_and_sounds` now expects `FontMissing(font8.tbl)` (the app's hooks answer text from the spec; fonts are not loaded there) | — |
| `crates/d2-client/tests/game_text.rs` | new, `#[ignore]`: the real-font vectors | §Test vectors |

Tests: 15 in `ui::tests::text` (all spec vectors that do not need real
metrics, with synthetic fonts whose advances meet the vector totals;
`text-fonts.tsv` checked against `FONTS`), 3 in
`world_view::ui_bind::text_tests`, 1 in d2-formats, 2 ignored in
`tests/game_text.rs`. Gate: see the commit (`sh tools/gate.sh`).

Not done here (outside this task's files): §2 string decoding as UTF-8
and lookup by id (`d2-data::strings`, `tbl.md`); loading the 14 fonts
and the act PL2 text-color maps into `ViewAssets` / the `MapTable` in the
app (asset wiring: `OriginalTextHooks.colors` is `None` until then); the
by-code lookup of non-Latin locales (out of scope, English).

## Local run queue (game files)

1. `D2_GAME_DIR=<install> cargo test -p d2-client --test game_text -- --ignored`
   — expect both tests to pass: every font's `.tbl` has height per
   `text-fonts.tsv`, count 256, `code = frame = i`; every DC6 frame has
   the tsv size, offsets 0, flip 0; Font16 `Stash` = 40, `AB⏎C` centered
   at (100, 200) → (104, 200), (116, 200), (109, 184); Font8 `ÿc4Gold`
   width A 49 / width C 28, `ÿc9Rare` width C 50 / line width 29, `ÿmX`
   12; FontFormal10 wrap vectors (M 100, M 60, `Supercalifragilistic` M
   50). Note: the DC6 is read through `ArchiveSet::read`; if
   `fontformal10.dc6` / `reallythelastsucker.dc6` come from `d2data`
   instead of `d2exp` the frame sizes may differ (`text-fonts.tsv`
   `dc6_archive`), which then is an archive-order question for
   `client/assets.md` §B1.
2. Capture cases `text-0001` (main menu) / `text-0002` (magic item hover
   box) of the spec: pixels vs the CPU reference, once the capture tool
   and font asset wiring exist.

## Open questions (for the spec owner)

1. **Wrap, trailing empty line.** §10 read literally: when the last line
   ends at `e = L − 1` (the NUL does not fit with it), the next round has
   `s = L`, and `text[L..=L]` (just the NUL) is emitted as a last, empty
   line (`ab  cd`, M 15 → `ab `, `cd`, `""`). Implemented and tested
   that way; confirm with a capture or a Ghidra read of `0x00502970`
   (does the loop stop at `s ≥ L`, or does the caller drop empty lines?).
2. **Line width skip over `LF` / end.** §6 line width skips `ÿ` + the next
   two units; implemented as an unconditional skip clamped at the string
   end, so an `LF` among the two skipped units does not end the line
   (and max width continues after where the skip landed). Confirm
   `0x00501910` for `ÿ⏎…` and `ÿ` at the last two units.
3. **`LF` in widths A/B/C**: read as "contributes 0" (table column `LF` =
   0), not `adv` of record 10. Confirm.
4. **Horizontal window stop test**: implemented as checked before each
   unit (`pen x > x + w` → stop), including before `ÿc` codes and `LF`.
   Confirm the order in `0x00501FE0`.
5. **No-color variant** (`0x00502190`): implemented with §7's centering
   argument ignored (not centered) and `ÿ` skipping 3 units clamped at the
   end. No caller known (spec OQ 6).
