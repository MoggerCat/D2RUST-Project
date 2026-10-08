# q-menu-create: character create screen

Branch `claude/q-menu-create`. File `ui/front_end/screens/create.rs` (+ `tests/front_end_create.rs`). Spec `ui/frontend-menus.md` §F3.1–F3.8. Open points: REC-181 (HANDOFF §7).

## Links connected

| Link | Where |
|---|---|
| Control list (background, titles, EXIT/OK, name box, check boxes, 5/7 heroes, fire) with the descriptor positions | `CreateScreen::build` |
| Hero line-up and creation order, classic/expansion | `CLASSIC`, `EXPANSION` |
| Anim table (frames, speed) and file names per class/state; frame math (loop over frames−1, one-shot walks); update/transition rules | `anim_entry`, `anim_file`, `HeroAnim` |
| Click rules (`0x00433BF0`): walk-back, ignored while walking, deselect, re-check Expansion | `CreateState::click` |
| Hover texts (name/description ids) | `CreateState::hover/texts` |
| Name filter (letters; one `-`/`_`, not first; 15 max), OK rule, Backspace | `accepts_char`, `name_valid` |
| Hardcore/Expansion boxes, grey box for Assassin/Druid | `toggle_*`, `*_visible` |
| OK: disabled → ignored; duplicate name (case-insensitive, `name_taken_in`); hardcore warning; Exit/Esc → char select | `CreateState::ok`, flow |
| OK result reaches the host | `NewCharacterSink` (`register_with`) |

Test: `cargo nextest run -p d2-client --test front_end_create`.

## PROVISIONAL (REC-181)

- A click on an idle hero counts as hovered (screens get no pointer-move yet).
- Hero hit box is the 88×184 descriptor, not the DC6 frame-0 box (§F3.3 r5; needs real art).
- Name caret is at the end (so a separator is never first).

## Left (needs shared front_end code or the host)

- Pointer-move to screens (hover texts, hover anim); dynamic controls (show/hide name box, OK, boxes, popup 5149/5165/5303) and live text for 197/198: the model has all of it (`CreateState` getters) but `FrontEnd` builds controls only on entry.
- The host must write the 335-byte stub `.d2s` (`d2s.md` §2.6) from `NewCharacterSink`, and use `register_with` with `name_taken_in(save_dir, ..)`. Free-space check (5149) not done.
- `DrawItem` for hero frames (use `hero_frame`), name text, the edit box.

## Local check

`cargo nextest run -p d2-client --test front_end_create` — all pass; no visible change in `play` yet.
