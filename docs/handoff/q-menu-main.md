# q-menu-main: the main menu

Branch `claude/q-menu-main`. File `crates/d2-client/src/ui/front_end/screens/main_menu.rs`; test `crates/d2-client/tests/front_end_main_menu.rs`. Spec `ui/frontend-menus.md` §F1.4–F1.5. PROVISIONAL points: REC-179 (HANDOFF §7).

## Connected
- Background (expansion / classic), both logo halves, seven buttons at the §F1.4 table positions (classic: first four 100 px higher), the "v 1.14d" text, in creation order.
- Single Player → `SinglePlayer` (saves → character select, none → create), Credits, Cinematics, Exit and Esc → `Exit`. Enter is unbound.
- Battle.net, Open Battle.net, Other Multiplayer: built disabled, ignore clicks.
- Logo frame sequence 0–28 checked through `FrontEnd::draw()`.
- `main_menu::button_frame(control, tile, pressed)` is the per-state art rule (normal / pressed / disabled).

## Left
- Shared `FrontEnd::draw()` (not my file) always draws buttons unpressed and has no additive fire overlay pass. Needs: pass the pressed control to `button_frame`, and draw FireLeft/FireRight with mode 3 over the base. The front-end host is still unwritten (see q-menu-shell.md).
- Sounds named only; not wired.

## Local check
```
cargo test -p d2-client --test front_end --test front_end_main_menu
```
All 15 tests pass, no game files. Nothing visible in `play` until the host exists.
