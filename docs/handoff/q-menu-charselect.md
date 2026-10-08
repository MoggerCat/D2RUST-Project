# q-menu-charselect: character select

Branch `claude/q-menu-charselect`. Files: `ui/front_end/screens/char_select.rs`, `tests/front_end_char_select.rs`. Spec: `ui/frontend-menus.md` §F2.1–F2.7, F2.9. Provisional points: REC-180 (HANDOFF §7).

## Connected
- Scan of `<dir>/*.d2s` (name to first `.`, length/`-`/`_` filter, header check, no checksum), sort newest-first (expansion) or name order (classic): `scan`, `Model::rescan`.
- Model: 8-slot 2×4 layout, selection, Home/End/arrows/scroll vectors (n=11), click, double click (500 ms), button states, OK (dead hardcore 5304, difficulty thresholds), Delete (all `<name>.*`, confirm), Convert (status 0x20 + checksum rewrite, failure 21872).
- Screen: controls per §F2.9 (positions, art, string ids), flow triggers (Esc → main menu, Create New → create, OK → difficulty box or game load). `difficulties_open` = 1/2/3 (Normal / + Nightmare / + Hell). The chosen character is left in a `SelectionHandle` (`register_with`) for the host and the difficulty screen. `DirSaves` is a `SaveFolder` over the same scan.

## Left
- The host must call `register_with(reg, Some(save_dir), handle)`; `register_all` registers an empty list.
- Name colour, paper doll and selected-character animation are in the model, not drawn.
- Pop-ups and selection changes redraw by re-entering the screen (REC-180 (1)); a `Refresh` trigger would be cleaner.

## Local check
`cargo test -p d2-client --test front_end_char_select --test front_end` (no game files): all pass. With real saves: scan the user's Save folder with `DirSaves` and compare the list order with the original's.
