# q-fe-host-screens: front-end host hookups

Branch `claude/q-fe-host-screens`. PROVISIONAL points: REC-231 (HANDOFF §7).

## Links connected

| Hookup | Where |
|---|---|
| New shell inputs `Wheel`, `KeyUp`, `Middle`; `Screen::{pointer,wheel,key_up,middle_down,overlay,sync}` (defaults no-op); `FrontEnd::overlay`, `goto`, `pointer`; `DrawItem::{Rect,Border}` | `ui/front_end/{mod,screen}.rs` |
| Host feeds mouse wheel, key-up, middle button; draws `front.draw()` + `front.overlay()` | `app/front_host.rs` |
| Credits: `visible_rows` → `DrawItem::Text` per tick; text from `D2_GAME_DIR` loose file, else the archives | `credits.rs` `overlay`, `front_host::registry` |
| Create: pointer-move → hover / hero hover state; hero frames, hover texts, check marks, name + caret, popup text as overlay; show / hide / enable of the name box, boxes, OK, warning buttons (`sync`, run after build, every event and every tick) | `create.rs` |
| Create: stub `.d2s` (335 bytes, `D2s::new_stub` + `d2s::write`) at the game-load outcome; `name_taken_in(save_dir, ..)` for duplicates | `front_host::{registry, write_stub, write_created}` |
| Controls: `draw_list` → `DrawItem`s (font 13), wheel, key-up, middle button | `controls.rs` |
| Char select: `register_with(reg, Some(save_dir), handle)` | `front_host::registry` |
| `main.rs` passes the save dir (`FrontHost::with_save_dir`) | `main.rs` |

## PROVISIONAL / left

See REC-231. Text is still a bar until q-fe-draw's glyph path; the host's text width is a fixed 8 px per unit; no colour on `DrawItem::Text`. The `SelectionHandle` and the created character are not yet used to start the game (q-fe-start-flow). Options has no arm into CONTROLS (the tests use `FrontEnd::goto`).

## Tests

`cargo test -p d2-client --test front_host_screens` (synthetic data, no game files): credits heading in the draw list after ticks; create hover → hover animation file + class text, select + name + OK → 335-byte stub that `d2s::read` loads back, then the duplicate-name check sees it; char select controls name a saved character from a temp save dir; controls screen draws and the wheel changes the list.

## Local check

`D2_GAME_DIR=... cargo run -p d2-client --release -- play`: Credits scrolls (bars, not glyphs, until q-fe-draw); on Create New the heroes follow the pointer (hover animation), a click walks the hero forward and the name box / OK appear; typing a name and OK writes `<save dir>/<Name>.d2s` (335 bytes); the next run lists that character on char select.
