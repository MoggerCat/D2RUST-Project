# q-smoke-frontend: front-end and in-game menu smoke test

Branch `claude/q-smoke-frontend`. Readiness check before the user's local
testing: the whole front-end flow and the in-game Esc menu driven without a
window, the way `main`'s `play` runs them. REC-284 was assigned and not
needed (no new provisional point: every fix follows a spec row or an
existing REC).

## The test

`crates/d2-client/tests/smoke_frontend.rs` (8 tests, ~3 s, no game files):

| Test | Path driven |
|---|---|
| `each_class_is_created_and_starts_the_game` | for all 7 classes: start-up → trademark (click) → main menu → Single Player (no saves) → create: hero, name, OK → stub `.d2s` written → `play_start::resolve` → game app (`add_game`, 0x67 with the front end's flags, original UI, loading overlay) → loading screen → in game with that name / class at Normal |
| `esc_options_save_and_exit_then_reload_the_character` | create Sorceress "Tester" → in game → real key presses: Esc → Options → Sound / Video / Automap (Left changes a slider or choice in each, Previous back) → Configure Controls (62 rows on an expansion install; Inventory rebound to C; Accept click) → Esc closes, Esc opens, Save and Exit ends the app → front end comes back at **character select** listing "Tester" → select, OK → same character in game |
| `difficulty_popup_starts_the_game_on_nightmare` | saved character with Nightmare open → OK → popup → Esc back → OK → Nightmare → game runs on difficulty 1 |
| `credits_and_cinematics_and_back` | Credits (Esc and Exit), Cinematics (Exit and Esc) → main menu; Esc on the main menu exits |
| `delete_a_character` | Delete → No keeps, Delete → Yes removes the file and slot; Create New / Esc / Esc back to the main menu |
| `after_a_game_the_front_end_opens_at_character_select` | `Entry::AfterGame` (what `main` uses after a game) opens character select with the saved character |
| `controls_table_bindings_keep_esc_on_the_game_menu` | the Configure Controls table as play bindings keeps Esc on Game Menu |
| `play_cli_new_and_save_paths` | `--new necromancer Cli --save-dir --difficulty hell` joins as class 2 on Hell; taken name, `--save` on synthetic data and a save inside the install stop with their errors; no flag = default character, unsaved |

Harness: `front_start::front_host` (the host `main` builds) with synthetic
DC6 / font / palette fixtures; the game app mirrors `play::run`'s headless
parts (no window, no GPU, no audio). `main`'s start logic moved to
`app::play_start::resolve` so the test runs the same code as the binary.

## Breaks found and fixed (one commit each)

1. **A character made on the create screen could not start.** The create
   screen writes the stub `<name>.d2s`, then `play` treated that file as a
   taken name ("already exists") and quit before the game window.
   Fix: `StartChoice.file` (the character's own file) is the save path of a
   front-end character (`app/front_start.rs`, `app/play_start.rs`).
2. **After the game the front end reopened at the main menu**, not at
   character select (§F1.3 "in game" row, REC-200). Fix:
   `front_start::Entry::AfterGame` fires `Trigger::GameExit`; `main` uses it
   after every game.
3. **Accept on Configure Controls unbound Esc.** `vk_to_key` had no entry
   for VK 0x1B, so command 56 (Esc → Game Menu, fixed, `controls.md` §3) was
   dropped when the table became play bindings: in game Esc stopped working,
   and the saved `controls.toml` kept it unbound on every later run. Fix:
   `0x1B => Key::Escape` (`ui/front_end/screens/controls.rs`).
4. **In-game Configure Controls showed the classic table** (51 rows) on an
   expansion install: `present.rs` passed a hard-coded `false`. Fix: the UI's
   `expansion_installed` (`0x00408F20`, §O2/§O9).

Harness-only gaps (not bugs): the game's input system needs a
`PrimaryWindow` entity; the original UI needs fonts and the PL2 text
colours; loading ends only with the client level data — the test adds what
`play::run` adds.

## Still broken / not covered

- Synthetic data has no save tables, so Save and Exit does not rewrite the
  `.d2s` there (by design, `play: no save tables`); with game files it does.
  The reload in the test is by name/class from the stub.
- The in-game Esc menu is installed only with game files (`play::run` adds
  the original UI under `archives`): with `--synthetic` there is no Esc menu
  in the real binary.
- Real windows (two Bevy apps in one process, REC-178) are not exercised.
- `--save` with live data and the difficulty-box from a played save need
  game files (local check below).

## Local check

```
cargo nextest run -p d2-client --test smoke_frontend
D2_GAME_DIR=... cargo run -p d2-client --release -- play --save-dir /tmp/d2s
```
Create a character (any class) → OK: the game opens (before: it quit with
"already exists"). Esc → Options → Configure Controls: the list has the
expansion rows (Skill 9–16, etc.); rebind a key, Accept; Esc still closes and
opens the menu. Save and Exit: the menu window comes back on **character
select** with the character listed; select it, OK: the same character loads.
