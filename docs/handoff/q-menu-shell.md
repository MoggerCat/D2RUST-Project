# q-menu-shell: the front-end shell

Branch `claude/q-menu-shell`. Module `d2-client::ui::front_end` (plain Rust, no Bevy types but the video stub's log line). Everything not in the specs is `d2rs-own, unverified` under REC-230 (HANDOFF §7).

## What exists

| Part | File | Spec |
|---|---|---|
| Control descriptor, hit box (y = bottom edge), button tiles/frames, label font/k | `control.rs` | frontend-menus §F1.1 r2–r5 |
| Screen-flow table `flow::next(from, trigger, ctx)` | `flow.rs` | §F1.3 |
| Start-up chain, video stub (`video stub: <path>`), progress byte N | `startup.rs` | frontend-credits C1, C5, C6 |
| `FrontEnd` driver: 40 ms tick, input queue + flush, timers (whole seconds), logo frame, sky palette, draw list | `mod.rs` | C0, §F1.1 r7, §F1.5 r3, §F1.6 |
| `Screen` trait, `Registry`, `Placeholder` | `screen.rs` | |
| Trademark screen (real): click, C2 key list, 9 s timer | `screens/trademark.rs` | C2 |
| The nine other screens: empty `register`, run as `Placeholder` (Esc → Exit, Enter → Ok) | `screens/*.rs` | |

Test: `cargo nextest run -p d2-client --test front_end` (start-up → trademark → main menu, every §F1.3 row, timer tick 250, key list, hit-box vectors, logo frames).

## Adding a screen (the interface the nine sessions use)

Each screen owns **one file**, already created: `screens/{main_menu,char_select,create,difficulty,loading,options,controls,credits,cinematics}.rs`. Edit only that file (and its own tests). Nothing shared changes.

1. Implement `Screen` (all methods default): `build(&mut FrontCtx) -> Vec<Control>` (called at every entry; controls in creation = draw order), `action(ctx, id) -> Option<Trigger>` for `Action::Custom(id)`, `tick(ctx)` (every 40 ms), `char(ctx, unit)`.
2. In the file's `register(reg)`, call `reg.register(<ID>, Box::new(MyScreen))`. Ids are consts in `screens::ids` (`MAIN_MENU`, `CHAR_SELECT`, `CHAR_CREATE`, `DIFFICULTY`, `CREDITS`, `CINEMATICS`, `LOADING`, `OPTIONS`, `CONTROLS`, `TRADEMARK`).
3. Controls never name a screen. A button carries `Action::Trigger(Trigger::X)`; `flow::next` resolves it. Triggers: `Continue`, `SinglePlayer`, `Credits`, `Cinematics`, `Exit`, `CreateNew`, `Ok`, `Difficulty(0..2)`, `GameExit`. A screen that needs a new transition (e.g. Options from a menu) adds one `match` arm in `flow.rs` (small additive edit), plus a trigger variant if needed.
4. Conditions come from `FrontCtx.flow` (`FlowCtx`): character select sets `ctx.flow.difficulties_open` before firing `Ok`; `saves_found` is read from the `SaveFolder` the host gave `FrontEnd::new`.
5. Control builders: `Control::new(kind, x, y_bottom, w, h)` + `.with_art/.with_string/.with_hotkey/.with_action`; `Control::timer(seconds, action)`; `Control::key_only(vk, action)`. Draw: `FrontEnd::draw()` returns `DrawItem::{Art, Text}` in creation order (button tiles, label font by §F1.1 r5); a screen needing more draws extends `DrawItem` additively.
6. `loads_sky_palette()` defaults to true (screens built via `0x0043C4F0`); character select / create return false.
7. Outcome: `FrontEnd::outcome()` is `Exit` or `GameLoad(GameLoad { difficulty, new_character })`.

Tests for a screen: `FrontEnd::with_screens(expansion, Box::new(saves_found))`, `start(..)`, `trigger(..)` / `input(..)` + `tick()`; see `tests/front_end.rs`.

## PROVISIONAL (REC-230)

Click fires on button-up; label k for h 33–34; progress byte in memory; character screens keep the palette; in-game exit → character select. Multiplayer buttons are not built (Phase 7+).

## Left

- The host: `play` still opens the game directly. It needs a Bevy window that runs the front end first (feed `FrontInput` from the window, call `tick` every 40 ms, map `DrawItem`s to art and text), then hands `Outcome::GameLoad` to the existing `play::run` path; `--new` / `--save` stay as shortcuts that skip it. Not done here because the game is built before the window opens (`main.rs`), a change larger than this shell.
- Persisting the progress byte N in client settings.
- Real art/strings for every screen but the trademark's controls (the trademark's legal text string is not yet bound).

## Local check

```
cargo nextest run -p d2-client --test front_end
```
All tests pass (no game files). No visible change in `play` yet.
