# Spec: UI — Front-end menus (title, main menu, character select, character create, difficulty)

- **Status:** draft (2026-10-08, RE on the 1.14d `Game.exe` merged front end; no capture yet). Points
  that need a capture are PROVISIONAL with a REC id (METHODS M22).
- **Target version:** 1.14d, English install
- **Crate/module:** `d2-client::ui::front_end`
- **Related specs:** `ui/text.md` (fonts, text draw), `ui/panels.md` (UI art conventions, DC6 draw,
  states), `render/sprite-placement.md` (DC6 placement), `render/blend-modes.md` (draw modes),
  `formats/d2s.md`, `formats/d2s-load.md`, `formats/d2s-appearance.md` (save fields, paper doll),
  `items/generation.md` §10.3 (start items), `client/model.md` §7 r9 (the C→S 0x67 the game start sends),
  `sim/intents-events.md` §8 (the single-player session sequence), `client/msg-ui.md` OQ8 (the
  end-of-game target `[0x0070EE8C]`), `client/audio.md` (sounds, deferred).

## Summary

The screens a single-player session passes through before the first game frame: the title / main
menu with its fire and logo animation, Single Player → character select (the saves found on disk),
then either character create (class line-up, name, hardcore / expansion) or the difficulty box, and
finally the game start (`client/model.md` §7 r9). Covers the flow and back paths, every button
(multiplayer ones named only: Phase 7+), the art files, positions, string ids and the input rules.
Menu sounds are named only (sound is deferred).

## Inputs

| Input | Source |
|---|---|
| Front-end art (DC6 / palette files under `data\global\ui\FrontEnd\`, `data\global\ui\CharSelect\`) | the user's MPQs (`client/assets.md`) |
| Strings (button labels, messages, class descriptions) | `.tbl` string tables (`formats/tbl.md`, `ui/text.md`) |
| Saves | the Save folder and its `.d2s` files (`formats/d2s.md`, `formats/d2s-appearance.md`) |
| Install facts | expansion installed (`client/model.md` Inputs `expansion_installed`) |
| Pointer and keys | the client input edge (`ui/controls.md`, `client/ui.md` §A4) |
| Wall-clock time | animation timing of the title fire / logo and the class animations (front end only; not game logic) |

## Outputs / state changes

| Output | Where |
|---|---|
| The screen drawn each frame (800 × 600 frame of `client/ui.md`) | the front-end draw pass |
| A new character's stub save written to the Save folder | `formats/d2s.md` (stub), §F3 of this spec |
| A deleted save (file removed) | §F2 of this spec |
| The chosen character, difficulty and hardcore / expansion flags handed to the game start | `client/model.md` §7 r9 (C→S 0x67), `sim/intents-events.md` §8 |
| Process exit (Exit Diablo II) | §F1 of this spec |
